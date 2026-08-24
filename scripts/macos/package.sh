#!/usr/bin/env bash
set -euo pipefail

APP_NAME="${APP_NAME:-Dxtr Cleaner}"
BUNDLE_ID="${BUNDLE_ID:-com.cnxdev.dxtr-cleaner}"
VERSION="${VERSION:-0.1.0}"
BUILD_NUMBER="${BUILD_NUMBER:-1}"
DIST_DIR="${DIST_DIR:-dist}"
TARGET_DIR="${CARGO_TARGET_DIR:-target}"
APP_PATH="${DIST_DIR}/${APP_NAME}.app"
CONTENTS_DIR="${APP_PATH}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
PLIST_PATH="${CONTENTS_DIR}/Info.plist"
GUI_BINARY="${TARGET_DIR}/release/dxtr-cleaner-gui"
CLI_BINARY="${TARGET_DIR}/release/dxtr-cleaner"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS packaging requires macOS" >&2
  exit 1
fi

if [[ "${APP_NAME}" == */* || -z "${APP_NAME}" ]]; then
  echo "APP_NAME must be a non-empty file name without '/'" >&2
  exit 1
fi

cargo build --release -p cleaner-gui -p cleaner-cli

for binary in "${GUI_BINARY}" "${CLI_BINARY}"; do
  if [[ ! -f "${binary}" ]]; then
    echo "expected release binary not found: ${binary}" >&2
    exit 1
  fi
done

rm -rf "${APP_PATH}"
mkdir -p "${MACOS_DIR}"
cp "${GUI_BINARY}" "${MACOS_DIR}/${APP_NAME}"
cp "${CLI_BINARY}" "${MACOS_DIR}/dxtr-cleaner"
chmod +x "${MACOS_DIR}/${APP_NAME}" "${MACOS_DIR}/dxtr-cleaner"

# Build Info.plist with plist-aware tooling so configurable metadata is escaped safely.
plutil -create xml1 "${PLIST_PATH}"
plutil -insert CFBundleDisplayName -string "${APP_NAME}" "${PLIST_PATH}"
plutil -insert CFBundleExecutable -string "${APP_NAME}" "${PLIST_PATH}"
plutil -insert CFBundleIdentifier -string "${BUNDLE_ID}" "${PLIST_PATH}"
plutil -insert CFBundleInfoDictionaryVersion -string "6.0" "${PLIST_PATH}"
plutil -insert CFBundleName -string "${APP_NAME}" "${PLIST_PATH}"
plutil -insert CFBundlePackageType -string "APPL" "${PLIST_PATH}"
plutil -insert CFBundleShortVersionString -string "${VERSION}" "${PLIST_PATH}"
plutil -insert CFBundleVersion -string "${BUILD_NUMBER}" "${PLIST_PATH}"
plutil -insert NSHighResolutionCapable -bool true "${PLIST_PATH}"

if [[ -n "${SIGNING_IDENTITY:-}" ]]; then
  codesign --force --options runtime --timestamp --sign "${SIGNING_IDENTITY}" "${MACOS_DIR}/dxtr-cleaner"
  codesign --force --options runtime --timestamp --sign "${SIGNING_IDENTITY}" "${MACOS_DIR}/${APP_NAME}"
  codesign --force --options runtime --timestamp --sign "${SIGNING_IDENTITY}" "${APP_PATH}"
else
  echo "SIGNING_IDENTITY is not set; applying ad-hoc signatures for local smoke testing" >&2
  codesign --force --sign - "${MACOS_DIR}/dxtr-cleaner"
  codesign --force --sign - "${MACOS_DIR}/${APP_NAME}"
  codesign --force --sign - "${APP_PATH}"
fi

codesign --verify --deep --strict --verbose=2 "${APP_PATH}"
plutil -lint "${PLIST_PATH}"

rm -f "${DIST_DIR}/${APP_NAME}.zip"
ditto -c -k --keepParent "${APP_PATH}" "${DIST_DIR}/${APP_NAME}.zip"

echo "app: ${APP_PATH}"
echo "zip: ${DIST_DIR}/${APP_NAME}.zip"
