#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GENERATOR="${ROOT_DIR}/scripts/macos/generate_cask.sh"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/dxtr-cleaner-cask-test.XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT

UPPER_SHA="AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
LOWER_SHA="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
VALID_URL="https://github.com/dexter-cnx/dxtr_cleaner/releases/download/v0.1.0/Dxtr%20Cleaner.zip"
OUTPUT="${TMP_DIR}/dxtr-cleaner.rb"

VERSION="1.2.3-#{puts 'literal'}" \
SHA256="${UPPER_SHA}" \
URL="${VALID_URL}" \
OUTPUT="${OUTPUT}" \
bash "${GENERATOR}"

grep -Fq "sha256 '${LOWER_SHA}'" "${OUTPUT}"
grep -Fq "#{puts" "${OUTPUT}"
grep -Fq "url '${VALID_URL}'" "${OUTPUT}"
ruby -c "${OUTPUT}" >/dev/null

if VERSION="0.1.0" \
  SHA256="${UPPER_SHA}" \
  URL="https://example.com/Dxtr%20Cleaner.zip" \
  OUTPUT="${TMP_DIR}/invalid.rb" \
  bash "${GENERATOR}" >/dev/null 2>&1; then
  echo "generator accepted a release URL outside dexter-cnx/dxtr_cleaner" >&2
  exit 1
fi

for traversal_url in \
  "https://github.com/dexter-cnx/dxtr_cleaner/releases/download/../../../../other/project/releases/download/v1/file.zip" \
  "https://github.com/dexter-cnx/dxtr_cleaner/releases/download/%2e%2e/%2E%2E/other/file.zip"; do
  if VERSION="0.1.0" \
    SHA256="${UPPER_SHA}" \
    URL="${traversal_url}" \
    OUTPUT="${TMP_DIR}/traversal.rb" \
    bash "${GENERATOR}" >/dev/null 2>&1; then
    echo "generator accepted a release URL with dot-segment traversal" >&2
    exit 1
  fi
done

printf 'generate_cask regression tests passed\n'
