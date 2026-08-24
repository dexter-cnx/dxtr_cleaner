#!/usr/bin/env bash
set -euo pipefail

APP_NAME="${APP_NAME:-Dxtr Cleaner}"
DIST_DIR="${DIST_DIR:-dist}"
APP_PATH="${DIST_DIR}/${APP_NAME}.app"
ZIP_PATH="${DIST_DIR}/${APP_NAME}.zip"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS notarization requires macOS" >&2
  exit 1
fi

if [[ -z "${NOTARY_PROFILE:-}" ]]; then
  echo "NOTARY_PROFILE must name an xcrun notarytool keychain profile" >&2
  exit 1
fi

if [[ ! -d "${APP_PATH}" || ! -f "${ZIP_PATH}" ]]; then
  echo "package first: expected ${APP_PATH} and ${ZIP_PATH}" >&2
  exit 1
fi

codesign --verify --deep --strict --verbose=2 "${APP_PATH}"
xcrun notarytool submit "${ZIP_PATH}" --keychain-profile "${NOTARY_PROFILE}" --wait
xcrun stapler staple "${APP_PATH}"
xcrun stapler validate "${APP_PATH}"
spctl --assess --type execute --verbose=4 "${APP_PATH}"

rm -f "${ZIP_PATH}"
ditto -c -k --keepParent "${APP_PATH}" "${ZIP_PATH}"

echo "notarized app: ${APP_PATH}"
echo "distribution zip: ${ZIP_PATH}"
