// Read-only compatibility probe. Archive layout reference:
// https://github.com/WolvenKit/WolvenKit/blob/main/WolvenKit.RED4/Archive/IO/ArchiveReader.cs
// Header fields: https://github.com/FFmpeg/FFmpeg/blob/master/libavformat/bink.c
// This is not an unpacker, converter, or replacement tool.
import { open } from "node:fs/promises";
import path from "node:path";
import assert from "node:assert/strict";

assert(process.argv[2], "Pass the Cyberpunk installation folder.");
const archive = path.join(path.resolve(process.argv[2]), "archive", "pc", "content", "basegame_5_video.archive");
const file = await open(archive, "r");
try {
  const size = (await file.stat()).size;
  const read = async (offset, length) => {
    assert(Number.isSafeInteger(offset) && offset >= 0 && offset + length <= size, "Archive read is out of bounds");
    const buffer = Buffer.alloc(length);
    const { bytesRead } = await file.read(buffer, 0, length, offset);
    assert.equal(bytesRead, length, "Archive changed or read was incomplete");
    return buffer;
  };
  const header = await read(0, 40);
  assert.equal(header.toString("ascii", 0, 4), "RDAR", "Not a RED archive");
  assert.equal(header.readUInt32LE(4), 12, "Unsupported archive version");
  const indexPosition = Number(header.readBigUInt64LE(8));
  const indexSize = header.readUInt32LE(16);
  assert(indexSize >= 28 && indexSize <= 64 * 1024 * 1024, "Unsupported index size");
  const index = await read(indexPosition, indexSize);
  const entries = index.readUInt32LE(16);
  const segments = index.readUInt32LE(20);
  const segmentStart = 28 + entries * 56;
  assert(segmentStart + segments * 16 <= index.length, "Invalid archive index");
  const signatures = {};
  const dimensions = {};
  let fourKHeaders = 0;
  let fourKStoredBytes = 0;
  const examples = [];
  let compressedFirstSegments = 0;
  for (let entry = 0; entry < entries; entry++) {
    const offset = 28 + entry * 56;
    const first = index.readUInt32LE(offset + 20);
    const end = index.readUInt32LE(offset + 24);
    assert(first < end && end <= segments, "Invalid segment range");
    const segment = segmentStart + first * 16;
    const stored = index.readUInt32LE(segment + 8);
    const expanded = index.readUInt32LE(segment + 12);
    if (stored !== expanded) { compressedFirstSegments++; continue; }
    if (stored < 44) continue;
    const start = Number(index.readBigUInt64LE(segment));
    const content = await read(start, 44);
    const magic = content.toString("ascii", 0, 4);
    signatures[magic] = (signatures[magic] ?? 0) + 1;
    if (!/^(KB2[adfghijk]|BIK[bfghik])$/.test(magic)) continue;
    const width = content.readUInt32LE(20);
    const height = content.readUInt32LE(24);
    const numerator = content.readUInt32LE(28);
    const denominator = content.readUInt32LE(32);
    const audioTracks = content.readUInt32LE(40);
    assert(width > 0 && width <= 32767 && height > 0 && height <= 32767 && numerator > 0 && denominator > 0 && audioTracks <= 256, "Invalid Bink header fields");
    dimensions[`${width}x${height}`] = (dimensions[`${width}x${height}`] ?? 0) + 1;
    // Header dimensions are only candidates, never approval for replacement.
    if (Math.max(width, height) >= 3840 && Math.min(width, height) >= 2160) {
      fourKHeaders++;
      fourKStoredBytes += stored;
    }
    if (examples.length < 3) examples.push({ hash: index.readBigUInt64LE(offset).toString(), magic, width, height, fps: numerator / denominator, audioTracks });
  }
  console.log(JSON.stringify({ archiveBytes: size, entries, segments, compressedFirstSegments, signatures, dimensions, fourKHeaders, fourKStoredBytes, examples }, null, 2));
} finally {
  await file.close();
}
