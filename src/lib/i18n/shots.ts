/** The screenshot offer, tile and preview. tech.md 6.13, 6.28. */

import type { Copy } from './index.svelte';

const en = {
  screenshot: 'Screenshot',
  openScreenshot: (size: string | null) => `Open the screenshot${size ? `, ${size}` : ''}`,
  offer: (project: string) => `Screenshot to ${project}`,
  how: 'Press the up arrow to attach it',
  secs: (n: number) => `${n}s`,
  closeShot: 'Close the shot',
};

export const SHOTS: Copy<typeof en> = {
  en,
  ru: {
    screenshot: 'Снимок',
    openScreenshot: (size) => `Открыть снимок${size ? `, ${size}` : ''}`,
    offer: (project) => `Снимок для ${project}`,
    how: 'Нажмите стрелку вверх, чтобы прикрепить',
    secs: (n) => `${n} с`,
    closeShot: 'Закрыть снимок',
  },
};
