//! The general rustc plugin framework for extracting function signatures.
//! Inspired by lockbud
#![feature(rustc_private)]
#![feature(box_patterns)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

mod app;
mod classify;
mod collect;
mod graph;
mod io;
mod settings;

use log::debug;
use rustc_session::config::ErrorOutputType;
use rustc_session::EarlyDiagCtxt;

fn main() -> std::process::ExitCode {
    // Initialize loggers.
    let handler = EarlyDiagCtxt::new(ErrorOutputType::default());
    if std::env::var("RUSTC_LOG").is_ok() {
        rustc_driver::init_rustc_env_logger(&handler);
    }
    if std::env::var("MIR_PUBLIC_LOG").is_ok() {
        let e = env_logger::Env::new()
            .filter("MIR_PUBLIC_LOG")
            .write_style("MIR_PUBLIC_LOG_STYLE");
        env_logger::init_from_env(e);
    } else if std::env::var("TAINT_ANA_LOG").is_ok() {
        let e = env_logger::Env::new()
            .filter("TAINT_ANA_LOG")
            .write_style("TAINT_ANA_LOG_STYLE");
        env_logger::init_from_env(e);
    }
    
    let mut args = std::env::args_os()
        .enumerate()
        .map(|(i, arg)| {
            arg.into_string().unwrap_or_else(|arg| {
                handler.early_fatal(format!("Argument {i} is not valid Unicode: {arg:?}"))
            })
        })
        .collect::<Vec<_>>();
    assert!(!args.is_empty());

    // Setting RUSTC_WRAPPER causes Cargo to pass 'rustc' as the first argument.
    // We're invoking the compiler programmatically, so we remove it if present.
    if args.len() > 1 && std::path::Path::new(&args[1]).file_stem() == Some("rustc".as_ref()) {
        args.remove(1);
    }

    if let Ok(flags) = std::env::var("MIR_PUBLIC_FLAGS").or_else(|_| std::env::var("TAINT_ANA_FLAGS")) {
        args.extend(flags.split_whitespace().map(str::to_string));
    }

    let mut rustc_command_line_arguments = args;
    rustc_driver::install_ice_hook("ice ice ice baby", |_| ());
    let exit_code = rustc_driver::catch_with_exit_code(|| {
        let print = "--print=";
        if rustc_command_line_arguments
            .iter()
            .any(|arg| arg.starts_with(print))
        {
            // If a --print option is given on the command line we wont get called to analyze
            // anything. We also don't want to the caller to know that we add configuration
            // parameters to the command line, lest the caller be cargo and it panics because
            // the output from --print=cfg is not what it expects.
        } else {
            let sysroot = "--sysroot";
            if !rustc_command_line_arguments
                .iter()
                .any(|arg| arg.starts_with(sysroot))
            {
                // Tell compiler where to find the std library and so on.
                // The compiler relies on the standard rustc driver to tell it, so we have to do likewise.
                rustc_command_line_arguments.push(format!("{sysroot}={}", find_sysroot()));
            }

            let always_encode_mir = "always-encode-mir";
            if !rustc_command_line_arguments
                .iter()
                .any(|arg| arg.ends_with(always_encode_mir))
            {
                // Tell compiler to emit MIR into crate for every function with a body.
                rustc_command_line_arguments.push(format!("-Z{always_encode_mir}"));
            }
        }

        let cfg = settings::AnalysisConfig::from_env();
        let mut callbacks = app::MirPublicCallbacks::new(cfg);
        debug!("rustc_command_line_arguments {rustc_command_line_arguments:?}");
        rustc_driver::run_compiler(&rustc_command_line_arguments, &mut callbacks);
    });
    exit_code
}

fn find_sysroot() -> String {
    let home = option_env!("RUSTUP_HOME");
    let toolchain = option_env!("RUSTUP_TOOLCHAIN");
    #[allow(clippy::option_env_unwrap)]
    match (home, toolchain) {
        (Some(home), Some(toolchain)) => format!("{}/toolchains/{}", home, toolchain),
        _ => option_env!("RUST_SYSROOT")
            .expect(
                "Could not find sysroot. Specify the RUST_SYSROOT environment variable, \
                 or use rustup to set the compiler to use for mir_public",
            )
            .to_owned(),
    }
}
