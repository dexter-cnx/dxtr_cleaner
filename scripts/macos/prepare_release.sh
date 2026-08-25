#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DIST_DIR="${DIST_DIR:-${ROOT_DIR}/dist}"
APP_NAME="${APP_NAME:-Dxtr Cleaner}"
ZIP_PATH="${DIST_DIR}/${APP_NAME}.zip"
PREP_EVIDENCE_DIR="${PREP_EVIDENCE_DIR:-${DIST_DIR}/prepublish-evidence}"

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
  bash "${ROOT_DIR}/scripts/macos/package.sh"

DIST_DIR="${DIST_DIR}" APP_NAME="${APP_NAME}" NOTARY_PROFILE="${NOTARY_PROFILE}" \
  bash "${ROOT_DIR}/scripts/macos/notarize.sh"

# Pre-publication diagnostic verification deliberately skips quarantine only here.
# The published/downloaded ZIP must still pass verify_release.sh with its default
# QUARANTINE_REQUIRED=1 before the M4 release gate can be closed.
ZIP_PATH="${ZIP_PATH}" EVIDENCE_DIR="${PREP_EVIDENCE_DIR}" QUARANTINE_REQUIRED=0 \
  bash "${ROOT_DIR}/scripts/macos/verify_release.sh"

SHA256="$(awk -F= '/^sha256=/{print $2}' "${PREP_EVIDENCE_DIR}/summary.txt")"
if [[ ! "${SHA256}" =~ ^[0-9a-fA-F]{64}$ ]]; then
  echo "prepublish verification did not produce a valid SHA-256" >&2
  exit 1
fi

VERSION="${VERSION}" SHA256="${SHA256}" URL="${URL}" \
  bash "${ROOT_DIR}/scripts/macos/generate_cask.sh"

printf 'macOS release preparation passed\n'
printf 'zip: %s\n' "${ZIP_PATH}"
printf 'sha256: %s\n' "${SHA256}"
printf 'prepublish evidence: %s\n' "${PREP_EVIDENCE_DIR}"
printf 'next: publish/download the exact ZIP, then run make verify-macos-release on the quarantined download\n'
