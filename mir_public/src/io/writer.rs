use crate::graph::ir::GraphIr;
use std::path::Path;

pub fn write_graph(path: &Path, graph: &GraphIr) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create output directory {}: {err}", parent.display()))?;
        }
    }
    let payload = serde_json::to_string_pretty(graph)
        .map_err(|err| format!("failed to serialize GraphIR: {err}"))?;
    std::fs::write(path, payload).map_err(|err| format!("failed to write {}: {err}", path.display()))
}
