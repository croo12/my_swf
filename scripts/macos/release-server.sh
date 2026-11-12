#!/usr/bin/env bash
# Runs the Factory server on a Mac from GitHub Release binaries and keeps it up to date.
#
#   release-server.sh install   One-time setup: installs the latest release, a LaunchAgent for the
#                               server, and a LaunchAgent that checks for new releases every 5 minutes.
#   release-server.sh update    Installs the latest release if it differs from the installed one.
#   release-server.sh uninstall Removes both LaunchAgents (keeps the binary, config, and data).
#
# Pushing a new release tag is all it takes to deploy: this Mac pulls it. Nothing connects in from GitHub.
set -euo pipefail

REPO="${FACTORY_RELEASE_REPO:-croo12/my_swf}"
TARGET="aarch64-apple-darwin"
APP_DIR="$HOME/.local/share/software-factory-server"
BIN="$APP_DIR/bin/factory-server"
SELF="$APP_DIR/bin/release-server.sh"
VERSION_FILE="$APP_DIR/VERSION"
ENV_FILE="$HOME/.config/software-factory/server.env"
LA_DIR="$HOME/Library/LaunchAgents"
LOG_DIR="$HOME/Library/Logs/SoftwareFactory"
SERVER_LABEL="dev.softwarefactory.server"
UPDATE_LABEL="dev.softwarefactory.server-update"
CHECK_INTERVAL_SECONDS=300
# A planning job runs for at most ~11 minutes; job files touched more recently than this may still be live.
ACTIVE_JOB_WINDOW_MINUTES=20

TMP_DIR=""
cleanup() { if [[ -n "$TMP_DIR" ]]; then rm -rf "$TMP_DIR"; fi; }
trap cleanup EXIT

log() { echo "$(date '+%Y-%m-%d %H:%M:%S') $*"; }
die() { log "ERROR: $*" >&2; exit 1; }

require_supported_mac() {
  [[ "$(uname -s)" == "Darwin" && "$(uname -m)" == "arm64" ]] || die "Releases only ship a server build for Apple Silicon macOS."
}

# The "latest" page redirects to /releases/tag/<tag>. Unlike the REST API this is not rate limited.
latest_tag() {
  local url
  url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")"
  [[ "$url" == */releases/tag/* ]] || die "No published release found for $REPO."
  echo "${url##*/}"
}

# Restarting the server orphans a running Herdr job, so wait for it to finish.
has_active_job() {
  local data_dir="${FACTORY_DATA_DIR:-}"
  [[ -n "$data_dir" && -d "$data_dir/jobs" ]] || return 1
  find "$data_dir/jobs" -maxdepth 2 -name job.json -mmin "-$ACTIVE_JOB_WINDOW_MINUTES" -print0 2>/dev/null \
    | xargs -0 grep -lE '"status": *"(running|queued)"' 2>/dev/null | grep -q .
}

update() {
  require_supported_mac
  local tag installed name tmp
  tag="$(latest_tag)"
  installed="$(cat "$VERSION_FILE" 2>/dev/null || true)"
  if [[ "$tag" == "$installed" && -x "$BIN" ]]; then return 0; fi

  # shellcheck disable=SC1090
  [[ -f "$ENV_FILE" ]] && source "$ENV_FILE"
  if [[ -x "$BIN" ]] && has_active_job; then
    log "Release $tag is available but a planning job is running; will retry."
    return 0
  fi

  name="factory-server-$tag-$TARGET"
  tmp="$(mktemp -d)"
  TMP_DIR="$tmp"
  log "Installing $tag (installed: ${installed:-none})"
  curl -fsSL --retry 3 -o "$tmp/server.tar.gz" "https://github.com/$REPO/releases/download/$tag/$name.tar.gz" \
    || die "Could not download $name.tar.gz. The release may still be building."
  tar -xzf "$tmp/server.tar.gz" -C "$tmp"
  file "$tmp/$name/factory-server" | grep -Eq 'Mach-O 64-bit.*arm64' \
    || die "Downloaded file is not an Apple Silicon executable."

  mkdir -p "$APP_DIR/bin"
  [[ -x "$BIN" ]] && cp -f "$BIN" "$BIN.previous"
  install -m 755 "$tmp/$name/factory-server" "$BIN.new"
  mv -f "$BIN.new" "$BIN"
  echo "$tag" > "$VERSION_FILE"

  # Restart only if the server LaunchAgent is loaded (it is not yet during the first install).
  if launchctl print "gui/$(id -u)/$SERVER_LABEL" >/dev/null 2>&1; then
    launchctl kickstart -k "gui/$(id -u)/$SERVER_LABEL"
    log "Server restarted on $tag"
  fi
}

write_plist() { # label, shell command, extra plist keys
  cat > "$LA_DIR/$1.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>$1</string>
  <key>ProgramArguments</key><array>
    <string>/bin/zsh</string><string>-c</string>
    <string>$2</string>
  </array>
  <key>WorkingDirectory</key><string>$APP_DIR</string>
  <key>RunAtLoad</key><true/>
  $3
  <key>StandardOutPath</key><string>$LOG_DIR/$1.log</string>
  <key>StandardErrorPath</key><string>$LOG_DIR/$1.err.log</string>
</dict></plist>
PLIST
}

load_agent() {
  launchctl bootout "gui/$(id -u)" "$LA_DIR/$1.plist" 2>/dev/null || true
  launchctl bootstrap "gui/$(id -u)" "$LA_DIR/$1.plist"
}

install_agents() {
  require_supported_mac
  if [[ ! -f "$ENV_FILE" ]]; then
    die "Create $ENV_FILE first (see scripts/macos/server.env.example) with FACTORY_REPO_PATH / FACTORY_API_TOKEN / FACTORY_DATA_DIR."
  fi
  chmod 600 "$ENV_FILE"
  [[ -f "${BASH_SOURCE[0]}" ]] || die "Save this script to a file and run it from there; it installs a copy of itself."

  mkdir -p "$APP_DIR/bin" "$LA_DIR" "$LOG_DIR"
  [[ "${BASH_SOURCE[0]}" -ef "$SELF" ]] || install -m 755 "${BASH_SOURCE[0]}" "$SELF"
  update
  [[ -x "$BIN" ]] || die "Server binary was not installed."

  # launchd does not inherit the shell environment, so the server reads its settings from the env file.
  write_plist "$SERVER_LABEL" "source \"$ENV_FILE\"; exec \"$BIN\"" "<key>KeepAlive</key><true/>"
  write_plist "$UPDATE_LABEL" "exec /bin/bash \"$SELF\" update" "<key>StartInterval</key><integer>$CHECK_INTERVAL_SECONDS</integer>"
  load_agent "$SERVER_LABEL"
  load_agent "$UPDATE_LABEL"

  log "Installed $(cat "$VERSION_FILE"). Server: $BIN"
  log "New releases of $REPO are checked every $((CHECK_INTERVAL_SECONDS / 60)) minutes. Logs: $LOG_DIR"
  log "Start Herdr separately under this same user: herdr server"
}

uninstall_agents() {
  for label in "$UPDATE_LABEL" "$SERVER_LABEL"; do
    launchctl bootout "gui/$(id -u)" "$LA_DIR/$label.plist" 2>/dev/null || true
    rm -f "$LA_DIR/$label.plist"
  done
  log "LaunchAgents removed. Binary ($APP_DIR), config ($ENV_FILE), and data were left in place."
}

case "${1:-}" in
  install) install_agents ;;
  update) update ;;
  uninstall) uninstall_agents ;;
  *) echo "Usage: $(basename "$0") install | update | uninstall" >&2; exit 2 ;;
esac
