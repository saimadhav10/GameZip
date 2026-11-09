# Video and backup implementation checkpoint

The native core now includes two foundations. Neither is wired to game-video replacement or presented as finished game support in the desktop UI.

## Backup vault

`backups::Vault` stores a SQLite ledger beside UUID-named original copies in a dedicated chosen directory. Copies are streamed and SHA-256 verified. Retention must be at least seven days and has no expiry until a successful conversion is recorded and the converted file's hash is checked. Expiry deletes only the opened, verified backup file by Windows handle. Preparing, ready and restoring records are retained, including after restart. A failed deletion can be reconciled after restart.

If a game update changes the converted video, beginning a restore is refused and the original can be exported to a separate new file. Export never overwrites an existing destination. The restore reservation prevents expiry but does not yet implement replacing a game's current video. Automatic cleanup scheduling, backup settings/inventory UI, replacement/restore transaction recovery and user-facing export remain to be connected.

## Sample preview engine

`media::Tools::preview_1080p` creates a separate preview from a standalone 3840×2160 H.264, 8-bit, constant-frame-rate MP4 sample. It rejects unsupported tracks, Bink/archives, rotation, HDR and non-square pixels. The source is held read-only against modification during conversion. Video uses H.264 at 1920×1080; audio is stream-copied. Output resolution, frame rate, duration and audio stream metadata are checked, followed by a full decode check. Tool processes run below-normal CPU priority, with bounded metadata output, cancellation and timeouts.

This proves the sample pipeline only. It does not establish any game's codec/container requirements. No installed-game video replacement is enabled. Cyberpunk's inspected main archive had no 4K candidates and uses Bink formats that this pipeline cannot encode.

## Validation and distribution

- Generated sample conversion test checks original bytes unchanged, output 1080p and identical compressed audio packet hashes. The test is opt-in because FFmpeg binaries are not a repository dependency: set `GAMEZIP_TEST_FFMPEG` and `GAMEZIP_TEST_FFPROBE` to verified absolute paths, then run `cargo test --manifest-path app/src-tauri/Cargo.toml --lib media::tests::sample_conversion -- --ignored`.
- Local verification uses Gyan's FFmpeg 9.0.2 essentials build, downloaded through the Windows build provider linked by [FFmpeg](https://www.ffmpeg.org/download.html) and checked against the provider's SHA-256 checksum. Test tools live under ignored `.local/media-tools` and are not bundled or committed.
- The current test encoder uses libx264. Review the exact FFmpeg build and its GPL/dependency/source obligations before distribution; the application's MIT licence does not replace those obligations. See [FFmpeg's legal guidance](https://ffmpeg.org/legal.html).
- Next integration must validate free/temporary space, coordinate one active job and running-game checks, bind original-file identity to the backup transaction, expose previews and chosen retention/location, implement recoverable replacement/restore, and certify an actual supported game/build before enabling replacement.
# App integration (2026-11-09)

Game details → Videos lists up to 200 standalone videos. MP4 files can be submitted for profile validation and separate 1080p preview generation; the original remains in place. Jobs opens the completed preview. Settings → Videos & backups chooses an existing trusted FFmpeg bin folder, a dedicated backup folder outside registered games, and retention of at least seven days. FFmpeg is not bundled. Historical vault locations remain registered when the chosen location changes.

Backup inventory can export verified originals to new files. Startup/hourly maintenance expires eligible completed copies and retries unavailable locations on the next pass. Incomplete copies and restore reservations remain protected. These controls do not enable game-file replacement or in-place restoration. No installed game profile has passed playback validation, and the owner has no additional test game available.

Validation: generated-media native WebView2 preview, unsupported media UI errors, backup preferences/export UI, and disposable-vault offline/reconnect expiry tests. Previews stay in the local app-data `Previews` directory and use additional storage. Failed/interrupted preview output may remain there for manual removal; it is never used by a game.

