/**
 * The rule every primitive obeys, checked on all of them at once rather than
 * remembered: a class in a primitive never wears the name of a Tailwind
 * utility. tech.md 9.
 *
 * Svelte scopes its styles, but the element still carries the bare class and
 * the global utility applies over it. The symptom reads as a rendering fault
 * rather than as somebody else's rule, which is why it took two attempts to
 * find the first time (v17) and why the compact ring wore a white outline
 * afterwards.
 */

import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

const UI = join(process.cwd(), 'src/lib/ui');

/** The utilities named in tech.md 9. */
const UTILITIES = [
  'ring',
  'shadow',
  'outline',
  'border',
  'filter',
  'blur',
  'container',
  'table',
  'grid',
  'flex',
];

function primitives(): string[] {
  return readdirSync(UI).filter((name) => name.endsWith('.svelte'));
}

/** Every class name written in the markup, from `class="a b"` and `class:c`. */
function classNames(source: string): string[] {
  const names: string[] = [];
  for (const [, list] of source.matchAll(/\bclass="([^"{}]*)"/g)) {
    names.push(...list.split(/\s+/).filter(Boolean));
  }
  for (const [, name] of source.matchAll(/\bclass:([A-Za-z0-9_-]+)/g)) {
    names.push(name);
  }
  return names;
}

describe('the primitives in src/lib/ui', () => {
  it('there are some to check', () => {
    expect(primitives().length).toBeGreaterThan(20);
  });

  it.each(primitives())('%s wears no Tailwind utility name', (file) => {
    const source = readFileSync(join(UI, file), 'utf8');
    const worn = classNames(source).filter((name) => UTILITIES.includes(name));

    expect(worn, `${file} carries ${worn.join(', ')}`).toEqual([]);
  });
});
