/**
 * v87.2 acceptance: the version in the settings. tech.md 9.
 *
 * Criteria, from the owner's request: the settings say which version of the
 * app this is. The row says it and asks nothing, and the number can be
 * selected for a bug report.
 */

import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import InfoRow from '$lib/ui/InfoRow.svelte';

describe('the info row', () => {
  it('says its label and its value, and offers nothing to press', () => {
    render(InfoRow, { props: { label: 'Version', value: '0.1.1' } });

    expect(screen.getByText('Version')).toBeInTheDocument();
    expect(screen.getByText('0.1.1')).toBeInTheDocument();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
  });

  it('carries a second line only when it is given one', () => {
    const { container, unmount } = render(InfoRow, {
      props: { label: 'Version', value: '0.1.1' },
    });
    expect(container.querySelector('.hint')).toBeNull();
    unmount();

    render(InfoRow, { props: { label: 'Version', value: '0.1.1', hint: 'Debug build' } });
    expect(screen.getByText('Debug build')).toBeInTheDocument();
  });
});
