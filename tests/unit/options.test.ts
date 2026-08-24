/**
 * The recommendation a choice carries in its own label. tech.md 9.
 *
 * There is no separate field for it, the same way `AskUserQuestion` marks its
 * own recommended answer: a trailing "(Recommended)" in the label text.
 */

import { render, screen } from '@testing-library/svelte';
import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { parseLabel } from '$lib/logic/options';
import OptionList from '$lib/ui/OptionList.svelte';
import type { ChoiceOption } from '$lib/types/generated/ChoiceOption';

describe('parsing a label', () => {
  it('strips the marker and reports the recommendation', () => {
    expect(parseLabel('Ship it now (Recommended)')).toEqual({
      text: 'Ship it now',
      recommended: true,
    });
  });

  it('is case insensitive and tolerates the spacing around it', () => {
    expect(parseLabel('Ship it(recommended)')).toEqual({
      text: 'Ship it',
      recommended: true,
    });
    expect(parseLabel('Ship it   (RECOMMENDED)')).toEqual({
      text: 'Ship it',
      recommended: true,
    });
  });

  it('leaves a plain label untouched', () => {
    expect(parseLabel('Deny')).toEqual({ text: 'Deny', recommended: false });
  });

  /// A marker in the middle of the text is part of what was actually said,
  /// not a badge: only a trailing one is the convention.
  it('only reads the marker at the very end', () => {
    expect(parseLabel('(Recommended) but risky')).toEqual({
      text: '(Recommended) but risky',
      recommended: false,
    });
  });

  it('is total and never throws, for any string at all', () => {
    fc.assert(
      fc.property(fc.string(), (label) => {
        const parsed = parseLabel(label);
        expect(typeof parsed.text).toBe('string');
        expect(typeof parsed.recommended).toBe('boolean');
      }),
    );
  });

  /// Round trip: appending the exact marker the parser looks for always
  /// comes back off, whatever the label in front of it is.
  it('strips exactly the marker it was given, for any label in front of it', () => {
    fc.assert(
      fc.property(
        fc.string().filter((s) => !/\(recommended\)\s*$/i.test(s)),
        (label) => {
          const marked = `${label} (Recommended)`;
          const parsed = parseLabel(marked);
          expect(parsed.recommended).toBe(true);
          expect(parsed.text).toBe(label.trimEnd());
        },
      ),
    );
  });
});

const options: ChoiceOption[] = [
  { id: 'a', label: 'Do it now (Recommended)', hint: null, kind: 'Custom' },
  { id: 'b', label: 'Wait and see', hint: null, kind: 'Custom' },
];

describe('OptionList', () => {
  it('badges the recommended row and strips the marker from its text', () => {
    render(OptionList, { props: { options } });

    expect(screen.getByText('Do it now')).toBeInTheDocument();
    expect(screen.getByText('Recommended')).toBeInTheDocument();
  });

  it('leaves every other row without a badge', () => {
    render(OptionList, { props: { options } });

    expect(screen.getByText('Wait and see')).toBeInTheDocument();
    expect(screen.getAllByText('Recommended')).toHaveLength(1);
  });
});
