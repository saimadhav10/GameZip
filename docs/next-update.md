# Next update

Authorised 2026-11-06. Keep the supplied minimal visual design and local-only data model. Implement incrementally; this checklist is not a claim that the public beta is ready.

## Implemented in the first increment

- Compression/decompression Pause and Resume at file boundaries; Cancel and tray Quit also work while paused. One job still owns the engine while paused. Restart uses interrupted-job recovery, not automatic resumption.
- Fast (XPRESS4K), Balanced (XPRESS8K, UI default), Maximum (LZX) choices. All three can be decompressed. Already-compressed files remain excluded; changing their strength requires decompression first.
- Background CPU, memory and I/O scheduling for GameZip's preparation, scan and compression worker threads. The external compact process still has below-normal CPU priority; its disk use is not capped. No measured responsiveness improvement is claimed yet.
- Storage categories: graphics/textures, videos, audio, mods, DLLs/executables, game archives and other. Mod classification uses explicit mod directories within the installation; opaque archives remain archives. Existing reports retain their previous labels until rescanned.
- Chart size/count percentages, category hover and keyboard-focus details.
- MIT licence approved by the owner and added. Third-party distribution review remains outstanding.

## Remaining implementation

- Quiet mode with controlled background resource use and idle-only work. Benchmark on disposable fixtures; do not modify real games during automated tests.
- Tested optional 4K-to-1080p video profiles, probing, preview, validated output and transactional replacement. Cyberpunk's inspected main video archive had no 4K candidates; no game is certified for conversion yet.
- Local original backups: chosen location and duration, minimum seven days, retention after success, automatic expiry while running/at next start, incomplete recovery copies retained, active restore protected from deletion.
- Restore conflicts: preserve a newer/changed game file and offer export of its original backup separately (owner approved 2026-11-06). Verify backup integrity before restore/export.
- Provider sign-in feasibility and remembered local sessions; installed games only. No GameZip backend, cloud backup, hosted accounts or cross-device sync. Local use must work without sign-in.
- Signed updater: startup/periodic checks, user-triggered installation, safe restart after all active jobs, including paused work, have finished or stopped. Public release hosting does not store user data.
- Windows 10/11 x64 installer, Start menu entry, uninstall and GitHub Releases. WinGet manifests and submission after a downloadable, tested release exists; never advertise a working package ID before acceptance.
- Dependency/font/artwork notices, FFmpeg build licence review, Windows signing and clean-machine installation/update/uninstall verification before public beta.
- Diagnose the orange border without reopening capture sessions unnecessarily. It may originate from the demonstration's capture tool; no confirmed app cause yet.

## Release dependencies

Provider product registrations/credentials, signing identity and updater signing material must be configured securely before their dependent features can be published. Do not commit secrets or substitute fake endpoints/keys. Video profiles require actual supported media and playback validation. No fixed savings or universal compatibility promises.
