#!/usr/bin/env bash
set -euo pipefail

APP_NAME="${APP_NAME:-Dxtr Cleaner}"
BUNDLE_ID="${BUNDLE_ID:-com.cnxdev.dxtr-cleaner}"
VERSION="${VERSION:-0.1.0}"
BUILD_NUMBER="${BUILD_NUMBER:-1}"
DIST_DIR="${DIST_DIR:-dist}"
APP_PATH="${DIST_DIR}/${APP_NAME}.app"
CONTENTS_DIR="${APP_PATH}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
GUI_BINARY="target/release/dxtr-cleaner-gui"
CLI_BINARY="target/release/dxtr-cleaner"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS packaging requires macOS" >&2
  exit 1
fi

cargo build --release -p cleaner-gui -p cleaner-cli

rm -rf "${APP_PATH}"
mkdir -p "${MACOS_DIR}"
cp "${GUI_BINARY}" "${MACOS_DIR}/${APP_NAME}"
cp "${CLI_BINARY}" "${MACOS_DIR}/dxtr-cleaner"
chmod +x "${MACOS_DIR}/${APP_NAME}" "${MACOS_DIR}/dxtr-cleaner"

cat > "${CONTENTS_DIR}/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDisplayName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleExecutable</key>
  <string>${APP_NAME}</string>
  <key>CFBundleIdentifier</key>
  <string>${BUNDLE_ID}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${BUILD_NUMBER}</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST

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
plutil -lint "${CONTENTS_DIR}/Info.plist"

rm -f "${DIST_DIR}/${APP_NAME}.zip"
ditto -c -k --keepParent "${APP_PATH}" "${DIST_DIR}/${APP_NAME}.zip"

echo "app: ${APP_PATH}"
echo "zip: ${DIST_DIR}/${APP_NAME}.zip"
