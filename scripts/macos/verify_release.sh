#!/usr/bin/env bash
set -euo pipefail

APP_PATH="${APP_PATH:-dist/Dxtr Cleaner.app}"
ZIP_PATH="${ZIP_PATH:-dist/Dxtr Cleaner.zip}"
EVIDENCE_DIR="${EVIDENCE_DIR:-dist/release-evidence}"
QUARANTINE_REQUIRED="${QUARANTINE_REQUIRED:-1}"
CLI_PATH="${APP_PATH}/Contents/MacOS/dxtr-cleaner"
GUI_PATH="${APP_PATH}/Contents/MacOS/Dxtr Cleaner"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "release verification requires macOS" >&2
  exit 1
fi

for path in "${APP_PATH}" "${ZIP_PATH}" "${GUI_PATH}" "${CLI_PATH}"; do
  if [[ ! -e "${path}" ]]; then
    echo "required release artifact is missing: ${path}" >&2
    exit 1
  fi
done

if [[ ! -f "${CLI_PATH}" || -L "${CLI_PATH}" || ! -x "${CLI_PATH}" ]]; then
  echo "bundled dxtr-cleaner must be an executable regular file" >&2
  exit 1
fi

mkdir -p "${EVIDENCE_DIR}"

run_and_capture() {
  local name="$1"
  shift
  "$@" >"${EVIDENCE_DIR}/${name}.txt" 2>&1
}

run_and_capture codesign-verify codesign --verify --deep --strict --verbose=2 "${APP_PATH}"
run_and_capture codesign-details codesign -dv --verbose=4 "${APP_PATH}"
run_and_capture stapler-validate xcrun stapler validate "${APP_PATH}"
run_and_capture gatekeeper spctl --assess --type execute --verbose=4 "${APP_PATH}"

if [[ "${QUARANTINE_REQUIRED}" == "1" ]]; then
  run_and_capture quarantine-zip xattr -p com.apple.quarantine "${ZIP_PATH}"
  run_and_capture quarantine-app xattr -p com.apple.quarantine "${APP_PATH}"
fi

SHA256="$(shasum -a 256 "${ZIP_PATH}" | awk '{print $1}')"
if [[ ! "${SHA256}" =~ ^[0-9a-fA-F]{64}$ ]]; then
  echo "failed to calculate release ZIP SHA-256" >&2
  exit 1
fi
printf '%s  %s\n' "${SHA256}" "${ZIP_PATH}" >"${EVIDENCE_DIR}/sha256.txt"

printf 'app=%s\nzip=%s\nsha256=%s\nquarantine_required=%s\n' \
  "${APP_PATH}" "${ZIP_PATH}" "${SHA256}" "${QUARANTINE_REQUIRED}" \
  >"${EVIDENCE_DIR}/summary.txt"

printf 'macOS release verification passed\n'
printf 'evidence: %s\n' "${EVIDENCE_DIR}"
printf 'sha256: %s\n' "${SHA256}"
