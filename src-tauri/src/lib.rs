mod commands;
mod events;
mod hooks;
mod hotkey;
mod notify;
mod platform;
mod shots;
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

    let builder = tauri::Builder::default();
    // The panel plugin is macOS: NSPanel is the thing it converts to, and on
    // every other platform the window stays a window. tech.md 6.27.
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    builder
        // Banners for a finished turn. Registering the plugin posts
        // nothing and asks nothing: macOS raises its dialog on the
        // first banner, and the first banner is the toggle's own.
        // tech.md 6.17.
        .plugin(tauri_plugin_notification::init())
        // The one system dialog the product raises: the folder a new chat
        // works in. Registering it opens nothing. tech.md 6.23.
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    // Fire on press only. The plugin reports both edges and a
                    // release would toggle straight back.
                    if event.state() != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        return;
                    }
                    // Two keys reach this handler now, and only one of them is
                    // held all the time. Dispatch by combination rather than by
                    // assumption: the attach key is a bare arrow, and toggling
                    // the whole product on it would be a bad surprise.
                    // tech.md 6.9 and 6.13.
                    match hotkey::role_of(app, shortcut) {
                        Some(hotkey::Role::Toggle) => commands::toggle_enabled(app.clone()),
                        // Off the handler before anything else. Attaching
                        // releases the very key that fired, and releasing one
                        // from in here deadlocks the app twice over: the
                        // plugin calls this on the main thread while holding
                        // its registry lock, and its `unregister` hops back to
                        // the main thread and then takes that same lock.
                        // tech.md 6.13.
                        Some(hotkey::Role::Attach) => {
                            let app = app.clone();
                            tauri::async_runtime::spawn_blocking(move || shots::attach(&app));
                        }
                        None => tracing::debug!("a combination nobody claims fired"),
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

            let state = Arc::new(state::AppState::new(
                config,
                provider,
                Arc::new(platform::SystemPasteboard::new()),
            ));
            app.manage(Arc::clone(&state));

            match app.get_webview_window(platform::ISLAND) {
                Some(window) => tracing::debug!(
                    url = %window.url().map(|u| u.to_string()).unwrap_or_default(),
                    "island window created"
                ),
                None => tracing::error!("island window missing"),
            }

            platform::convert_all(app.handle())?;

            // A borderless webview gets no safe area of its own, so Rust hands
            // the measured notch over to the route. tech.md 6.7.
            if let (Some(window), Some((height, width))) = (
                app.get_webview_window(platform::ISLAND),
                platform::active_notch(app.handle()),
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

            // Past dialogues live in Claude Code's own transcripts. Reading
            // them is the only way an island opened on a fresh start shows
            // anything at all. tech.md 6.11.
            // What the user called their sessions, before anything fills the
            // registry: the backfill must not raise what they put away.
            // tech.md 6.8.
            state.restore_sessions();

            backfill_sessions(app.handle(), Arc::clone(&state));
            rest_stale_sessions(app.handle(), Arc::clone(&state));
            watch_working_transcripts(app.handle(), Arc::clone(&state));

            hotkey::install(app.handle(), &toggle);

            // A screenshot lives on the pasteboard and nowhere else, so the
            // island watches for one and offers to carry it into a session.
            // tech.md 6.13.
            shots::watch(app.handle(), Arc::clone(&state));

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
            commands::start_sign_in,
            commands::submit_sign_in_code,
            commands::open_sign_in_page,
            commands::cancel_sign_in,
            commands::set_usage_enabled,
            commands::window_ready,
            commands::set_view,
            commands::island_bounds,
            commands::set_preview,
            commands::set_composing,
            commands::start_session,
            commands::continue_session,
            commands::send_message,
            commands::end_session,
            commands::stop_session,
            commands::get_sessions,
            commands::notify_enabled,
            commands::set_notify_enabled,
            commands::usage_badge,
            commands::set_usage_badge,
            commands::rename_session,
            commands::delete_session,
            commands::get_models,
            commands::get_defaults,
            commands::set_model,
            commands::set_effort,
            commands::set_mode,
            commands::set_thinking,
            commands::set_ultracode,
            commands::compact_session,
            commands::paste_shot,
            commands::open_bug_report,
            commands::choose_folder,
            commands::choose_files,
            commands::set_session_cwd,
            commands::answer_trust,
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
            commands::start_sign_in,
            commands::submit_sign_in_code,
            commands::open_sign_in_page,
            commands::cancel_sign_in,
            commands::set_usage_enabled,
            commands::window_ready,
            commands::set_view,
            commands::island_bounds,
            commands::set_preview,
            commands::set_composing,
            commands::start_session,
            commands::continue_session,
            commands::send_message,
            commands::end_session,
            commands::stop_session,
            commands::get_sessions,
            commands::notify_enabled,
            commands::set_notify_enabled,
            commands::usage_badge,
            commands::set_usage_badge,
            commands::rename_session,
            commands::delete_session,
            commands::get_models,
            commands::get_defaults,
            commands::set_model,
            commands::set_effort,
            commands::set_mode,
            commands::set_thinking,
            commands::set_ultracode,
            commands::compact_session,
            commands::paste_shot,
            commands::open_bug_report,
            commands::choose_folder,
            commands::choose_files,
            commands::set_session_cwd,
            commands::answer_trust,
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
            Box::new(peekle_usage::SystemCredentialStore::for_current_user()),
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
/// Puts a session that stopped reporting back to rest.
///
/// Nothing else can: `Working` arrives on a hook and leaves on a hook, so an
/// agent that died takes the island's spinner with it forever. tech.md 6.3.
/// Re-reads the transcript of every working session on a short tick.
///
/// The feed is refreshed from the file after each hook (tech.md 6.11), and
/// one end of a turn comes with no hook at all: an API error. Nothing fires
/// after it -- no `Stop`, nothing -- so without this the error stays unread
/// and the card spins `Working` until the stale sweep gives up on it ten
/// minutes later. The file says the turn is over; this is how the file gets
/// read. Only cards that are working, and only while they are: the cost of a
/// read (R-16) is paid by the sessions that have something to report.
fn watch_working_transcripts(app: &tauri::AppHandle, state: Arc<state::AppState>) {
    const WORKING_TICK: std::time::Duration = std::time::Duration::from_secs(2);

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(root) = peekle_core::transcripts::default_root() else {
            return;
        };
        let mut ticker = tokio::time::interval(WORKING_TICK);
        loop {
            ticker.tick().await;
            for card in state.sessions() {
                // And every session in the middle of a compact, for the same
                // reason: no hook fires when one ends, so the file is the
                // only thing that can say it did. tech.md 6.21.
                let watching = card.status == peekle_core::types::SessionStatus::Working
                    || card.compacting.is_some();
                if !watching {
                    continue;
                }
                let path = peekle_core::transcripts::transcript_path(
                    &root,
                    &card.session.cwd,
                    &card.session.session_id,
                );
                if !path.is_file() {
                    continue;
                }
                hooks::refresh_session(
                    handle.clone(),
                    Arc::clone(&state),
                    card.session.session_id.clone(),
                    path.to_string_lossy().into_owned(),
                );
            }
        }
    });
}

fn rest_stale_sessions(app: &tauri::AppHandle, state: Arc<state::AppState>) {
    const TICK: std::time::Duration = std::time::Duration::from_secs(60);
    /// Longer than any single tool call has a right to be silent for.
    const STALE_AFTER_MS: i64 = 10 * 60 * 1000;

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(TICK);
        loop {
            ticker.tick().await;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or_default();

            // A compact that ends in neither a boundary nor a refusal ends
            // here: the process behind it died, and nothing else will ever
            // take the orange sign off. tech.md 6.21.
            let rested = state.rest_stale_work(now, STALE_AFTER_MS);
            if rested.is_some() {
                tracing::debug!("a session stopped reporting, putting it back to idle");
            }

            let given_up = state.rest_stale_compacts(now, peekle_core::sessions::COMPACT_LIMIT_MS);
            if given_up.is_some() {
                tracing::debug!("a compact never ended, taking the sign off it");
            }

            // A stop request nothing ever answered: a button that never comes
            // back is worse than one that lets you ask twice. tech.md 6.5.
            let unanswered = state.rest_stale_stops(now, peekle_core::sessions::STOP_WAIT_MS);
            if unanswered.is_some() {
                tracing::debug!("a stop request went unanswered, giving the button back");
            }

            let Some(cards) = rested.or(given_up).or(unanswered) else {
                continue;
            };
            if let Err(err) = handle.emit(events::SESSIONS, &cards) {
                tracing::warn!(error = %err, "failed to emit rested sessions");
            }
        }
    });
}

/// Reads past sessions off disk and hands them to the registry.
///
/// Off the main thread and off the async runtime: it opens files, and nothing
/// about a hook may wait on a directory scan. Failure is silence, because a
/// missing history is not a reason to say anything to the user.
fn backfill_sessions(app: &tauri::AppHandle, state: Arc<state::AppState>) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(root) = peekle_core::transcripts::default_root() else {
            return;
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or_default();

        let Ok(cards) = tauri::async_runtime::spawn_blocking(move || {
            peekle_core::transcripts::scan(&root, now)
        })
        .await
        else {
            return;
        };
        if cards.is_empty() {
            return;
        }

        let found = cards.len();
        let cards = state.seed_sessions(cards);
        tracing::debug!(
            found,
            total = cards.len(),
            "backfilled sessions from transcripts"
        );

        if let Err(err) = handle.emit(events::SESSIONS, &cards) {
            tracing::warn!(error = %err, "failed to emit backfilled sessions");
        }
    });
}

fn poll_usage(app: &tauri::AppHandle, state: Arc<state::AppState>) {
    const EVERY: std::time::Duration = std::time::Duration::from_secs(300);

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        // How many network failures in a row, which is what decides how soon
        // to come back. A flat wait made a returning wifi cost half a minute
        // of dashes; the ladder catches it in seconds. tech.md 6.4.
        let mut misses: u32 = 0;

        loop {
            // Fetch first and sleep after, so a granted account has numbers a
            // second after launch rather than five minutes into the session.
            let wait = if state.may_fetch_usage() {
                let snapshot = commands::fetch_usage(&handle, &state).await;
                match snapshot.reason {
                    Some(peekle_core::types::UsageUnavailable::Network) => {
                        misses = misses.saturating_add(1);
                        peekle_usage::retry_after(misses)
                    }
                    // Asked to wait, so wait exactly that long: coming back
                    // sooner is what got us rate limited, and the server said
                    // how long in its own header. Never shorter than the
                    // ordinary interval. tech.md 6.4.
                    Some(peekle_core::types::UsageUnavailable::RateLimited) => {
                        misses = 0;
                        match snapshot.retry_after_ms {
                            Some(ms) if ms > 0 => {
                                let asked = std::time::Duration::from_millis(ms as u64);
                                tracing::info!(
                                    seconds = asked.as_secs(),
                                    "the usage endpoint asked to be left alone"
                                );
                                asked.max(EVERY)
                            }
                            _ => EVERY,
                        }
                    }
                    _ => {
                        misses = 0;
                        EVERY
                    }
                }
            } else {
                misses = 0;
                EVERY
            };

            tokio::time::sleep(wait).await;
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
