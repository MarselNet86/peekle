//! `peekle` command line. tech.md S9.

use std::process::ExitCode;

use peekle_cli::doctor::{overall, stop_block_cap, timeout_gap, Check, Health};
use peekle_cli::{merge_handlers, remove_handlers, ClaudeSettings, FileSettings};
use peekle_core::config::{self, Config};
use peekle_core::MANAGED_HOOK_EVENTS;
use serde_json::Value;

const USAGE: &str = "\
peekle, an overlay on top of Claude Code

  peekle init        write the hooks and the config, merged and idempotent
  peekle uninstall   remove only Peekle's own hook entries
  peekle doctor      check the install and say how to fix what is wrong
  peekle status      print the state as JSON
";

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("init") => run(init),
        Some("uninstall") => run(uninstall),
        Some("doctor") => doctor(),
        Some("status") => run(status),
        Some("-h") | Some("--help") | None => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("unknown command: {other}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run(action: fn() -> Result<(), String>) -> ExitCode {
    match action() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("peekle: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Loads the config, or writes a fresh one with a new token. Running twice
/// keeps the same token: rewriting it would orphan the hooks already installed.
fn load_or_create() -> Result<(Config, std::path::PathBuf), String> {
    let path = config::config_path().map_err(|e| e.to_string())?;

    if path.exists() {
        let config = Config::load(&path).map_err(|e| e.to_string())?;
        return Ok((config, path));
    }

    let config = Config::default();
    config.save(&path).map_err(|e| e.to_string())?;
    Ok((config, path))
}

fn settings() -> Result<FileSettings, String> {
    let path = FileSettings::default_path().ok_or("cannot find ~/.claude")?;
    Ok(FileSettings::new(path))
}

fn init() -> Result<(), String> {
    let (config, config_path) = load_or_create()?;
    let store = settings()?;

    let before = store.read().map_err(|e| e.to_string())?;
    let after = merge_handlers(&before, config.server.port, &config.server.token);

    if before == after {
        println!("already installed, nothing changed");
        println!("config  {}", config_path.display());
        return Ok(());
    }

    let backup = store.write(&after).map_err(|e| e.to_string())?;
    println!("hooks installed for {} events", MANAGED_HOOK_EVENTS.len());
    println!("backup  {}", backup.display());
    println!("config  {}", config_path.display());
    println!("\nstart Peekle, then run a Claude Code session as usual");
    Ok(())
}

fn uninstall() -> Result<(), String> {
    let (config, _) = load_or_create()?;
    let store = settings()?;

    let before = store.read().map_err(|e| e.to_string())?;
    let after = remove_handlers(&before, config.server.port);

    if before == after {
        println!("nothing of Peekle's was installed");
        return Ok(());
    }

    let backup = store.write(&after).map_err(|e| e.to_string())?;
    println!("hooks removed, everything else left alone");
    println!("backup  {}", backup.display());
    Ok(())
}

fn status() -> Result<(), String> {
    let (config, _) = load_or_create()?;
    let health = health_body(config.server.port);

    let state = serde_json::json!({
        "port": config.server.port,
        "hooks_installed": hooks_installed(config.server.port)?,
        "running": health.is_some(),
        "app": health,
        "usage_provider": format!("{:?}", config.usage.provider).to_lowercase(),
        "hotkey": config.hotkey.toggle,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn doctor() -> ExitCode {
    let checks = match collect_checks() {
        Ok(checks) => checks,
        Err(err) => {
            eprintln!("peekle: {err}");
            return ExitCode::FAILURE;
        }
    };

    for check in &checks {
        let mark = match check.health {
            Health::Ok => "ok  ",
            Health::Warn => "warn",
            Health::Fail => "fail",
        };
        println!("{mark}  {:<16} {}", check.name, check.detail);
        if !check.fix.is_empty() {
            println!("      {:<16} {}", "", check.fix);
        }
    }

    match overall(&checks) {
        Health::Fail => ExitCode::FAILURE,
        _ => ExitCode::SUCCESS,
    }
}

fn collect_checks() -> Result<Vec<Check>, String> {
    let (config, config_path) = load_or_create()?;
    let mut checks = vec![Check::ok("config", config_path.display().to_string())];

    checks.push(if config.server.token.len() == 32 {
        Check::ok("token", "32 hex characters")
    } else {
        Check::fail(
            "token",
            format!("{} characters", config.server.token.len()),
            "delete the config and run peekle init to mint a fresh one",
        )
    });

    checks.push(match hooks_installed(config.server.port)? {
        true => Check::ok("hooks", "installed in ~/.claude/settings.json"),
        false => Check::fail("hooks", "not installed", "run peekle init"),
    });

    checks.push(match health_body(config.server.port) {
        Some(body) => Check::ok(
            "app",
            format!(
                "running, core {}",
                body.get("core").and_then(Value::as_str).unwrap_or("?")
            ),
        ),
        None => Check::warn(
            "app",
            format!("nothing answering on 127.0.0.1:{}", config.server.port),
            "start Peekle; without it hooks fail open and the agent runs as usual",
        ),
    });

    checks.push(timeout_gap(config.behavior.prompt_timeout_secs, 900));
    checks.push(stop_block_cap(
        std::env::var("CLAUDE_CODE_STOP_HOOK_BLOCK_CAP")
            .ok()
            .as_deref(),
    ));

    Ok(checks)
}

fn hooks_installed(port: u16) -> Result<bool, String> {
    let store = settings()?;
    let value = store.read().map_err(|e| e.to_string())?;
    let stripped = remove_handlers(&value, port);
    Ok(value != stripped)
}

/// Asks the running app for its health. A dead port is an answer, not an error:
/// Peekle not running is a supported state.
fn health_body(port: u16) -> Option<Value> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let mut stream = TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().ok()?,
        Duration::from_millis(300),
    )
    .ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok()?;

    let request = format!("GET /v1/health HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n");
    stream.write_all(request.as_bytes()).ok()?;

    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;

    let body = response.split("\r\n\r\n").nth(1)?;
    serde_json::from_str(body).ok()
}
