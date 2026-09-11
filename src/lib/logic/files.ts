/**
 * Files attached to a message. tech.md 6.25.
 *
 * A file is attached by its path and nothing else: Claude Code opens it with
 * its own `Read` once it is named, so nothing is copied and nothing is read
 * here. These are the two rules that need stating -- which paths may travel,
 * and which lines of a reply were paths rather than words.
 */

import { isShot, shotName } from './shots';

/**
 * A line that is an attached file rather than prose about one.
 *
 * The whole line, an absolute path, and a last segment with an extension.
 * Spaces are allowed because filenames have them; what is not allowed is
 * anything after the extension, which is what keeps a sentence that merely
 * starts with a path out of this.
 */
const FILE_LINE = /^\/(?:[^/\n]+\/)*[^/\n]+\.[A-Za-z0-9]{1,12}$/;

/** A reply split into the files it carries and the words that go with them. */
export interface SaidWithFiles {
  files: string[];
  said: string;
}

/**
 * What a reply says and what it carries, the same split `shotLines` does for
 * screenshots and in the same direction: `compose` puts every path on a line
 * of its own ahead of the text. tech.md 6.25.
 *
 * A screenshot is left alone here. It has a block of its own (6.13), and a
 * shot whose picture would not load has to come back as its path rather than
 * as a file block, which is what would happen if this took it.
 */
export function fileLines(text: string): SaidWithFiles {
  const files: string[] = [];
  const words: string[] = [];

  for (const line of text.split('\n')) {
    const path = line.trim();
    if (!isShot(path) && FILE_LINE.test(path)) {
      files.push(path);
      continue;
    }
    words.push(line);
  }

  return { files, said: words.join('\n').trim() };
}

/**
 * Whether this path can be attached at all.
 *
 * One line per path is the whole delivery protocol (6.13), so a path that
 * breaks itself across two lines would arrive as two paths, neither of which
 * exists. macOS allows it in a filename; the message cannot carry it.
 */
export const SPLIT_PATH_NOTE = 'That file cannot be attached: its name runs onto a second line';

export function attachable(path: string): boolean {
  return path.trim().length > 0 && !/[\n\r]/.test(path);
}

/**
 * What a file is called: the last part of its path. One question about a
 * path, answered in one place, whatever put the path there.
 */
export function fileName(path: string): string {
  return shotName(path);
}
