#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERIFY_SCRIPT="${ROOT_DIR}/scripts/macos/verify_release.sh"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/dxtr-cleaner-release-verify.XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT

MOCK_BIN="${TMP_DIR}/bin"
APP_PATH="${TMP_DIR}/Dxtr Cleaner.app"
ZIP_PATH="${TMP_DIR}/Dxtr Cleaner.zip"
EVIDENCE_DIR="${TMP_DIR}/evidence"
mkdir -p "${MOCK_BIN}" "${APP_PATH}/Contents/MacOS"
printf '#!/bin/sh\nexit 0\n' >"${APP_PATH}/Contents/MacOS/Dxtr Cleaner"
printf '#!/bin/sh\nexit 0\n' >"${APP_PATH}/Contents/MacOS/dxtr-cleaner"
chmod +x "${APP_PATH}/Contents/MacOS/Dxtr Cleaner" "${APP_PATH}/Contents/MacOS/dxtr-cleaner"
printf 'zip-bytes' >"${ZIP_PATH}"

make_mock() {
  local name="$1"
  shift
  cat >"${MOCK_BIN}/${name}" <<EOF
#!/usr/bin/env bash
$*
EOF
  chmod +x "${MOCK_BIN}/${name}"
}

make_mock uname 'printf "Darwin\\n"'
make_mock codesign 'printf "codesign ok\\n" >&2; exit 0'
make_mock xcrun 'printf "stapler ok\\n"; exit 0'
make_mock spctl 'printf "accepted\\n"; exit 0'
make_mock xattr 'printf "0081;mock;Safari;\\n"; exit 0'
make_mock shasum 'printf "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  %s\\n" "$3"'

PATH="${MOCK_BIN}:/usr/bin:/bin" \
APP_PATH="${APP_PATH}" \
ZIP_PATH="${ZIP_PATH}" \
EVIDENCE_DIR="${EVIDENCE_DIR}" \
bash "${VERIFY_SCRIPT}"

grep -Fq 'sha256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' "${EVIDENCE_DIR}/summary.txt"
test -s "${EVIDENCE_DIR}/codesign-verify.txt"
test -s "${EVIDENCE_DIR}/codesign-details.txt"
test -s "${EVIDENCE_DIR}/stapler-validate.txt"
test -s "${EVIDENCE_DIR}/gatekeeper.txt"
test -s "${EVIDENCE_DIR}/quarantine-zip.txt"
test -s "${EVIDENCE_DIR}/quarantine-app.txt"

rm -f "${APP_PATH}/Contents/MacOS/dxtr-cleaner"
if PATH="${MOCK_BIN}:/usr/bin:/bin" APP_PATH="${APP_PATH}" ZIP_PATH="${ZIP_PATH}" bash "${VERIFY_SCRIPT}" >/dev/null 2>&1; then
  echo "verifier accepted a bundle without the scheduled CLI" >&2
  exit 1
fi

printf 'release verifier regression tests passed\n'
