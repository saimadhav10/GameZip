# GameZip

A local Windows game storage analyser, built with Tauri, React, Rust, and SQLite.

## Current build

- Discover installed Steam/Epic games or add a folder manually.
- Inspect allocated disk usage, logical size, file distribution, and the 100 largest files.
- Keep a local library and scan history; cancel scans without replacing previous results.
- Continue scanning in the system tray when the window closes.

This is the analyser milestone. Compression, video conversion/backups, and account connections are not enabled yet. Scanning never modifies game files. See [project_plan.md](project_plan.md) for the agreed scope and [implementation_status.md](implementation_status.md) for the implementation checkpoint.

## Develop on Windows

Install Node.js (22.12 or later), Rust's stable MSVC toolchain, Visual Studio C++ build tools, and WebView2. Then:

```powershell
cd app
npm ci
cd ..
powershell -NoProfile -File scripts/desktop.ps1 dev
```

The launcher adds the default Rust installation to its process PATH if needed. It does not change the system PATH.

Build an NSIS installer:

```powershell
powershell -NoProfile -File scripts/desktop.ps1 build
```

Build a debug executable for native smoke tests:

```powershell
powershell -NoProfile -File scripts/desktop.ps1 build -DebugBuild --no-bundle
powershell -NoProfile -File scripts/native-smoke.ps1
```

Native smoke tests use an isolated profile and generated fixture files under ignored `.local/`. Close any running GameZip instance first. They do not scan or alter real games. The profile override exists only in debug builds.

## Checks

```powershell
cd app
npm run build
npm run format:check
npm test
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Browser tests use installed Microsoft Edge and mock only the native command boundary. Rust tests exercise actual temporary files and SQLite. Native smoke tests exercise the compiled app through WebView2.

## Storage and scan limits

SQLite lives in the app's local application-data directory, visible under Settings. No cloud service is used. Read failures and skipped reparse points are reported. LZX-compressed files are measured through Windows' normal file APIs and covered by a round-trip fixture test. Hard links are counted once within a game; links shared between separate game roots can contribute to both games' totals. Numbers describe the last scan, not continuous filesystem monitoring. Archive internals are not inspected.

UI references remain untouched under `uiassts/`. Game artwork is not downloaded; library covers use locally generated lettermarks. Fonts and icons are bundled locally.
