#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PACKAGE_SCRIPT="${ROOT_DIR}/scripts/macos/package.sh"

require_literal() {
  local needle="$1"
  if ! grep -Fq -- "${needle}" "${PACKAGE_SCRIPT}"; then
    echo "packaging contract missing: ${needle}" >&2
    exit 1
  fi
}

require_literal 'cargo build --release -p cleaner-gui -p cleaner-cli'
require_literal 'CLI_BINARY="${TARGET_DIR}/release/dxtr-cleaner"'
require_literal 'cp "${CLI_BINARY}" "${MACOS_DIR}/dxtr-cleaner"'
require_literal 'chmod +x "${MACOS_DIR}/${APP_NAME}" "${MACOS_DIR}/dxtr-cleaner"'
require_literal 'codesign --force --options runtime --timestamp --sign "${SIGNING_IDENTITY}" "${MACOS_DIR}/dxtr-cleaner"'
require_literal 'codesign --force --sign - "${MACOS_DIR}/dxtr-cleaner"'
require_literal 'ditto -c -k --keepParent "${APP_PATH}" "${DIST_DIR}/${APP_NAME}.zip"'

printf 'macOS packaging contract tests passed\n'
