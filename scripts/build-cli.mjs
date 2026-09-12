/**
 * Builds the `peekle` CLI and puts it where the bundler expects a sidecar.
 *
 * The app cannot install its own hooks: `peekle init` writes them, and until
 * v82.6 the bundle carried only `peekle-app`. Anyone who installed Peekle from
 * an installer therefore had no way to install the hooks at all, and the
 * product sat there saying nothing -- no spinner, no feed, and every reply
 * failing for want of a `UserPromptSubmit` that could never arrive. So the CLI
 * ships beside the app. tech.md 6.1 and 6.27.
 *
 * Run from `beforeBuildCommand`, because `tauri-build` resolves `externalBin`
 * while the app crate compiles: a sidecar that appears later is a sidecar that
 * was missing.
 */

import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const sidecars = join(root, 'src-tauri', 'binaries');

/** The triple the bundler looks the sidecar up by. Tauri names it for a
 * cross-compile; otherwise it is whatever this host builds for. */
function triple() {
  const named = process.env.TAURI_ENV_TARGET_TRIPLE;
  if (named) return named;
  const spec = execFileSync('rustc', ['-vV'], { encoding: 'utf8' });
  const host = spec.match(/^host:\s*(\S+)$/m);
  if (!host) throw new Error('rustc did not name its host triple');
  return host[1];
}

// `tauri build --debug` says so here, and the CLI beside the app is built the
// same way the app is: a release bundle carrying a debug CLI would ship an
// unoptimised binary nobody asked for.
const debug = process.env.TAURI_ENV_DEBUG === 'true';
const profile = debug ? 'debug' : 'release';
const target = triple();
const suffix = target.includes('windows') ? '.exe' : '';

const args = ['build', '-p', 'peekle-cli', '--bin', 'peekle'];
if (!debug) args.push('--release');
if (process.env.TAURI_ENV_TARGET_TRIPLE) args.push('--target', target);

execFileSync('cargo', args, { cwd: root, stdio: 'inherit' });

// `--target` puts the binary one directory deeper, and no flag puts it at the
// top of the target directory. Both are the same binary.
const built = process.env.TAURI_ENV_TARGET_TRIPLE
  ? join(root, 'target', target, profile, `peekle${suffix}`)
  : join(root, 'target', profile, `peekle${suffix}`);

mkdirSync(sidecars, { recursive: true });
const sidecar = join(sidecars, `peekle-${target}${suffix}`);
copyFileSync(built, sidecar);
console.log(`cli sidecar ${sidecar}`);
