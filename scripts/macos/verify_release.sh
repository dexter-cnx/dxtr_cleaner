#!/usr/bin/env bash
set -euo pipefail

APP_NAME="${APP_NAME:-Dxtr Cleaner}"
ZIP_PATH="${ZIP_PATH:-dist/Dxtr Cleaner.zip}"
EVIDENCE_DIR="${EVIDENCE_DIR:-dist/release-evidence}"
QUARANTINE_REQUIRED="${QUARANTINE_REQUIRED:-1}"
EXPECTED_SHA256="${EXPECTED_SHA256:-}"
EXPECTED_SHA256_FILE="${EXPECTED_SHA256_FILE:-}"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "release verification requires macOS" >&2
  exit 1
fi

if [[ ! -f "${ZIP_PATH}" || -L "${ZIP_PATH}" ]]; then
  echo "release ZIP must be a regular file: ${ZIP_PATH}" >&2
  exit 1
fi

if [[ -n "${EXPECTED_SHA256_FILE}" ]]; then
  if [[ ! -f "${EXPECTED_SHA256_FILE}" || -L "${EXPECTED_SHA256_FILE}" ]]; then
    echo "expected SHA-256 file must be a regular file: ${EXPECTED_SHA256_FILE}" >&2
    exit 1
  fi
  FILE_SHA256="$(tr -d '[:space:]' <"${EXPECTED_SHA256_FILE}")"
  if [[ -n "${EXPECTED_SHA256}" && "${EXPECTED_SHA256}" != "${FILE_SHA256}" ]]; then
    echo "EXPECTED_SHA256 and EXPECTED_SHA256_FILE disagree" >&2
    exit 1
  fi
  EXPECTED_SHA256="${FILE_SHA256}"
fi

if [[ "${QUARANTINE_REQUIRED}" == "1" && ! "${EXPECTED_SHA256}" =~ ^[0-9a-fA-F]{64}$ ]]; then
  echo "final release verification requires EXPECTED_SHA256 or EXPECTED_SHA256_FILE from prepare-macos-release" >&2
  exit 1
fi

EVIDENCE_PARENT="$(dirname "${EVIDENCE_DIR}")"
EVIDENCE_NAME="$(basename "${EVIDENCE_DIR}")"
mkdir -p "${EVIDENCE_PARENT}"
rm -rf "${EVIDENCE_DIR}"
STAGING_DIR="$(mktemp -d "${EVIDENCE_PARENT}/.${EVIDENCE_NAME}.staging.XXXXXX")"
EXTRACT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/dxtr-cleaner-release-extract.XXXXXX")"
cleanup() {
  rm -rf "${STAGING_DIR}" "${EXTRACT_DIR}"
}
trap cleanup EXIT

run_and_capture() {
  local name="$1"
  shift
  "$@" >"${STAGING_DIR}/${name}.txt" 2>&1
}

if [[ "${QUARANTINE_REQUIRED}" == "1" ]]; then
  run_and_capture quarantine-zip xattr -p com.apple.quarantine "${ZIP_PATH}"
  if [[ ! -s "${STAGING_DIR}/quarantine-zip.txt" ]]; then
    echo "release ZIP quarantine attribute must be non-empty" >&2
    exit 1
  fi
fi

ditto -x -k "${ZIP_PATH}" "${EXTRACT_DIR}"
APP_PATH="${EXTRACT_DIR}/${APP_NAME}.app"
GUI_PATH="${APP_PATH}/Contents/MacOS/${APP_NAME}"
CLI_PATH="${APP_PATH}/Contents/MacOS/dxtr-cleaner"

for path in "${APP_PATH}" "${GUI_PATH}" "${CLI_PATH}"; do
  if [[ ! -e "${path}" ]]; then
    echo "required release artifact is missing from ZIP: ${path}" >&2
    exit 1
  fi
done

if [[ ! -f "${CLI_PATH}" || -L "${CLI_PATH}" || ! -x "${CLI_PATH}" ]]; then
  echo "bundled dxtr-cleaner must be an executable regular file" >&2
  exit 1
fi

run_and_capture codesign-verify codesign --verify --deep --strict --verbose=2 "${APP_PATH}"
run_and_capture codesign-details codesign -dv --verbose=4 "${APP_PATH}"
run_and_capture stapler-validate xcrun stapler validate "${APP_PATH}"
run_and_capture gatekeeper spctl --assess --type execute --verbose=4 "${APP_PATH}"

if [[ "${QUARANTINE_REQUIRED}" == "1" ]]; then
  run_and_capture quarantine-app xattr -p com.apple.quarantine "${APP_PATH}"
  if [[ ! -s "${STAGING_DIR}/quarantine-app.txt" ]]; then
    echo "extracted app quarantine attribute must be non-empty" >&2
    exit 1
  fi
fi

SHA256="$(shasum -a 256 "${ZIP_PATH}" | awk '{print $1}')"
if [[ ! "${SHA256}" =~ ^[0-9a-fA-F]{64}$ ]]; then
  echo "failed to calculate release ZIP SHA-256" >&2
  exit 1
fi
if [[ -n "${EXPECTED_SHA256}" ]]; then
  NORMALIZED_SHA256="$(printf '%s' "${SHA256}" | tr '[:upper:]' '[:lower:]')"
  NORMALIZED_EXPECTED_SHA256="$(printf '%s' "${EXPECTED_SHA256}" | tr '[:upper:]' '[:lower:]')"
  if [[ "${NORMALIZED_SHA256}" != "${NORMALIZED_EXPECTED_SHA256}" ]]; then
    echo "release ZIP SHA-256 does not match prepared digest" >&2
    exit 1
  fi
fi
printf '%s  %s\n' "${SHA256}" "${ZIP_PATH}" >"${STAGING_DIR}/sha256.txt"

printf 'zip=%s\nverified_app=%s\nsha256=%s\nexpected_sha256=%s\nquarantine_required=%s\n' \
  "${ZIP_PATH}" "${APP_NAME}.app (extracted from ZIP)" "${SHA256}" "${EXPECTED_SHA256:-not-required}" "${QUARANTINE_REQUIRED}" \
  >"${STAGING_DIR}/summary.txt"

mv "${STAGING_DIR}" "${EVIDENCE_DIR}"
STAGING_DIR="${EVIDENCE_DIR}/.published"

printf 'macOS release verification passed\n'
printf 'evidence: %s\n' "${EVIDENCE_DIR}"
printf 'sha256: %s\n' "${SHA256}"
