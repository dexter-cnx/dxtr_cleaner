# Homebrew Cask Release Flow

The Homebrew cask must point only at a notarized release ZIP produced by the macOS release pipeline. Do not publish a cask that references an ad-hoc-signed or pre-notarization artifact.

## Generate the cask

After `Dxtr Cleaner.zip` has passed Developer ID signing, notarization, stapling, and Gatekeeper verification, compute the final archive digest and generate the cask:

```bash
shasum -a 256 "dist/Dxtr Cleaner.zip"
VERSION=0.1.0 \
SHA256=<64-hex-digest> \
URL=https://github.com/dexter-cnx/dxtr_cleaner/releases/download/v0.1.0/Dxtr%20Cleaner.zip \
make generate-cask
```

The generator writes `Casks/dxtr-cleaner.rb` and validates Ruby syntax before returning success.

## Release invariants

- `URL` must point to an asset under `https://github.com/dexter-cnx/dxtr_cleaner/releases/download/`; arbitrary HTTPS hosts are rejected because the generated cask declares this repository as the verified source.
- `SHA256` must be the digest of the exact final notarized ZIP uploaded to the GitHub release. Uppercase hexadecimal input is accepted and normalized without relying on Bash 4-only syntax, so generation remains compatible with the default Bash 3.2 shipped by macOS.
- The canonical archive name produced by packaging is `Dxtr Cleaner.zip`; encode the space as `%20` in release URLs.
- The cask installs `Dxtr Cleaner.app` only; the bundled `dxtr-cleaner` CLI remains inside the app bundle for scheduled scan integration.
- `zap` may remove the Dxtr Cleaner LaunchAgent plist and logs, but Homebrew install/uninstall must not bypass cleanup or uninstall safety policy.
- A generated cask is not considered release-ready until `brew install --cask` and `brew uninstall --cask` are smoke-tested against the actual published artifact.

## Regression coverage

`make script-check` executes `scripts/macos/test_generate_cask.sh`. The regression test verifies SHA normalization, non-interpolating Ruby literals, Ruby syntax, and rejection of release URLs outside this repository.

## Publication

The repository keeps the generator and may keep a generated cask for a tagged release. A future dedicated tap can consume the same generated cask without moving cleanup logic outside the Rust application.
