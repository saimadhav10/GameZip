import { chromium } from "@playwright/test";
import { mkdir, writeFile, readFile } from "node:fs/promises";
import path from "node:path";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";

const root = path.resolve(process.argv[2]);
assert(
  root.includes(`${path.sep}.local${path.sep}smoke-`),
  "An isolated smoke directory is required",
);
const fixture = path.join(root, "Fixture Game");
await mkdir(fixture, { recursive: true });
await writeFile(path.join(fixture, "content.pak"), Buffer.alloc(819200, 1));
await writeFile(path.join(fixture, "intro.mp4"), Buffer.alloc(5001, 2));
let browser;
for (let attempt = 0; attempt < 40; attempt++) {
  try {
    browser = await chromium.connectOverCDP("http://127.0.0.1:9227");
    break;
  } catch {
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
}
assert(browser, "WebView2 debugging endpoint did not start");
try {
  const context = browser.contexts()[0];
  let page;
  for (let attempt = 0; attempt < 40; attempt++) {
    page = context
      .pages()
      .find((candidate) => candidate.url().includes("tauri.localhost"));
    if (page) break;
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  assert(page, "The bundled GameZip webview did not load");
  await page.waitForFunction(() => !!window.__TAURI_INTERNALS__);
  const invoke = (command, args = {}) =>
    page.evaluate(
      ([command, args]) => window.__TAURI_INTERNALS__.invoke(command, args),
      [command, args],
    );
  const info = await invoke("app_info");
  assert.equal(
    path.resolve(info.databasePath),
    path.join(root, "profile", "gamezip.sqlite"),
    "Refusing to touch a non-test profile",
  );
  const game = await invoke("add_game", {
    name: "GameZip smoke fixture",
    path: fixture,
    launcher: "Manual",
  });
  const job = await invoke("start_scan", { id: game.id });
  let result;
  for (let attempt = 0; attempt < 80; attempt++) {
    result = (await invoke("list_jobs")).find((item) => item.id === job.id);
    if (!["running", "stopping"].includes(result.state)) break;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  assert.equal(result.state, "completed");
  const saved = (await invoke("list_games")).find(
    (item) => item.id === game.id,
  );
  assert.equal(saved.scan.files, 2);
  assert.equal(saved.scan.logicalBytes, 824201);
  assert(saved.scan.allocatedBytes > saved.scan.logicalBytes);
  assert.equal(saved.scan.allocationUnknown, 0);
  assert.equal(saved.scan.skipped, 0);
  await page.reload();
  await page.getByRole("button", { name: "View details" }).click();
  await page.getByRole("heading", { name: "What's taking space" }).waitFor();
  await page.screenshot({
    path: path.join(root, "native-details.png"),
    fullPage: true,
  });
  for (const operation of ["compress", "decompress"]) {
    if (operation === "compress") {
      await invoke("set_performance", {
        value: { quiet: false, idleOnly: true },
      });
    }
    if (operation === "decompress") {
      await page.getByRole("button", { name: "Library", exact: true }).click();
      await page.getByRole("button", { name: "View details" }).click();
    }
    await page
      .getByRole("button", {
        name: operation === "compress" ? "Compress" : "Decompress",
        exact: true,
      })
      .click();
    const start = page.getByRole("button", {
      name:
        operation === "compress" ? "Start compression" : "Start decompression",
    });
    await start.waitFor();
    await page.screenshot({
      path: path.join(root, `native-${operation}.png`),
      fullPage: true,
    });
    await start.click();
    if (operation === "compress") {
      // Recent UI input keeps the native worker at its idle gate. This checks
      // the real scheduler and IPC, not a mocked browser job.
      let waiting;
      for (let attempt = 0; attempt < 40; attempt++) {
        waiting = (await invoke("list_jobs")).find(
          (item) => item.operation === operation,
        );
        if (waiting?.state === "waiting") break;
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
      assert.equal(waiting?.state, "waiting");
      assert.equal(waiting.files, 0);
      await invoke("pause_job", { id: waiting.id, paused: true });
      await invoke("set_performance", {
        value: { quiet: false, idleOnly: false },
      });
      await new Promise((resolve) => setTimeout(resolve, 250));
      const paused = (await invoke("list_jobs")).find(
        (item) => item.id === waiting.id,
      );
      assert.equal(paused.state, "paused");
      assert.equal(paused.files, 0);
      await invoke("pause_job", { id: waiting.id, paused: false });
    }
    let outcome;
    for (let attempt = 0; attempt < 200; attempt++) {
      outcome = (await invoke("list_jobs")).find(
        (item) => item.operation === operation,
      );
      if (outcome && !["running", "stopping"].includes(outcome.state)) break;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert.equal(outcome?.state, "completed", outcome?.message);
    assert(outcome.files > 0);
    if (operation === "compress")
      assert(outcome.beforeBytes > outcome.afterBytes);
    assert.deepEqual(
      await readFile(path.join(fixture, "content.pak")),
      Buffer.alloc(819200, 1),
    );
    assert.deepEqual(
      await readFile(path.join(fixture, "intro.mp4")),
      Buffer.alloc(5001, 2),
    );
    // Wait for React's polling refresh before starting the next operation.
    await page
      .getByRole("button", { name: "Cancel", exact: true })
      .waitFor({ state: "detached" });
  }
  if (process.env.GAMEZIP_TEST_FFMPEG) {
    const ffmpeg = path.resolve(process.env.GAMEZIP_TEST_FFMPEG);
    const sample = path.join(fixture, "sample-4k.mp4");
    execFileSync(
      ffmpeg,
      [
        "-nostdin",
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=c=blue:s=3840x2160:r=24:d=1",
        "-c:v",
        "libx264",
        "-threads",
        "2",
        "-pix_fmt",
        "yuv420p",
        sample,
      ],
      { windowsHide: true },
    );
    const original = await readFile(sample);
    const backup = path.join(root, "originals");
    await mkdir(backup);
    await assert.rejects(
      invoke("save_video_settings", {
        value: {
          toolsDirectory: path.dirname(ffmpeg),
          backupDirectory: backup,
          retentionDays: 3,
        },
      }),
      /seven days/,
    );
    await assert.rejects(
      invoke("save_video_settings", {
        value: {
          toolsDirectory: path.dirname(ffmpeg),
          backupDirectory: fixture,
          retentionDays: 7,
        },
      }),
      /outside your game folders/,
    );
    await invoke("save_video_settings", {
      value: {
        toolsDirectory: path.dirname(ffmpeg),
        backupDirectory: backup,
        retentionDays: 14,
      },
    });
    assert.equal((await invoke("video_settings")).retentionDays, 14);
    await page.getByRole("button", { name: "Library", exact: true }).click();
    await page.getByRole("button", { name: "View details" }).click();
    await page.getByRole("button", { name: "Videos", exact: true }).click();
    await page
      .locator(".video-row")
      .filter({ hasText: "sample-4k.mp4" })
      .getByRole("button", { name: "Create 1080p preview" })
      .click();
    await page
      .getByRole("button", { name: "Open preview" })
      .waitFor({ timeout: 60000 });
    const preview = (await invoke("list_jobs")).find(
      (j) => j.operation === "video_preview",
    );
    assert.equal(preview.state, "completed", preview.message);
    const probe = JSON.parse(
      execFileSync(
        path.join(path.dirname(ffmpeg), "ffprobe.exe"),
        ["-v", "error", "-show_streams", "-of", "json", preview.outputPath],
        { windowsHide: true, encoding: "utf8" },
      ),
    );
    assert.equal(probe.streams[0].width, 1920);
    assert.equal(probe.streams[0].height, 1080);
    assert.deepEqual(await readFile(sample), original);
    await page.screenshot({
      path: path.join(root, "native-video-preview.png"),
    });
    assert.deepEqual((await invoke("video_backups")).backups, []);
    console.log(
      "Native video preview: 4K fixture to 1080p through UI, original unchanged; backup settings and retention guards passed.",
    );
  }
  await invoke("remove_game", { id: game.id });
  assert.equal((await invoke("list_games")).length, 0);
  assert.deepEqual(
    await readFile(path.join(fixture, "content.pak")),
    Buffer.alloc(819200, 1),
  );
  assert.deepEqual(
    await readFile(path.join(fixture, "intro.mp4")),
    Buffer.alloc(5001, 2),
  );
  console.log(
    `Native WebView2 smoke passed: add, scan, compression/decompression UI, measured savings, unchanged fixture contents, and non-destructive removal. Screenshots: ${root}`,
  );
} finally {
  await browser.close();
}
