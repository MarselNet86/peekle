/**
 * The little bit of Markdown a turn actually uses. Pure, so the property tests
 * can hammer it.
 *
 * Parsed rather than injected. The text comes out of an agent turn and the
 * window sits over the whole screen, so handing it to `{@html}` would be an
 * XSS hole in the most expensive place in the product. Segments render through
 * components, which cannot execute anything. tech.md 6.12.
 */

export type Piece =
  | { kind: 'text'; value: string }
  | { kind: 'code'; value: string }
  | { kind: 'bold'; value: string };

export type Block =
  { kind: 'prose'; pieces: Piece[] } | { kind: 'code'; value: string; language: string };

const FENCE = /```/;

/**
 * Splits a message into prose and fenced code. An unclosed fence takes the
 * rest of the message with it: that is what the writer meant, and it is what
 * every other renderer does.
 */
export function blocks(text: string): Block[] {
  const out: Block[] = [];
  const lines = text.split('\n');

  let prose: string[] = [];
  let code: string[] | null = null;
  let language = '';

  const flushProse = () => {
    const joined = prose.join('\n').replace(/^\n+|\n+$/g, '');
    if (joined.trim()) out.push({ kind: 'prose', pieces: pieces(joined) });
    prose = [];
  };

  for (const line of lines) {
    if (FENCE.test(line.trim()) && line.trim().startsWith('```')) {
      if (code === null) {
        flushProse();
        code = [];
        language = line.trim().slice(3).trim();
      } else {
        out.push({ kind: 'code', value: code.join('\n'), language });
        code = null;
        language = '';
      }
      continue;
    }
    (code ?? prose).push(line);
  }

  if (code !== null) out.push({ kind: 'code', value: code.join('\n'), language });
  else flushProse();

  return out;
}

/**
 * Inline spans: `code` and **bold**. Everything else stays text, including a
 * lone backtick or a stray pair of asterisks, because a message about code is
 * full of both.
 */
export function pieces(text: string): Piece[] {
  const out: Piece[] = [];
  const pattern = /`([^`\n]+)`|\*\*([^*\n]+)\*\*/g;
  let last = 0;

  for (const match of text.matchAll(pattern)) {
    const at = match.index ?? 0;
    if (at > last) out.push({ kind: 'text', value: text.slice(last, at) });

    if (match[1] !== undefined) out.push({ kind: 'code', value: match[1] });
    else out.push({ kind: 'bold', value: match[2] });

    last = at + match[0].length;
  }

  if (last < text.length) out.push({ kind: 'text', value: text.slice(last) });
  return out;
}
