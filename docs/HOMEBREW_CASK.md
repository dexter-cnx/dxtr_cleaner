# Homebrew Cask Release Flow

The Homebrew cask must point only at a notarized release ZIP produced by the macOS release pipeline. Do not publish a cask that references an ad-hoc-signed or pre-notarization artifact.

## Generate the cask

After `Dxtr Cleaner.zip` has passed Developer ID signing, notarization, stapling, and Gatekeeper verification, compute the final archive digest and generate the cask:

```bash
shasum -a 256 "dist/Dxtr Cleaner.zip"
VERSION=0.1.0 \
SHA256=<64-hex-digest> \
URL=https://github.com/dexter-cnx/dxtr_cleaner/releases/download/v0.1.0/Dxtr.Cleaner.zip \
make generate-cask
```

The generator writes `Casks/dxtr-cleaner.rb` and validates Ruby syntax before returning success.

## Release invariants

- `URL` must use HTTPS.
- `SHA256` must be the digest of the exact final notarized ZIP uploaded to the GitHub release.
- The cask installs `Dxtr Cleaner.app` only; the bundled `dxtr-cleaner` CLI remains inside the app bundle for scheduled scan integration.
- `zap` may remove the Dxtr Cleaner LaunchAgent plist and logs, but Homebrew install/uninstall must not bypass cleanup or uninstall safety policy.
- A generated cask is not considered release-ready until `brew install --cask` and `brew uninstall --cask` are smoke-tested against the actual published artifact.

## Publication

The repository keeps the generator and may keep a generated cask for a tagged release. A future dedicated tap can consume the same generated cask without moving cleanup logic outside the Rust application.
