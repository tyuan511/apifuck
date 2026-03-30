# Auto Update

This project now uses Tauri v2 updater with GitHub Releases as the update source.

macOS is configured to use ad-hoc signing via `bundle.macOS.signingIdentity = "-"`. This works without an Apple Developer certificate, but users may still need to allow the app manually in Privacy & Security on first launch.

## 1. Generate signing keys

Run this once on a trusted machine:

```bash
bunx tauri signer generate --password
```

Save the generated private key in the GitHub Actions secret `TAURI_SIGNING_PRIVATE_KEY`, save its password in `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, and replace the `pubkey` placeholder in `src-tauri/tauri.conf.json` with the generated public key.

For this repository, keep the local key files under `.secrets/` so they stay inside the project folder but remain ignored by git:

```bash
.secrets/updater.key
.secrets/updater.key.pub
```

## 2. Publish a release

1. Commit the code you want to release.
2. Create and push a Git tag like `v0.1.1`.

GitHub Actions will read the version from the pushed tag, sync it into `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` inside the CI workspace, then build platform installers, sign updater artifacts, and upload `latest.json` plus installers to the GitHub Release page.

After the release job succeeds, CI also checks out `main`, updates `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` to the tagged version, and pushes a commit like `chore: bump version to 0.1.1` back to the main branch.

GitHub Release notes come from `docs/release/<version>.md`, for example `docs/release/0.1.1.md`. If that file does not exist when the tag is pushed, the release workflow fails on purpose.

The version sync logic is centralized in `scripts/release/sync-version.mjs`, so both CI jobs use the same implementation.

## 3. Update behavior in app

The app checks once after startup. When a newer version is available, it shows a toast with an install action. After download and install complete, the app relaunches automatically.

## 4. Notes

- The updater endpoint is `https://github.com/tyuan511/apifuck/releases/latest/download/latest.json`.
- `bundle.createUpdaterArtifacts` must stay enabled.
- macOS uses ad-hoc signing, not Developer ID signing or notarization.
- Auto update works for desktop targets only.
