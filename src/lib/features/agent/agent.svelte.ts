/**
 * The model, the effort and the context of the session on screen. Copies the
 * island layout: bridge in, runes hold what the window renders, Rust owns the
 * facts. tech.md 6.15.
 *
 * Everything here knows one thing the rest of the app does not: a pick has
 * been sent and not yet confirmed. Nothing confirms a write to a pty, so the
 * confirmation is the transcript naming the value back, and until it does the
 * row stands dimmed on what was asked for.
 */

import { commands } from '$lib/bridge';
import { currentModel } from '$lib/logic/agent';
import type { AgentSetup } from '$lib/types/generated/AgentSetup';
import type { Effort } from '$lib/types/generated/Effort';
import type { ModelChoice } from '$lib/types/generated/ModelChoice';

/** What was asked for in one session and has not come back yet. */
type Picked = {
  model: string | null;
  effort: Effort | null;
  /** What the context stood at when the compact was asked for. It is done
   * when the number falls below it, which is the only signal there is. */
  compactFrom: number | null;
};

const NOTHING: Picked = { model: null, effort: null, compactFrom: null };

export function createAgent() {
  let models = $state<ModelChoice[]>([]);
  let defaults = $state<AgentSetup | null>(null);
  let picked = $state<Record<string, Picked>>({});
  let error = $state<string | null>(null);

  function mark(sessionId: string, change: Partial<Picked>) {
    picked = {
      ...picked,
      [sessionId]: { ...NOTHING, ...(picked[sessionId] ?? {}), ...change },
    };
  }

  /** A command that refused leaves nothing dimmed and says why. */
  async function send(sessionId: string, change: Partial<Picked>, run: () => Promise<unknown>) {
    error = null;
    mark(sessionId, change);
    try {
      await run();
    } catch (err) {
      mark(sessionId, { model: null, effort: null, compactFrom: null });
      error = String(err);
    }
  }

  return {
    get models() {
      return models;
    },
    /** What a session runs as before it has answered once. tech.md 6.15. */
    get defaults() {
      return defaults;
    },
    get error() {
      return error;
    },

    /**
     * Whether each control is waiting on the agent. Cleared by the value
     * arriving and never by a clock: a setting that has not applied yet is
     * not a setting that was lost. tech.md 6.15.
     */
    pendingFor(sessionId: string, agent: AgentSetup | null) {
      const waiting = picked[sessionId] ?? NOTHING;
      return {
        model: waiting.model !== null && currentModel(agent, models) !== waiting.model,
        effort: waiting.effort !== null && (agent?.effort ?? null) !== waiting.effort,
        compact:
          waiting.compactFrom !== null && (agent?.context_tokens ?? 0) >= waiting.compactFrom,
      };
    },

    setModel(sessionId: string, alias: string) {
      return send(sessionId, { model: alias }, () => commands.setModel(sessionId, alias));
    },

    setEffort(sessionId: string, effort: Effort) {
      return send(sessionId, { effort }, () => commands.setEffort(sessionId, effort));
    },

    compact(sessionId: string, agent: AgentSetup | null) {
      return send(sessionId, { compactFrom: agent?.context_tokens ?? 0 }, () =>
        commands.compactSession(sessionId),
      );
    },

    /** The menu rows and the defaults, once: the catalog ships with the
     * build, and the defaults are read off Claude Code's own settings. */
    async start(): Promise<() => void> {
      const [rows, saved] = await Promise.all([commands.getModels(), commands.getDefaults()]);
      models = rows ?? [];
      defaults = saved;
      return () => {};
    },
  };
}
