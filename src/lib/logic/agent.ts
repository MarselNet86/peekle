/**
 * What the row under the field says. Pure, so the tests can hammer it.
 * tech.md 6.15.
 */

import type { AgentSetup } from '$lib/types/generated/AgentSetup';
import type { Effort } from '$lib/types/generated/Effort';
import type { ModelChoice } from '$lib/types/generated/ModelChoice';

/** One row of a menu. */
export type PickOption = {
  /** What goes back to the caller when the row is picked. */
  id: string;
  label: string;
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
 * never honestly described by nothing. tech.md 6.15.
 */
export function modelLabel(agent: AgentSetup | null): string {
  if (!agent) return '';
  return agent.label ?? agent.model ?? 'Unknown model';
}

export function modelOptions(models: ModelChoice[]): PickOption[] {
  return models.map((model) => ({ id: model.alias, label: model.label }));
}

export function effortOptions(agent: AgentSetup | null): PickOption[] {
  return (agent?.levels ?? []).map((level) => ({ id: level, label: EFFORT_LABELS[level] }));
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
 */
export function contextLabel(agent: AgentSetup | null): string {
  if (!agent) return '';
  const pct = Math.round(agent.context_pct);
  const used = Math.round(agent.context_tokens / 1000);
  const window = Math.round(agent.context_window / 1000);
  return `${pct}% of context used, ${used}k of ${window}k. Click to compact.`;
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
