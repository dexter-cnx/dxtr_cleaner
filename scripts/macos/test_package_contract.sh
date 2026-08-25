#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PACKAGE_SCRIPT="${ROOT_DIR}/scripts/macos/package.sh"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/dxtr-cleaner-package-test.XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT

MOCK_BIN="${TMP_DIR}/bin"
TARGET_DIR="${TMP_DIR}/target"
DIST_DIR="${TMP_DIR}/dist"
LOG="${TMP_DIR}/commands.log"
mkdir -p "${MOCK_BIN}" "${TARGET_DIR}/release"
: > "${TARGET_DIR}/release/dxtr-cleaner-gui"
: > "${TARGET_DIR}/release/dxtr-cleaner"

cat > "${MOCK_BIN}/uname" <<'EOF'
#!/usr/bin/env bash
printf 'Darwin\n'
EOF

cat > "${MOCK_BIN}/cargo" <<'EOF'
#!/usr/bin/env bash
printf 'cargo %s\n' "$*" >> "${PACKAGE_TEST_LOG}"
EOF

cat > "${MOCK_BIN}/plutil" <<'EOF'
#!/usr/bin/env bash
printf 'plutil %s\n' "$*" >> "${PACKAGE_TEST_LOG}"
if [[ "${1:-}" == "-create" ]]; then
  : > "${3}"
fi
EOF

cat > "${MOCK_BIN}/codesign" <<'EOF'
#!/usr/bin/env bash
printf 'codesign %s\n' "$*" >> "${PACKAGE_TEST_LOG}"
EOF

cat > "${MOCK_BIN}/ditto" <<'EOF'
#!/usr/bin/env bash
printf 'ditto %s\n' "$*" >> "${PACKAGE_TEST_LOG}"
: > "${@: -1}"
EOF

chmod +x "${MOCK_BIN}/uname" "${MOCK_BIN}/cargo" "${MOCK_BIN}/plutil" "${MOCK_BIN}/codesign" "${MOCK_BIN}/ditto"

run_package() {
  local identity="$1"
  : > "${LOG}"
  if [[ -n "${identity}" ]]; then
    PATH="${MOCK_BIN}:${PATH}" \
    PACKAGE_TEST_LOG="${LOG}" \
    CARGO_TARGET_DIR="${TARGET_DIR}" \
    DIST_DIR="${DIST_DIR}" \
    SIGNING_IDENTITY="${identity}" \
    bash "${PACKAGE_SCRIPT}" >/dev/null
  else
    PATH="${MOCK_BIN}:${PATH}" \
    PACKAGE_TEST_LOG="${LOG}" \
    CARGO_TARGET_DIR="${TARGET_DIR}" \
    DIST_DIR="${DIST_DIR}" \
    bash "${PACKAGE_SCRIPT}" >/dev/null 2>&1
  fi
}

require_log() {
  local needle="$1"
  if ! grep -Fq -- "${needle}" "${LOG}"; then
    echo "packaging contract missing executed command: ${needle}" >&2
    exit 1
  fi
}

run_package "Developer ID Application: Example"
require_log 'cargo build --release -p cleaner-gui -p cleaner-cli'
require_log 'codesign --force --options runtime --timestamp --sign Developer ID Application: Example'
require_log "${DIST_DIR}/Dxtr Cleaner.app/Contents/MacOS/dxtr-cleaner"
require_log "ditto -c -k --keepParent ${DIST_DIR}/Dxtr Cleaner.app ${DIST_DIR}/Dxtr Cleaner.zip"
[[ -x "${DIST_DIR}/Dxtr Cleaner.app/Contents/MacOS/dxtr-cleaner" ]]
[[ -f "${DIST_DIR}/Dxtr Cleaner.zip" ]]

run_package ""
require_log 'codesign --force --sign -'
require_log "${DIST_DIR}/Dxtr Cleaner.app/Contents/MacOS/dxtr-cleaner"

printf 'macOS packaging contract tests passed\n'
