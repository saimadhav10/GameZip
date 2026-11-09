# Windows release preparation

2026-11-09: native updater, startup/six-hour checks, explicit install confirmation, job/backup-I/O installation guards and release-metadata generator are implemented. The generated updater key lives only in the restricted, ignored `.local/signing/` directory; `.env.local` contains its path. Back up that key securely before release and configure the matching `TAURI_SIGNING_PRIVATE_KEY` GitHub Actions secret. Do not regenerate it after distributing builds. The GitHub connector in this session cannot configure repository secrets.

Only the public key is committed. `tauri.updater.conf.json` enables signing artifacts when explicitly selected; ordinary developer builds do not require the private key. The manual installer workflow accepts `sign_updates` to produce installer signatures and `latest.json` review artifacts. It still does not publish automatically. `scripts/update-manifest.mjs` creates metadata/checksums from a real matching installer and signature. Upload the installer, `.sig`, checksum and `latest.json` together under the matching `v<version>` release tag after validation.

The current debug review installer (5.73 MiB) and updater signature built successfully. **Updater signing is separate from Windows Authenticode publisher signing; the latter remains unconfigured.** Signature/upgrade execution and clean Windows 10/11 installation need separate validation. Dependency/font notices for the locked Windows graph are collected in `THIRD_PARTY_NOTICES.md` and bundled, with supplemental source provenance in `docs/licenses/README.md`. Regenerate with `node scripts/third-party-notices.mjs` after dependency changes.

`scripts/install.ps1` resolves a real published GitHub release, restricts download URLs to this repository, verifies the exact installer's SHA-256, and launches the interactive per-user installer. It supports download-only review and explicit version selection. No public feed/package exists yet, so no successful public-install test or WinGet command is claimed.

The `Build review installer` GitHub Actions workflow is manual. It builds a per-user Windows NSIS installer and a SHA-256 checksum as review artifacts. It does not publish a GitHub Release or create a WinGet listing. The installer includes the project's MIT text. It remains unsigned until a Windows signing identity is configured.

A local development installer was built successfully on 2026-11-06 at `app/src-tauri/target/debug/bundle/nsis/GameZip_0.1.0_x64-setup.exe`. This verifies bundling only; clean-machine installation and the release-mode workflow have not been run.

Before a public beta:

1. Finish dependency/font/artwork notices and review any redistributed media tools. The local FFmpeg test download is not part of the installer.
2. Finish and validate the updater using a securely stored signing key, the matching embedded public key and real release metadata. Never publish placeholder signatures or install URLs. Keep paused/waiting jobs protected from updater restart.
3. Test fresh install, upgrade, uninstall and tray shutdown on clean Windows 10/11 x64 machines. Validate job recovery separately from installer tests.
4. Update the consistent application version, build/sign the release installer and publish it with checksums and release notes on GitHub Releases.
5. Generate and validate WinGet manifests using that immutable release URL and actual checksum; submit them for acceptance. Only then advertise a working `winget install` command.

The original development builds have no updater. The 2026-11-09 build has updater checks and a real embedded public key, but no public feed, beta, Windows code-signing certificate or WinGet package is provisioned. A local signature verification test accepted the review installer and rejected a modified copy in memory. A full installed-version upgrade/restart remains untested.
