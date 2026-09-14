/**
 * v87 acceptance. A new version is pulled down in the background and asked
 * about once, with the file already on disk: the panel is one line and two
 * buttons in the shape of a permission, Install is the one the eye lands on,
 * and Later means not again until the app restarts. tech.md 6.30.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { i18n } from '$lib/i18n/index.svelte';
import { offered, readableSize } from '$lib/logic/update';
import { WINDOW, shapeBounds } from '$lib/logic/shape';
import UpdatePanel from '$lib/ui/UpdatePanel.svelte';
import type { IslandView } from '$lib/types/generated/IslandView';
import type { Update } from '$lib/types/generated/Update';
import type { UpdateState } from '$lib/types/generated/UpdateState';

const update: Update = {
  version: '0.1.2',
  notes_url: 'https://github.com/MarselNet86/peekle/releases/tag/v0.1.2',
  size: 104_857_600,
  install: 'Bundle',
};

describe('what the island is told', () => {
  /// Ready is the one state that carries a question. Checking and Downloading
  /// are the island staying quiet on purpose, and a failure never reaches the
  /// screen at all. tech.md 6.30.
  it('offers a version only once the file is on disk', () => {
    expect(offered({ Ready: update })).toEqual(update);

    const quiet: UpdateState[] = [
      'Idle',
      'Checking',
      { Downloading: update },
      { Failed: 'Offline' },
      { Failed: 'RateLimited' },
      { Failed: 'NoAsset' },
      { Failed: 'Download' },
    ];
    for (const state of quiet) expect(offered(state)).toBeNull();
    expect(offered(null)).toBeNull();
  });
});

describe('the size on the line', () => {
  it('reads as a person reads a download', () => {
    expect(readableSize(104_857_600)).toBe('105 MB');
    expect(readableSize(1_500_000)).toBe('1.5 MB');
    expect(readableSize(999)).toBe('999 B');
    expect(readableSize(1_000)).toBe('1 kB');
  });

  /// A release that named no size leaves the line without one rather than
  /// printing a zero: the panel has a sentence for that case.
  it('says nothing at all for a size that is not one', () => {
    expect(readableSize(0)).toBe('');
    expect(readableSize(-1)).toBe('');
    expect(readableSize(Number.NaN)).toBe('');
    expect(readableSize(Number.POSITIVE_INFINITY)).toBe('');
  });
});

describe('the panel', () => {
  it('names the version, the size and both answers', () => {
    i18n.set('en');
    render(UpdatePanel, { version: '0.1.2', size: 104_857_600 });

    expect(screen.getByText('Update to 0.1.2?')).toBeTruthy();
    expect(screen.getByText('105 MB downloaded and ready to install.')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Install' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Later' })).toBeTruthy();
  });

  /// Install is `prominent` and Later is `muted`: this is the one panel where
  /// the eye should land on the action, because the file is already downloaded
  /// and putting it off costs more than taking it. tech.md 6.30.
  it('puts the white button under Install, not under Later', () => {
    i18n.set('en');
    render(UpdatePanel, { version: '0.1.2', size: 1_000_000 });

    expect(screen.getByRole('button', { name: 'Install' })).toHaveAttribute(
      'data-variant',
      'prominent',
    );
    expect(screen.getByRole('button', { name: 'Later' })).toHaveAttribute('data-variant', 'muted');
  });

  it('answers Install, Later and the release page', async () => {
    i18n.set('en');
    const oninstall = vi.fn();
    const onlater = vi.fn();
    const onnotes = vi.fn();
    render(UpdatePanel, { version: '0.1.2', size: 1_000_000, oninstall, onlater, onnotes });

    await userEvent.click(screen.getByRole('button', { name: 'Install' }));
    await userEvent.click(screen.getByRole('button', { name: 'Later' }));
    await userEvent.click(screen.getByRole('button', { name: 'Update to 0.1.2?' }));

    expect(oninstall).toHaveBeenCalledOnce();
    expect(onlater).toHaveBeenCalledOnce();
    expect(onnotes).toHaveBeenCalledOnce();
  });

  /// Escape is the second way out, the same as it is for the quit question:
  /// the panel does not hold the keyboard until it is clicked. tech.md 6.30.
  it('takes Escape as Later', async () => {
    const onlater = vi.fn();
    render(UpdatePanel, { version: '0.1.2', size: 1_000_000, onlater });

    await userEvent.keyboard('{Escape}');

    expect(onlater).toHaveBeenCalledOnce();
  });

  /// Install pressed once: the buttons lock while Rust hands the file over and
  /// stands aside, so a second press cannot start a second install.
  it('locks both answers while it is leaving', async () => {
    const oninstall = vi.fn();
    const onlater = vi.fn();
    render(UpdatePanel, { version: '0.1.2', size: 1_000_000, busy: true, oninstall, onlater });

    await userEvent.click(screen.getByRole('button', { name: 'Install' }));
    await userEvent.keyboard('{Escape}');

    expect(oninstall).not.toHaveBeenCalled();
    expect(onlater).not.toHaveBeenCalled();
  });

  /// A copy Homebrew owns is never handed a file: the answer is the command,
  /// and the line says why. tech.md 6.30.
  it('offers the command rather than a file to a Homebrew copy', () => {
    i18n.set('en');
    render(UpdatePanel, { version: '0.1.2', size: 0, brew: true });

    expect(
      screen.getByText('Homebrew installed this copy. Copy the command to upgrade.'),
    ).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Copy command' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Install' })).toBeNull();
  });

  /// A release that named no size still has a line that reads. tech.md 6.30.
  it('drops the size from the line when the release named none', () => {
    i18n.set('en');
    render(UpdatePanel, { version: '0.1.2', size: 0 });

    expect(screen.getByText('Downloaded and ready to install.')).toBeTruthy();
  });

  /// Every string a person sees has both halves, or it does not ship.
  /// tech.md 6.28.
  it('speaks Russian too', () => {
    i18n.set('ru');
    render(UpdatePanel, { version: '0.1.2', size: 104_857_600 });

    expect(screen.getByText('Установить Peekle 0.1.2?')).toBeTruthy();
    expect(screen.getByText('105 MB скачано, можно ставить.')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Установить' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Позже' })).toBeTruthy();
    i18n.set('en');
  });
});

describe('the shape it opens to', () => {
  /// The update question is the permission panel's shape: one line, two
  /// buttons, the island open just enough to ask. tech.md 6.30.
  it('is the shape of the permission panel', () => {
    const notch = { width: 200, height: 32 };
    expect(shapeBounds('Update', notch)).toEqual(shapeBounds('Ask', notch));
    expect(shapeBounds('Update', notch)).toEqual(shapeBounds('Quit', notch));
  });

  /// The window never resizes, so every shape has to fit inside it, notch or
  /// no notch. tech.md 6.7.
  it('fits the window on any display', () => {
    const view: IslandView = 'Update';
    for (const notch of [
      { width: 200, height: 32 },
      { width: 0, height: 0 },
      { width: 3000, height: 200 },
    ]) {
      const bounds = shapeBounds(view, notch);
      expect(bounds.width).toBeLessThanOrEqual(WINDOW.width);
      expect(bounds.height).toBeLessThanOrEqual(WINDOW.height);
    }
  });
});
