// Renders the app icon set from src-tauri/icons/icon.svg, the single source of
// the mark. Run it after editing that file:
//
//   node scripts/make-icon.mjs
//
// Chromium comes from the Playwright install the e2e suite already uses, so
// this adds no dependency. `iconutil` builds the .icns and ships with macOS.
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { chromium } from '@playwright/test';

const root = fileURLToPath(new URL('..', import.meta.url));
const icons = join(root, 'src-tauri', 'icons');
const svg = readFileSync(join(icons, 'icon.svg'), 'utf8');

/** Sizes Tauri lists in tauri.conf.json, plus the 64 it keeps beside them. */
const TAURI = [
  [32, '32x32.png'],
  [64, '64x64.png'],
  [128, '128x128.png'],
  [256, '128x128@2x.png'],
  [512, 'icon.png'],
];

/** Every size `iconutil` wants for a complete .icns. */
const ICONSET = [
  [16, 'icon_16x16.png'],
  [32, 'icon_16x16@2x.png'],
  [32, 'icon_32x32.png'],
  [64, 'icon_32x32@2x.png'],
  [128, 'icon_128x128.png'],
  [256, 'icon_128x128@2x.png'],
  [256, 'icon_256x256.png'],
  [512, 'icon_256x256@2x.png'],
  [512, 'icon_512x512.png'],
  [1024, 'icon_512x512@2x.png'],
];

const browser = await chromium.launch();
const page = await browser.newPage();

/** One render at one size, transparent outside the tile. */
async function shoot(size, path) {
  await page.setViewportSize({ width: size, height: size });
  await page.setContent(
    `<style>html,body{margin:0;background:transparent}svg{display:block;width:${size}px;height:${size}px}</style>${svg}`,
  );
  await page.screenshot({ path, omitBackground: true });
}

for (const [size, name] of TAURI) await shoot(size, join(icons, name));

const iconset = join(mkdtempSync(join(tmpdir(), 'peekle-icon-')), 'icon.iconset');
mkdirSync(iconset);
for (const [size, name] of ICONSET) await shoot(size, join(iconset, name));
execFileSync('iconutil', ['-c', 'icns', iconset, '-o', join(icons, 'icon.icns')]);
rmSync(iconset, { recursive: true, force: true });

// The shot the owner opens: the tile at the sizes it is actually seen at, on a
// neutral ground, because a logo is judged at 32 px and not at 512.
const board = `<style>
  html,body{margin:0}
  body{display:flex;align-items:flex-end;gap:44px;padding:56px;background:#d8d8d8;
       font:13px -apple-system,system-ui,sans-serif;color:#555}
  figure{display:flex;flex-direction:column;align-items:center;gap:10px;margin:0}
  svg{display:block}
</style>
${[256, 128, 64, 32]
  .map(
    (s) =>
      `<figure>${svg.replace('width="512" height="512"', `width="${s}" height="${s}"`)}<figcaption>${s}px</figcaption></figure>`,
  )
  .join('')}`;
await page.setViewportSize({ width: 720, height: 400 });
await page.setContent(board);
const shot = join(root, 'features', 'shots', 'app-icon.png');
await page.locator('body').screenshot({ path: shot });

await browser.close();
console.log('icons written to src-tauri/icons, shot written to features/shots/app-icon.png');
