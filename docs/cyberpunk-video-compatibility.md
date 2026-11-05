# Cyberpunk video compatibility checkpoint

Inspected read-only on 2026-11-05. No game file was extracted, replaced, compressed, or otherwise modified by this investigation.

## Observed installation data

The installed `archive/pc/content/basegame_5_video.archive` is 13,895,094,272 bytes. Its index contains 552 entries, each with one uncompressed archive segment. Reading only their headers found:

- 550 `KB2j` Bink 2 headers and two `BIKi` Bink headers.
- Zero headers with dimensions at least 3840×2160, including rotated equivalents.
- Many small in-world videos and 1080p videos. The widest observed header is 2625×995.

This is a header inventory of the main video archive, not a decoded playback validation or an inventory of every DLC/mod archive. No profile is approved for replacement. Do not downscale lower-resolution files or convert Bink to a different codec just to offer a button. The agreed feature targets prerecorded 4K-to-1080p conversion.

Reproduce with:

```powershell
node scripts/inspect-cyberpunk.mjs "<Cyberpunk installation folder>"
```

The script opens the archive with read-only access, validates bounds/version, reads the index and small file headers, and prints metadata. It neither unpacks nor writes game data. Compressed first segments are counted as uninspected rather than guessed.

## Format evidence and remaining work

- Archive layout and Bink signatures were checked against [WolvenKit's archive reader](https://github.com/WolvenKit/WolvenKit/blob/main/WolvenKit.RED4/Archive/IO/ArchiveReader.cs).
- Header dimensions, frame rate fields, and recognised Bink signatures were checked against [FFmpeg's Bink demuxer](https://github.com/FFmpeg/FFmpeg/blob/master/libavformat/bink.c).
- WolvenKit documents [video archive extraction](https://wiki.redmodding.org/wolvenkit/wolvenkit-cli/usage/video-and-audio-files-cli) and external [RAD tools for viewing](https://wiki.redmodding.org/wolvenkit/wolvenkit-app/usage/video-and-audio). This is not evidence of an approved redistribution/encoding pipeline for GameZip.

Next: inspect other relevant archives if broader Cyberpunk coverage is needed, build the backup/conversion plumbing with generated samples, and validate an actual 4K game profile before enabling replacement. Bink encoding, redistribution, archive rewriting, and in-game playback remain unvalidated. Do not claim that installing Cyberpunk establishes video optimisation compatibility.

The user approved retention starting after successful conversion, preserving incomplete recovery copies, and postponing deletion during restore. The minimum remains seven days, with location and duration chosen by the user.
