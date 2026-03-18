# mir_public Graph+OpsIR 操作文档

本文档说明 `mir_public` 如何按 Graph+OpsIR 框架运行，并解释模块职责与扩展方式。

## 1. 框架目标

`mir_public` 当前输出 `GraphIR v0.1`，用于给大模型提供结构化静态证据：

- 图结构：`nodes` + `edges`
- 预定义操作：`ops`
- 保守占位：`holes`
- 跟踪元信息：`target` + `trace`

## 2. 功能分层（模块分类）

`src/main.rs`
- rustc wrapper 入口
- 构建编译参数与顶层模块装配

`src/app/mod.rs`
- rustc 回调主流程
- 执行：MIR 采集 -> 图构建 -> JSON 输出

`src/collect/mir.rs`
- 从本地 crate MIR 收集行级记录
- 抽取基础字段：函数名、文件行号、MIR 文本、def/use

`src/graph/builder.rs`
- 将行级记录聚合成 GraphIR `nodes/edges`
- 生成 DataDep/TemporalDep
- 调用操作分类器补 `ops/holes`

`src/classify/ops.rs`
- 规则驱动的操作分类（内存安全优先）
- 冲突消歧（优先级）
- 不确定时生成 `OpHole`

`src/graph/ir.rs`
- GraphIR 数据结构定义（serde 可序列化）

`src/io/writer.rs`
- 统一 JSON 输出

`src/settings/mod.rs`
- 分析配置读取（输出路径、深度参数）

## 3. 运行方式

### 3.1 直接构建

```bash
cd static_analysis/mir_public_frame/mir_public
cargo +nightly build
```

### 3.2 通过 cargo 子命令运行（推荐）

在目标 Rust 项目根目录执行：

```bash
cargo +nightly mir-public -- --bin your_bin_name
```

可选环境变量：

```bash
MIR_PUBLIC_OUTPUT=./out/graph.json
MIR_PUBLIC_DEPTH_K=0
MIR_PUBLIC_LOG=info
```

## 4. 输出格式说明

输出文件默认为 `mir_public_graph.json`（可由 `MIR_PUBLIC_OUTPUT` 覆盖）。

关键字段：

- `schema_version`：当前 `graphir.v0.1`
- `target`：入口文件、入口函数、深度参数
- `trace`：rustc 版本、生成器、时间戳
- `nodes`：行级节点（含 `step_id`、`span`、`mir_items`、`defs/uses`）
- `edges`：`DataDep` / `TemporalDep`
- `ops`：规则命中的预定义操作
- `holes`：规则不确定时的保守占位

## 5. 预定义操作分类（第一阶段）

当前优先内存安全相关操作：

- `Allocate`
- `Deallocate`
- `Drop`
- `IntoRaw`
- `FromRaw`
- `Offset`
- `MemCopy`
- `SetLen`
- `BoundsCheck`
- `Deref`

说明：
- 同行命中多个候选时，按优先级消歧（`FromRaw`/`IntoRaw` 优先于泛化操作）。
- 证据不足或冲突时，输出 `OpHole`，避免误导后续大模型推理。

## 6. 如何扩展新操作规则

在 `src/classify/ops.rs` 中扩展：

1. 在 `infer_op_candidates()` 增加新模式匹配。
2. 设置 `category/operation/operands/required_operands/evidence`。
3. 如有必要，在 `resolve_candidates()` 中调整优先级。
4. 使用样例重新运行并检查 `ops/holes` 变化。

建议规则准则：
- 先高精度后高召回
- 宁可 `hole` 也不误判
- `evidence` 必须可追溯到 MIR 文本

## 7. 常见问题

1) 没有输出文件  
- 检查是否使用 `cargo +nightly`
- 检查 wrapper 是否生效：`cargo +nightly mir-public ...`

2) `ops` 很少、`holes` 偏多  
- 当前为保守策略，优先保证准确性
- 可针对目标漏洞类型增强 `classify/ops.rs` 规则

3) 输出变量名是 `_1/_2` 之类 MIR 临时变量  
- 这是第一阶段预期行为
- 下一阶段可加入 MIR 到源码变量名映射层
