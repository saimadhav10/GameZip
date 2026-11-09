# Implementation checkpoint

2026-11-09: video previews and local backup controls are now connected to the app. Game details → Videos discovers standalone files, rejects unsupported media profiles, and creates a separate 1080p MP4. Jobs opens completed previews. Collapsed Settings controls choose tools, backup location and retention (minimum seven days), list retained backups and export originals without overwriting. Cleanup runs at startup/hourly and retries unavailable drives; incomplete copies remain. No game replacement/restore is enabled. The owner confirmed no other game is available for playback validation.

Validation: 31 standard Rust tests passed plus the opt-in FFmpeg test; 11 Playwright UI flows, Clippy, frontend/desktop builds passed. Screenshots reviewed at 720px. Native WebView2 smoke produced 1920×1080 from a generated 4K fixture through the real UI, kept the original bytes, and checked retention/location rejection. Native evidence: `.local/smoke-1794225487284`. No real game files were modified. Updater/release work continues below the video milestone.

2026-11-06 third increment: `backups.rs` and `media.rs` implement the tested video foundations described in [docs/video-pipeline.md](docs/video-pipeline.md). Standard suite: 29 passed, one media integration test opt-in; the opt-in test also passed locally with checksum-verified FFmpeg 9.0.2, confirming 1080p output, full decode, unchanged original and identical audio packets. Clippy passed. No game replacement or video UI is enabled. Installer review workflow/configuration added; local debug NSIS installer built successfully at `app/src-tauri/target/debug/bundle/nsis/GameZip_0.1.0_x64-setup.exe` (4.17 MiB). This is unsigned and has not received clean-machine install/upgrade/uninstall validation; it is not a public release. Remaining work is in [docs/next-update.md](docs/next-update.md).

2026-11-06 second increment: persistent Quiet and Idle-only settings implemented. Quiet rests between files; Idle-only waits for two minutes without session input. No strict per-file disk bandwidth cap is claimed. Validation: 24 Rust tests, eight UI flows, Clippy and desktop build passed. Isolated native WebView2 smoke verified the idle gate (debug test input), Pause/Resume, compression/decompression, and unchanged generated files. Native test output: `.local/smoke-1793958755175`. Next: local video-backup foundation and supported conversion profiles.

2026-11-06 first update increment: Pause/Resume, Fast/Balanced/Maximum compression, background worker scheduling, richer interactive storage categories and MIT licensing. Validation: 21 Rust tests, seven Playwright flows, Clippy with warnings denied, formatting and frontend build passed. Native round-trip tests cover all three algorithms and unchanged fixture contents. Small-window chart screenshot inspected. Debug desktop build refreshed; no real games modified during tests. This increment has not received a new native UI smoke run or performance benchmark. See [docs/next-update.md](docs/next-update.md) for the entire authorised backlog. Game-video replacement and public release remain unavailable.

Updated: 2026-11-05. Analyser and compression milestones validated locally; next work: Cyberpunk video compatibility and account feasibility.

## Implemented and validated

- Tauri/React desktop shell using the supplied visual direction with reduced information density.
- Local SQLite library and job history; manual folders and Steam/Epic installed-library discovery.
- Read-only scanning with file counts, logical/allocated sizes, distribution donut, and largest files.
- Hard-link deduplication within roots, overlap rejection, skipped junctions/locked files, cancellation, interrupted-scan history.
- Window-close-to-tray, reopen, explicit Quit, and single-instance handling.
- LZX/decompression preparation and minimal confirmation UI, per-file Windows operations, durable recovery records, cancellation boundaries, and measured results.
- Running-game checks before preparation/start/each file; no game process is terminated. Compact runs at below-normal CPU priority.

## Validation checkpoint

- Frontend production build passed.
- Six Playwright UI flows passed, including compression, running-game errors, low-space gating, and 720px layout checks; screenshots reviewed.
- Eighteen Rust tests passed, including actual LZX/decompression, unchanged contents, allocation measurements, running-process detection, changed/shared/locked files, cancellation, and failure to write the file journal before mutation.
- Clippy passed with warnings denied; Rust and frontend formatting passed.
- Latest debug desktop executable built and native WebView2 smoke passed: real folder registration, scan, compression/decompression via the UI, measured savings, non-destructive removal, and unchanged fixture bytes. All test files and the profile were isolated under `.local/`.
- Release executable is from the earlier analyser build; rebuild before distributing it. Installer, physical tray interactions, forced shutdown during compact, real low-space volumes, and clean Windows 10/11 machines remain unvalidated. GitHub CI status could not be retrieved through the available connector; gh is not installed.

## Resume here

1. Cyberpunk's main video archive was inspected read-only: 552 Bink/Bink 2 headers, zero 4K candidates. See `docs/cyberpunk-video-compatibility.md` and `scripts/inspect-cyberpunk.mjs`. The installed game was found through its uninstall registry entry. Other DLC/mod archives and actual playback are not yet validated. Never modify real game files during automated tests.
2. LZX fixture verification confirmed that Windows exposes WOF files through ordinary file APIs with the correct allocated size. The scanner includes them without following junctions. Do not assume WOF files are always visible as reparse points; validate through WofIsExternalFile.
3. Failed final SQLite writes now retain an in-memory failed result, preventing a phantom running job. A restart reconciles the durable record as interrupted.
4. Video backup lifecycle approved on 2026-11-05: retention starts after successful conversion; incomplete recovery copies remain; deletion is postponed during a restore. Minimum seven days, user-selected location/duration. Restore conflict behavior and tested codecs/profiles remain unresolved.
5. Investigate provider sign-in without a GameZip backend. Epic requires product/client setup and brand review for public users; no credentials or product configuration have been supplied. Steam's documented OpenID flow targets websites; a local-only desktop callback still needs validation.
6. Compression limitations are in README.md: local NTFS, classic Windows path lengths/names, no shared links or unsupported compression. If extending them, validate on disposable fixtures first.
7. Continue the local video-backup foundation using generated samples. Do not enable game-video replacement until a supported 4K profile and restore-conflict behavior are validated. The hourly continuation remains active because videos, accounts, and release validation are incomplete.

Use `scripts/desktop.ps1` for development/builds; Rust was installed under the user's default `.cargo/bin` directory without changing system PATH. The heartbeat `continue-gamezip-implementation` is active hourly for requested continuation after usage-limit interruptions.
