extern crate rustc_driver;
use log::{debug, info, warn};
use rustc_driver::Compilation;
use rustc_middle::ty::TyCtxt;
use rustc_session::config::Input;

use crate::collect::mir::collect_mir_lines;
use crate::graph::builder::build_graph_ir;
use crate::io::writer::write_graph;
use crate::settings::AnalysisConfig;

pub struct MirPublicCallbacks {
    file_name: String,
    config: AnalysisConfig,
}

impl MirPublicCallbacks {
    pub fn new(config: AnalysisConfig) -> Self {
        Self {
            file_name: String::new(),
            config,
        }
    }
}

impl rustc_driver::Callbacks for MirPublicCallbacks {
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        self.file_name = match &config.input {
            Input::File(path) => path.to_string_lossy().to_string(),
            Input::Str { name, .. } => format!("{name:?}"),
        };
        debug!("Processing input file: {}", self.file_name);
    }

    fn after_analysis(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        tcx: TyCtxt<'_>,
    ) -> rustc_driver::Compilation {
        let records = collect_mir_lines(tcx, &self.file_name);
        if records.is_empty() {
            warn!("no MIR line records extracted from {}", self.file_name);
            return Compilation::Continue;
        }

        let graph = build_graph_ir(
            records,
            self.file_name.clone(),
            "crate_entry".to_string(),
            self.config.depth_k,
            rustc_version_string(),
        );
        if let Err(err) = write_graph(&self.config.output_path, &graph) {
            warn!("failed to write graph output: {err}");
        } else {
            info!("wrote GraphIR to {}", self.config.output_path.display());
        }

        Compilation::Continue
    }
}

fn rustc_version_string() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown-rustc".to_string())
}
