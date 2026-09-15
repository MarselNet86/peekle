/**
 * What the feed draws once the calls are folded away. Pure, so the property
 * test can hammer it. tech.md 6.12.
 *
 * A dialogue is what was said. The calls between two replies are how the
 * agent got there, and printing one clipped shell line per call answers none
 * of the questions asked of an open dialogue: is it going, and how long has
 * it been going. So a run of them becomes one line with a clock.
 */

import { FEED } from '$lib/i18n/feed';
import { copy } from '$lib/i18n/index.svelte';
import type { FeedEntry } from '$lib/types/generated/FeedEntry';

/** A reply, an answer or a line about the conversation: it stands as itself. */
export interface SaidRow {
  kind: 'said';
  id: string;
  entry: FeedEntry;
}

/** A run of calls and reasoning, as one line with a clock. */
export interface WorkRow {
  kind: 'work';
  id: string;
  /** The record before the run: where the person starts counting. */
  from: number | null;
  /** When the agent spoke again. `null` while the run is still going. */
  to: number | null;
  /** How many calls and thoughts the line stands for. */
  steps: number;
}

export type FeedRow = SaidRow | WorkRow;

/** The key of the line a working session carries with nothing to fold yet. */
export const TAIL_ID = 'work-tail';

/** What the feed folds away: the how, never the what. */
export function isWork(entry: FeedEntry): boolean {
  return entry.kind === 'Tool' || entry.kind === 'Thought';
}

/**
 * The feed as rows: everything spoken in order, every run of calls between
 * two of them as a single work row.
 *
 * A run is dated from the record before it rather than from its own first
 * call, because that is where the person started counting: they sent, and
 * the first long thought of the turn is part of what they waited through.
 * It ends at the record after it; while there is none and the session is
 * `Working`, it ends nowhere, and the line runs its own clock.
 */
export function feedRows(entries: FeedEntry[], working: boolean): FeedRow[] {
  const rows: FeedRow[] = [];
  let run: FeedEntry[] = [];
  let before: FeedEntry | null = null;

  const fold = (after: FeedEntry | null) => {
    const first = run[0];
    const last = run[run.length - 1];
    if (!first || !last) return;
    rows.push({
      kind: 'work',
      id: first.id,
      from: before ? before.at : first.at,
      to: after ? after.at : working ? null : last.at,
      steps: run.length,
    });
    run = [];
  };

  for (const entry of entries) {
    if (isWork(entry)) {
      run.push(entry);
      continue;
    }
    fold(entry);
    rows.push({ kind: 'said', id: entry.id, entry });
    before = entry;
  }
  fold(null);

  // The turn has started and called nothing yet. Standing still through it
  // reads as broken, so the line is there from the first second.
  const tail = rows[rows.length - 1];
  if (working && (!tail || tail.kind !== 'work')) {
    const last = entries[entries.length - 1];
    rows.push({ kind: 'work', id: TAIL_ID, from: last ? last.at : null, to: null, steps: 0 });
  }

  return rows;
}

/**
 * A span of work in the shortest true form: `9s`, `1m 4s`, `2h 7m`.
 *
 * Two units at most, and the small one is dropped as soon as the big one
 * carries the answer. Nothing here is negative: clocks disagree, and no line
 * should ever say it worked for minus four seconds.
 */
export function elapsedLabel(ms: number): string {
  const t = copy(FEED);
  if (!Number.isFinite(ms) || ms <= 0) return t.secs(0);

  const secs = Math.floor(ms / 1000);
  if (secs < 60) return t.secs(secs);

  const mins = Math.floor(secs / 60);
  if (mins < 60) return t.minsSecs(mins, secs % 60);

  const hours = Math.floor(mins / 60);
  return t.hoursMins(hours, mins % 60);
}

/**
 * A line Rust wrote about the conversation, in the language in force.
 * tech.md 6.28.
 *
 * Rust sends these in English and in a handful of fixed shapes
 * (`transcripts::COMPACTED`, `compact_label`, `ASKED_TO_STOP`, the model
 * switch and the thought). Each is recognised by its exact shape and said
 * again; anything else is the conversation's own text and comes back as it
 * came. In English every shape comes back byte for byte.
 */
export function noticeText(text: string): string {
  const t = copy(FEED);

  if (text === 'Compacted') return t.compacted;
  if (text === 'Asked Claude to stop') return t.askedClaudeToStop;

  const parts = text.split(' · ');
  if (parts[0] === 'Compacted chat' && parts.length > 1 && parts.length <= 3) {
    const said = [t.compactedChat];
    for (const part of parts.slice(1)) {
      const freed = /^(\d+k?) tokens freed$/.exec(part);
      said.push(freed ? t.tokensFreed(freed[1]) : (t.triggers[part] ?? part));
    }
    return said.join(' · ');
  }

  const switched = /^Switched to (.+)$/.exec(text);
  if (switched) return t.switchedTo(switched[1]);

  const thought = /^Thought for (\d+)s$/.exec(text);
  if (thought) return t.thoughtFor(elapsedLabel(Number(thought[1]) * 1000));

  return text;
}
