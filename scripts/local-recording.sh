#!/usr/bin/env bash
# Local-only demo recording (no CI services required). Not part of the repo.
set -euo pipefail
cd "$(dirname "$0")/.."

export DISPLAY="${DISPLAY:-:99}"
export PATH="$HOME/.local/bin:$HOME/fiex-tools/usr/sbin:$PATH"
export LD_LIBRARY_PATH="$HOME/fiex-tools/lib/x86_64-linux-gnu:$HOME/fiex-tools/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export FIEX_WEBDAV_URL=http://127.0.0.1:8080/
export FIEX_CI_IMAGES="$PWD/.ci/filesystems"
export FIEX_UPLOAD_ELECTRON="$PWD/.ci/browser/node_modules/electron/dist/electron"
export FIEX_CI_SKIP="${FIEX_CI_SKIP:-file-manager,smb}"
BINARY="${1:-src-tauri/target/release/fiex}"

mkdir -p .ci/webdav
printf 'hello webdav\n' > .ci/webdav/hello.txt
python3 -m wsgidav.server.server_cli --host=127.0.0.1 --port=8080 --root="$PWD/.ci/webdav" --auth=anonymous > .ci/wsgidav.log 2>&1 &
WEBDAV=$!
trap 'kill $WEBDAV 2>/dev/null || true' EXIT
for _ in $(seq 1 30); do
  curl -sS -X PROPFIND -H 'Depth: 1' http://127.0.0.1:8080/ >/dev/null 2>&1 && break
  sleep 1
done
curl -sS -X PROPFIND -H 'Depth: 1' http://127.0.0.1:8080/ >/dev/null

rm -rf artifacts/native
dbus-run-session -- python3 scripts/native-ci.py "$BINARY"
