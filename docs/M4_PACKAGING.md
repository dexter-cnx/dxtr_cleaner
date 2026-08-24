# M4 macOS packaging

The direct-distribution path is a signed and notarized `.app` bundle. Packaging keeps GPUI in the frontend while shipping the existing Rust CLI inside the same bundle so the LaunchAgent scheduler can use an absolute in-bundle executable path.

## Bundle layout

```text
Dxtr Cleaner.app/
└── Contents/
    ├── Info.plist
    └── MacOS/
        ├── Dxtr Cleaner      # GPUI executable
        └── dxtr-cleaner      # shared Rust CLI used by scheduled read-only scan
```

The default bundle identifier is `com.cnxdev.dxtr-cleaner`.

## Local package smoke test

```bash
make package-macos
```

Without `SIGNING_IDENTITY`, the script applies ad-hoc signatures so the bundle structure and launch behavior can be tested locally. Ad-hoc output is not a distributable notarized build.

## Developer ID signing

Set `SIGNING_IDENTITY` to the exact Developer ID Application identity available in the login keychain:

```bash
SIGNING_IDENTITY='Developer ID Application: Example (TEAMID)' make package-macos
```

The packaging script signs both executables first, then the app bundle, using Hardened Runtime and a secure timestamp. It verifies the resulting code signature and `Info.plist`, then creates `dist/Dxtr Cleaner.zip` for notarization.

Do not store Developer ID certificates, private keys, passwords, App Store Connect keys, or notarization credentials in the repository.

## Notarization

Store notarization credentials in a local keychain profile using `xcrun notarytool store-credentials`, then provide only the profile name to the script:

```bash
NOTARY_PROFILE='dxtr-cleaner-notary' make notarize-macos
```

The notarization script:

1. verifies the Developer ID signature,
2. submits the ZIP with `xcrun notarytool submit --wait`,
3. staples the accepted ticket to the `.app`,
4. validates the staple,
5. runs Gatekeeper assessment with `spctl`,
6. recreates the distribution ZIP so it contains the stapled app.

## Release gate

A release is not considered signed/notarized complete until a real Developer ID build passes all of the following on macOS:

```bash
codesign --verify --deep --strict --verbose=2 'dist/Dxtr Cleaner.app'
xcrun stapler validate 'dist/Dxtr Cleaner.app'
spctl --assess --type execute --verbose=4 'dist/Dxtr Cleaner.app'
```

CI can validate Rust code and packaging source, but notarization itself requires Apple credentials and therefore remains an explicit release job/local release operation until repository secrets and release automation are configured.
