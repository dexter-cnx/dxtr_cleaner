# M4 Release Verification

M4 implementation is functionally complete in code, but the milestone is **not release-complete** until a real macOS release artifact passes Developer ID signing, notarization, Gatekeeper, and Homebrew smoke testing.

This document is the authoritative evidence checklist for that final gate.

## 1. Preconditions

Required external credentials and tooling:

- Apple Developer ID Application certificate available in the signing keychain
- `SIGNING_IDENTITY` set to the exact Developer ID Application identity
- notarization credentials stored in a `notarytool` keychain profile
- `NOTARY_PROFILE` set to that profile name
- Homebrew installed for the final cask smoke test
- release version selected and consistent with the GitHub release tag

Do not commit certificates, passwords, App Store Connect keys, notary credentials, or keychain exports to the repository.

## 2. Build and sign the release bundle

Run:

```bash
SIGNING_IDENTITY="Developer ID Application: ..." \
VERSION=<release-version> \
make package-macos
```

Required evidence:

- `dist/Dxtr Cleaner.app` exists
- `dist/Dxtr Cleaner.zip` exists
- GPUI executable is present in `Contents/MacOS`
- bundled `dxtr-cleaner` CLI is present in `Contents/MacOS`
- `codesign --verify --deep --strict --verbose=2 "dist/Dxtr Cleaner.app"` succeeds
- `codesign -dv --verbose=4 "dist/Dxtr Cleaner.app"` shows the intended Developer ID identity and hardened runtime

Ad-hoc signing is only a local packaging smoke test and does **not** satisfy this gate.

## 3. Notarize and staple

Run:

```bash
NOTARY_PROFILE=<profile> make notarize-macos
```

Required evidence:

- `notarytool` submission returns an accepted result
- `stapler staple` succeeds
- `stapler validate "dist/Dxtr Cleaner.app"` succeeds
- `spctl --assess --type execute --verbose=4 "dist/Dxtr Cleaner.app"` succeeds
- the final ZIP is rebuilt after stapling so the published archive contains the stapled app

Record the notarization submission ID in the release notes or attached verification log.

## 4. Fresh-machine Gatekeeper smoke test

Use the exact final ZIP intended for publication, preferably on a clean macOS user account or another Mac.

Required evidence:

1. download/copy the final ZIP
2. extract it normally through Finder or `ditto`
3. launch `Dxtr Cleaner.app`
4. confirm Gatekeeper does not block the app as unidentified or damaged
5. open Settings and confirm Daily Smart Scan status loads
6. confirm Smart Scan itself is read-only until the user explicitly starts cleanup
7. confirm cleanup remains Trash-only

## 5. LaunchAgent scheduling smoke test

From an installed durable app location such as `/Applications`:

- enable Daily Smart Scan in GPUI Settings
- confirm the LaunchAgent is installed
- confirm the scheduled executable is the bundled `Contents/MacOS/dxtr-cleaner`
- confirm the plist command remains exactly the read-only `scan --category user` flow
- disable the schedule and confirm the LaunchAgent is removed
- move or rename the app after enabling, reopen Settings, and confirm the UI reports the schedule as stale / needing repair
- confirm a translocated launch may inspect/disable an existing schedule but cannot enable/repair one

No scheduled Trash, uninstall, or permanent-delete command is allowed.

## 6. Publish release artifact

Upload the exact notarized ZIP that passed the checks above to the GitHub release.

Record its SHA-256:

```bash
shasum -a 256 "dist/Dxtr Cleaner.zip"
```

The digest must be calculated **after** notarization/stapling and from the exact uploaded ZIP.

## 7. Generate and validate the Homebrew cask

Run with the real release URL and final digest:

```bash
VERSION=<release-version> \
SHA256=<64-hex-final-digest> \
URL=https://github.com/dexter-cnx/dxtr_cleaner/releases/download/v<release-version>/<release-asset> \
make generate-cask
```

Required evidence:

- generated `Casks/dxtr-cleaner.rb` passes Ruby syntax validation
- URL points to the real published notarized ZIP
- SHA-256 matches the downloaded release asset byte-for-byte
- no placeholder digest remains

## 8. Homebrew install/uninstall smoke test

Using the generated cask and published release artifact:

- install the cask
- launch the app successfully
- run a Smart Scan
- enable then disable Daily Smart Scan
- uninstall the cask
- optionally run uninstall with `--zap` and confirm only documented Dxtr Cleaner LaunchAgent/log locations are removed

Homebrew must not bypass Rust cleanup/uninstall safety policy.

## 9. Evidence to retain

Retain a small release evidence bundle containing:

- release version and Git commit SHA
- `codesign --verify` output
- `codesign -dv` identity/runtime output
- notarization accepted result and submission ID
- `stapler validate` output
- `spctl --assess` output
- SHA-256 of the final ZIP
- generated cask
- Homebrew install/uninstall command output
- short manual smoke-test notes

Do not include credentials or private keys.

## 10. M4 completion rule

M4 may be marked complete only when all of these are true:

- GPUI scheduling controls are merged
- Developer ID signed build verified
- notarization accepted and stapled
- Gatekeeper assessment passes
- exact final ZIP published
- Homebrew cask generated from that ZIP's real SHA-256
- Homebrew install/uninstall smoke test passes

Until then, roadmap packaging and Homebrew items remain open even though their implementation scripts are already merged.
