mod commands;
mod events;
mod hooks;
mod hotkey;
mod panel;
mod state;
mod windows;

use std::sync::Arc;

use peekle_core::config::UsageProviderKind;
use peekle_core::config::{self, Config};
use peekle_server::routes::ServerState;
use peekle_usage::{FakeUsage, UsageProvider};
use tauri::{Emitter, Manager};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_nspanel::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    // Fire on press only. The plugin reports both edges and a
                    // release would toggle straight back.
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        commands::toggle_enabled(app.clone());
                    }
                })
                .build(),
        )
        .on_page_load(|window, payload| {
            tracing::debug!(
                label = window.label(),
                url = %payload.url(),
                event = ?payload.event(),
                "webview page load"
            );
        })
        .invoke_handler(build_handler())
        .setup(|app| {
            // Accessory keeps Peekle out of the dock and out of the menu bar.
            // tech.md section 6.7 makes this a product decision, not a default.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let config = load_config();
            let port = config.server.port;
            let token = config.server.token.clone();
            let toggle = config.hotkey.toggle.clone();
            let provider = usage_provider(&config);

            let state = Arc::new(state::AppState::new(config, provider));
            app.manage(Arc::clone(&state));

            match app.get_webview_window(panel::ISLAND) {
                Some(window) => tracing::debug!(
                    url = %window.url().map(|u| u.to_string()).unwrap_or_default(),
                    "island window created"
                ),
                None => tracing::error!("island window missing"),
            }

            panel::convert_all(app.handle())?;

            // A borderless webview gets no safe area of its own, so Rust hands
            // the measured notch over to the route. tech.md 6.7.
            if let (Some(window), Some((height, width))) = (
                app.get_webview_window(panel::ISLAND),
                panel::active_notch(app.handle()),
            ) {
                let url = format!("/island/?notch={height}&notch_width={width}");
                if let Err(err) = window.eval(format!(
                    "if (location.search.indexOf('notch=') === -1) location.replace('{url}')"
                )) {
                    tracing::warn!(error = %err, "could not hand the notch over");
                }
            }

            // The island goes up once and stays up, transparent while
            // collapsed. tech.md section 8.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                windows::open_island(&handle).await;
            });

            // The resting mark is the only clickable part of a collapsed
            // island, and nothing but a poll can tell when the pointer reaches
            // it. tech.md 6.7.
            windows::track_pointer(app.handle());

            hotkey::install(app.handle(), &toggle);

            poll_usage(app.handle(), Arc::clone(&state));

            let sink = Arc::new(hooks::AppSink::new(app.handle().clone(), state));
            serve(port, token, sink);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("peekle failed to start");
}

/// Tokens and credentials never reach a log line, so the default filter can
/// stay chatty without leaking anything. tech.md rule 11.
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_env("PEEKLE_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("peekle=info,peekle_server=info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

/// Debug builds carry one extra command so the panel can be driven without a
/// live Claude Code session. tech.md 6.5.
fn build_handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    #[cfg(debug_assertions)]
    {
        tauri::generate_handler![
            commands::get_state,
            commands::answer_prompt,
            commands::dismiss_prompt,
            commands::set_enabled,
            commands::refresh_usage,
            commands::request_usage_access,
            commands::set_usage_enabled,
            commands::window_ready,
            commands::set_view,
            commands::island_bounds,
            commands::dev_emit_prompt,
        ]
    }
    #[cfg(not(debug_assertions))]
    {
        tauri::generate_handler![
            commands::get_state,
            commands::answer_prompt,
            commands::dismiss_prompt,
            commands::set_enabled,
            commands::refresh_usage,
            commands::request_usage_access,
            commands::set_usage_enabled,
            commands::window_ready,
            commands::set_view,
            commands::island_bounds,
        ]
    }
}

/// A broken config file must not stop the overlay from starting. Defaults keep
/// the app usable and `doctor` is where the user finds out what happened.
fn load_config() -> Config {
    match config::config_path().and_then(|path| Config::load(&path)) {
        Ok(config) => config,
        Err(err) => {
            tracing::warn!(error = %err, "falling back to default config");
            Config::default()
        }
    }
}

fn usage_provider(config: &Config) -> Arc<dyn UsageProvider> {
    match config.usage.provider {
        UsageProviderKind::Account => Arc::new(peekle_usage::AccountUsage::new(
            Box::new(peekle_usage::SecurityToolStore::for_current_user()),
            VERSION,
        )),
        // Off still needs a provider: the bars render the reason rather than
        // disappearing, so the user can see why they are empty.
        UsageProviderKind::Fake | UsageProviderKind::Off => Arc::new(FakeUsage::default()),
    }
}

/// The background poll of tech.md 6.4.
///
/// It never starts before the user has granted Keychain access. Reading the
/// Keychain is what raises the dialog, and rule 12 forbids this app from
/// raising it on its own: only `request_usage_access` may.
fn poll_usage(app: &tauri::AppHandle, state: Arc<state::AppState>) {
    const EVERY: std::time::Duration = std::time::Duration::from_secs(300);

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(EVERY).await;

            let (enabled, granted, denied) = {
                let config = state.lock_config();
                (
                    config.usage.enabled,
                    config.usage.keychain_granted,
                    config.usage.keychain_denied,
                )
            };
            if !enabled || !granted || denied {
                continue;
            }

            // The provider blocks on a network call, so it never runs on the
            // async runtime: a slow answer must not hold up a hook.
            let provider = Arc::clone(&state.usage_provider);
            let Ok(snapshot) =
                tauri::async_runtime::spawn_blocking(move || provider.snapshot()).await
            else {
                continue;
            };

            state.set_usage(snapshot.clone());
            if let Err(err) = handle.emit(events::USAGE, &snapshot) {
                tracing::warn!(error = %err, "failed to emit usage");
            }
        }
    });
}

/// A port already in use must not take the app down: hooks stop arriving, the
/// agent degrades to its normal mode, and `doctor` reports the conflict.
fn serve(port: u16, token: String, sink: Arc<hooks::AppSink>) {
    let server_state = ServerState {
        sink,
        token,
        version: VERSION.to_string(),
    };

    tauri::async_runtime::spawn(async move {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => listener,
            Err(err) => {
                tracing::error!(port, error = %err, "hook server could not bind");
                return;
            }
        };
        tracing::info!(port, "hook server listening");

        if let Err(err) = axum::serve(listener, peekle_server::router(server_state)).await {
            tracing::error!(error = %err, "hook server stopped");
        }
    });
}
