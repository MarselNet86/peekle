//! Every line Rust says to a person, in the language they chose. tech.md 6.28.
//!
//! Each line is a function of `Language`, so the language is read from the
//! config at the moment of speaking and a change needs no telling. English is
//! the language the copy is written in, and its lines are the exact strings the
//! app said before there was a choice: tests on both sides compare them.
//!
//! What is not here stays in English on purpose: the conversation, model and
//! mode names the CLI owns, terminal commands, paths and project names, and
//! the reason words Claude Code itself prints. tech.md 6.28.

use peekle_core::auth::SignInError;
use peekle_core::pty::PtyError;
use peekle_core::types::Language;

/// Picks one of two spellings of the same line.
const fn say(language: Language, en: &'static str, ru: &'static str) -> &'static str {
    match language {
        Language::En => en,
        Language::Ru => ru,
    }
}

// The toggle. tech.md 6.9.

pub fn peekle_on(l: Language) -> &'static str {
    say(l, "Peekle is ON", "Peekle включён")
}

pub fn peekle_off(l: Language) -> &'static str {
    say(l, "Peekle is OFF", "Peekle выключен")
}

pub fn hotkey_invalid(l: Language) -> &'static str {
    say(
        l,
        "Hotkey is not a valid combination",
        "Такое сочетание клавиш не подходит",
    )
}

/// The combination is spelled the way the key caps are, in either language.
pub fn hotkey_taken(l: Language, combination: &str) -> String {
    match l {
        Language::En => format!("{combination} is taken by another app"),
        Language::Ru => format!("{combination} занято другим приложением"),
    }
}

// The account screen. tech.md 6.16.

pub fn no_command_to_copy(l: Language) -> &'static str {
    say(l, "there is no command to copy", "Копировать нечего")
}

pub fn could_not_copy_command(l: Language) -> &'static str {
    say(
        l,
        "could not copy the command",
        "Не удалось скопировать команду",
    )
}

pub fn could_not_open_browser(l: Language) -> &'static str {
    say(
        l,
        "could not open your browser",
        "Не удалось открыть браузер",
    )
}

pub fn no_sign_in_page(l: Language) -> &'static str {
    say(l, "there is no sign-in page to open", "Страницы входа нет")
}

pub fn cannot_sign_out_here(l: Language) -> &'static str {
    say(
        l,
        "This Claude Code cannot sign out from Peekle.",
        "Эта версия Claude Code не может выйти из аккаунта через Peekle.",
    )
}

pub fn could_not_sign_out(l: Language) -> &'static str {
    say(
        l,
        "Claude Code could not sign out.",
        "Claude Code не смог выйти из аккаунта.",
    )
}

pub fn not_installed(l: Language) -> &'static str {
    say(
        l,
        "Claude Code is not installed on this Mac.",
        "Claude Code не установлен на этом Mac.",
    )
}

pub fn cannot_sign_in_here(l: Language) -> &'static str {
    say(
        l,
        "This version of Claude Code cannot sign in from Peekle.",
        "Эта версия Claude Code не может войти через Peekle.",
    )
}

pub fn could_not_start_sign_in(l: Language) -> &'static str {
    say(
        l,
        "Claude Code could not start the sign-in.",
        "Claude Code не смог начать вход.",
    )
}

// Starting and talking to sessions. tech.md 6.5.

pub fn folder_missing(l: Language) -> &'static str {
    say(l, "That folder does not exist", "Такой папки нет")
}

pub fn session_gone(l: Language) -> &'static str {
    say(l, "That session is gone", "Этой сессии больше нет")
}

pub fn session_has_field(l: Language) -> &'static str {
    say(
        l,
        "That session already has an input field",
        "У этой сессии уже есть поле ввода",
    )
}

pub fn not_installed_where_found(l: Language) -> &'static str {
    say(
        l,
        "Claude Code is not installed where Peekle can find it",
        "Peekle не находит установленный Claude Code",
    )
}

pub fn only_own_sessions(l: Language) -> &'static str {
    say(
        l,
        "Peekle can only talk to sessions it started",
        "Peekle пишет только в сессии, которые запустил сам",
    )
}

pub fn no_longer_listening(l: Language) -> &'static str {
    say(
        l,
        "That session is no longer listening",
        "Эта сессия больше не слушает",
    )
}

// Model, mode, thinking and compact. tech.md 6.15, 6.19 and 6.20.

pub fn not_a_model(l: Language) -> &'static str {
    say(l, "That is not a model name", "Это не название модели")
}

pub fn mode_only_own(l: Language) -> &'static str {
    say(
        l,
        "Peekle can only set the mode of sessions it started",
        "Peekle меняет режим только у сессий, которые запустил сам",
    )
}

pub fn mode_unseen(l: Language) -> &'static str {
    say(
        l,
        "Peekle has not seen which mode this session is in yet",
        "Peekle ещё не знает, в каком режиме эта сессия",
    )
}

pub fn mode_off_cycle(l: Language) -> &'static str {
    say(
        l,
        "That mode is not on the cycle Shift+Tab walks",
        "Shift+Tab не переключает в этот режим",
    )
}

pub fn thinking_only_own(l: Language) -> &'static str {
    say(
        l,
        "Peekle can only set this on sessions it starts",
        "Peekle задаёт это только сессиям, которые запускает сам",
    )
}

pub fn thinking_at_start(l: Language) -> &'static str {
    say(
        l,
        "Thinking is set when a session starts",
        "Размышления задаются при запуске сессии",
    )
}

pub fn nothing_to_compact(l: Language) -> &'static str {
    say(l, "There is nothing to compact yet", "Сжимать пока нечего")
}

// Trust and Stop. tech.md 6.5.

pub fn nothing_asking_folder(l: Language) -> &'static str {
    say(
        l,
        "Nothing is asking about a folder there",
        "Здесь никто не спрашивает о папке",
    )
}

pub fn chat_gone(l: Language) -> &'static str {
    say(l, "That chat is gone", "Этого чата больше нет")
}

pub fn nothing_running(l: Language) -> &'static str {
    say(l, "Nothing is running there", "Там ничего не выполняется")
}

pub fn held_elsewhere(l: Language) -> &'static str {
    say(
        l,
        "Another app is holding that chat and takes no messages",
        "Этот чат занят другим приложением и не принимает сообщения",
    )
}

pub fn refused_the_stop(l: Language) -> &'static str {
    say(
        l,
        "That chat did not take the stop",
        "Этот чат не принял остановку",
    )
}

// Folders and files. tech.md 6.23 and 6.25.

pub fn choose_folder_title(l: Language) -> &'static str {
    say(
        l,
        "Choose the folder this chat works in",
        "Выберите папку для этого чата",
    )
}

pub fn folder_dialog_gone(l: Language) -> &'static str {
    say(
        l,
        "the folder dialog went away",
        "Окно выбора папки закрылось",
    )
}

pub fn folder_unreachable(l: Language) -> &'static str {
    say(
        l,
        "That folder cannot be reached",
        "К этой папке нет доступа",
    )
}

pub fn attach_files_title(l: Language) -> &'static str {
    say(
        l,
        "Attach files to this message",
        "Прикрепите файлы к сообщению",
    )
}

pub fn file_dialog_gone(l: Language) -> &'static str {
    say(
        l,
        "the file dialog went away",
        "Окно выбора файлов закрылось",
    )
}

pub fn aim_only_own(l: Language) -> &'static str {
    say(
        l,
        "Peekle can only aim sessions it started",
        "Peekle меняет папку только у сессий, которые запустил сам",
    )
}

pub fn already_running_in_folder(l: Language) -> &'static str {
    say(
        l,
        "This chat is already running in its folder",
        "Этот чат уже работает в своей папке",
    )
}

pub fn already_begun(l: Language) -> &'static str {
    say(l, "This chat has already begun", "Этот чат уже начат")
}

// Screenshots. tech.md 6.13.

pub fn shot_gone(l: Language) -> &'static str {
    say(l, "The screenshot is gone", "Снимка экрана больше нет")
}

pub fn shot_nowhere(l: Language) -> &'static str {
    say(
        l,
        "Nowhere to save the screenshot",
        "Снимок экрана некуда сохранить",
    )
}

pub fn shot_not_saved(l: Language) -> &'static str {
    say(
        l,
        "Could not save the screenshot",
        "Не удалось сохранить снимок экрана",
    )
}

// Notices and the bug report. tech.md 6.2, 6.17 and 6.22.

pub fn notice_title(l: Language) -> &'static str {
    say(l, "Claude is waiting for you", "Claude ждёт вас")
}

pub fn notices_switched_on(l: Language) -> &'static str {
    say(
        l,
        "Turn notices are on. This is what one looks like.",
        "Уведомления о ходах включены. Вот так они выглядят.",
    )
}

pub fn notice_refused(l: Language) -> &'static str {
    say(
        l,
        "macOS would not show a notification",
        "macOS не показывает уведомление",
    )
}

pub fn could_not_open_telegram(l: Language) -> &'static str {
    say(l, "could not open Telegram", "Не удалось открыть Telegram")
}

pub fn claude_needs_you(l: Language) -> &'static str {
    say(l, "Claude needs you", "Вы нужны Claude")
}

/// A `Notification` hook's message, in the person's language.
///
/// The CLI writes it in English and in words of its own, so only the exact
/// texts captured in `fixtures/hooks/` are known and translated. Anything else
/// passes through as the CLI said it: a guessed translation of a message
/// nobody has seen would be worse than the original. tech.md 6.2 and 6.28.
pub fn hook_notice(l: Language, message: &str) -> String {
    let known = match (l, message) {
        (Language::Ru, "Claude is waiting for your input") => Some("Claude ждёт вашего ответа"),
        _ => None,
    };
    known.map_or_else(|| message.to_string(), str::to_string)
}

/// A message `peekle-core` hands the person, in the person's language.
///
/// The core crate speaks English, so its fixed lines are mapped here at the
/// boundary. The `could not sign out: <reason>` form keeps the reason words:
/// they are Claude Code's own. Unknown text passes through unchanged.
/// tech.md 6.16 and 6.28.
pub fn core_message(l: Language, message: &str) -> String {
    const SIGN_OUT_BECAUSE: &str = "Claude Code could not sign out: ";
    if l == Language::En {
        return message.to_string();
    }
    let fixed = match message {
        "Claude Code could not sign out." => Some(could_not_sign_out(l)),
        "Claude Code is not installed on this Mac." => Some(not_installed(l)),
        "Claude Code did not finish signing out." => Some(say(
            l,
            "Claude Code did not finish signing out.",
            "Claude Code не завершил выход из аккаунта.",
        )),
        "Claude Code could not be started." => Some(say(
            l,
            "Claude Code could not be started.",
            "Не удалось запустить Claude Code.",
        )),
        "Claude Code did not finish signing in." => Some(say(
            l,
            "Claude Code did not finish signing in.",
            "Claude Code не завершил вход.",
        )),
        _ => None,
    };
    if let Some(fixed) = fixed {
        return fixed.to_string();
    }
    match message.strip_prefix(SIGN_OUT_BECAUSE) {
        Some(reason) => format!("Claude Code не смог выйти из аккаунта: {reason}"),
        None => message.to_string(),
    }
}

/// A sign-in that went nowhere, in the person's language. English is the
/// error's own `Display`, as it was. tech.md 6.16.
pub fn sign_in_error(l: Language, err: &SignInError) -> String {
    match (l, err) {
        (Language::En, err) => err.to_string(),
        (Language::Ru, SignInError::NoBinary) => not_installed(l).to_string(),
        (Language::Ru, SignInError::NotRunning) => "Вход не запущен".to_string(),
        (Language::Ru, SignInError::EmptyCode) => "Код пустой".to_string(),
        (Language::Ru, SignInError::Pty(detail)) => format!("Вход не удался: {detail}"),
    }
}

/// A session that would not start or take a key, in the person's language.
/// English is the error's own `Display`, as it was. tech.md 6.5.
pub fn pty_error(l: Language, err: &PtyError) -> String {
    match (l, err) {
        (Language::En, err) => err.to_string(),
        (Language::Ru, PtyError::NoCwd) => folder_missing(l).to_string(),
        (Language::Ru, PtyError::NoBinary) => not_installed_where_found(l).to_string(),
        (Language::Ru, PtyError::NotOwned) => only_own_sessions(l).to_string(),
        (Language::Ru, PtyError::Pty(detail)) => {
            format!("Терминал сессии не ответил: {detail}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Line = fn(Language) -> &'static str;

    /// Every fixed line, so a new one cannot slip in untranslated.
    const LINES: &[Line] = &[
        peekle_on,
        peekle_off,
        hotkey_invalid,
        no_command_to_copy,
        could_not_copy_command,
        could_not_open_browser,
        no_sign_in_page,
        cannot_sign_out_here,
        could_not_sign_out,
        not_installed,
        cannot_sign_in_here,
        could_not_start_sign_in,
        folder_missing,
        session_gone,
        session_has_field,
        not_installed_where_found,
        only_own_sessions,
        no_longer_listening,
        not_a_model,
        mode_only_own,
        mode_unseen,
        mode_off_cycle,
        thinking_only_own,
        thinking_at_start,
        nothing_to_compact,
        nothing_asking_folder,
        chat_gone,
        nothing_running,
        held_elsewhere,
        refused_the_stop,
        choose_folder_title,
        folder_dialog_gone,
        folder_unreachable,
        attach_files_title,
        file_dialog_gone,
        aim_only_own,
        already_running_in_folder,
        already_begun,
        shot_gone,
        shot_nowhere,
        shot_not_saved,
        notice_title,
        notices_switched_on,
        notice_refused,
        could_not_open_telegram,
        claude_needs_you,
    ];

    /// English is what the app said before there was a choice, byte for byte:
    /// the webview's tests and the kitchen sink still carry these. tech.md 6.28.
    #[test]
    fn english_is_the_copy_as_it_was() {
        let en = Language::En;
        assert_eq!(peekle_on(en), "Peekle is ON");
        assert_eq!(peekle_off(en), "Peekle is OFF");
        assert_eq!(folder_missing(en), "That folder does not exist");
        assert_eq!(session_gone(en), "That session is gone");
        assert_eq!(nothing_to_compact(en), "There is nothing to compact yet");
        assert_eq!(nothing_running(en), "Nothing is running there");
        assert_eq!(
            held_elsewhere(en),
            "Another app is holding that chat and takes no messages"
        );
        assert_eq!(refused_the_stop(en), "That chat did not take the stop");
        assert_eq!(could_not_sign_out(en), "Claude Code could not sign out.");
        assert_eq!(notice_title(en), "Claude is waiting for you");
        assert_eq!(
            notices_switched_on(en),
            "Turn notices are on. This is what one looks like."
        );
        assert_eq!(
            choose_folder_title(en),
            "Choose the folder this chat works in"
        );
        assert_eq!(attach_files_title(en), "Attach files to this message");
        assert_eq!(claude_needs_you(en), "Claude needs you");
        assert_eq!(hotkey_taken(en, "⌥Space"), "⌥Space is taken by another app");
    }

    /// Every line says something in both languages, and the Russian is not the
    /// English left behind. tech.md 6.28.
    #[test]
    fn every_line_is_translated() {
        for line in LINES {
            let (en, ru) = (line(Language::En), line(Language::Ru));
            assert!(!en.trim().is_empty(), "empty English line");
            assert!(!ru.trim().is_empty(), "empty Russian line for {en:?}");
            assert_ne!(en, ru, "untranslated line {en:?}");
        }
        assert_ne!(
            hotkey_taken(Language::En, "⌥Space"),
            hotkey_taken(Language::Ru, "⌥Space")
        );
        assert!(hotkey_taken(Language::Ru, "⌥Space").starts_with("⌥Space "));
    }

    /// The core's fixed lines are translated, the CLI's reason words are kept,
    /// and anything unknown goes through as it came. tech.md 6.28.
    #[test]
    fn core_messages_are_mapped_at_the_boundary() {
        let ru = Language::Ru;
        assert_eq!(
            core_message(ru, "Claude Code could not sign out."),
            "Claude Code не смог выйти из аккаунта."
        );
        assert_eq!(
            core_message(ru, "Claude Code did not finish signing in."),
            "Claude Code не завершил вход."
        );
        assert_eq!(
            core_message(ru, "Claude Code did not finish signing out."),
            "Claude Code не завершил выход из аккаунта."
        );
        assert_eq!(
            core_message(ru, "Claude Code is not installed on this Mac."),
            "Claude Code не установлен на этом Mac."
        );
        assert_eq!(
            core_message(ru, "Claude Code could not be started."),
            "Не удалось запустить Claude Code."
        );
        assert_eq!(
            core_message(
                ru,
                "Claude Code could not sign out: could not reach in time"
            ),
            "Claude Code не смог выйти из аккаунта: could not reach in time"
        );
        assert_eq!(
            core_message(ru, "OAuth error: invalid_grant"),
            "OAuth error: invalid_grant"
        );
        for known in [
            "Claude Code could not sign out.",
            "Claude Code could not sign out: could not reach in time",
            "Something nobody has seen",
        ] {
            assert_eq!(core_message(Language::En, known), known);
        }
    }

    /// The captured `Notification` message is translated; a message not in the
    /// fixtures passes through. tech.md 6.2.
    #[test]
    fn hook_notices_translate_only_what_was_captured() {
        let captured = "Claude is waiting for your input";
        assert_eq!(hook_notice(Language::En, captured), captured);
        assert_eq!(
            hook_notice(Language::Ru, captured),
            "Claude ждёт вашего ответа"
        );
        let unseen = "Claude needs your permission to use Bash";
        assert_eq!(hook_notice(Language::Ru, unseen), unseen);
    }

    /// English keeps each core error's own words; Russian replaces them.
    #[test]
    fn core_errors_keep_their_english_and_gain_russian() {
        assert_eq!(
            sign_in_error(Language::En, &SignInError::EmptyCode),
            SignInError::EmptyCode.to_string()
        );
        assert_eq!(
            sign_in_error(Language::Ru, &SignInError::EmptyCode),
            "Код пустой"
        );
        assert_eq!(
            pty_error(Language::En, &PtyError::NotOwned),
            PtyError::NotOwned.to_string()
        );
        assert_eq!(pty_error(Language::Ru, &PtyError::NoCwd), "Такой папки нет");
        assert_eq!(
            pty_error(Language::Ru, &PtyError::Pty("eof".into())),
            "Терминал сессии не ответил: eof"
        );
    }
}
