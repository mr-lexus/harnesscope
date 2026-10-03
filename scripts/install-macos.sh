#!/usr/bin/env bash
# User-scoped installation. Requires a locally built macOS binary and Python 3.
set -euo pipefail
binary="${1:-target/release/harnesscope}"
port="${HARNESSCOPE_SERVER_PORT:-4242}"
codex_root="${CODEX_HOME:-$HOME/.codex}"
install_dir="$HOME/.local/bin"
data_dir="${HARNESSCOPE_DATA_DIR:-$HOME/Library/Application Support/com.harnesscope.harnesscope}"
agent="$HOME/Library/LaunchAgents/com.harnesscope.collector.plist"
if [[ "$(uname -s)" != Darwin ]]; then echo 'This installer requires macOS.' >&2; exit 1; fi
command -v python3 >/dev/null || { echo 'Python 3 is required to write and validate the LaunchAgent.' >&2; exit 1; }
[[ -x "$binary" ]] || { echo 'Build a macOS binary with cargo build --release first.' >&2; exit 1; }
[[ "$port" =~ ^[0-9]+$ ]] && (( port >= 1024 && port <= 65535 )) || { echo 'Invalid port.' >&2; exit 1; }
mkdir -p "$install_dir" "$data_dir" "$HOME/Library/LaunchAgents"
export HARNESSCOPE_DB_PATH="$data_dir/harnesscope.db"
export HARNESSCOPE_DATA_DIR="$data_dir"
export HARNESSCOPE_SERVER_PORT="$port"
export HARNESSCOPE_SERVER_URL="http://127.0.0.1:$port"
# A listener on this port must belong to this exact database before stopping it.
python3 - "$port" "$HARNESSCOPE_DB_PATH" <<'PY'
import json, pathlib, socket, sys, urllib.request
port, database = int(sys.argv[1]), pathlib.Path(sys.argv[2]).resolve()
try:
    with socket.create_connection(('127.0.0.1', port), timeout=1): pass
except OSError:
    sys.exit(0)
opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
with opener.open(f'http://127.0.0.1:{port}/api/v1/collection', timeout=3) as response:
    status = json.load(response)
if pathlib.Path(status.get('database_path') or '/unrelated').resolve() != database:
    sys.exit('Port belongs to another collector or application; installation stopped.')
opener.open(urllib.request.Request(f'http://127.0.0.1:{port}/api/v1/shutdown', method='POST'), timeout=3).close()
PY
launchctl bootout "gui/$(id -u)/com.harnesscope.collector" 2>/dev/null || true
if [[ -f "$HARNESSCOPE_DB_PATH" ]]; then
  mkdir -p "$data_dir/backups"
  "$binary" backup create --output "$data_dir/backups/before-install-$(date +%Y%m%d-%H%M%S)-$$.db"
fi
# Publish a new executable without modifying one that is still mapped by a process.
install -m 755 "$binary" "$install_dir/harnesscope.new"
mv "$install_dir/harnesscope.new" "$install_dir/harnesscope"
for directory in sessions archived_sessions; do
  if [[ -d "$codex_root/$directory" ]]; then
    "$install_dir/harnesscope" sources add-codex --path "$codex_root/$directory"
  fi
done
python3 - "$agent" "$install_dir/harnesscope" "$data_dir" "$port" <<'PY'
import os, pathlib, plistlib, sys, tempfile
destination, executable, data, port = sys.argv[1:]
value = {'Label': 'com.harnesscope.collector', 'ProgramArguments': [executable, 'serve', '--port', port],
         'RunAtLoad': True, 'KeepAlive': {'SuccessfulExit': False}, 'ThrottleInterval': 10,
         'EnvironmentVariables': {'HARNESSCOPE_DATA_DIR': data, 'HARNESSCOPE_DB_PATH': str(pathlib.Path(data)/'harnesscope.db'), 'HARNESSCOPE_SERVER_PORT': port},
         'StandardOutPath': str(pathlib.Path(data)/'server.log'), 'StandardErrorPath': str(pathlib.Path(data)/'server-error.log')}
with tempfile.NamedTemporaryFile(dir=pathlib.Path(destination).parent, delete=False) as temporary:
    plistlib.dump(value, temporary); temporary.flush(); os.fsync(temporary.fileno()); name = temporary.name
os.replace(name, destination)
PY
plutil -lint "$agent"
launchctl bootstrap "gui/$(id -u)" "$agent"
echo "Harnesscope: http://127.0.0.1:$port/evidence"
echo 'Enable sanitized evidence in Sources. To add hooks and local OTel:'
printf '  "%s" capture setup --codex-home "%s"\n' "$install_dir/harnesscope" "$codex_root"
echo 'Review and trust the added hooks inside Codex. The installer does not bypass trust.'
echo 'To stop auto-start: launchctl bootout gui/$(id -u)/com.harnesscope.collector'
