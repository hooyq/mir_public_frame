# mir_public 项目分析文档

本文档面向项目维护者与使用者，说明 `mir_public` 的目标、实现路径和设计动机，并给出当前版本的局限与可演进方向。

---

## 1. 项目在做什么

`mir_public` 是一个基于 `rustc` 内部接口（`rustc_private`）的 MIR 静态分析工具。  
它不直接输出“漏洞结论”，而是输出一个结构化中间表示（`GraphIR`），作为后续分析器或大模型推理的证据输入。

当前输出重点是：

- 图结构：`nodes` + `edges`
- 预定义操作：`ops`
- 不确定占位：`holes`
- 追踪元信息：`target` + `trace`

默认输出文件为 `mir_public_graph.json`（可通过环境变量覆盖）。

---

## 2. 总体架构与执行链路

项目包含两个二进制：

- `mir_public`：真正的 rustc wrapper（`src/main.rs`）
- `cargo-mir-public`：cargo 子命令入口（`src/bin/cargo-taint-ana.rs`）

典型执行流程：

1. 用户执行 `cargo +nightly mir-public -- [cargo build args...]`
2. `cargo-mir-public` 启动 `cargo build`，注入 `RUSTC_WRAPPER=mir_public`
3. `mir_public` 接管每次 rustc 调用，注册回调
4. 在 `after_analysis` 阶段执行分析管线：
   - MIR 采集 (`collect_mir_lines`)
   - 图构建 (`build_graph_ir`)
   - JSON 写出 (`write_graph`)

---

## 3. 具体实现细节

### 3.1 `src/main.rs`：wrapper 启动与编译参数注入

关键职责：

- 初始化日志（兼容 `MIR_PUBLIC_LOG` 与旧变量 `TAINT_ANA_LOG`）
- 处理 `RUSTC_WRAPPER` 场景下的参数差异（去掉多余 `rustc` 参数）
- 自动补 `--sysroot`（若调用方未提供）
- 自动追加 `-Zalways-encode-mir` 以确保 MIR 可提取
- 读取环境配置并创建 `MirPublicCallbacks`

这一步保证“在不破坏 cargo 标准构建体验的情况下”拿到足够分析信息。

### 3.2 `src/bin/cargo-taint-ana.rs`：cargo 子命令封装

关键职责：

- 把 `cargo mir-public` 转换为 `cargo build`
- 设置 `RUSTC_WRAPPER` 指向 `mir_public`
- 将 `--` 前的自定义参数写入 `MIR_PUBLIC_FLAGS`
- 将 `--` 后参数原样透传给 `cargo build`
- 默认设置日志等级为 `info`（可由环境变量覆盖）

该设计让工具以“cargo 原生扩展”形式接入项目，降低使用门槛。

### 3.3 `src/app/mod.rs`：核心调度层

`after_analysis` 中串联完整主流程：

1. `collect_mir_lines(tcx, file_hint)` 收集 MIR 行级记录
2. `build_graph_ir(...)` 生成 GraphIR
3. `write_graph(path, graph)` 持久化 JSON

如果没有提取到记录，会写告警并继续编译流程，不阻断原有构建。

### 3.4 `src/collect/mir.rs`：MIR 行级采集

核心逻辑：

- 从 codegen units 中遍历 `MonoItem::Fn` 实例
- 仅处理 `LOCAL_CRATE`（避免把外部 crate 全量带入）
- 逐 basic block 采集 `statement` 与 `terminator`
- 用 `Span` 定位源码文件与行号
- 构建 `MirLineRecord`

`MirLineRecord` 字段：

- `function` / `file` / `line` / `snippet`
- `mir_items`
- `defs` / `uses`
- `succ_blocks`

`defs/uses` 抽取策略：

- 文本包含 `=`：`lhs -> defs`，`rhs -> uses`
- 否则：出现的 place（`_\d+`）统一视为 `uses`

该策略偏轻量，利于快速建立可用依赖图。

### 3.5 `src/graph/builder.rs`：图聚合与边构建

主要步骤：

1. `build_nodes`
   - 按 `(function, file, line)` 聚合同一行多个 MIR 记录
   - 合并 `defs/uses/mir_items`
   - 生成递增 `id` 与 `step_id`

2. `build_edges`
   - `DataDep`：若当前节点使用变量在历史有 `last_def`，建立 def-use 边
   - `TemporalDep`：按节点顺序建立相邻时序边
   - 使用 `dedup` 集合去重，避免重复边

3. `classify_ops`
   - 对每个节点进行语义操作识别，产出 `ops` 与 `holes`

### 3.6 `src/classify/ops.rs`：规则驱动操作识别

当前主要覆盖内存安全相关操作：

- `Allocate` / `Deallocate`
- `Drop`
- `IntoRaw` / `FromRaw`
- `Offset`
- `MemCopy`
- `SetLen`
- `BoundsCheck`
- `Deref`

工作机制：

1. `infer_op_candidates(node)` 依据 MIR 文本和 snippet 做模式匹配
2. `resolve_candidates` 按优先级消歧
3. `classify_ops` 生成最终输出：
   - 0 候选：若内存敏感，则生成 `OpHole`
   - 1 候选：若操作数不足则 `OpHole`，否则生成 `GraphOp`
   - 多候选：生成 `ambiguous_rule_match` 类型 `OpHole`

这是“保守优先”的分类策略：宁可缺失，不轻易误判。

### 3.7 `src/graph/ir.rs` 与 `src/io/writer.rs`：模型定义与落盘

- `graph/ir.rs` 定义 serde 可序列化结构：
  - `GraphIr` / `GraphNode` / `GraphEdge` / `GraphOp` / `GraphHole`
- `io/writer.rs` 负责创建目录、pretty JSON 序列化与写文件

配置在 `src/settings/mod.rs`：

- `MIR_PUBLIC_OUTPUT`：输出路径（默认 `mir_public_graph.json`）
- `MIR_PUBLIC_DEPTH_K`：保留深度参数（默认 `0`）

---

## 4. 为什么这么做（设计动机）

### 4.1 选择 MIR 而不是源码正则或浅层 AST

MIR 比源码更接近编译后的语义形态，适合做数据依赖和内存相关行为抽取。  
对于安全分析，这比字符串级规则稳定性更高。

### 4.2 选择“证据 IR”而非直接漏洞判定

项目将问题分层：

- 第一层：抽取可追踪证据（Graph+OpsIR）
- 第二层：由后续规则/模型做风险判断

这样可以提升通用性与可审计性，也便于迭代不同下游策略。

### 4.3 选择“ops + holes”双通道输出

`ops` 表示高置信识别，`holes` 显式表达不确定区域。  
这种设计可减少误导，尤其在 LLM 作为下游推理器时更稳健。

### 4.4 选择规则驱动冷启动

规则方案具备：

- 可解释（有证据文本）
- 可控（问题可定点修复）
- 成本低（无需标注训练）

对早期版本尤其有效，后续可以逐步引入更复杂分类器。

---

## 5. 当前局限与风险点

1. `defs/uses` 依赖文本与 `_\d+` place 提取，语义粒度较粗。  
2. `DataDep` 基于“最后定义”近似，尚未处理更复杂别名与路径敏感场景。  
3. 控制流后继块虽然已采集（`succ_blocks`），但尚未充分用于边构建。  
4. 节点当前是按源码行聚合，信息损失与噪声并存。  
5. 分类器主要是关键词规则，覆盖面与稳健性仍需随样本增长持续扩展。

---

## 6. 演进建议（按优先级）

1. **先增强证据质量**：提升 `defs/uses` 解析精度，减少 `_1/_2` 语义不透明问题。  
2. **再增强图语义**：将 `succ_blocks` 显式纳入 CFG/控制依赖边。  
3. **扩展操作规则库**：按漏洞类型（UAF、越界、双重释放）逐类补规则。  
4. **建立回归样例集**：固定输入程序，比较 `ops/holes/edges` 差异。  
5. **引入分层置信度**：给 `op`/`hole` 增加 score，便于下游排序与过滤。

---

## 7. 一句话结论

`mir_public` 当前是一个“Rust MIR 到图证据 IR”的分析前端：  
强调结构化、可追踪、保守输出，为后续自动化安全分析或大模型推理提供可靠输入基础。
