#!/bin/sh
# Installs Peekle on macOS in one go: the latest dmg, checked against the
# sha256 in that release's own cask, copied to /Applications, cleared of
# quarantine, its `peekle` command linked, the hooks written, the app opened.
#
#   curl -fsSL https://raw.githubusercontent.com/MarselNet86/peekle/main/install.sh | sh
#
# Knobs, for checks and CI (tech.md 6.27):
#   PEEKLE_DMG=<path>       install this dmg instead of downloading one
#   PEEKLE_APPDIR=<dir>     put Peekle.app here instead of /Applications
#   PEEKLE_INSTALL_ONLY=1   place the bundle and stop: no quit, link, init or open
set -eu

REPO="MarselNet86/peekle"
LATEST="https://github.com/$REPO/releases/latest/download"
APP="Peekle.app"
BUNDLE_ID="app.peekle.overlay"

note() { printf '%s\n' "$*"; }
fail() {
  printf 'peekle: %s\n' "$*" >&2
  exit 1
}

[ "$(uname -s)" = "Darwin" ] || fail "Peekle runs on macOS only"
version=$(sw_vers -productVersion)
[ "${version%%.*}" -ge 13 ] || fail "Peekle needs macOS 13 or newer, this is $version"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/peekle-install.XXXXXX")
mnt="$tmp/mnt"
cleanup() {
  if [ -d "$mnt" ]; then
    hdiutil detach "$mnt" -quiet 2>/dev/null || hdiutil detach "$mnt" -force -quiet 2>/dev/null || true
  fi
  rm -rf "$tmp"
}
trap cleanup EXIT

# 1. The dmg, and the cask that vouches for it.
if [ -n "${PEEKLE_DMG:-}" ]; then
  dmg="$PEEKLE_DMG"
  [ -f "$dmg" ] || fail "no dmg at $dmg"
  note "Installing from $dmg"
else
  dmg="$tmp/Peekle-mac-universal.dmg"
  note "Downloading the latest Peekle..."
  curl -fsSL --retry 3 -o "$dmg" "$LATEST/Peekle-mac-universal.dmg" || fail "could not download the dmg"
  curl -fsSL --retry 3 -o "$tmp/peekle.rb" "$LATEST/peekle.rb" || fail "could not download the release's cask to check the dmg against"
  want=$(sed -n 's/^ *sha256 "\([0-9a-f]\{64\}\)".*/\1/p' "$tmp/peekle.rb")
  [ -n "$want" ] || fail "the release's cask carries no sha256"
  have=$(shasum -a 256 "$dmg" | cut -d' ' -f1)
  [ "$have" = "$want" ] || fail "the dmg does not match the release: got $have, expected $want"
fi

# 2. Where the app goes.
appdir="${PEEKLE_APPDIR:-/Applications}"
[ -d "$appdir" ] || mkdir -p "$appdir" || fail "cannot create $appdir"
if [ ! -w "$appdir" ]; then
  appdir="$HOME/Applications"
  mkdir -p "$appdir"
  note "/Applications is not writable, using $appdir"
fi
target="$appdir/$APP"

# 3. Mount and copy.
mkdir -p "$mnt"
hdiutil attach "$dmg" -nobrowse -readonly -mountpoint "$mnt" -quiet || fail "could not mount the dmg"
[ -d "$mnt/$APP" ] || fail "no $APP inside the dmg"

if [ -z "${PEEKLE_INSTALL_ONLY:-}" ] && pgrep -x peekle-app >/dev/null 2>&1; then
  note "Asking the running Peekle to quit..."
  osascript -e "tell application id \"$BUNDLE_ID\" to quit" >/dev/null 2>&1 || true
  i=0
  while pgrep -x peekle-app >/dev/null 2>&1 && [ "$i" -lt 50 ]; do
    sleep 0.1
    i=$((i + 1))
  done
  if pgrep -x peekle-app >/dev/null 2>&1; then
    pkill -x peekle-app || true
  fi
fi

rm -rf "$target"
ditto "$mnt/$APP" "$target" || fail "could not copy $APP to $appdir"
hdiutil detach "$mnt" -quiet || hdiutil detach "$mnt" -force -quiet
rmdir "$mnt" 2>/dev/null || true

# The bundle is signed ad hoc, not notarized; with the flag on, Gatekeeper
# refuses it. The Homebrew cask clears it the same way.
xattr -cr "$target"

cli="$target/Contents/MacOS/peekle"
[ -x "$cli" ] || fail "the bundle carries no peekle command"
note "Installed $("$cli" --version) at $target"

if [ -n "${PEEKLE_INSTALL_ONLY:-}" ]; then
  exit 0
fi

# 4. The command on the PATH.
if [ -d /usr/local/bin ] && [ -w /usr/local/bin ]; then
  bindir=/usr/local/bin
else
  bindir="$HOME/.local/bin"
  mkdir -p "$bindir"
fi
ln -sf "$cli" "$bindir/peekle"
case ":$PATH:" in
  *":$bindir:"*) note "Linked peekle into $bindir" ;;
  *) note "Linked peekle into $bindir, which is not on your PATH: add it, or run $cli" ;;
esac

# 5. The hooks, then the app.
"$cli" init
open "$target"
note "Peekle is in the notch. It shows nothing until an agent needs you."
