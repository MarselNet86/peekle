/**
 * The only module that touches `@tauri-apps/api`. Command names and event
 * names are the literal tables of tech.md 6.5 and 6.6.
 *
 * Outside the app shell, under `vite dev` or Playwright, there is no Tauri to
 * talk to. Commands become no-ops and events never fire, so every route still
 * renders on its own.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import type { Delivery } from '$lib/types/generated/Delivery';
import type { IslandView } from '$lib/types/generated/IslandView';
import type { PeekleState } from '$lib/types/generated/PeekleState';
import type { PromptAnswer } from '$lib/types/generated/PromptAnswer';
import type { PromptRequest } from '$lib/types/generated/PromptRequest';
import type { SessionCard } from '$lib/types/generated/SessionCard';
import type { TaskItem } from '$lib/types/generated/TaskItem';
import type { ToastRequest } from '$lib/types/generated/ToastRequest';
import type { UsageSnapshot } from '$lib/types/generated/UsageSnapshot';
import type { PromptOutcome } from '$lib/types/generated/PromptOutcome';

export const EVENTS = {
  promptOpen: 'peekle://prompt-open',
  promptClose: 'peekle://prompt-close',
  sessions: 'peekle://sessions',
  tasks: 'peekle://tasks',
  usage: 'peekle://usage',
  enabled: 'peekle://enabled',
  toast: 'peekle://toast',
  view: 'peekle://view',
  notch: 'peekle://notch',
  driving: 'peekle://driving',
} as const;

export function hasTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T | null> {
  if (!hasTauri()) return null;
  return invoke<T>(command, args);
}

export const commands = {
  getState: () => call<PeekleState>('get_state'),
  answerPrompt: (answer: PromptAnswer) => call<void>('answer_prompt', { answer }),
  dismissPrompt: (promptId: string) => call<void>('dismiss_prompt', { promptId }),
  setEnabled: (enabled: boolean) => call<void>('set_enabled', { enabled }),
  refreshUsage: () => call<UsageSnapshot>('refresh_usage'),
  requestUsageAccess: () => call<UsageSnapshot>('request_usage_access'),
  setUsageEnabled: (enabled: boolean) => call<void>('set_usage_enabled', { enabled }),
  windowReady: (label: string) => call<void>('window_ready', { label }),
  setView: (view: IslandView) => call<void>('set_view', { view }),
  islandBounds: (width: number, height: number) => call<void>('island_bounds', { width, height }),
  sendMessage: (sessionId: string, text: string) =>
    call<Delivery>('send_message', { sessionId, text }),
  deliveryFor: (sessionId: string) => call<Delivery>('delivery_for', { sessionId }),
  setTakeover: (sessionId: string, on: boolean) => call<void>('set_takeover', { sessionId, on }),
  renameSession: (sessionId: string, title: string) =>
    call<void>('rename_session', { sessionId, title }),
  hideSession: (sessionId: string) => call<void>('hide_session', { sessionId }),
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
  onEnabled: (handler: (payload: { enabled: boolean }) => void) =>
    on<{ enabled: boolean }>(EVENTS.enabled, handler),
  onToast: (handler: (toast: ToastRequest) => void) => on<ToastRequest>(EVENTS.toast, handler),
  onView: (handler: (view: IslandView) => void) => on<IslandView>(EVENTS.view, handler),
  onNotch: (handler: (notch: { height: number; width: number }) => void) =>
    on<{ height: number; width: number }>(EVENTS.notch, handler),
  onDriving: (handler: (payload: { driving: string | null }) => void) =>
    on<{ driving: string | null }>(EVENTS.driving, handler),
};
