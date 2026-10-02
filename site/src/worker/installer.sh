#!/bin/bash
# Rustyvibes installer · https://rustyvibes.kunalbagaria.com
#
#   curl -fsSL https://rustyvibes.kunalbagaria.com/install | bash
#   curl -fsSL https://rustyvibes.kunalbagaria.com/install | bash -s -- --uninstall
#
# Downloads Rustyvibes @@VERSION@@, checks its SHA-256 checksum and code signature,
# installs it in /Applications (or ~/Applications) and opens it. Rustyvibes is signed but
# not notarized by Apple; a copy fetched with curl carries no quarantine flag, so
# Gatekeeper doesn't stop it. macOS still asks before it can hear key presses.
#
# Options:  --uninstall   remove Rustyvibes
#           --no-open     install without opening it
#           --help        show this help
# RUSTYVIBES_INSTALL_DIR overrides the destination folder.

set -euo pipefail

VERSION="@@VERSION@@"
URL="@@URL@@"
SHA256="@@SHA256@@"
TEAM_ID="@@TEAM_ID@@"
MIN_MACOS="@@MIN_MACOS@@"
SIZE="@@SIZE@@"
APP="Rustyvibes.app"
EXE="Contents/MacOS/rustyvibes"
WORK=""

if [ -t 1 ]; then
  BOLD=$'\033[1m' DIM=$'\033[2m' RED=$'\033[31m' GREEN=$'\033[32m' RESET=$'\033[0m'
else
  BOLD="" DIM="" RED="" GREEN="" RESET=""
fi

step() { printf '%s\n' "${DIM}→ $*${RESET}"; }
ok() { printf '%s\n' "${GREEN}✓${RESET} $*"; }
fail() { printf '%s\n' "${RED}✗ $*${RESET}" >&2; exit 1; }
cleanup() { if [ -n "$WORK" ]; then rm -rf "$WORK"; fi; }

usage() {
  cat <<'HELP'
Rustyvibes installer

  curl -fsSL https://rustyvibes.kunalbagaria.com/install | bash
  curl -fsSL https://rustyvibes.kunalbagaria.com/install | bash -s -- --uninstall

Options:
  --uninstall   remove Rustyvibes
  --no-open     install without opening it
  --help        show this help

RUSTYVIBES_INSTALL_DIR overrides the destination folder.
HELP
}

# version_at_least 13.6.1 13.0 → success
version_at_least() {
  local have want i h w
  IFS=. read -r -a have <<< "$1"
  IFS=. read -r -a want <<< "$2"
  for i in 0 1 2; do
    h="${have[$i]:-0}" w="${want[$i]:-0}"
    if [ "$h" -gt "$w" ]; then return 0; fi
    if [ "$h" -lt "$w" ]; then return 1; fi
  done
  return 0
}

check_macos() {
  [ "$(uname -s)" = "Darwin" ] || fail "Rustyvibes is a Mac app; this installer runs on macOS only."
  local have
  have="$(sw_vers -productVersion)"
  version_at_least "$have" "$MIN_MACOS" || fail "Rustyvibes needs macOS $MIN_MACOS or later (this Mac has $have)."
}

destination() {
  if [ -n "${RUSTYVIBES_INSTALL_DIR:-}" ]; then
    mkdir -p "$RUSTYVIBES_INSTALL_DIR" && printf '%s' "$RUSTYVIBES_INSTALL_DIR"
  elif [ -w /Applications ]; then
    printf '%s' /Applications
  else
    mkdir -p "$HOME/Applications" && printf '%s' "$HOME/Applications"
  fi
}

# Quits Rustyvibes wherever it was opened from, so the new version is the one that runs.
# With RUSTYVIBES_INSTALL_DIR set, only the copy in $1 is quit.
quit_running() {
  local match tries=0
  if [ -n "${RUSTYVIBES_INSTALL_DIR:-}" ]; then match=(-f "$1/$APP/$EXE"); else match=(-x rustyvibes); fi
  pgrep "${match[@]}" >/dev/null 2>&1 || return 0
  step "Quitting Rustyvibes"
  pkill "${match[@]}" 2>/dev/null || true
  while pgrep "${match[@]}" >/dev/null 2>&1; do
    tries=$((tries + 1))
    if [ "$tries" -ge 15 ]; then pkill -9 "${match[@]}" 2>/dev/null || true; break; fi
    sleep 0.2
  done
}

download() {
  local progress=(--silent --show-error) actual
  if [ -t 2 ]; then progress=(--progress-bar); fi
  step "Downloading Rustyvibes $VERSION ($SIZE)"
  curl -fL "${progress[@]}" -o "$WORK/Rustyvibes.zip" "$URL" || fail "The download failed. Check your connection and try again."
  actual="$(shasum -a 256 "$WORK/Rustyvibes.zip" | awk '{print $1}')"
  [ "$actual" = "$SHA256" ] || fail "Checksum mismatch: the download is damaged or was tampered with. Nothing was installed."
  ok "Downloaded and checked the checksum"
}

verify() {
  ditto -x -k "$WORK/Rustyvibes.zip" "$WORK/unpacked" || fail "Couldn't unpack the download. Nothing was installed."
  [ -d "$WORK/unpacked/$APP" ] || fail "The download doesn't contain $APP. Nothing was installed."
  codesign --verify --deep --strict "$WORK/unpacked/$APP" 2>/dev/null || fail "The app's code signature is broken. Nothing was installed."
  if [ -n "$TEAM_ID" ]; then
    codesign --verify -R="anchor apple generic and certificate leaf[subject.OU] = \"$TEAM_ID\"" "$WORK/unpacked/$APP" 2>/dev/null \
      || fail "The app isn't signed by the expected developer ($TEAM_ID). Nothing was installed."
    ok "Checked the code signature (team $TEAM_ID)"
  else
    ok "Checked the code signature"
  fi
}

# Stages the new app beside the old one, then swaps them with renames, so a failure at any
# point leaves the current copy as it was.
install_app() {
  local dest="$1" staged="$1/.Rustyvibes.app.new.$$" old="$1/.Rustyvibes.app.old.$$"
  if ! ditto "$WORK/unpacked/$APP" "$staged" 2>/dev/null; then
    rm -rf "${staged:?}" 2>/dev/null
    fail "Couldn't copy Rustyvibes into $dest: the folder isn't writable or the disk is full. Nothing was changed."
  fi
  xattr -dr com.apple.quarantine "$staged" 2>/dev/null || true
  quit_running "$dest"
  if [ -e "$dest/$APP" ] && ! mv "$dest/$APP" "$old" 2>/dev/null; then
    rm -rf "${staged:?}" 2>/dev/null
    fail "Couldn't replace the Rustyvibes in $dest. Your current copy is unchanged."
  fi
  if ! mv "$staged" "$dest/$APP" 2>/dev/null; then
    if [ -e "$old" ]; then mv "$old" "$dest/$APP" 2>/dev/null; fi
    rm -rf "${staged:?}" 2>/dev/null
    fail "Couldn't install Rustyvibes in $dest. Your current copy is unchanged."
  fi
  if [ -e "$old" ] && ! rm -rf "${old:?}" 2>/dev/null; then
    printf '%s\n' "${DIM}Couldn't delete the previous copy at $old; remove it in Finder when you like.${RESET}"
  fi
  ok "Installed Rustyvibes $VERSION in $dest"
}

uninstall() {
  local dirs dir removed=0
  if [ -n "${RUSTYVIBES_INSTALL_DIR:-}" ]; then dirs=("$RUSTYVIBES_INSTALL_DIR"); else dirs=(/Applications "$HOME/Applications"); fi
  for dir in "${dirs[@]}"; do
    [ -d "$dir/$APP" ] || continue
    quit_running "$dir"
    rm -rf "${dir:?}/$APP" 2>/dev/null || fail "Couldn't remove $dir/$APP. Remove it yourself in Finder."
    ok "Removed $dir/$APP"
    removed=1
  done
  [ "$removed" = 1 ] || ok "Rustyvibes isn't installed"
  printf '%s\n' "${DIM}To clear its Input Monitoring entry, open System Settings → Privacy & Security → Input Monitoring.${RESET}"
}

main() {
  local mode=install open_app=1 arg dest tmp
  for arg in "$@"; do
    case "$arg" in
      --uninstall) mode=uninstall ;;
      --no-open) open_app=0 ;;
      -h | --help) usage; return 0 ;;
      *) fail "Unknown option: $arg (try --help)" ;;
    esac
  done
  if [ "${RUSTYVIBES_NO_OPEN:-0}" = 1 ]; then open_app=0; fi
  check_macos
  if [ "$mode" = uninstall ]; then uninstall; return 0; fi

  printf '%s\n' "${BOLD}Rustyvibes $VERSION${RESET}"
  dest="$(destination)" || fail "Couldn't create the folder to install into."
  tmp="${TMPDIR:-/tmp}"
  WORK="$(mktemp -d "${tmp%/}/rustyvibes.XXXXXX")"
  trap cleanup EXIT
  download
  verify
  install_app "$dest"
  if [ "$open_app" = 1 ]; then
    open "$dest/$APP" || fail "Installed, but macOS wouldn't open it. Open $dest/$APP yourself."
    ok "Opened Rustyvibes: look for the keycap in your menu bar"
    printf '%s\n' "  When macOS asks, allow Rustyvibes in Privacy & Security → Input Monitoring."
  fi
}

# RUSTYVIBES_SOURCE_ONLY=1 loads the functions without running anything (used by the tests).
[ "${RUSTYVIBES_SOURCE_ONLY:-}" = 1 ] || main "$@"
