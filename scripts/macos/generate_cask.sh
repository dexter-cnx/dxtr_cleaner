#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:-}"
SHA256="${SHA256:-}"
URL="${URL:-}"
OUTPUT="${OUTPUT:-Casks/dxtr-cleaner.rb}"
RELEASE_URL_PREFIX="https://github.com/dexter-cnx/dxtr_cleaner/releases/download/"

if [[ -z "${VERSION}" ]]; then
  echo "VERSION is required" >&2
  exit 1
fi
if [[ ! "${SHA256}" =~ ^[0-9a-fA-F]{64}$ ]]; then
  echo "SHA256 must be a 64-character hexadecimal digest" >&2
  exit 1
fi
if [[ "${URL}" != "${RELEASE_URL_PREFIX}"* ]]; then
  echo "URL must point to a dxtr_cleaner GitHub release asset" >&2
  exit 1
fi

ruby_single_quote() {
  local value="$1"
  value=${value//\\/\\\\}
  value=${value//\'/\\\'}
  printf '%s' "$value"
}

NORMALIZED_SHA256="$(printf '%s' "${SHA256}" | tr '[:upper:]' '[:lower:]')"

mkdir -p "$(dirname "${OUTPUT}")"

cat > "${OUTPUT}" <<CASK
cask "dxtr-cleaner" do
  version '$(ruby_single_quote "${VERSION}")'
  sha256 '$(ruby_single_quote "${NORMALIZED_SHA256}")'

  url '$(ruby_single_quote "${URL}")',
      verified: "github.com/dexter-cnx/dxtr_cleaner/"
  name "Dxtr Cleaner"
  desc "Safe macOS cleaner and application uninstaller"
  homepage "https://github.com/dexter-cnx/dxtr_cleaner"

  app "Dxtr Cleaner.app"

  zap trash: [
    "~/Library/LaunchAgents/com.cnxdev.dxtr-cleaner.smart-scan.plist",
    "~/Library/Logs/DxtrCleaner",
  ]
end
CASK

ruby -c "${OUTPUT}" >/dev/null

# Regression guard: generated release values must remain non-interpolating Ruby data.
if grep -Eq '^[[:space:]]*(version|sha256|url) "' "${OUTPUT}"; then
  echo "generated release values must use non-interpolating Ruby literals" >&2
  exit 1
fi

printf 'generated: %s\n' "${OUTPUT}"
