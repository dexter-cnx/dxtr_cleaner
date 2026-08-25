#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DIST_DIR="${DIST_DIR:-${ROOT_DIR}/dist}"
APP_NAME="${APP_NAME:-Dxtr Cleaner}"
ZIP_PATH="${DIST_DIR}/${APP_NAME}.zip"
PREP_EVIDENCE_DIR="${PREP_EVIDENCE_DIR:-${DIST_DIR}/prepublish-evidence}"
EXPECTED_SHA256_FILE="${EXPECTED_SHA256_FILE:-${PREP_EVIDENCE_DIR}/expected-sha256.txt}"
PACKAGE_SCRIPT="${PACKAGE_SCRIPT:-${ROOT_DIR}/scripts/macos/package.sh}"
NOTARIZE_SCRIPT="${NOTARIZE_SCRIPT:-${ROOT_DIR}/scripts/macos/notarize.sh}"
VERIFY_SCRIPT="${VERIFY_SCRIPT:-${ROOT_DIR}/scripts/macos/verify_release.sh}"
CASK_SCRIPT="${CASK_SCRIPT:-${ROOT_DIR}/scripts/macos/generate_cask.sh}"

require_env() {
  local name="$1"
  if [[ -z "${!name:-}" ]]; then
    echo "${name} is required" >&2
    exit 1
  fi
}

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS release preparation requires macOS" >&2
  exit 1
fi

require_env SIGNING_IDENTITY
require_env NOTARY_PROFILE
require_env VERSION
require_env URL

if [[ "${SIGNING_IDENTITY}" == "-" ]]; then
  echo "SIGNING_IDENTITY must be a real Developer ID Application identity" >&2
  exit 1
fi

DIST_DIR="${DIST_DIR}" APP_NAME="${APP_NAME}" VERSION="${VERSION}" SIGNING_IDENTITY="${SIGNING_IDENTITY}" \
  bash "${PACKAGE_SCRIPT}"

DIST_DIR="${DIST_DIR}" APP_NAME="${APP_NAME}" NOTARY_PROFILE="${NOTARY_PROFILE}" \
  bash "${NOTARIZE_SCRIPT}"

# Pre-publication diagnostic verification deliberately skips quarantine only here.
ZIP_PATH="${ZIP_PATH}" EVIDENCE_DIR="${PREP_EVIDENCE_DIR}" QUARANTINE_REQUIRED=0 \
  bash "${VERIFY_SCRIPT}"

SHA256="$(awk -F= '/^sha256=/{print $2}' "${PREP_EVIDENCE_DIR}/summary.txt")"
if [[ ! "${SHA256}" =~ ^[0-9a-fA-F]{64}$ ]]; then
  echo "prepublish verification did not produce a valid SHA-256" >&2
  exit 1
fi

mkdir -p "$(dirname "${EXPECTED_SHA256_FILE}")"
printf '%s\n' "${SHA256}" >"${EXPECTED_SHA256_FILE}"

VERSION="${VERSION}" SHA256="${SHA256}" URL="${URL}" \
  bash "${CASK_SCRIPT}"

printf 'macOS release preparation passed\n'
printf 'zip: %s\n' "${ZIP_PATH}"
printf 'sha256: %s\n' "${SHA256}"
printf 'expected sha file: %s\n' "${EXPECTED_SHA256_FILE}"
printf 'prepublish evidence: %s\n' "${PREP_EVIDENCE_DIR}"
printf 'next: publish/download the exact ZIP, then run ZIP_PATH=<downloaded-zip> EXPECTED_SHA256_FILE=%s make verify-macos-release\n' "${EXPECTED_SHA256_FILE}"
