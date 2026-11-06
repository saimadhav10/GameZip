# Implementation checkpoint

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
