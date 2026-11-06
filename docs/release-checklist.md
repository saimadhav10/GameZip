# Windows release preparation

The `Build review installer` GitHub Actions workflow is manual. It builds a per-user Windows NSIS installer and a SHA-256 checksum as review artifacts. It does not publish a GitHub Release or create a WinGet listing. The installer includes the project's MIT text. It remains unsigned until a Windows signing identity is configured.

A local development installer was built successfully on 2026-11-06 at `app/src-tauri/target/debug/bundle/nsis/GameZip_0.1.0_x64-setup.exe`. This verifies bundling only; clean-machine installation and the release-mode workflow have not been run.

Before a public beta:

1. Finish dependency/font/artwork notices and review any redistributed media tools. The local FFmpeg test download is not part of the installer.
2. Finish and validate the updater using a securely stored signing key, the matching embedded public key and real release metadata. Never publish placeholder signatures or install URLs. Keep paused/waiting jobs protected from updater restart.
3. Test fresh install, upgrade, uninstall and tray shutdown on clean Windows 10/11 x64 machines. Validate job recovery separately from installer tests.
4. Update the consistent application version, build/sign the release installer and publish it with checksums and release notes on GitHub Releases.
5. Generate and validate WinGet manifests using that immutable release URL and actual checksum; submit them for acceptance. Only then advertise a working `winget install` command.

Existing development builds do not automatically update. No public beta, code-signing certificate, updater key or WinGet package has been provisioned by this checkpoint.
