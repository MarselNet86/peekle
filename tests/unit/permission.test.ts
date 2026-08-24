/**
 * S4 acceptance. A permission is the one prompt that must never be answered by
 * accident, so these check that each affordance reaches exactly one choice and
 * that nothing answers on its own.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { choiceFor, isPermission } from '$lib/features/permission/permission.svelte';
import PermissionRow from '$lib/ui/PermissionRow.svelte';
import type { PromptRequest } from '$lib/types/generated/PromptRequest';

const request: PromptRequest = {
  id: '01J0',
  kind: 'Permission',
  session: { session_id: 's', cwd: '/Users/x/peekle', project: 'peekle', pid: null, tty: null },
  title: 'Bash needs permission',
  last_message: null,
  detail: '{"command":"rm -rf target"}',
  options: [
    { id: 'allow_once', label: 'Allow once', hint: null, kind: 'AllowOnce' },
    { id: 'allow_always', label: 'Allow for this session', hint: null, kind: 'AllowAlways' },
    { id: 'deny', label: 'Deny', hint: null, kind: 'Deny' },
  ],
  questions: [],
  allow_free_text: true,
  created_at: 0,
  expires_at: 0,
};

describe('choice mapping', () => {
  it('sends allow_once for Allow, never allow_always', () => {
    expect(choiceFor(request, 'allow')).toBe('allow_once');
  });

  it('sends deny for Deny', () => {
    expect(choiceFor(request, 'deny')).toBe('deny');
  });

  it('reports nothing rather than guessing when the option is absent', () => {
    const stripped = { ...request, options: [] };
    expect(choiceFor(stripped, 'allow')).toBeUndefined();
    expect(choiceFor(null, 'deny')).toBeUndefined();
  });

  it('tells a permission apart from anything else', () => {
    expect(isPermission(request)).toBe(true);
    expect(isPermission({ ...request, kind: 'Idle' })).toBe(false);
    expect(isPermission(null)).toBe(false);
  });
});

describe('PermissionRow', () => {
  it('shows what is being agreed to, verbatim', () => {
    render(PermissionRow, { props: { request } });

    expect(screen.getByText('Bash needs permission')).toBeInTheDocument();
    expect(screen.getByText('{"command":"rm -rf target"}')).toBeInTheDocument();
  });

  it('answers on a click, one call per button', async () => {
    const onallow = vi.fn();
    const ondeny = vi.fn();
    render(PermissionRow, { props: { request, onallow, ondeny } });

    await userEvent.click(screen.getByRole('button', { name: 'Allow' }));
    expect(onallow).toHaveBeenCalledOnce();
    expect(ondeny).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole('button', { name: 'Deny' }));
    expect(ondeny).toHaveBeenCalledOnce();
  });

  it('answers on the digits, deny first so 1 is never the permissive one', async () => {
    const onallow = vi.fn();
    const ondeny = vi.fn();
    render(PermissionRow, { props: { request, onallow, ondeny } });

    await userEvent.keyboard('1');
    expect(ondeny).toHaveBeenCalledOnce();
    expect(onallow).not.toHaveBeenCalled();

    await userEvent.keyboard('2');
    expect(onallow).toHaveBeenCalledOnce();
  });

  it('answers on the arrows', async () => {
    const onallow = vi.fn();
    const ondeny = vi.fn();
    render(PermissionRow, { props: { request, onallow, ondeny } });

    await userEvent.keyboard('{ArrowLeft}');
    await userEvent.keyboard('{ArrowRight}');

    expect(ondeny).toHaveBeenCalledOnce();
    expect(onallow).toHaveBeenCalledOnce();
  });

  it('answers nothing on its own', async () => {
    const onallow = vi.fn();
    const ondeny = vi.fn();
    render(PermissionRow, { props: { request, onallow, ondeny } });

    await userEvent.keyboard('{Enter} 3 x{Escape}');

    expect(onallow).not.toHaveBeenCalled();
    expect(ondeny).not.toHaveBeenCalled();
  });

  it('keeps its hands off a field the user is typing in', async () => {
    const onallow = vi.fn();
    const ondeny = vi.fn();
    const { container } = render(PermissionRow, { props: { request, onallow, ondeny } });

    const field = document.createElement('textarea');
    container.appendChild(field);
    field.focus();
    await userEvent.keyboard('12');

    expect(field.value).toBe('12');
    expect(onallow).not.toHaveBeenCalled();
    expect(ondeny).not.toHaveBeenCalled();
  });
});
