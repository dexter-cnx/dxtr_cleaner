# M4 Release Verification

M4 implementation is functionally complete in code, but the milestone is **not release-complete** until a real macOS release artifact passes Developer ID signing, notarization, quarantined Gatekeeper validation, and Homebrew install/uninstall smoke testing.

This document is the authoritative evidence checklist for the final M4 gate.

## 1. Preconditions

Required external credentials and tooling:

- Apple Developer ID Application certificate available in the signing keychain
- `SIGNING_IDENTITY` set to the exact Developer ID Application identity
- notarization credentials stored in an `xcrun notarytool` keychain profile
- `NOTARY_PROFILE` set to that profile name
- Homebrew installed for the final cask smoke test
- release version selected and consistent with the GitHub release tag
- final GitHub release asset URL selected

Do not commit certificates, passwords, App Store Connect keys, notary credentials, or keychain exports.

## 2. Canonical phase 1 — prepare the release

Use the repository's release runner rather than invoking package/notarize/cask scripts independently:

```bash
SIGNING_IDENTITY="Developer ID Application: ..." \
NOTARY_PROFILE=<profile> \
VERSION=<release-version> \
URL="https://github.com/dexter-cnx/dxtr_cleaner/releases/download/v<release-version>/Dxtr%20Cleaner.zip" \
make prepare-macos-release
```

`prepare-macos-release` runs, in fail-fast order:

1. Developer ID packaging with hardened runtime
2. notarization submission
3. staple + staple validation
4. Gatekeeper assessment of the locally prepared artifact
5. pre-publication verification with quarantine intentionally disabled
6. SHA-256 calculation from the final ZIP after stapling
7. persistence of the prepared digest
8. Homebrew cask generation using that exact digest and release URL

Ad-hoc signing is rejected by the release runner and does **not** satisfy this gate.

Expected outputs include:

- `dist/Dxtr Cleaner.app`
- `dist/Dxtr Cleaner.zip`
- `dist/prepublish-evidence/`
- persisted expected SHA-256 for the post-publication gate
- generated `Casks/dxtr-cleaner.rb`

The app bundle must contain both the GPUI executable and bundled `Contents/MacOS/dxtr-cleaner` CLI used by LaunchAgent scheduling.

## 3. Notarization evidence

Retain evidence that:

- `notarytool` returns an accepted result
- `stapler staple` succeeds
- `stapler validate` succeeds
- `spctl --assess --type execute --verbose=4` succeeds
- the distribution ZIP is rebuilt after stapling

Record the notarization submission ID in release notes or retained logs.

## 4. Publish the exact prepared ZIP

Upload **the exact ZIP produced by phase 1** to the GitHub release URL used when generating the cask.

Do not rebuild, recompress, rename-and-recreate, or otherwise mutate the archive after the prepared SHA-256 is persisted. A byte-different ZIP is a different release artifact even when it contains an otherwise valid signed/notarized app.

## 5. Canonical phase 2 — verify the published download

Download the published asset through a normal browser/download path that applies macOS quarantine metadata. Then run the final verifier against that downloaded ZIP.

The post-publication verifier must be given the SHA-256 persisted by phase 1, either directly or through the expected-digest file produced by the release preparation flow.

Example shape:

```bash
ZIP_PATH="/path/to/downloaded/Dxtr Cleaner.zip" \
EXPECTED_SHA256_FILE="/path/to/prepared-expected-sha256" \
make verify-macos-release
```

`verify-macos-release`:

- requires the ZIP to be a regular non-symlink file
- requires non-empty `com.apple.quarantine` evidence by default
- extracts the app from the **same ZIP whose digest is checked**
- verifies the bundled scheduled CLI exists and is executable
- verifies code signing
- validates the staple
- performs Gatekeeper assessment
- verifies quarantine on the extracted app
- calculates the downloaded ZIP SHA-256
- rejects the download when that digest differs from the prepared digest used to generate the cask
- publishes a complete evidence directory only after every check succeeds

A valid signed/notarized ZIP from an older or different build must fail this gate if its bytes do not match the prepared digest.

## 6. Quarantine / first-launch Gatekeeper evidence

Before first launch, the downloaded ZIP and extracted app must both carry a non-empty quarantine attribute:

```bash
xattr -p com.apple.quarantine "Dxtr Cleaner.zip"
xattr -p com.apple.quarantine "Dxtr Cleaner.app"
```

A local copy, USB copy, `scp`, or direct local `ditto` extraction is not sufficient by itself if it lacks quarantine metadata.

If the chosen download path does not apply quarantine, explicitly apply a test quarantine attribute and record that fact. Do not count an unquarantined launch as sufficient Gatekeeper evidence.

For manual smoke validation:

1. launch `Dxtr Cleaner.app` for the first time
2. confirm Gatekeeper does not report the app as unidentified or damaged
3. open Settings and confirm Daily Smart Scan status loads
4. confirm Smart Scan remains read-only until explicit cleanup
5. confirm cleanup/uninstall execution remains Trash-only

## 7. LaunchAgent scheduling smoke test

From a durable installed location such as `/Applications`:

- enable Daily Smart Scan
- confirm the LaunchAgent is installed
- confirm the scheduled executable is bundled `Contents/MacOS/dxtr-cleaner`
- confirm command remains exactly `scan --category user`
- disable the schedule and confirm it is removed
- move/rename the app after enabling and confirm Settings reports the schedule as stale / needing repair
- confirm a translocated launch may inspect/disable an existing schedule but cannot enable/repair one

No scheduled Trash, uninstall, or permanent-delete command is allowed.

## 8. Homebrew install/uninstall smoke test

Using the generated cask and the published release artifact:

- install the cask
- launch the app successfully
- run a Smart Scan
- enable then disable Daily Smart Scan
- uninstall the cask
- optionally test `--zap` and confirm only documented Dxtr Cleaner LaunchAgent/log locations are removed

The cask SHA must match the phase-1 prepared digest, and the downloaded asset must have passed the phase-2 digest-binding verification.

Homebrew must not bypass Rust cleanup/uninstall safety policy.

## 9. Evidence to retain

Retain a small non-secret evidence bundle containing:

- release version and Git commit SHA
- Developer ID signing verification/details
- notarization accepted result and submission ID
- staple validation output
- Gatekeeper assessment output
- quarantine evidence from downloaded ZIP and extracted app
- prepared SHA-256
- downloaded SHA-256 proving byte identity
- generated cask
- Homebrew install/uninstall output
- LaunchAgent smoke notes
- first-launch smoke notes

Do not include credentials or private keys.

## 10. M4 completion rule

M4 may be marked complete only when all of these are true:

- GPUI scheduling controls are merged
- Developer ID signed build verified
- notarization accepted and stapled
- exact final ZIP prepared and its SHA persisted
- that exact ZIP published
- published/downloaded ZIP matches the prepared digest byte-for-byte
- quarantined first-launch Gatekeeper smoke test passes
- Homebrew cask uses the same prepared digest
- Homebrew install/uninstall smoke test passes

Until then, roadmap packaging and Homebrew items remain open even though all supporting implementation and verification tooling is merged.
