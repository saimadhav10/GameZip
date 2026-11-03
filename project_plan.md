# GameZip — Project Plan

Last updated: 2026-11-03
Stage: Planning; implementation has not started.

## Project location and handoff

- Intended project folder: `S:\GameZip`.
- The project was accidentally created inside `S:\kochi_traffic\GameZip`. That folder was empty. Windows blocked moving it because another process held it open, so the new empty folder was created at `S:\GameZip`. The old empty folder may still exist.
- The original chat remained attached to the old working directory. Use the new folder explicitly for further work.
- The parent Kochi Traffic repository is unrelated. Do not commit GameZip there.
- The user will create a separate repository; its URL and Git setup are pending.
- This document preserves the planning conversation. Distinguish confirmed requirements from proposals below; do not treat proposals as user approval.

## Confirmed product direction

GameZip is a game storage analyser and optimiser focused on games with large installations. It should reduce disk usage through LZX compression and selective conversion of prerecorded 4K videos to 1080p.

- Start with Windows, with future Linux expansion.
- Users can add games through Steam/Epic integration or manually select folders.
- Display file distribution using a pie/donut chart, including file counts and storage usage.
- Let users choose how video originals are backed up or retained.
- The user will provide the UI design after receiving screen/page requirements. No colour scheme has been selected.

## Confirmed first-release scope

### Analyser

- Folder selection and a game library sorted by installation size.
- Charts for game-file distribution.
- Largest-file inspection.

### Compression

- LZX compression jobs.
- Progress, cancellation, and interrupted-job recovery.
- Decompression.
- Before/after disk usage reporting.

### Video optimisation

- A small set of tested games and supported media formats.
- Preview of conversion results.
- Backup and restore functionality, with user choice over retention.
- Convert supported prerecorded 4K videos to 1080p; this is separate from gameplay rendering resolution.

## Longer-term direction, outside the selected first-release scope

- Launch optimiser/booster: the original request included clearing caches and adjusting settings before starting games.
- Compatibility with AnyPS5, which ports PS5 executables for Windows and Linux.
- Linux support. Windows LZX integration must not be assumed to work on Linux; a future backend needs separate evaluation.

Booster behaviour and AnyPS5 integration depth are not yet agreed. Proposed future behaviour includes reversible launch profiles, measured performance comparisons, and support for already-converted AnyPS5 installations before integrating conversion itself.

## Proposed screen outline supplied to the user

The following is a design brief, awaiting the user's UI. Extra controls and behaviours are proposals, not additional confirmed release requirements.

### 1. Game library

- Summary: game count, current disk usage, space recovered.
- Add games through Steam, Epic, or a manual folder picker.
- Game entries: name, location, launcher, disk usage, optimisation status, last scan.
- Default sort by size, largest first; search and filters for drive, launcher, and status.
- Actions: Analyse, Open details, Open folder, Remove from library.
- Active-job indicator linking to Jobs.
- States: empty, scanning, populated, missing folder, scan failure.

### 2. Game details / analyser

- Game name, path, launcher, drive; proposed Launch game action.
- Logical file size, actual size on disk, file count, and space recovered.
- Distribution chart with Disk usage / File count toggle and clickable legend.
- Largest-files table: filename, type, logical size, disk usage, compression status.
- File search and folder navigation.
- Compression candidates, detected high-resolution videos, unsupported items.
- Actions: Configure compression, Optimise videos, Rescan, View history.
- Keep estimates distinct from measured savings. Classify packed archives as archives unless their contents are actually inspected.

### 3. Compression setup

- Selected game and folders/files; included and excluded items with explanations.
- LZX option, with room for future algorithms.
- Current disk usage; savings estimates only when supported by evidence.
- Preflight checks: filesystem support, permissions, available space, running game.
- File selection, exclusions, Start compression, and Decompress actions.
- Explain cancellation: completed files remain compressed; decompression is a separate action.

### 4. Video optimisation

- Detected videos: resolution, duration, format, size, support status.
- Individual selection or tested game profile.
- 1080p target preserving aspect ratio.
- Original and converted sample preview.
- Estimated output size and duration when available.
- Proposed backup choices: retain originals at a selected location; retain temporarily and offer deletion after testing; or proceed without retained originals with recovery implications explained.
- Display temporary space, backup space, and net space recovered separately.
- Actions: Generate preview, Start conversion, Restore originals.
- Show unsupported items and reasons.

### 5. Jobs / activity

- Scanning, compression, decompression, conversion, and restoration jobs.
- Game, operation, stage, progress, elapsed time, estimated remaining time.
- Processed files/bytes and current file.
- Queue and cancellation controls; show safe-stop status.
- Completed jobs with actual before/after disk usage.
- Failed/skipped files, retry actions, and interrupted-job recovery.
- Only expose Pause if the underlying operation reliably supports it.

### 6. Settings / storage and recovery

- Steam/Epic library paths and manual scan locations.
- Backup folder and retention preferences.
- Temporary working directory and available space.
- Exclusions and processing limits.
- Backup inventory with game, date, location, size, Restore, and Delete backup.
- Diagnostic export; app version and update information.

### Supporting dialogs

| Dialog | Contents |
| --- | --- |
| Add games | Steam/Epic discovery or folder picker; selection of detected games |
| Preflight issue | Blocking issue, affected items, corrective action |
| Cancel job | What stops and what remains completed |
| Restore originals | Source, destination, conflicts, required space |
| Delete backup | Space reclaimed and loss of local restoration |
| Remove game | Distinguish removing a library entry from deleting installed files |

Main flow: Library → Game details → Compression or video setup → Jobs → Updated results.

## Technical findings and proposed safeguards

- Windows `compact` supports LZX and XPRESS algorithms. Microsoft describes LZX as its most compact option. Compression savings and loading performance must be measured per game; do not promise a fixed percentage or FPS improvement.
- FFmpeg can convert supported media, but producing a valid output file does not prove game compatibility. Preserve required container/codec characteristics, timing, audio tracks, and filenames, and test playback in supported games.
- Video conversion is lossy. Restoration requires originals or another recovery source. Retained backups on the same drive can negate net disk savings.
- Do not label opaque game archives as known video/audio/texture contents based on guesses.
- Future booster design should preserve shader caches by default: rebuilding them can introduce compilation work and stutter. Treat targeted cache repair as maintenance, not an automatic launch speedup.
- AnyPS5 support should be tested per game and tool version; it is not a guarantee that arbitrary PS5 games will run.
- Future architecture should separate library discovery, analysis, platform compression, media conversion, and job persistence. No framework, programming language, database, or packaging stack has been selected.

Sources consulted during planning (recheck time-sensitive integration details before implementation):

- Microsoft compact documentation: https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/compact
- FFmpeg documentation: https://ffmpeg.org/ffmpeg.html
- Microsoft shader caching explanation: https://devblogs.microsoft.com/directx/introducing-advanced-shader-delivery/
- AnyPS5 repository: https://github.com/boykopovar/AnyPS5

## Pending decisions and next steps

1. Receive the user's UI designs based on the screen outline above.
2. Receive the new repository URL and establish Git in `S:\GameZip`.
3. Clarify Steam/Epic integration: local installed-library discovery without account login was proposed, but the user has not explicitly confirmed it.
4. Choose the initial games/media formats for validation and settle exact backup defaults and no-backup behaviour.
5. Select the implementation stack and minimum supported Windows version based on the UI, native integration needs, and future Linux goals.
6. Finalise detailed job cancellation/recovery and update-conflict handling before implementing affected features.
7. Implement in milestones: analyser, compression, then selective video optimisation; validate actual disk savings, restoration, interruption recovery, and supported-game playback.

## Existing user workflow instructions

For subsequent software planning or implementation, read and follow `C:\Users\Sai\.codex\skills\project-workflow\SKILL.md`. The user's supplied instructions also reference `C:\Users\Sai\.codex\RTK.md`, which requires prefixing shell commands with `rtk`.

The current task is planning only. The user will supply the UI and create the repository. Do not begin implementing unresolved material requirements or invent a repository destination.
