//! `cargo mir-public $FLAGS $ARGS` calls `cargo build` with RUSTC_WRAPPER set to `mir_public`.
//! The flags are passed to `mir_public` through env var `MIR_PUBLIC_FLAGS`.
//! The remaining args are unchanged.
//! To re-run `cargo mir-public` with different flags on the same crate, please `cargo clean` first.
use std::env;
use std::ffi::OsString;
use std::process::Command;

const CARGO_MIR_PUBLIC_HELP: &str = r#"Extract function signatures from Rust project
Usage:
    cargo mir-public [options] [--] [<cargo build options>...]
Common options:
    -h, --help               Print this message
    -V, --version            Print version info and exit
    
Options after the first "--" are the same arguments that `cargo build` accepts.

Examples:
    # Extract function signatures from the project
    cargo mir-public
    # With specific target
    cargo +nightly mir-public -- --target x86_64-unknown-linux-gnu
"#;

fn show_help() {
    println!("{}", CARGO_MIR_PUBLIC_HELP);
}

fn show_version() {
    println!("mir_public 0.1.0");
}

fn cargo() -> Command {
    Command::new(env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}

// Determines whether a `--flag` is present.
fn has_arg_flag(name: &str) -> bool {
    let mut args = std::env::args().take_while(|val| val != "--");
    args.any(|val| val == name)
}

fn in_cargo_taint_ana() {
    // Now we run `cargo build $FLAGS $ARGS`, giving the user the
    // chance to add additional arguments. `FLAGS` is set to identify
    // this target. The user gets to control what gets actually passed to taint-ana.
    let mut cmd = cargo();
    cmd.arg("build");
    
    // Get the path to the analyzer binary.
    let exe_name = if cfg!(windows) { "mir_public.exe" } else { "mir_public" };
    
    // Try multiple locations to find the binary
    let wrapper_path = std::env::current_exe()
        .ok()
        .and_then(|exe| {
            // First, try same directory as cargo-mir-public
            exe.parent().and_then(|dir| {
                let path = dir.join(exe_name);
                if path.exists() { Some(path) } else { None }
            })
        })
        .or_else(|| {
            // Try in this project's target directory
            let mut path = std::env::current_dir().ok()?;
            // Go up to find taintAna directory
            loop {
                let test_path = path.join("experiment").join("fn-signature-extractor")
                    .join("taintAna").join("target").join("debug").join(exe_name);
                if test_path.exists() {
                    return Some(test_path);
                }
                let test_path = path.join("experiment").join("fn-signature-extractor")
                    .join("taintAna").join("target").join("release").join(exe_name);
                if test_path.exists() {
                    return Some(test_path);
                }
                if !path.pop() {
                    break;
                }
            }
            None
        })
        .or_else(|| {
            // Fallback: try current directory's target
            let mut path = std::env::current_dir().ok()?;
            path.push("target");
            path.push("debug");
            path.push(exe_name);
            if path.exists() {
                Some(path)
            } else {
                path.pop();
                path.push("release");
                path.push(exe_name);
                if path.exists() {
                    Some(path)
                } else {
                    None
                }
            }
        })
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| {
                eprintln!("Warning: Could not find mir_public binary. Please build it first with: cargo build");
            "mir_public".to_string()
        });
    
    cmd.env("RUSTC_WRAPPER", &wrapper_path);
    cmd.env("RUST_BACKTRACE", "full");

    // Pass MIR_PUBLIC_LOG if specified by the user. Default to info if not specified.
    const MIR_PUBLIC_LOG: &str = "MIR_PUBLIC_LOG";
    let log_level = env::var(MIR_PUBLIC_LOG).ok();
    cmd.env(MIR_PUBLIC_LOG, log_level.as_deref().unwrap_or("info"));

    let mut args = std::env::args().skip(2);

    let flags: Vec<_> = args.by_ref().take_while(|arg| arg != "--").collect();
    let flags = flags.join(" ");
    cmd.env("TAINT_ANA_FLAGS", flags);

    let exit_status = cmd
        .args(args)
        .spawn()
        .expect("could not run cargo")
        .wait()
        .expect("failed to wait for cargo?");
    if !exit_status.success() {
        std::process::exit(exit_status.code().unwrap_or(-1))
    };
}

fn main() {
    if has_arg_flag("--help") || has_arg_flag("-h") {
        show_help();
        return;
    }
    if has_arg_flag("--version") || has_arg_flag("-V") {
        show_version();
        return;
    }
    if let Some("mir-public") = std::env::args().nth(1).as_deref() {
        in_cargo_taint_ana();
    }
}

