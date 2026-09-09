/**
 * What the row under the field says. Pure, so the tests can hammer it.
 * tech.md 6.15.
 */

import type { AgentSetup } from '$lib/types/generated/AgentSetup';
import type { Effort } from '$lib/types/generated/Effort';
import type { ModelChoice } from '$lib/types/generated/ModelChoice';
import type { PermissionMode } from '$lib/types/generated/PermissionMode';

/** One row of a menu. */
/** Which sign a row wears, when it wears one. The names are the icon set's,
 * not ours: a hand drawn by hand comes out a blob. tech.md 9. */
export type PickIcon = 'hand' | 'code' | 'plan' | 'bolt';

export type PickOption = {
  /** What goes back to the caller when the row is picked. */
  id: string;
  label: string;
  /** One line under the label, for a menu whose rows need explaining.
   * tech.md 6.19. */
  hint?: string;
  icon?: PickIcon;
};

/** What `/effort <level>` is called on screen. */
const EFFORT_LABELS: Record<Effort, string> = {
  Low: 'Low',
  Medium: 'Medium',
  High: 'High',
  XHigh: 'XHigh',
  Max: 'Max',
};

export function effortLabel(effort: Effort | null): string {
  return effort === null ? '' : EFFORT_LABELS[effort];
}

/**
 * What the model button says.
 *
 * The catalog names every model it knows; one it does not is shown by its own
 * id rather than by a blank, because a session that answers with something is
 * never honestly described by nothing. Nothing at all -- a settings file that
 * names no model -- comes back empty, and the row says `Model`: there is a
 * choice to make and nothing to report. tech.md 6.15.
 */
export function modelLabel(agent: AgentSetup | null): string {
  return agent?.label ?? agent?.model ?? '';
}

export function modelOptions(models: ModelChoice[]): PickOption[] {
  return models.map((model) => ({ id: model.alias, label: model.label }));
}

export function effortOptions(agent: AgentSetup | null): PickOption[] {
  return (agent?.levels ?? []).map((level) => ({ id: level, label: EFFORT_LABELS[level] }));
}

/**
 * The permission modes, by Claude Code's own names and in its own order.
 *
 * Four of the six the CLI accepts. `bypassPermissions` and `dontAsk` are not
 * offered: a mode that asks for nothing is not something to hand over in a
 * menu, and a session already in one still reads as it. tech.md 6.19.
 */
const MODE_ROWS: ReadonlyArray<{
  id: PermissionMode;
  label: string;
  hint: string;
  icon: PickIcon;
}> = [
  { id: 'Manual', label: 'Manual', hint: 'Asks before every edit', icon: 'hand' },
  {
    id: 'AcceptEdits',
    label: 'Edit automatically',
    hint: 'Edits go through, everything else asks',
    icon: 'code',
  },
  { id: 'Plan', label: 'Plan', hint: 'Reads and plans, changes nothing', icon: 'plan' },
  { id: 'Auto', label: 'Auto', hint: 'Approves what passes its safety check', icon: 'bolt' },
];

/** What each mode is called on screen, including the two never offered. */
const MODE_LABELS: Record<PermissionMode, string> = {
  Manual: 'Manual',
  AcceptEdits: 'Edit automatically',
  Plan: 'Plan',
  Auto: 'Auto',
  Bypass: 'No permissions',
  DontAsk: 'Never asks',
};

export function modeOptions(): PickOption[] {
  return MODE_ROWS.map((row) => ({
    id: row.id,
    label: row.label,
    hint: row.hint,
    icon: row.icon,
  }));
}

/** The sign a mode wears, wherever it is drawn. The two that are never
 * offered wear the hand: they are still permissions. tech.md 6.19. */
export function modeIcon(mode: PermissionMode | null): PickIcon {
  return MODE_ROWS.find((row) => row.id === mode)?.icon ?? 'hand';
}

export function modeLabel(mode: PermissionMode | null): string {
  return mode ? MODE_LABELS[mode] : '';
}

/**
 * Which menu row is the one the session is on.
 *
 * Matched by catalog id and not by label: two rows can read the same and only
 * one of them is what the transcript named.
 */
export function currentModel(agent: AgentSetup | null, models: ModelChoice[]): string {
  const id = agent?.model;
  if (!id) return '';
  return models.find((model) => id === model.id || id.startsWith(model.id))?.alias ?? '';
}

/**
 * How full the context is, as a whole number for the ring's tooltip.
 *
 * The invitation to click is only there when clicking does something. A ring
 * that cannot be compacted still reports the number, because the number is
 * true whoever is driving the session. tech.md 6.15.
 */
export function contextLabel(agent: AgentSetup | null, canCompact = true): string {
  if (!agent) return '';
  const pct = Math.round(agent.context_pct);
  const used = Math.round(agent.context_tokens / 1000);
  const window = Math.round(agent.context_window / 1000);
  const reading = `${pct}% of context used, ${used}k of ${window}k`;
  return canCompact ? `${reading}. Click to compact.` : `${reading}.`;
}

/**
 * Why the row reads and does not change, or an empty string when it changes.
 *
 * The inbox of a live process takes text and refuses commands, and refuses
 * them on purpose: it stamps every message it receives as coming from
 * another session and enqueues it with slash commands switched off. So there
 * is no way to send `/model` into a chat another app is running, and no
 * cleverness will make one.
 *
 * There is a way to get the controls back, though, and the note names it
 * rather than leaving the reader at a wall: the moment the other app lets go
 * of the chat, the next message from here continues it in a session of our
 * own, and the row comes alive with it. tech.md 6.15 and 6.5.
 */
/** What is so, and what to do about it. Two halves because they are read
 * differently: the first answers the press, the second is only worth reading
 * if the first is unwelcome. */
export type SettingsNote = { fact: string; how?: string };

export const ELSEWHERE_NOTE: SettingsNote = {
  fact: 'Another app is running this chat, so its model and effort are set there.',
  how: 'Close it there and your next message brings the chat here, controls and all.',
};
export const FINISHED_NOTE: SettingsNote = { fact: 'This session has finished.' };

/**
 * Why the mode reads rather than picks once a session is under way.
 *
 * `--permission-mode` is a spawn flag and Claude Code has no slash command
 * for the mode: `Shift+Tab` cycles it in its own window. Stepping a
 * permission setting blind, on someone's behalf, through a cycle that
 * contains `bypassPermissions` is not a thing to do quietly, so the island
 * says where the switch is instead of pretending to be it. tech.md 6.19.
 */
export const MODE_NOTE: SettingsNote = {
  fact: 'The mode is chosen when a session starts.',
  how: 'Claude Code switches it with Shift+Tab in its own window.',
};

export function settingsNote(
  card: { origin: 'Owned' | 'Observed'; status: 'Working' | 'Idle' | 'Ended' } | null | undefined,
): SettingsNote | null {
  if (!card) return null;
  if (card.origin !== 'Owned') return ELSEWHERE_NOTE;
  return card.status === 'Ended' ? FINISHED_NOTE : null;
}

/** The whole note on one line, for a tooltip that has no room for two. */
export function noteTitle(note: SettingsNote | null): string {
  if (!note) return '';
  return note.how ? `${note.fact} ${note.how}` : note.fact;
}

/**
 * What the row says while a pick is still on its way.
 *
 * The value asked for, not the one still in force: the user just chose it,
 * and a row that answers a choice by repeating the old value reads as a
 * choice that did not land. Dimming says it has not applied yet; the label
 * says what is coming. A pick that names nothing the catalog knows shows its
 * own alias rather than a blank. tech.md 6.15.
 */
export function askedLabel(alias: string | null, models: ModelChoice[]): string {
  if (!alias) return '';
  return models.find((model) => model.alias === alias)?.label ?? alias;
}

/**
 * Whether a choice the user just made has been confirmed by the agent.
 *
 * Nothing confirms a write to a pty (tech.md 6.5), so a pick stands dimmed
 * until the transcript names it back. Cleared by the value arriving, never by
 * a clock: a reply that goes missing is a lost message, but a setting that has
 * not applied yet is just a setting that has not applied yet. tech.md 6.15.
 */
export function stillWaiting<T>(picked: T | null, current: T | null): boolean {
  return picked !== null && picked !== current;
}
