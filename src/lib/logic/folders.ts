/**
 * The folder a new chat works in. tech.md 6.23.
 *
 * `start_session` only aims: the card exists, the process starts with the
 * first message, and the folder is an intention nobody has read until then.
 * These are the rules of that window -- which chats are still in it, and what
 * the menu offers -- and they are here rather than in the route because a rule
 * inside a route can only be read by rendering it.
 */

import type { SessionCard } from '$lib/types/generated/SessionCard';

import type { PickOption } from './agent';

/**
 * The row that raises the macOS dialog.
 *
 * It travels as an option id, beside real paths, so it must be something no
 * path can be: every path the dialog can answer with is absolute, and this
 * starts with no slash at all. tech.md 6.23.
 */
export const CHOOSE = 'choose-folder';

/** What the last row says. */
export const CHOOSE_LABEL = 'Open folder…';

export type Folder = { cwd: string; project: string };

/**
 * The folders the island already knows, newest first, each named once.
 *
 * Nothing is scanned for this: these are the chats already on screen, so the
 * list is exactly the projects Claude Code has been run in. A folder that
 * appears in five chats is one row -- the list answers "where can this chat
 * work", and that question has one answer per folder.
 */
export function knownFolders(cards: SessionCard[]): Folder[] {
  const seen = new Set<string>();
  const folders: Folder[] = [];
  for (const card of cards) {
    const { cwd, project } = card.session;
    if (!cwd || seen.has(cwd)) continue;
    seen.add(cwd);
    folders.push({ cwd, project });
  }
  return folders;
}

/**
 * Whether this chat's folder can still be chosen.
 *
 * Ours, and nothing said in it yet. A chat somebody else runs has no folder of
 * ours to move, and one that has spoken is already living somewhere: the
 * process was started in that folder and no `cd` reaches a running agent. Rust
 * holds the same rule and refuses on its own; this is what decides whether the
 * control is drawn at all, because a control that always refuses is worse than
 * none. tech.md 6.23.
 */
export function canPickFolder(card: SessionCard | undefined): boolean {
  if (!card) return false;
  return card.origin === 'Owned' && card.status !== 'Ended' && card.entries.length === 0;
}

/**
 * The menu: every folder the island knows, the one in force ticked, and the
 * way out to the rest of the disk last.
 *
 * A folder chosen through the dialog is not in any chat yet, so it is put at
 * the top: the tick belongs on a row, and a menu whose current value is
 * nowhere in it reads as a menu that forgot what was picked.
 */
export function folderOptions(cards: SessionCard[], cwd: string): PickOption[] {
  const folders = knownFolders(cards);
  const rows =
    cwd && !folders.some((folder) => folder.cwd === cwd)
      ? [{ cwd, project: projectOf(cwd) || cwd }, ...folders]
      : folders;

  return [
    ...rows.map((folder) => ({ id: folder.cwd, label: folder.project, hint: folder.cwd })),
    { id: CHOOSE, label: CHOOSE_LABEL, icon: 'folder' as const },
  ];
}

/**
 * What a path is called, the way Rust names it (`sessions::project_of`): the
 * last part of the path that is not empty, and nothing when there is none.
 * Only needed for a folder no chat has yet, since every other name arrives on
 * a card, and it answers the same as Rust so the name does not change under
 * the user the moment the card comes back.
 */
export function projectOf(cwd: string): string {
  const parts = cwd.split('/').filter((part) => part.length > 0);
  return parts[parts.length - 1] ?? '';
}
