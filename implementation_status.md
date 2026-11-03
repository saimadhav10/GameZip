# Implementation checkpoint

Updated: 2026-11-03. Analyser milestone validated; next work: compression groundwork and account feasibility.

## Implemented and validated

- Tauri/React desktop shell using the supplied visual direction with reduced information density.
- Local SQLite library and job history; manual folders and Steam/Epic installed-library discovery.
- Read-only scanning with file counts, logical/allocated sizes, distribution donut, and largest files.
- Hard-link deduplication within roots, overlap rejection, skipped junctions/locked files, cancellation, interrupted-scan history.
- Window-close-to-tray, reopen, explicit Quit, and single-instance handling.

## Validation checkpoint

- Frontend production build passed.
- Three Playwright UI flows passed at desktop and 720px widths after fixing collapsed navigation labels.
- Thirteen Rust tests passed, including Windows locked files, junctions, hard links, cluster allocation, discovery, persistence, interruption recovery, failed final writes, and an actual LZX compression/decompression round trip on generated files.
- Clippy passed with warnings denied; Rust and frontend formatting passed.
- Debug desktop executable built and native WebView2 smoke passed: real folder registration, scan, saved results, rendered details, non-destructive removal, and unchanged fixture bytes. All test files and the profile were isolated under `.local/`.
- Initial release compilation passed. The debug build includes the latest allocation fix; rebuild release before distributing it. Installer, physical tray interactions, and clean Windows 10/11 machines are not yet validated.

## Resume here

1. Continue compression planning/implementation using disposable fixtures only. Validate LZX/decompression semantics, running-game checks, cancellation boundaries, persistence, and actual disk accounting before enabling it.
2. LZX fixture verification confirmed that Windows exposes WOF files through ordinary file APIs with the correct allocated size. The scanner includes them without following junctions. Do not assume WOF files are always visible as reparse points; validate through WofIsExternalFile.
3. Failed final SQLite writes now retain an in-memory failed result, preventing a phantom running job. A restart reconciles the durable record as interrupted.
4. Investigate provider sign-in without a GameZip backend. Video game profiles and restoration conflict policies remain unresolved; do not guess.

Use `scripts/desktop.ps1` for development/builds; Rust was installed under the user's default `.cargo/bin` directory without changing system PATH. The heartbeat `continue-gamezip-implementation` is active hourly for requested continuation after usage-limit interruptions.
