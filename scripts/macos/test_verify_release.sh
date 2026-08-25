#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERIFY_SCRIPT="${ROOT_DIR}/scripts/macos/verify_release.sh"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/dxtr-cleaner-release-verify.XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT

MOCK_BIN="${TMP_DIR}/bin"
ZIP_PATH="${TMP_DIR}/Dxtr Cleaner.zip"
EVIDENCE_DIR="${TMP_DIR}/evidence"
mkdir -p "${MOCK_BIN}"
printf 'verified-zip-bytes' >"${ZIP_PATH}"

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
make_mock xattr 'if [[ "${EMPTY_QUARANTINE:-0}" == "1" ]]; then exit 0; fi; printf "0081;mock;Safari;\\n"; exit 0'
make_mock shasum 'printf "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  %s\\n" "$3"'
make_mock ditto '
if [[ "$1" != "-x" || "$2" != "-k" ]]; then
  echo "unexpected ditto args" >&2
  exit 1
fi
if grep -Fq "bad-zip" "$3"; then
  mkdir -p "$4/Dxtr Cleaner.app/Contents/MacOS"
  printf "#!/bin/sh\\nexit 0\\n" >"$4/Dxtr Cleaner.app/Contents/MacOS/Dxtr Cleaner"
  chmod +x "$4/Dxtr Cleaner.app/Contents/MacOS/Dxtr Cleaner"
  exit 0
fi
mkdir -p "$4/Dxtr Cleaner.app/Contents/MacOS"
printf "#!/bin/sh\\nexit 0\\n" >"$4/Dxtr Cleaner.app/Contents/MacOS/Dxtr Cleaner"
printf "#!/bin/sh\\nexit 0\\n" >"$4/Dxtr Cleaner.app/Contents/MacOS/dxtr-cleaner"
chmod +x "$4/Dxtr Cleaner.app/Contents/MacOS/Dxtr Cleaner" "$4/Dxtr Cleaner.app/Contents/MacOS/dxtr-cleaner"
'

PATH="${MOCK_BIN}:/usr/bin:/bin" \
ZIP_PATH="${ZIP_PATH}" \
EVIDENCE_DIR="${EVIDENCE_DIR}" \
bash "${VERIFY_SCRIPT}"

grep -Fq 'sha256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' "${EVIDENCE_DIR}/summary.txt"
grep -Fq 'verified_app=Dxtr Cleaner.app (extracted from ZIP)' "${EVIDENCE_DIR}/summary.txt"
test -s "${EVIDENCE_DIR}/codesign-verify.txt"
test -s "${EVIDENCE_DIR}/codesign-details.txt"
test -s "${EVIDENCE_DIR}/stapler-validate.txt"
test -s "${EVIDENCE_DIR}/gatekeeper.txt"
test -s "${EVIDENCE_DIR}/quarantine-zip.txt"
test -s "${EVIDENCE_DIR}/quarantine-app.txt"

# A different ZIP that extracts without the scheduled CLI must fail even if prior evidence succeeded.
printf 'bad-zip' >"${ZIP_PATH}"
if PATH="${MOCK_BIN}:/usr/bin:/bin" ZIP_PATH="${ZIP_PATH}" EVIDENCE_DIR="${EVIDENCE_DIR}" bash "${VERIFY_SCRIPT}" >/dev/null 2>&1; then
  echo "verifier accepted a ZIP without the scheduled CLI" >&2
  exit 1
fi
if [[ -e "${EVIDENCE_DIR}/summary.txt" ]]; then
  echo "failed rerun retained stale successful evidence" >&2
  exit 1
fi

# Empty quarantine values must not satisfy the release gate.
printf 'verified-zip-bytes' >"${ZIP_PATH}"
if PATH="${MOCK_BIN}:/usr/bin:/bin" EMPTY_QUARANTINE=1 ZIP_PATH="${ZIP_PATH}" EVIDENCE_DIR="${EVIDENCE_DIR}" bash "${VERIFY_SCRIPT}" >/dev/null 2>&1; then
  echo "verifier accepted an empty quarantine attribute" >&2
  exit 1
fi

printf 'release verifier regression tests passed\n'
