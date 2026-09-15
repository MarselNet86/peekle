//! `peekle --version` names the workspace version: the one the app reports,
//! the tag carries and the cask pins. tech.md S10.

use std::process::Command;

fn peekle(arg: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_peekle"))
        .arg(arg)
        .output()
        .expect("the peekle binary runs")
}

#[test]
fn version_is_the_workspace_version() {
    for flag in ["--version", "-V"] {
        let out = peekle(flag);
        assert!(out.status.success(), "{flag} exits 0");
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            format!("peekle {}", env!("CARGO_PKG_VERSION"))
        );
    }
}

#[test]
fn help_names_every_command() {
    let out = peekle("--help");
    let text = String::from_utf8_lossy(&out.stdout);
    for command in ["init", "uninstall", "doctor", "status", "--version"] {
        assert!(text.contains(command), "help mentions {command}");
    }
}
