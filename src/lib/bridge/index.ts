/**
 * The only module that touches `@tauri-apps/api`. Command names and event
 * names are the literal tables of tech.md 6.5 and 6.6.
 *
 * Outside the app shell, under `vite dev` or Playwright, there is no Tauri to
 * talk to. Commands become no-ops and events never fire, so every route still
 * renders on its own.
 */

import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import type { IslandView } from '$lib/types/generated/IslandView';
import type { PermissionMode } from '$lib/types/generated/PermissionMode';
import type { PeekleState } from '$lib/types/generated/PeekleState';
import type { PromptAnswer } from '$lib/types/generated/PromptAnswer';
import type { AgentSetup } from '$lib/types/generated/AgentSetup';
import type { Effort } from '$lib/types/generated/Effort';
import type { ModelChoice } from '$lib/types/generated/ModelChoice';
import type { PromptRequest } from '$lib/types/generated/PromptRequest';
import type { SessionCard } from '$lib/types/generated/SessionCard';
import type { SessionRef } from '$lib/types/generated/SessionRef';
import type { TaskItem } from '$lib/types/generated/TaskItem';
import type { ToastRequest } from '$lib/types/generated/ToastRequest';
import type { UsageSnapshot } from '$lib/types/generated/UsageSnapshot';
import type { PromptOutcome } from '$lib/types/generated/PromptOutcome';
import type { ShotOffer } from '$lib/types/generated/ShotOffer';
import type { SignInState } from '$lib/types/generated/SignInState';

export const EVENTS = {
  promptOpen: 'peekle://prompt-open',
  promptClose: 'peekle://prompt-close',
  sessions: 'peekle://sessions',
  tasks: 'peekle://tasks',
  usage: 'peekle://usage',
  signIn: 'peekle://sign-in',
  enabled: 'peekle://enabled',
  toast: 'peekle://toast',
  view: 'peekle://view',
  notch: 'peekle://notch',
  shot: 'peekle://shot',
  shotAttached: 'peekle://shot-attached',
} as const;

export function hasTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T | null> {
  if (!hasTauri()) return null;
  return invoke<T>(command, args);
}

/**
 * A file in the shots cache, as a URL the webview may load. tech.md 6.13.
 *
 * Outside the app shell there is no `asset:` to convert to, and an empty
 * string is the honest answer: the chip has a frame to fall back on.
 */
export function fileSrc(path: string): string {
  return hasTauri() ? convertFileSrc(path) : '';
}

export const commands = {
  getState: () => call<PeekleState>('get_state'),
  answerPrompt: (answer: PromptAnswer) => call<void>('answer_prompt', { answer }),
  dismissPrompt: (promptId: string) => call<void>('dismiss_prompt', { promptId }),
  setEnabled: (enabled: boolean) => call<void>('set_enabled', { enabled }),
  refreshUsage: () => call<UsageSnapshot>('refresh_usage'),
  requestUsageAccess: () => call<UsageSnapshot>('request_usage_access'),
  startSignIn: () => call<SignInState>('start_sign_in'),
  submitSignInCode: (code: string) => call<SignInState>('submit_sign_in_code', { code }),
  openSignInPage: () => call<void>('open_sign_in_page'),
  cancelSignIn: () => call<void>('cancel_sign_in'),
  // The address lives in Rust and this takes no argument, so a page cannot
  // point it anywhere of its own. tech.md 6.22.
  openBugReport: () => call<void>('open_bug_report'),
  // The macOS folder dialog. `null` is a cancel, which is not an error and not
  // an event. tech.md 6.23.
  chooseFolder: () => call<string | null>('choose_folder'),
  // The files to attach to the message being written. An empty list is a
  // cancel. Nothing is copied: the paths travel as they are. tech.md 6.25.
  chooseFiles: () => call<string[]>('choose_files'),
  setSessionCwd: (sessionId: string, cwd: string) =>
    call<void>('set_session_cwd', { sessionId, cwd }),
  // The CLI's own question about the folder, answered by the person in the
  // island. tech.md 6.24.
  answerTrust: (sessionId: string, trust: boolean) =>
    call<void>('answer_trust', { sessionId, trust }),
  setUsageEnabled: (enabled: boolean) => call<void>('set_usage_enabled', { enabled }),
  windowReady: (label: string) => call<void>('window_ready', { label }),
  setView: (view: IslandView) => call<void>('set_view', { view }),
  islandBounds: (width: number, height: number) => call<void>('island_bounds', { width, height }),
  // A shot opened at full size. No view change: all it buys is the island
  // staying up while the picture is on screen. tech.md 6.13.
  setPreview: (open: boolean) => call<void>('set_preview', { open }),
  // The field is being written in, or holds something unsent: the island
  // is in the person's hands and nothing puts it away or writes over it
  // but them. tech.md 6.7.
  setComposing: (active: boolean) => call<void>('set_composing', { active }),
  startSession: (cwd: string) => call<SessionRef>('start_session', { cwd }),
  // Forks an observed chat into an owned one, the way Desktop opens an
  // existing chat: claude --resume=<id> in a pty of our own. tech.md 6.5.
  // The first message rides with the fork: a TUI that is still starting
  // swallows anything written into it, so it is handed to the spawn.
  continueSession: (sessionId: string, text: string, shots: string[] = []) =>
    call<SessionRef>('continue_session', { sessionId, text, shots }),
  // `shots` are the screenshots attached to this message. They travel as
  // lines of the message itself, and Rust composes them: what goes on the wire
  // is a delivery detail and belongs next to the pty. tech.md 6.13.
  sendMessage: (sessionId: string, text: string, shots: string[] = []) =>
    call<void>('send_message', { sessionId, text, shots }),
  endSession: (sessionId: string) => call<void>('end_session', { sessionId }),
  // Stops the running turn: Esc into a pty we own, or a request to stop into
  // the inbox of a live process that is not ours. tech.md 6.5.
  stopSession: (sessionId: string) => call<void>('stop_session', { sessionId }),
  renameSession: (sessionId: string, title: string) =>
    call<void>('rename_session', { sessionId, title }),
  hideSession: (sessionId: string) => call<void>('hide_session', { sessionId }),
  // The row under the field. A slash command is text, so all three take the
  // channel a reply takes: written into the pty of a session we own.
  // tech.md 6.15.
  getModels: () => call<ModelChoice[]>('get_models'),
  // What a session that has not answered yet is running as, so the row can be
  // aimed before the first turn rather than after it. tech.md 6.15.
  getDefaults: () => call<AgentSetup>('get_defaults'),
  setModel: (sessionId: string, model: string) => call<void>('set_model', { sessionId, model }),
  setEffort: (sessionId: string, effort: Effort) => call<void>('set_effort', { sessionId, effort }),
  compactSession: (sessionId: string) => call<void>('compact_session', { sessionId }),

  // ⌘V in the field, when the clipboard holds a picture rather than text. The
  // PNG becomes a file and arrives back as `shot-attached`, exactly as the
  // attach key's image does. Null means the pasteboard held no image after
  // all. tech.md 6.13.
  pasteShot: (sessionId: string) => call<string | null>('paste_shot', { sessionId }),
  // The cards as they stand. Asked for once on mount: an event sent before
  // the subscription landed reaches nobody, and the backfill emits at start.
  // tech.md 6.1 and section 8.
  getSessions: () => call<SessionCard[]>('get_sessions'),
  // The banner a finished turn puts on the screen. Reading the switch costs
  // nothing; flicking it on posts the first banner, which is what makes macOS
  // ask for the permission at all. tech.md 6.17.
  notifyEnabled: () => call<boolean>('notify_enabled'),
  setNotifyEnabled: (on: boolean) => call<void>('set_notify_enabled', { on }),

  // The permission mode, and only before the session has answered: it is a
  // spawn flag, not a line. tech.md 6.19.
  setMode: (sessionId: string, mode: PermissionMode) => call<void>('set_mode', { sessionId, mode }),

  // Thinking is an environment variable the process reads once, so it is set
  // before a session runs and read after. tech.md 6.20.
  setThinking: (sessionId: string, on: boolean) => call<void>('set_thinking', { sessionId, on }),

  // `/effort ultracode`: xhigh plus dynamic workflows, this session only. Its
  // own call because `--effort` does not take it. tech.md 6.15.
  setUltracode: (sessionId: string) => call<void>('set_ultracode', { sessionId }),

  // Whether a resting island shows the percent when the window steps into a
  // new ten. Nothing to ask the system for, so nothing to fail. tech.md 6.18.
  usageBadge: () => call<boolean>('usage_badge'),
  setUsageBadge: (on: boolean) => call<void>('set_usage_badge', { on }),
};

async function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  if (!hasTauri()) return () => {};
  return listen<T>(event, (message) => handler(message.payload));
}

export const events = {
  onPromptOpen: (handler: (request: PromptRequest) => void) =>
    on<PromptRequest>(EVENTS.promptOpen, handler),
  onPromptClose: (handler: (payload: { prompt_id: string; outcome: PromptOutcome }) => void) =>
    on<{ prompt_id: string; outcome: PromptOutcome }>(EVENTS.promptClose, handler),
  onSessions: (handler: (sessions: SessionCard[]) => void) =>
    on<SessionCard[]>(EVENTS.sessions, handler),
  onTasks: (handler: (tasks: TaskItem[]) => void) => on<TaskItem[]>(EVENTS.tasks, handler),
  onUsage: (handler: (usage: UsageSnapshot) => void) => on<UsageSnapshot>(EVENTS.usage, handler),
  onSignIn: (handler: (state: SignInState) => void) => on<SignInState>(EVENTS.signIn, handler),
  onEnabled: (handler: (payload: { enabled: boolean }) => void) =>
    on<{ enabled: boolean }>(EVENTS.enabled, handler),
  onToast: (handler: (toast: ToastRequest) => void) => on<ToastRequest>(EVENTS.toast, handler),
  onView: (handler: (view: IslandView) => void) => on<IslandView>(EVENTS.view, handler),
  onNotch: (handler: (notch: { height: number; width: number }) => void) =>
    on<{ height: number; width: number }>(EVENTS.notch, handler),
  onShot: (handler: (payload: { offer: ShotOffer | null }) => void) =>
    on<{ offer: ShotOffer | null }>(EVENTS.shot, handler),
  onShotAttached: (handler: (payload: { session_id: string; path: string }) => void) =>
    on<{ session_id: string; path: string }>(EVENTS.shotAttached, handler),
};
