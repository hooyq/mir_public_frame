use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    pub output_path: PathBuf,
    pub depth_k: u32,
}

impl AnalysisConfig {
    pub fn from_env() -> Self {
        let output_path = std::env::var("MIR_PUBLIC_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("mir_public_graph.json"));

        let depth_k = std::env::var("MIR_PUBLIC_DEPTH_K")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);

        Self {
            output_path,
            depth_k,
        }
    }
}
