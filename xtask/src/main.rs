//! Project tasks: `cargo xtask <task>`.
//!
//!   install     build the release binary, install the login session with sudo,
//!               and restart the running shell so it uses the new binary
//!   uninstall   remove the login session with sudo

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "\
usage: cargo xtask <task>

tasks:
  install [--no-restart]  build release, run contrib/session/install.sh with sudo,
                          then restart the running osmia so it uses the new binary
  uninstall               run contrib/session/uninstall.sh with sudo";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["install"] => install(true),
        ["install", "--no-restart"] => install(false),
        ["uninstall"] => sudo_script("uninstall.sh"),
        ["help" | "-h" | "--help"] => {
            println!("{USAGE}");
            Ok(())
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    // xtask/ sits in the repository root.
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask has a parent").to_path_buf()
}

fn run(cmd: &mut Command) -> Result<(), String> {
    let status = cmd.status().map_err(|e| format!("could not run {:?}: {e}", cmd.get_program()))?;
    if status.success() { Ok(()) } else { Err(format!("{:?} failed ({status})", cmd.get_program())) }
}

fn install(restart: bool) -> Result<(), String> {
    let root = root();
    println!("Building release");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    run(Command::new(cargo).current_dir(&root).args(["build", "--release", "--package", "osmia"]))?;
    sudo_script("install.sh")?;
    if restart {
        restart_shell();
    }
    Ok(())
}

fn sudo_script(name: &str) -> Result<(), String> {
    let script = root().join("contrib/session").join(name);
    println!("Running sudo sh {}", script.display());
    run(Command::new("sudo").arg("sh").arg(&script))
}

/// The session's autostart loop starts osmia again two seconds after it stops,
/// from /usr/local/bin, so stopping the running one is enough.
fn restart_shell() {
    // Only the shell itself: its command line is exactly "osmia" (or the installed path).
    // A test instance run as ./target/release/osmia is left alone.
    let stopped = ["osmia", "/usr/local/bin/osmia"].iter().any(|cmd| {
        Command::new("pkill")
            .args(["-u", &whoami(), "-x", "-f", cmd])
            .status()
            .is_ok_and(|s| s.success())
    });
    if stopped {
        println!("Restarted osmia. The session starts the new binary in about two seconds.");
    } else {
        println!("No running osmia shell found. The new binary starts with your next session.");
    }
}

fn whoami() -> String {
    std::env::var("USER").unwrap_or_else(|_| uid())
}

fn uid() -> String {
    // pkill takes a numeric uid as well.
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .unwrap_or_default()
}
