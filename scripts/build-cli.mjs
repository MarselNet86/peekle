#!/usr/bin/env node
// Builds the `peekle` command for the app bundle. tech.md 6.27, "The CLI in
// the bundle".
//
// Runs from `beforeBuildCommand`, so every `pnpm tauri build` ends with the
// CLI at `target/cli/peekle`, which `bundle.macOS.files` copies to
// `Contents/MacOS/peekle`. A universal build (`--target
// universal-apple-darwin`) gets both slices through lipo, the way the app
// binary does. The other platforms have no bundle entry for it and skip.
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

if (process.platform !== 'darwin') process.exit(0);

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = join(root, 'target', 'cli');
const target = process.env.TAURI_ENV_TARGET_TRIPLE || hostTriple();

function run(command, args) {
  execFileSync(command, args, { cwd: root, stdio: 'inherit' });
}

function hostTriple() {
  const info = execFileSync('rustc', ['-vV'], { encoding: 'utf8' });
  const host = /^host: (.+)$/m.exec(info);
  if (!host) throw new Error('rustc -vV did not name a host triple');
  return host[1].trim();
}

function build(triple) {
  run('cargo', [
    'build',
    '--release',
    '--package',
    'peekle-cli',
    '--bin',
    'peekle',
    '--target',
    triple,
  ]);
  return join(root, 'target', triple, 'release', 'peekle');
}

const slices =
  target === 'universal-apple-darwin' ? ['aarch64-apple-darwin', 'x86_64-apple-darwin'] : [target];
const built = slices.map(build);

mkdirSync(out, { recursive: true });
if (built.length === 1) copyFileSync(built[0], join(out, 'peekle'));
else run('lipo', ['-create', '-output', join(out, 'peekle'), ...built]);
