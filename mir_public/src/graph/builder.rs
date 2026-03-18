use crate::classify::ops::classify_ops;
use crate::collect::mir::MirLineRecord;
use crate::graph::ir::{GraphEdge, GraphIr, GraphNode, GraphTarget, GraphTrace};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn build_graph_ir(
    records: Vec<MirLineRecord>,
    entry_file: String,
    entry_function: String,
    depth_k: u32,
    rustc_version: String,
) -> GraphIr {
    let nodes = build_nodes(records);
    let edges = build_edges(&nodes);
    let (ops, holes) = classify_ops(&nodes);

    GraphIr {
        schema_version: "graphir.v0.1".to_string(),
        target: GraphTarget {
            entry_file,
            entry_function,
            depth_k,
        },
        trace: GraphTrace {
            rustc_version,
            generator: "mir_public::graph_builder".to_string(),
            generated_at: now_unix_millis_string(),
        },
        nodes,
        edges,
        ops,
        holes,
    }
}

fn build_nodes(records: Vec<MirLineRecord>) -> Vec<GraphNode> {
    let mut grouped: BTreeMap<(String, String, u32), Vec<MirLineRecord>> = BTreeMap::new();
    for r in records {
        grouped
            .entry((r.function.clone(), r.file.clone(), r.line))
            .or_default()
            .push(r);
    }

    let mut nodes = Vec::new();
    let mut id = 1u32;
    for ((function, file, line), items) in grouped {
        let mut defs = BTreeSet::new();
        let mut uses = BTreeSet::new();
        let mut mir_items = Vec::new();
        let mut snippet = String::new();
        for item in items {
            if snippet.is_empty() {
                snippet = item.snippet.clone();
            }
            for d in item.defs {
                defs.insert(d);
            }
            for u in item.uses {
                uses.insert(u);
            }
            for m in item.mir_items {
                mir_items.push(m);
            }
        }
        nodes.push(GraphNode {
            id,
            step_id: id,
            span: format!("{file}:{line}"),
            snippet,
            function,
            depth: 0,
            mir_items,
            defs: defs.into_iter().collect(),
            uses: uses.into_iter().collect(),
        });
        id += 1;
    }

    nodes
}

fn build_edges(nodes: &[GraphNode]) -> Vec<GraphEdge> {
    let mut edges = Vec::new();
    let mut dedup = BTreeSet::new();
    let mut last_def: HashMap<String, u32> = HashMap::new();

    for node in nodes {
        for used in &node.uses {
            if let Some(from) = last_def.get(used) {
                if *from != node.id {
                    push_edge(
                        &mut edges,
                        &mut dedup,
                        GraphEdge {
                            from: *from,
                            to: node.id,
                            edge_type: "DataDep".to_string(),
                            evidence: format!("def({used})@{from} -> use({used})@{}", node.id),
                        },
                    );
                }
            }
        }
        for defined in &node.defs {
            last_def.insert(defined.clone(), node.id);
        }
    }

    for pair in nodes.windows(2) {
        push_edge(
            &mut edges,
            &mut dedup,
            GraphEdge {
                from: pair[0].id,
                to: pair[1].id,
                edge_type: "TemporalDep".to_string(),
                evidence: format!("node-order: {} -> {}", pair[0].id, pair[1].id),
            },
        );
    }

    edges
}

fn push_edge(edges: &mut Vec<GraphEdge>, dedup: &mut BTreeSet<String>, edge: GraphEdge) {
    let key = format!("{}:{}:{}:{}", edge.from, edge.to, edge.edge_type, edge.evidence);
    if dedup.insert(key) {
        edges.push(edge);
    }
}

fn now_unix_millis_string() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    millis.to_string()
}
