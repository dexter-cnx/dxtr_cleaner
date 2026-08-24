#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:-}"
SHA256="${SHA256:-}"
URL="${URL:-}"
OUTPUT="${OUTPUT:-Casks/dxtr-cleaner.rb}"

if [[ -z "${VERSION}" ]]; then
  echo "VERSION is required" >&2
  exit 1
fi
if [[ ! "${SHA256}" =~ ^[0-9a-fA-F]{64}$ ]]; then
  echo "SHA256 must be a 64-character hexadecimal digest" >&2
  exit 1
fi
if [[ ! "${URL}" =~ ^https:// ]]; then
  echo "URL must use https://" >&2
  exit 1
fi

ruby_quote() {
  local value="$1"
  value=${value//\\/\\\\}
  value=${value//\"/\\\"}
  printf '%s' "$value"
}

mkdir -p "$(dirname "${OUTPUT}")"

cat > "${OUTPUT}" <<CASK
cask "dxtr-cleaner" do
  version "$(ruby_quote "${VERSION}")"
  sha256 "$(ruby_quote "${SHA256,,}")"

  url "$(ruby_quote "${URL}")",
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
printf 'generated: %s\n' "${OUTPUT}"
