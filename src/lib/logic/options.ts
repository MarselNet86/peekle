/**
 * Pulling a recommendation out of a choice's own label. tech.md 9.
 *
 * There is no separate field for it: a recommended option is marked the same
 * way a recommended `AskUserQuestion` answer already is, a trailing
 * "(Recommended)" in the label itself. Parsing it here means `OptionList`
 * renders it distinctly without a new field on `ChoiceOption`, and a payload
 * with no marker at all just keeps its label untouched.
 */

const MARKER = /\s*\(recommended\)\s*$/i;

export interface ParsedLabel {
  text: string;
  recommended: boolean;
}

export function parseLabel(label: string): ParsedLabel {
  const match = MARKER.exec(label);
  if (!match) return { text: label, recommended: false };
  return { text: label.slice(0, match.index).trimEnd(), recommended: true };
}
