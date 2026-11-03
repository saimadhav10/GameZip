# GameZip — Project Plan

Updated: 2026-11-03. Planning only; implementation has not started.

## Confirmed decisions

- **Product:** Analyse game storage, compress installations with LZX, and selectively convert supported prerecorded 4K videos to 1080p. Gameplay rendering resolution is unaffected.
- **Name:** GameZip, replacing the mockups' ApexDrive branding.
- **Platform:** Windows 10 and 11, x64; Linux later.
- **Stack:** Tauri + React/TypeScript + Rust + SQLite.
- **Library:** Installed games only, through Steam/Epic local discovery and manual folders. Steam/Epic account login remains in scope, but must not add owned games that are not installed. Exact account features remain to be defined.
- **Analysis:** Games sorted by installation size, file distribution with size/count views, and largest files.
- **Compression:** LZX, decompression, progress, cancellation, interrupted-job recovery, and measured before/after disk usage.
- **Videos:** A small tested set of games/formats, previews, backups, and restoration.
- **Backups:** Temporary retention selected; user requested a suggested period. Proposed: seven days, then offer deletion after playback confirmation. Automatic deletion and no-backup mode are not approved.
- **UI:** Keep the supplied style; minimise visible information and show only what is needed.

## Workspace and references

Work in S:\GameZip. Repository: [saimadhav10/GameZip](https://github.com/saimadhav10/GameZip), branch main. The old S:\kochi_traffic\GameZip folder and Kochi Traffic repository are unrelated.

The seven numbered folders in uiassts contain screen HTML and screenshots. The apex_precision_engine/DESIGN.md file supplies design tokens; image.png/screen.png is an additional dashboard reference. Preserve these originals. Earlier planning is retained in Git history; the simplified direction below supersedes its dense screen outline.

## Simplified interface — proposal

Keep charcoal surfaces, orange actions, green savings, cyan progress, and the supplied typography. Use short labels, one main action per view, and expandable details.

| View | Show by default | Show on demand |
| --- | --- | --- |
| Library | Add games, search, total storage/savings, game name, size, status, Analyse | Paths, filters, last scan, removal |
| Game details | Disk usage, savings, one distribution donut, Compress, Optimise videos | File-count toggle, logical size, largest files |
| Compression | Selected game/size, LZX, Start, actionable issues | File selection, exclusions, Decompress |
| Videos | Supported videos, 1080p target, preview, backup location, net savings, Start | Media properties, unsupported reasons |
| Jobs | Game, operation, progress, Cancel, result | Errors, retry, recovery, history |
| Settings | Account connections, library paths, backup location/retention, Restore | Backup inventory/deletion, temporary storage, diagnostics |

Proposed navigation: Library, Jobs, Settings. Game details and operation setup open within the selected game's flow. Retain the original donut requirement rather than displaying both a donut and the mockup's stacked bar.

Use one active-job indicator. Empty, loading, missing-folder, and error states appear only when relevant. Removing a library entry never deletes game files.

Omit drive-health panels, kernel/cluster jargon, throughput counters, theme editors, fake quality scores, and diagnostic showcases from the default UI. Fix clipped headings, wrapped buttons, and footer overlap in the references; verify resizing, keyboard access, and display scaling. Package required fonts/icons/styles locally and review artwork provenance before distribution.

## Scope boundaries

- LZX is the first-release algorithm. Extra algorithms, DirectStorage hooks, TRIM/cleanup, shader-cache clearing, and GOG account integration are not agreed features.
- Boosters, Linux, and AnyPS5 compatibility remain future work. AnyPS5 requires separate per-game/tool validation.
- Mockup games, sizes, savings, formats, and performance claims are examples, not results.
- No fixed savings, zero-latency, zero-corruption, or universal anti-cheat guarantees. Do not infer archive contents from filenames.
- Video restoration needs originals; LZX decompression is a separate operation. Backups on the same drive can eliminate net savings.
- Do not enable arbitrary codec replacement or assume Bink/Bink 2 conversion works. Preserve the game's required container, codec, tracks, timing, aspect ratio, and filenames.

## Proposed implementation behaviour

React presents the UI; Rust owns local operations; SQLite stores scans, jobs, settings, and backup manifests. Separate discovery, analysis, platform compression, media conversion, and recovery.

1. **Analysis:** Separate logical and allocated bytes. Handle overlapping roots, hard links, junctions, and denied paths without misleading totals. Measure savings from comparable snapshots; avoid double-counting repeated jobs.
2. **Compression:** Check NTFS support, permissions, space, and running-game conflicts. Validate per-file compact.exe LZX compression/decompression before selecting a native API alternative. Never change OS-wide CompactOS settings.
3. **Jobs:** Begin with one modifying job at a time. Persist per-file state. Cancel stops new work and shows “Stopping” while current work reaches a safe boundary. Completed files remain compressed. After restart, inspect real file state before retrying; do not promise chunk-level resume. Queue hold is distinct from active-job pause.
4. **Videos:** Probe, preview, create a separate output, validate it, preserve the original, then replace. Journal stages and hashes for recovery. Recheck files before replacement/restoration; updates or other changes become conflicts requiring review.
5. **Storage:** Show temporary space, backup space, and net savings separately. Restore from verified backups. Do not delete retained originals merely because a job failed or time elapsed.
6. **Accounts:** Keep local operation usable without login. Account connections must not populate the library with uninstalled titles; local installation discovery determines library eligibility. Use provider-hosted sign-in and disconnect support. Verify registration, permissions, callbacks, and any hosted service before coding. Keep application secrets out of desktop bundles/logs; protect runtime tokens with appropriate OS storage.

These are proposals, not additional approved requirements. Exact Windows builds, app-close behaviour during jobs, update/restore conflict choices, and installer/update policy remain to be settled.

## Milestones

| Milestone | Deliverable and acceptance |
| --- | --- |
| 1. Analyser | Simple UI and manual/local discovery; verify totals, missing/denied/duplicate paths, resizing, and scaling |
| 2. Accounts | Agreed Steam/Epic account features; verify installed-only library behaviour, provider support, cancelled/expired/private-account cases, and offline local use |
| 3. Compression | LZX/decompression and persistent jobs; verify unchanged file contents, disk savings, cancellation, restart, locks, and low space |
| 4. Videos | Tested profiles, preview, temporary backups, restore; verify in-game playback/audio, restored hashes, interrupted replacement, and update conflicts |
| 5. Release | Installer and dependency/licence checks; test fresh installs on Windows 10/11 x64; remove placeholders and unsupported claims |

Do not start coding while material questions for the affected milestone remain unresolved.

## Suggested initial test games

Use games already available to the user; no purchase is needed for planning.

- **Baldur's Gate 3:** candidate for analysis, compression, decompression, and load testing; Larian lists 150 GB available SSD space.
- **Cyberpunk 2077:** a second large-installation candidate for comparison; CD Projekt's published PC requirements list 70 GB SSD storage.

These are proposed test candidates, not certified GameZip support or predicted savings. Installed sizes vary. Video support must be decided after inspecting actual game files; neither game is currently approved for conversion. Test the pipeline with controlled media samples, then validate each selected game/build in-game.

## Remaining decisions

1. Define the useful account-login features for the installed-only library and verify Steam/Epic capabilities separately. Listing owned but uninstalled games is explicitly excluded.
2. Accept or adjust seven-day backups with a deletion prompt; decide whether no-backup mode is needed.
3. Choose available test games and validated video formats/builds.
4. Settle the proposed layout and job/restore behaviour before implementation.

## Evidence checked

- [Microsoft compact](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/compact): NTFS, LZX, decompression, interrupted-file handling. The mockup's ReFS and hardware-decompression claims are not established.
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/): Rust, Windows build tools, WebView2.
- [FFmpeg formats](https://www.ffmpeg.org/general.html): Bink is listed for input, not output; this does not establish a Bink 2 workflow. [FFmpeg usage](https://ffmpeg.org/ffmpeg.html).
- [Steam sign-in](https://partner.steamgames.com/doc/features/auth?l=english) and [owned-game API](https://partner.steamgames.com/doc/webapi/IPlayerService): identity and library access are separate; library access requires a key and depends on visibility.
- [Epic Account Services](https://dev.epicgames.com/docs/epic-online-services/accounts-and-social/eos-epic-account-services/epic-account-services-intro): account-service starting point; supported account features for this utility remain to be verified. Full owned-library listing is not required.
- [Baldur's Gate 3 requirements](https://larian.com/support/faqs/system-requirements_47) and [Cyberpunk requirements](https://www.cyberpunk.net/en/news/48271/update-to-pc-system-requirements): installation-scale references, not compression evidence.

## Working instructions

Follow C:\Users\Sai\.codex\skills\project-workflow\SKILL.md and C:\Users\Sai\.codex\RTK.md. Prefix shell commands with rtk. Preserve supplied assets, commit focused changes to the GameZip repository, ignore secret-bearing environment files, and never add Codex as an author or contributor.
