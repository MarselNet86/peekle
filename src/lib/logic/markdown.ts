/**
 * The Markdown a turn actually uses. Pure, so the property tests can hammer it.
 *
 * Parsed rather than injected. The text comes out of an agent turn and the
 * window sits over the whole screen, so handing it to `{@html}` would be an
 * XSS hole in the most expensive place in the product. Segments render through
 * components, which cannot execute anything. tech.md 6.12.
 *
 * What is here is what an answer is made of: paragraphs, fenced code,
 * headings, lists and tables. What is not here is not an oversight -- an
 * unsupported line stays the text it is, which is the only safe failure a
 * renderer of somebody else's words can have.
 */

export type Piece =
  | { kind: 'text'; value: string }
  | { kind: 'code'; value: string }
  | { kind: 'bold'; value: string };

/** Which way a table column reads, from the dashes under its heading. */
export type Align = 'left' | 'center' | 'right';

/** One cell, and one row of them. */
export type Cell = Piece[];
export type Row = Cell[];

/** A line of a list, with how deep it was indented. */
export interface Item {
  depth: number;
  pieces: Piece[];
}

export type Block =
  | { kind: 'prose'; pieces: Piece[] }
  | { kind: 'code'; value: string; language: string }
  | { kind: 'heading'; level: number; pieces: Piece[] }
  | { kind: 'list'; ordered: boolean; start: number; items: Item[] }
  | { kind: 'table'; head: Row | null; align: Align[]; rows: Row[] };

const FENCE = /^```/;
const HEADING = /^ {0,3}(#{1,6})\s+(.*)$/;
// The content has to be there. `- ` on its own is an empty item to a strict
// parser and a stray dash to a person, and a bullet with nothing beside it in
// a chat bubble reads as something the renderer ate.
const ITEM = /^(\s*)(?:([-*+])|(\d{1,9})[.)])\s+(\S.*)$/;
const DASHES = /^:?-+:?$/;

/** How far one nested list may be drawn in. Deeper than this is a list that
 * has stopped being a list and become an outline, and the bubble is 400px. */
const DEEPEST = 2;

/**
 * Splits a message into the blocks it is made of.
 *
 * An unclosed fence takes the rest of the message with it: that is what the
 * writer meant, and it is what every other renderer does.
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

  for (let at = 0; at < lines.length; at += 1) {
    const line = lines[at];

    if (FENCE.test(line.trim())) {
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
    if (code !== null) {
      code.push(line);
      continue;
    }

    const heading = HEADING.exec(line);
    if (heading) {
      flushProse();
      out.push({ kind: 'heading', level: heading[1].length, pieces: pieces(heading[2].trim()) });
      continue;
    }

    // A table before a list, because the dashes under a heading row would
    // otherwise read as a bullet.
    const table = takeTable(lines, at);
    if (table) {
      flushProse();
      out.push(table.block);
      at = table.next - 1;
      continue;
    }

    const list = takeList(lines, at);
    if (list) {
      flushProse();
      out.push(list.block);
      at = list.next - 1;
      continue;
    }

    prose.push(line);
  }

  if (code !== null) out.push({ kind: 'code', value: code.join('\n'), language });
  else flushProse();

  return out;
}

/**
 * A table, or null when this line does not begin one.
 *
 * The rule is GitHub's and it is the strict one: a row of cells, and under it
 * a row of nothing but dashes with the same number of cells. Nothing else is
 * a table, so a sentence with a pipe in it stays a sentence.
 */
function takeTable(lines: string[], from: number): { block: Block; next: number } | null {
  const header = lines[from];
  const divider = lines[from + 1];
  if (divider === undefined || !header.includes('|')) return null;
  if (!isDivider(divider)) return null;

  const align = cells(divider).map(alignOf);
  const head = cells(header);
  if (head.length !== align.length) return null;

  const rows: Row[] = [];
  let at = from + 2;
  while (at < lines.length && lines[at].includes('|') && !isDivider(lines[at])) {
    rows.push(fit(cells(lines[at]), align.length));
    at += 1;
  }

  return {
    // A heading row with nothing in it is a table with no heading. Written by
    // hand it means "just the grid", and drawn as a heading it is an empty
    // band of rules above the first line that has anything in it.
    block: {
      kind: 'table',
      head: head.every((cell) => cell === '') ? null : head.map(pieces),
      align,
      rows,
    },
    next: at,
  };
}

/** Whether this line is the row of dashes that makes the line above a table. */
function isDivider(line: string): boolean {
  if (!line.includes('-')) return false;
  const parts = cells(line);
  return parts.length > 0 && parts.every((cell) => DASHES.test(cell));
}

/**
 * One line of a table, split on its pipes.
 *
 * The outer pipes are a frame rather than cells, so the empty ends they leave
 * are dropped -- and only they, because an empty cell in the middle of a row
 * is a cell somebody left empty. A pipe inside a cell is written `\|`.
 */
function cells(line: string): string[] {
  const out: string[] = [];
  let cell = '';
  for (let at = 0; at < line.length; at += 1) {
    const char = line[at];
    if (char === '\\' && line[at + 1] === '|') {
      cell += '|';
      at += 1;
      continue;
    }
    if (char === '|') {
      out.push(cell);
      cell = '';
      continue;
    }
    cell += char;
  }
  out.push(cell);

  const edged = line.trim();
  if (out.length > 1 && out[0].trim() === '' && edged.startsWith('|')) out.shift();
  if (out.length > 1 && out[out.length - 1].trim() === '' && edged.endsWith('|')) out.pop();
  return out.map((each) => each.trim());
}

function alignOf(cell: string): Align {
  const left = cell.startsWith(':');
  const right = cell.endsWith(':');
  if (left && right) return 'center';
  return right ? 'right' : 'left';
}

/** Rows are drawn against the heading, so a short one is padded and a long one
 * is cut: a cell with no column over it has nothing to say. */
function fit(row: string[], width: number): Row {
  const out: Row = [];
  for (let at = 0; at < width; at += 1) out.push(pieces(row[at] ?? ''));
  return out;
}

/**
 * A list, or null when this line does not begin one.
 *
 * Bullets and numbers are two lists rather than one, because a run of bullets
 * that turns into a run of numbers is two things said in two ways.
 */
function takeList(lines: string[], from: number): { block: Block; next: number } | null {
  const first = ITEM.exec(lines[from]);
  if (!first) return null;
  const ordered = first[3] !== undefined;

  const items: Item[] = [];
  let at = from;
  while (at < lines.length) {
    const item = ITEM.exec(lines[at]);
    if (!item || (item[3] !== undefined) !== ordered) break;
    items.push({
      depth: Math.min(DEEPEST, Math.floor(item[1].length / 2)),
      pieces: pieces(item[4].trim()),
    });
    at += 1;
  }

  return {
    // The number the writer started on. A list that begins at 3 is the rest of
    // a list, and renumbering it from 1 says something they did not.
    block: { kind: 'list', ordered, start: ordered ? Number(first[3]) : 1, items },
    next: at,
  };
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
