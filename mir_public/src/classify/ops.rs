use crate::graph::ir::{GraphHole, GraphNode, GraphOp};

#[derive(Debug, Clone)]
struct OpCandidate {
    category: String,
    operation: String,
    operands: Vec<String>,
    required_operands: usize,
    evidence: String,
}

pub fn classify_ops(nodes: &[GraphNode]) -> (Vec<GraphOp>, Vec<GraphHole>) {
    let mut ops = Vec::new();
    let mut holes = Vec::new();
    let mut op_seq = 1u32;
    let mut hole_seq = 1u32;

    for node in nodes {
        let candidates = resolve_candidates(infer_op_candidates(node));
        match candidates.len() {
            0 => {
                if looks_memory_sensitive(node) {
                    holes.push(GraphHole {
                        id: format!("hole_op_{hole_seq}"),
                        step_id: node.step_id,
                        kind: "OpHole".to_string(),
                        reason: "memory_sensitive_but_no_high_confidence_rule".to_string(),
                        span: node.span.clone(),
                        function: node.function.clone(),
                        evidence: node
                            .mir_items
                            .first()
                            .cloned()
                            .unwrap_or_else(|| node.snippet.clone()),
                    });
                    hole_seq += 1;
                }
            }
            1 => {
                let cand = &candidates[0];
                if cand.required_operands > cand.operands.len() {
                    holes.push(GraphHole {
                        id: format!("hole_op_{hole_seq}"),
                        step_id: node.step_id,
                        kind: "OpHole".to_string(),
                        reason: "insufficient_operands".to_string(),
                        span: node.span.clone(),
                        function: node.function.clone(),
                        evidence: cand.evidence.clone(),
                    });
                    hole_seq += 1;
                } else {
                    ops.push(GraphOp {
                        id: format!("op_{op_seq}"),
                        step_id: node.step_id,
                        category: cand.category.clone(),
                        operation: cand.operation.clone(),
                        operands: cand.operands.clone(),
                        context: if node.snippet.contains("unsafe") {
                            "Unsafe".to_string()
                        } else {
                            "Normal".to_string()
                        },
                        span: node.span.clone(),
                        function: node.function.clone(),
                        evidence: cand.evidence.clone(),
                    });
                    op_seq += 1;
                }
            }
            _ => {
                holes.push(GraphHole {
                    id: format!("hole_op_{hole_seq}"),
                    step_id: node.step_id,
                    kind: "OpHole".to_string(),
                    reason: "ambiguous_rule_match".to_string(),
                    span: node.span.clone(),
                    function: node.function.clone(),
                    evidence: format!("candidates={}", candidates.len()),
                });
                hole_seq += 1;
            }
        }
    }
    (ops, holes)
}

fn infer_op_candidates(node: &GraphNode) -> Vec<OpCandidate> {
    let detect_text = detection_text(&node.mir_items);
    let mut out = Vec::new();

    if has_any(
        &detect_text,
        &[
            "::new(",
            "::with_capacity(",
            "exchange_malloc",
            "alloc::alloc::alloc",
            "slice::<impl [",
        ],
    ) {
        out.push(OpCandidate {
            category: "Mem".to_string(),
            operation: "Allocate".to_string(),
            operands: vec![
                node.defs
                    .first()
                    .cloned()
                    .or_else(|| node.uses.first().cloned())
                    .unwrap_or_else(|| "_v".to_string()),
                "UnknownType".to_string(),
            ],
            required_operands: 1,
            evidence: node.mir_items.join(" | "),
        });
    }
    if has_any(&detect_text, &["drop_in_place", "dealloc"]) {
        out.push(OpCandidate {
            category: "Mem".to_string(),
            operation: "Deallocate".to_string(),
            operands: one_operand(node),
            required_operands: 1,
            evidence: node.mir_items.join(" | "),
        });
    }
    if detect_text.contains("drop(") || node.snippet.contains("drop(") {
        out.push(OpCandidate {
            category: "Own".to_string(),
            operation: "Drop".to_string(),
            operands: one_operand(node),
            required_operands: 1,
            evidence: node.mir_items.join(" | "),
        });
    }
    if detect_text.contains("into_raw") {
        out.push(OpCandidate {
            category: "Own".to_string(),
            operation: "IntoRaw".to_string(),
            operands: vec![
                node.uses.first().cloned().unwrap_or_else(|| "_v".to_string()),
                node.defs.first().cloned().unwrap_or_else(|| "_p".to_string()),
            ],
            required_operands: 2,
            evidence: node.mir_items.join(" | "),
        });
    }
    if has_any(&detect_text, &["from_raw_parts", "from_raw"]) {
        out.push(OpCandidate {
            category: "Own".to_string(),
            operation: "FromRaw".to_string(),
            operands: vec![
                node.uses.first().cloned().unwrap_or_else(|| "_p".to_string()),
                node.defs.first().cloned().unwrap_or_else(|| "_v".to_string()),
            ],
            required_operands: 2,
            evidence: node.mir_items.join(" | "),
        });
    }
    if has_any(
        &detect_text,
        &["ptr::add", "ptr::sub", "ptr::offset", "wrapping_add", "::add(", "::offset("],
    ) {
        out.push(OpCandidate {
            category: "Ptr".to_string(),
            operation: "Offset".to_string(),
            operands: vec![
                node.uses.first().cloned().unwrap_or_else(|| "_p".to_string()),
                "_n".to_string(),
                node.defs.first().cloned().unwrap_or_else(|| "_q".to_string()),
            ],
            required_operands: 2,
            evidence: node.mir_items.join(" | "),
        });
    }
    if has_any(&detect_text, &["copy_nonoverlapping", "ptr::copy", "ptr::write_bytes"]) {
        out.push(OpCandidate {
            category: "Mem".to_string(),
            operation: "MemCopy".to_string(),
            operands: vec![
                node.uses.first().cloned().unwrap_or_else(|| "_dst".to_string()),
                node.uses.get(1).cloned().unwrap_or_else(|| "_src".to_string()),
                "_n".to_string(),
            ],
            required_operands: 2,
            evidence: node.mir_items.join(" | "),
        });
    }
    if detect_text.contains("set_len") {
        out.push(OpCandidate {
            category: "Mem".to_string(),
            operation: "SetLen".to_string(),
            operands: vec![
                node.uses.first().cloned().unwrap_or_else(|| "_v".to_string()),
                "_n".to_string(),
            ],
            required_operands: 1,
            evidence: node.mir_items.join(" | "),
        });
    }
    if detect_text.contains("BoundsCheck") || detect_text.starts_with("Assert") {
        out.push(OpCandidate {
            category: "Check".to_string(),
            operation: "BoundsCheck".to_string(),
            operands: vec![
                node.uses.first().cloned().unwrap_or_else(|| "_v".to_string()),
                "_idx".to_string(),
            ],
            required_operands: 1,
            evidence: node.mir_items.join(" | "),
        });
    }
    if detect_text.contains("ptr::read")
        || detect_text.contains("(*_")
        || (node.snippet.contains('*') && node.snippet.contains("unsafe"))
    {
        out.push(OpCandidate {
            category: "Ptr".to_string(),
            operation: "Deref".to_string(),
            operands: one_operand(node),
            required_operands: 1,
            evidence: node.mir_items.join(" | "),
        });
    }
    out
}

fn resolve_candidates(candidates: Vec<OpCandidate>) -> Vec<OpCandidate> {
    if candidates.len() <= 1 {
        return candidates;
    }
    let priority = [
        "FromRaw",
        "IntoRaw",
        "MemCopy",
        "SetLen",
        "BoundsCheck",
        "Offset",
        "Deref",
        "Deallocate",
        "Allocate",
        "Drop",
    ];
    for op in priority {
        if let Some(c) = candidates.iter().find(|c| c.operation == op) {
            return vec![c.clone()];
        }
    }
    candidates
}

fn one_operand(node: &GraphNode) -> Vec<String> {
    node.uses
        .first()
        .cloned()
        .or_else(|| node.defs.first().cloned())
        .map(|x| vec![x])
        .unwrap_or_default()
}

fn detection_text(items: &[String]) -> String {
    items
        .iter()
        .map(|it| it.split("//").next().unwrap_or(it.as_str()).trim().to_string())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn has_any(text: &str, keys: &[&str]) -> bool {
    keys.iter().any(|k| text.contains(k))
}

fn looks_memory_sensitive(node: &GraphNode) -> bool {
    let text = detection_text(&node.mir_items);
    node.snippet.contains("unsafe")
        || text.contains("drop(")
        || text.contains("dealloc")
        || text.contains("copy_nonoverlapping")
        || text.contains("into_raw")
        || text.contains("from_raw")
        || text.contains("ptr::read")
}
