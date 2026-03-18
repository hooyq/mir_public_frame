use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct GraphIr {
    pub schema_version: String,
    pub target: GraphTarget,
    pub trace: GraphTrace,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub ops: Vec<GraphOp>,
    pub holes: Vec<GraphHole>,
}

#[derive(Debug, Serialize)]
pub struct GraphTarget {
    pub entry_file: String,
    pub entry_function: String,
    pub depth_k: u32,
}

#[derive(Debug, Serialize)]
pub struct GraphTrace {
    pub rustc_version: String,
    pub generator: String,
    pub generated_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct GraphNode {
    pub id: u32,
    pub step_id: u32,
    pub span: String,
    pub snippet: String,
    pub function: String,
    pub depth: u32,
    pub mir_items: Vec<String>,
    pub defs: Vec<String>,
    pub uses: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct GraphEdge {
    pub from: u32,
    pub to: u32,
    #[serde(rename = "type")]
    pub edge_type: String,
    pub evidence: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct GraphOp {
    pub id: String,
    pub step_id: u32,
    pub category: String,
    pub operation: String,
    pub operands: Vec<String>,
    pub context: String,
    pub span: String,
    pub function: String,
    pub evidence: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct GraphHole {
    pub id: String,
    pub step_id: u32,
    pub kind: String,
    pub reason: String,
    pub span: String,
    pub function: String,
    pub evidence: String,
}
