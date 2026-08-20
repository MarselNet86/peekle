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
});
