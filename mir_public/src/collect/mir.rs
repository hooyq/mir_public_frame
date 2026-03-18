extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use regex::Regex;
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::ty::{Instance, TyCtxt, TypingEnv};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct MirLineRecord {
    pub function: String,
    pub file: String,
    pub line: u32,
    pub snippet: String,
    pub mir_items: Vec<String>,
    pub defs: Vec<String>,
    pub uses: Vec<String>,
    #[allow(dead_code)]
    pub succ_blocks: Vec<String>,
}

pub fn collect_mir_lines<'tcx>(tcx: TyCtxt<'tcx>, entry_file_hint: &str) -> Vec<MirLineRecord> {
    let mut records = Vec::new();
    let place_re = Regex::new(r"_\d+").expect("regex compile");
    let bb_re = Regex::new(r"bb\d+").expect("regex compile");
    let typing_env = TypingEnv::fully_monomorphized();
    let source_map = tcx.sess.source_map();

    let cgus = tcx.collect_and_partition_mono_items(()).codegen_units;
    let instances: Vec<Instance<'tcx>> = cgus
        .iter()
        .flat_map(|cgu| {
            cgu.items().iter().filter_map(|(mono_item, _)| {
                if let MonoItem::Fn(instance) = mono_item {
                    Some(*instance)
                } else {
                    None
                }
            })
        })
        .collect();

    for instance in instances {
        if instance.def_id().krate != LOCAL_CRATE {
            continue;
        }
        let body = tcx.instance_mir(instance.def);
        let function = tcx.def_path_str_with_args(instance.def_id(), instance.args);

        for (bb_idx, bb) in body.basic_blocks.iter_enumerated() {
            for statement in &bb.statements {
                let statement_text = format!("{:?}", statement.kind);
                let source_info = statement.source_info;
                if let Some(line_record) = build_line_record(
                    &function,
                    &place_re,
                    &bb_re,
                    source_map,
                    entry_file_hint,
                    source_info.span,
                    statement_text,
                    &[],
                ) {
                    records.push(line_record);
                }
            }

            if let Some(term) = &bb.terminator {
                let term_text = format!("{:?}", term.kind);
                let succ_blocks = term
                    .successors()
                    .map(|idx| format!("{idx:?}"))
                    .collect::<Vec<_>>();
                if let Some(line_record) = build_line_record(
                    &function,
                    &place_re,
                    &bb_re,
                    source_map,
                    entry_file_hint,
                    term.source_info.span,
                    term_text,
                    &succ_blocks,
                ) {
                    records.push(line_record);
                }
            }
            let _ = typing_env;
            let _ = bb_idx;
        }
    }

    records
}

#[allow(clippy::too_many_arguments)]
fn build_line_record(
    function: &str,
    place_re: &Regex,
    bb_re: &Regex,
    source_map: &rustc_span::source_map::SourceMap,
    entry_file_hint: &str,
    span: rustc_span::Span,
    text: String,
    successors: &[String],
) -> Option<MirLineRecord> {
    if span.is_dummy() {
        return None;
    }
    let loc = source_map.lookup_char_pos(span.lo());
    let file = format!("{:?}", loc.file.name);

    if !entry_file_hint.is_empty() && !file.ends_with(entry_file_hint) && !file.contains(".rs") {
        return None;
    }

    let line = loc.line as u32;
    let snippet = std::fs::read_to_string(&file)
        .ok()
        .and_then(|src| src.lines().nth((line.saturating_sub(1)) as usize).map(str::to_string))
        .unwrap_or_default();

    let (defs, uses) = split_defs_uses(place_re, &text);
    let mut succ_blocks = BTreeSet::new();
    for succ in successors {
        for m in bb_re.find_iter(succ) {
            succ_blocks.insert(m.as_str().to_string());
        }
    }

    Some(MirLineRecord {
        function: function.to_string(),
        file,
        line,
        snippet,
        mir_items: vec![text],
        defs,
        uses,
        succ_blocks: succ_blocks.into_iter().collect(),
    })
}

fn split_defs_uses(place_re: &Regex, text: &str) -> (Vec<String>, Vec<String>) {
    let mut defs = BTreeSet::new();
    let mut uses = BTreeSet::new();
    if text.contains('=') {
        let mut splitter = text.splitn(2, '=');
        let lhs = splitter.next().unwrap_or_default();
        let rhs = splitter.next().unwrap_or_default();
        for m in place_re.find_iter(lhs) {
            defs.insert(m.as_str().to_string());
        }
        for m in place_re.find_iter(rhs) {
            uses.insert(m.as_str().to_string());
        }
    } else {
        for m in place_re.find_iter(text) {
            uses.insert(m.as_str().to_string());
        }
    }
    (defs.into_iter().collect(), uses.into_iter().collect())
}
