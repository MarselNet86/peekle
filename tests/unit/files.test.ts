/**
 * Files attached to a message. tech.md 6.25.
 *
 * Acceptance, from the change: a file is attached by its path and travels as
 * a line of the message; a path that runs onto a second line is not attached
 * at all; and the reply that carried a path shows it as the file it is,
 * without mistaking a screenshot or a sentence for one.
 */

import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { attachable, fileLines, fileName } from '$lib/logic/files';

const SHOT = '/Users/dev/Library/Caches/peekle/shots/01J8Z4RDXS7A9M2C5K6N3PQVTB.png';

describe('what a reply carried from disk', () => {
  it('takes a line that is nothing but a path', () => {
    const carried = fileLines('/Users/dev/peekle/report.pdf\nhave a look at this');

    expect(carried.files).toEqual(['/Users/dev/peekle/report.pdf']);
    expect(carried.said).toBe('have a look at this');
  });

  it('takes every path, in the order they were sent', () => {
    const carried = fileLines('/a/one.txt\n/a/two.csv\nboth of these');

    expect(carried.files).toEqual(['/a/one.txt', '/a/two.csv']);
    expect(carried.said).toBe('both of these');
  });

  /// Filenames have spaces in them. What must not have anything after it is
  /// the extension, which is what keeps a sentence out of this.
  it('takes a name with spaces in it and leaves a sentence alone', () => {
    expect(fileLines('/Users/dev/my notes.md').files).toEqual(['/Users/dev/my notes.md']);
    expect(fileLines('/Users/dev/my notes.md is the one').files).toEqual([]);
    expect(fileLines('see /Users/dev/notes.md').files).toEqual([]);
  });

  it('leaves a relative path, a bare directory and prose where they are', () => {
    const said = 'notes.md\n/Users/dev/peekle\njust words';

    expect(fileLines(said).files).toEqual([]);
    expect(fileLines(said).said).toBe(said);
  });

  /// A screenshot has a block of its own (6.13), and one whose picture will
  /// not load has to come back as its path rather than as a file.
  it('keeps its hands off a screenshot', () => {
    const carried = fileLines(`${SHOT}\nwhat do you make of it`);

    expect(carried.files).toEqual([]);
    expect(carried.said).toBe(`${SHOT}\nwhat do you make of it`);
  });

  /// Nothing a person wrote may go missing between the message and the reply
  /// that shows it: every line is either carried as a file or still said.
  it('loses no line of any message', () => {
    fc.assert(
      fc.property(fc.array(fc.string({ maxLength: 40 }), { maxLength: 8 }), (lines) => {
        const text = lines.join('\n');
        const carried = fileLines(text);
        const back = [...carried.files, ...carried.said.split('\n')].filter(
          (line) => line.trim() !== '',
        );
        const sent = lines.map((line) => line.trim()).filter((line) => line !== '');
        expect(back.map((line) => line.trim()).sort()).toEqual(sent.sort());
      }),
    );
  });
});

describe('which files can travel at all', () => {
  /// The rule is about lines, and who may use it is decided where it is
  /// applied: only a reply of one's own. An agent naming a file it changed is
  /// not attaching it, and `FeedRow` is where that stands. tech.md 6.25.

  it('takes an ordinary path', () => {
    expect(attachable('/Users/dev/peekle/report.pdf')).toBe(true);
    expect(attachable('/Users/dev/my notes.md')).toBe(true);
  });

  /// One line per path is the whole delivery protocol (6.13). A name that
  /// breaks across two lines would arrive as two paths, neither of which is a
  /// file.
  it('refuses a path that runs onto a second line', () => {
    expect(attachable('/Users/dev/two\nlines.txt')).toBe(false);
    expect(attachable('/Users/dev/two\rlines.txt')).toBe(false);
  });

  it('refuses nothing at all', () => {
    expect(attachable('')).toBe(false);
    expect(attachable('   ')).toBe(false);
  });
});

describe('what a file is called', () => {
  it('is the last part of the path', () => {
    expect(fileName('/Users/dev/peekle/report.pdf')).toBe('report.pdf');
    expect(fileName('/report.pdf')).toBe('report.pdf');
  });
});
