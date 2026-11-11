#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ENV_FILE="$HOME/.config/software-factory/server.env"
LA_DIR="$HOME/Library/LaunchAgents"
LOG_DIR="$HOME/Library/Logs/SoftwareFactory"
PLIST="$LA_DIR/dev.softwarefactory.server.plist"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "Create $ENV_FILE first with FACTORY_REPO_PATH / FACTORY_API_TOKEN / FACTORY_DATA_DIR." >&2
  echo "Set permissions: chmod 600 '$ENV_FILE'" >&2
  exit 1
fi
chmod 600 "$ENV_FILE"
cd "$ROOT"
cargo build -p factory-server --release
mkdir -p "$LA_DIR" "$LOG_DIR"
# Use the absolute path; launchd does not inherit shell PATH or Cargo environment by default.
cat > "$PLIST" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>dev.softwarefactory.server</string>
  <key>ProgramArguments</key><array>
    <string>/bin/zsh</string><string>-c</string>
    <string>source "$ENV_FILE"; exec "$ROOT/target/release/factory-server"</string>
  </array>
  <key>WorkingDirectory</key><string>$ROOT</string>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>StandardOutPath</key><string>$LOG_DIR/server.log</string>
  <key>StandardErrorPath</key><string>$LOG_DIR/server.err.log</string>
</dict></plist>
PLIST
# Server may already be installed: unload idempotently.
launchctl bootout "gui/$(id -u)" "$PLIST" 2>/dev/null || true
launchctl bootstrap "gui/$(id -u)" "$PLIST"
echo "Factory server LaunchAgent installed: $PLIST"
echo "Start Herdr separately: herdr server (under this same user)."
