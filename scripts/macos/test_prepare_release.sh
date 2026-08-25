#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER="${ROOT_DIR}/scripts/macos/prepare_release.sh"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/dxtr-cleaner-release-prepare.XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT

MOCK_BIN="${TMP_DIR}/bin"
DIST_DIR="${TMP_DIR}/dist"
LOG="${TMP_DIR}/calls.log"
mkdir -p "${MOCK_BIN}" "${DIST_DIR}"

cat >"${MOCK_BIN}/uname" <<'EOF'
#!/usr/bin/env bash
printf 'Darwin\n'
EOF
chmod +x "${MOCK_BIN}/uname"

make_step() {
  local path="$1"
  local body="$2"
  cat >"${path}" <<EOF
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "${body}" >>"${LOG}"
EOF
  chmod +x "${path}"
}

PACKAGE="${TMP_DIR}/package.sh"
NOTARIZE="${TMP_DIR}/notarize.sh"
VERIFY="${TMP_DIR}/verify.sh"
CASK="${TMP_DIR}/cask.sh"
make_step "${PACKAGE}" 'package'
make_step "${NOTARIZE}" 'notarize'
cat >"${VERIFY}" <<EOF
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' verify >>"${LOG}"
mkdir -p "\${EVIDENCE_DIR}"
printf 'sha256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n' >"\${EVIDENCE_DIR}/summary.txt"
EOF
chmod +x "${VERIFY}"
cat >"${CASK}" <<EOF
#!/usr/bin/env bash
set -euo pipefail
printf 'cask:%s:%s:%s\n' "\${VERSION}" "\${SHA256}" "\${URL}" >>"${LOG}"
EOF
chmod +x "${CASK}"

PATH="${MOCK_BIN}:/usr/bin:/bin" \
DIST_DIR="${DIST_DIR}" \
SIGNING_IDENTITY='Developer ID Application: Example' \
NOTARY_PROFILE='dxtr-notary' \
VERSION='1.2.3' \
URL='https://github.com/dexter-cnx/dxtr_cleaner/releases/download/v1.2.3/Dxtr%20Cleaner.zip' \
PACKAGE_SCRIPT="${PACKAGE}" NOTARIZE_SCRIPT="${NOTARIZE}" VERIFY_SCRIPT="${VERIFY}" CASK_SCRIPT="${CASK}" \
bash "${RUNNER}" >/dev/null

test "$(sed -n '1p' "${LOG}")" = package
test "$(sed -n '2p' "${LOG}")" = notarize
test "$(sed -n '3p' "${LOG}")" = verify
grep -Fq 'cask:1.2.3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:https://github.com/dexter-cnx/dxtr_cleaner/releases/download/v1.2.3/Dxtr%20Cleaner.zip' "${LOG}"

if PATH="${MOCK_BIN}:/usr/bin:/bin" DIST_DIR="${DIST_DIR}" NOTARY_PROFILE=x VERSION=1 URL=https://example.invalid PACKAGE_SCRIPT="${PACKAGE}" NOTARIZE_SCRIPT="${NOTARIZE}" VERIFY_SCRIPT="${VERIFY}" CASK_SCRIPT="${CASK}" bash "${RUNNER}" >/dev/null 2>&1; then
  echo 'runner accepted missing SIGNING_IDENTITY' >&2
  exit 1
fi

if PATH="${MOCK_BIN}:/usr/bin:/bin" DIST_DIR="${DIST_DIR}" SIGNING_IDENTITY=- NOTARY_PROFILE=x VERSION=1 URL=https://example.invalid PACKAGE_SCRIPT="${PACKAGE}" NOTARIZE_SCRIPT="${NOTARIZE}" VERIFY_SCRIPT="${VERIFY}" CASK_SCRIPT="${CASK}" bash "${RUNNER}" >/dev/null 2>&1; then
  echo 'runner accepted ad-hoc signing identity' >&2
  exit 1
fi

printf 'release preparation regression tests passed\n'
