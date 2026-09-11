/**
 * The formatting a turn actually uses. Parsed rather than injected: the text
 * comes out of an agent turn into a window over the whole screen. tech.md 6.12.
 */

import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { blocks, pieces } from '$lib/logic/markdown';

describe('inline spans', () => {
  it('pulls code and bold out of the prose around them', () => {
    expect(pieces('run `cargo test` and **stop**')).toEqual([
      { kind: 'text', value: 'run ' },
      { kind: 'code', value: 'cargo test' },
      { kind: 'text', value: ' and ' },
      { kind: 'bold', value: 'stop' },
    ]);
  });

  it('leaves a lone marker alone, because a message about code is full of them', () => {
    for (const text of ['a ` backtick', 'two ** stars', '`unclosed', '**also unclosed']) {
      expect(pieces(text)).toEqual([{ kind: 'text', value: text }]);
    }
  });

  it('never loses or invents a character', () => {
    fc.assert(
      fc.property(fc.string(), (text) => {
        const rebuilt = pieces(text)
          .map((piece) =>
            piece.kind === 'code'
              ? `\`${piece.value}\``
              : piece.kind === 'bold'
                ? `**${piece.value}**`
                : piece.value,
          )
          .join('');
        expect(rebuilt).toBe(text);
      }),
    );
  });

  it('is total over anything a turn can carry', () => {
    fc.assert(
      fc.property(fc.string(), (text) => {
        expect(Array.isArray(pieces(text))).toBe(true);
      }),
    );
  });
});

describe('blocks', () => {
  it('separates fenced code from the prose around it', () => {
    const message = 'before\n```rust\nlet x = 1;\n```\nafter';

    expect(blocks(message)).toEqual([
      { kind: 'prose', pieces: [{ kind: 'text', value: 'before' }] },
      { kind: 'code', value: 'let x = 1;', language: 'rust' },
      { kind: 'prose', pieces: [{ kind: 'text', value: 'after' }] },
    ]);
  });

  /// A turn cut at 2000 characters ends mid fence often enough to matter.
  it('takes the rest of the message when a fence never closes', () => {
    const parsed = blocks('before\n```\nstill code');
    expect(parsed).toEqual([
      { kind: 'prose', pieces: [{ kind: 'text', value: 'before' }] },
      { kind: 'code', value: 'still code', language: '' },
    ]);
  });

  it('drops nothing but whitespace', () => {
    expect(blocks('')).toEqual([]);
    expect(blocks('   \n  ')).toEqual([]);
  });

  it('is total over anything a turn can carry', () => {
    fc.assert(
      fc.property(fc.string(), (text) => {
        expect(Array.isArray(blocks(text))).toBe(true);
      }),
    );
  });

  /// Plain lines are still one paragraph, whatever else the parser learned.
  it('leaves ordinary prose as the one paragraph it is', () => {
    fc.assert(
      fc.property(
        fc.array(fc.stringMatching(/^[a-z ]{1,20}$/), { minLength: 1, maxLength: 5 }),
        (lines) => {
          const text = lines.join('\n');
          if (!text.trim()) return;
          const parsed = blocks(text);
          expect(parsed).toHaveLength(1);
          expect(parsed[0].kind).toBe('prose');
        },
      ),
    );
  });
});

describe('headings', () => {
  it('reads the level off the hashes and keeps the words', () => {
    expect(blocks('## What changed')).toEqual([
      { kind: 'heading', level: 2, pieces: [{ kind: 'text', value: 'What changed' }] },
    ]);
  });

  /// A hash with no space after it is not a heading anywhere, and a message
  /// full of `#1` and `#peekle` would be all headings if it were.
  it('needs the space, so a number and a tag stay words', () => {
    for (const text of ['#1 priority', '#peekle', '#']) {
      expect(blocks(text)[0].kind).toBe('prose');
    }
  });
});

describe('lists', () => {
  it('takes a run of bullets as one list', () => {
    const parsed = blocks('- first\n- second\n\nafter');

    expect(parsed[0]).toEqual({
      kind: 'list',
      ordered: false,
      start: 1,
      items: [
        { depth: 0, pieces: [{ kind: 'text', value: 'first' }] },
        { depth: 0, pieces: [{ kind: 'text', value: 'second' }] },
      ],
    });
    expect(parsed[1]).toEqual({ kind: 'prose', pieces: [{ kind: 'text', value: 'after' }] });
  });

  /// A list that begins at three is the rest of a list, and renumbering it
  /// from one says something the writer did not.
  it('keeps the number a numbered list started on', () => {
    const parsed = blocks('3. third\n4. fourth');

    expect(parsed).toHaveLength(1);
    expect(parsed[0]).toMatchObject({ kind: 'list', ordered: true, start: 3 });
  });

  it('does not run a run of bullets into a run of numbers', () => {
    const parsed = blocks('- one\n1. two');

    expect(parsed.map((block) => block.kind)).toEqual(['list', 'list']);
    expect(parsed[0]).toMatchObject({ ordered: false });
    expect(parsed[1]).toMatchObject({ ordered: true });
  });

  it('carries how deep an item was written', () => {
    const parsed = blocks('- top\n  - under it\n    - deeper');

    expect(parsed[0]).toMatchObject({
      items: [{ depth: 0 }, { depth: 1 }, { depth: 2 }],
    });
  });

  /// A dash is a dash. A rule, a minus sign and a sentence that begins with
  /// one are not lists.
  it('leaves a dash that starts nothing alone', () => {
    for (const text of ['---', '-not a bullet', '- ']) {
      expect(blocks(text)[0]?.kind ?? 'prose').not.toBe('list');
    }
  });
});

describe('tables', () => {
  it('reads a table as its heading, its rows and how each column reads', () => {
    const parsed = blocks('| Name | Cost |\n| --- | ---: |\n| tea | 3 |');

    expect(parsed).toEqual([
      {
        kind: 'table',
        head: [[{ kind: 'text', value: 'Name' }], [{ kind: 'text', value: 'Cost' }]],
        align: ['left', 'right'],
        rows: [[[{ kind: 'text', value: 'tea' }], [{ kind: 'text', value: '3' }]]],
      },
    ]);
  });

  it('centres a column written between two colons', () => {
    expect(blocks('| a |\n| :-: |')[0]).toMatchObject({ align: ['center'] });
  });

  /// Written by hand, an empty heading row means "just the grid", and drawn
  /// as a heading it is an empty band of rules over the first real line.
  it('drops a heading row that has nothing in it', () => {
    const parsed = blocks('| | |\n|---|---|\n| Paid | 1908 |');

    expect(parsed[0]).toMatchObject({
      kind: 'table',
      head: null,
      rows: [[[{ kind: 'text', value: 'Paid' }], [{ kind: 'text', value: '1908' }]]],
    });
  });

  it('reads a table written without its outer pipes', () => {
    expect(blocks('a | b\n--- | ---')[0]).toMatchObject({ kind: 'table', align: ['left', 'left'] });
  });

  /// Rows are drawn against the heading: a short one is padded so the grid
  /// holds, and a long one is cut, because a cell with no column over it has
  /// nothing to say.
  it('fits every row to the columns the heading declared', () => {
    const parsed = blocks('| a | b |\n| --- | --- |\n| one |\n| one | two | three |');

    expect(parsed[0]).toMatchObject({
      rows: [
        [[{ kind: 'text', value: 'one' }], []],
        [[{ kind: 'text', value: 'one' }], [{ kind: 'text', value: 'two' }]],
      ],
    });
  });

  /// Without the dashes under it, a line with a pipe in it is a sentence
  /// with a pipe in it, and a shell command is full of them.
  it('is not a table without the row of dashes', () => {
    for (const text of ['ls | grep peekle', 'a | b\nc | d', '| a |\n| b |']) {
      expect(blocks(text)[0].kind).toBe('prose');
    }
  });

  it('does not lose a pipe that was written inside a cell', () => {
    const parsed = blocks('| what |\n| --- |\n| ls \\| wc |');

    expect(parsed[0]).toMatchObject({ rows: [[[{ kind: 'text', value: 'ls | wc' }]]] });
  });

  it('ends the table at the first line that is not a row', () => {
    const parsed = blocks('| a |\n| --- |\n| one |\nand then words');

    expect(parsed.map((block) => block.kind)).toEqual(['table', 'prose']);
  });
});
