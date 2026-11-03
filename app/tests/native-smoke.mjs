import { chromium } from "@playwright/test";
import { mkdir, writeFile, readFile } from "node:fs/promises";
import path from "node:path";
import assert from "node:assert/strict";

const root = path.resolve(process.argv[2]);
assert(
  root.includes(`${path.sep}.local${path.sep}smoke-`),
  "An isolated smoke directory is required",
);
const fixture = path.join(root, "Fixture Game");
await mkdir(fixture, { recursive: true });
await writeFile(path.join(fixture, "content.pak"), Buffer.alloc(8192, 1));
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
  assert.equal(saved.scan.logicalBytes, 13193);
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
  await invoke("remove_game", { id: game.id });
  assert.equal((await invoke("list_games")).length, 0);
  assert.deepEqual(
    await readFile(path.join(fixture, "content.pak")),
    Buffer.alloc(8192, 1),
  );
  assert.deepEqual(
    await readFile(path.join(fixture, "intro.mp4")),
    Buffer.alloc(5001, 2),
  );
  console.log(
    `Native WebView2 smoke passed: real add, scan, persistence, UI rendering, and non-destructive removal. Screenshot: ${path.join(root, "native-details.png")}`,
  );
} finally {
  await browser.close();
}
