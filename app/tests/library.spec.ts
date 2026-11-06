import { expect, test, type Page } from "@playwright/test";

// Only the native boundary is mocked. Test data is never bundled into the app.
async function native(page: Page, scenario = "normal") {
  await page.addInitScript((scenario) => {
    const w = window as unknown as {
      isTauri: boolean;
      __TAURI_INTERNALS__: {
        invoke: (
          cmd: string,
          args: Record<string, unknown>,
        ) => Promise<unknown>;
      };
    };
    w.isTauri = true;
    const games: Record<string, unknown>[] = [];
    const jobs: Record<string, unknown>[] = [];
    let finishAt = 0;
    let operation = "compress";
    let performance = { quiet: true, idleOnly: false };
    const scan = {
      files: 23,
      logicalBytes: 2147483648,
      allocatedBytes: 1610612736,
      allocationUnknown: 0,
      skipped: 0,
      duplicateLinks: 1,
      issues: [],
      completedAt: 1793680000,
      categories: [
        {
          name: "Game archives",
          files: 8,
          logicalBytes: 1073741824,
          allocatedBytes: 536870912,
        },
        {
          name: "Graphics & textures",
          files: 4,
          logicalBytes: 268435456,
          allocatedBytes: 268435456,
        },
        {
          name: "DLLs & executables",
          files: 4,
          logicalBytes: 134217728,
          allocatedBytes: 134217728,
        },
        {
          name: "Mods",
          files: 4,
          logicalBytes: 134217728,
          allocatedBytes: 134217728,
        },
        {
          name: "Videos",
          files: 3,
          logicalBytes: 536870912,
          allocatedBytes: 536870912,
        },
      ],
      largestFiles: [
        {
          path: "data/world.pak",
          category: "Archives",
          logicalBytes: 1610612736,
          allocatedBytes: 1073741824,
        },
        {
          path: "movies/intro.mp4",
          category: "Videos",
          logicalBytes: 536870912,
          allocatedBytes: 536870912,
        },
      ],
    };
    w.__TAURI_INTERNALS__ = {
      invoke: async (cmd, args = {}) => {
        if (cmd === "list_games") return structuredClone(games);
        if (cmd === "app_info")
          return {
            version: "0.1.0",
            databasePath: "C:\\TestData\\gamezip.sqlite",
            performance,
          };
        if (cmd === "set_performance") {
          performance = args.value as typeof performance;
          return;
        }
        if (cmd === "list_jobs") {
          if (jobs[0]?.state === "running" && Date.now() >= finishAt) {
            jobs[0].state = "completed";
            jobs[0].message = "Scan complete";
            if (jobs[0].operation === "compress") {
              jobs[0].beforeBytes = 1073741824;
              jobs[0].afterBytes = 536870912;
              jobs[0].message = "Finished. 20 files changed.";
            }
            games[0].scan = scan;
          }
          return structuredClone(jobs);
        }
        if (cmd === "plugin:dialog|open") return "D:\\Games\\Nebula Rift";
        if (cmd === "discover_games")
          return {
            games: [
              {
                name: "Nebula Rift",
                path: "D:\\Games\\Nebula Rift",
                launcher: "Steam",
              },
            ],
            warnings: [],
          };
        if (cmd === "add_game") {
          if (games.length)
            throw new Error("This folder is already in your library.");
          const game = {
            id: "test-game",
            name: args.name || "Nebula Rift",
            path: args.path,
            launcher: args.launcher,
            missing: false,
            scan: null,
          };
          games.push(game);
          return game;
        }
        if (cmd === "start_scan") {
          finishAt = Date.now() + 2200;
          const job = {
            id: `test-job-${jobs.length}`,
            gameId: "test-game",
            gameName: "Nebula Rift",
            state: "running",
            files: 23,
            logicalBytes: 2147483648,
            message: "Reading game files",
            startedAt: 1793680000,
            finishedAt: null,
            operation: "scan",
            totalFiles: null,
            beforeBytes: 0,
            afterBytes: 0,
          };
          jobs.unshift(job);
          return job;
        }
        if (cmd === "prepare_compression") {
          if (scenario === "running")
            throw new Error(
              "Close the game before continuing: fixture-game.exe",
            );
          operation = String(args.operation);
          return {
            token: "prepared-fixture",
            operation,
            files: 20,
            skipped: 3,
            allocatedBytes: 1073741824,
            freeBytes: scenario === "low-space" ? 1024 : 10737418240,
            requiredFreeBytes: 2147483648,
          };
        }
        if (cmd === "start_compression") {
          finishAt = Date.now() + 2200;
          games[0].scan = null;
          const job = {
            id: `test-job-${jobs.length}`,
            gameId: "test-game",
            gameName: "Nebula Rift",
            operation,
            totalFiles: 20,
            files: 10,
            beforeBytes: 0,
            afterBytes: 0,
            logicalBytes: 1073741824,
            state: "running",
            message: "Processing files",
            startedAt: 1793680000,
            finishedAt: null,
          };
          jobs.unshift(job);
          return job;
        }
        if (cmd === "cancel_scan") {
          jobs[0].state = "cancelled";
          jobs[0].message = "Scan cancelled. Previous results were kept.";
          return;
        }
        if (cmd === "pause_job") {
          jobs[0].state = args.paused ? "paused" : "running";
          jobs[0].message = args.paused
            ? "Paused. Resume when you are ready."
            : "Processing files";
          finishAt = Date.now() + 2200;
          return;
        }
        if (cmd === "remove_game") {
          games.splice(0);
          return;
        }
        if (cmd === "open_game_folder") return;
        throw new Error(`Unexpected native command: ${cmd}`);
      },
    };
  }, scenario);
}

async function scannedGame(page: Page, scenario = "normal") {
  await native(page, scenario);
  await page.goto("/");
  await page.getByRole("button", { name: "Add games" }).first().click();
  await page
    .getByRole("button", { name: "Choose a folder", exact: false })
    .click();
  await page.getByRole("button", { name: "View details" }).click();
  await page.getByRole("button", { name: "Analyse game" }).click();
  await expect(
    page.getByRole("heading", { name: "What's taking space" }),
  ).toBeVisible();
}

test("paused compression retains its reservation and can resume or cancel", async ({
  page,
}) => {
  await scannedGame(page);
  await page.getByRole("button", { name: "Compress", exact: true }).click();
  await page.getByRole("button", { name: "Start compression" }).click();
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  await expect(
    page.getByText("Paused. Resume when you are ready."),
  ).toBeVisible();
  await page.getByRole("button", { name: "Library", exact: true }).click();
  await page.getByRole("button", { name: "View details" }).click();
  await expect(
    page.getByRole("button", { name: "Analyse game" }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Jobs", exact: true }).click();
  await page.getByRole("button", { name: "Resume", exact: true }).click();
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Resume", exact: true }),
  ).toHaveCount(0);
});

test("compression preview starts a job and reports measured savings", async ({
  page,
}) => {
  await scannedGame(page);
  await page.getByRole("button", { name: "Compress", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText("20 files", { exact: true })).toBeVisible();
  await page.screenshot({
    path: "test-results/compression-preview.png",
    fullPage: true,
  });
  await page.setViewportSize({ width: 720, height: 700 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBeTruthy();
  await page.screenshot({ path: "test-results/compression-preview-small.png" });
  await dialog.getByRole("button", { name: "Start compression" }).click();
  await expect(
    page.getByText("512 MiB saved on processed files", { exact: false }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Library", exact: true }).click();
  await page.getByRole("button", { name: "View details" }).click();
  await page.getByRole("button", { name: "Decompress", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Start decompression" }),
  ).toBeEnabled();
  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
});

test("running games block compression with an actionable message", async ({
  page,
}) => {
  await scannedGame(page, "running");
  await page.getByRole("button", { name: "Compress", exact: true }).click();
  await expect(page.getByRole("dialog").getByRole("alert")).toContainText(
    "Close the game",
  );
  await expect(
    page.getByRole("button", { name: "Start compression" }),
  ).toHaveCount(0);
});

test("insufficient space disables compression before starting", async ({
  page,
}) => {
  await scannedGame(page, "low-space");
  await page.getByRole("button", { name: "Compress", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Start compression" }),
  ).toBeDisabled();
  await expect(
    page.getByText("This operation needs", { exact: false }),
  ).toBeVisible();
});

test("browser preview is honest about native access", async ({ page }) => {
  await page.goto("/");
  await expect(
    page.getByText("Desktop preview.", { exact: false }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Add games" }).first(),
  ).toBeDisabled();
  await expect(
    page.getByRole("heading", { name: "Start with your first game." }),
  ).toBeVisible();
});

test("performance settings stay selected when revisiting settings", async ({
  page,
}) => {
  await native(page);
  await page.goto("/");
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(
    page.getByRole("checkbox", { name: "Quiet compression", exact: true }),
  ).toBeChecked();
  await page
    .getByRole("checkbox", { name: "Quiet compression", exact: true })
    .uncheck();
  await page
    .getByRole("checkbox", { name: "Idle-only compression", exact: true })
    .check();
  await page.getByRole("button", { name: "Library", exact: true }).click();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(
    page.getByRole("checkbox", { name: "Quiet compression", exact: true }),
  ).not.toBeChecked();
  await expect(
    page.getByRole("checkbox", { name: "Idle-only compression", exact: true }),
  ).toBeChecked();
});

test("manual import, scan, distribution, cancellation and removal work together", async ({
  page,
}) => {
  await native(page);
  await page.goto("/");
  await page.screenshot({
    path: "test-results/library-empty.png",
    fullPage: true,
  });
  await page.getByRole("button", { name: "Add games" }).first().click();
  await page
    .getByRole("button", { name: "Choose a folder", exact: false })
    .click();
  await expect(
    page.getByRole("heading", { name: "Nebula Rift" }),
  ).toBeVisible();
  await page.screenshot({
    path: "test-results/library-populated.png",
    fullPage: true,
  });
  await page.getByRole("button", { name: "View details" }).click();
  await page.getByRole("button", { name: "Analyse game" }).click();
  await expect(
    page.getByRole("button", { name: "Analyse game" }),
  ).toBeDisabled();
  await expect(
    page.getByRole("heading", { name: "What's taking space" }),
  ).toBeVisible({ timeout: 10000 });
  await expect(
    page.getByRole("cell", { name: "data/world.pak" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "File count", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "File count", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "Disk usage", exact: true }).click();
  const graphics = page
    .locator(".legend > div")
    .filter({ hasText: "Graphics & textures" });
  await expect(graphics).toContainText("16.7%");
  await graphics.focus();
  await expect(page.locator(".donut")).toContainText("Graphics & textures");
  await page.getByRole("button", { name: "Disk usage", exact: true }).focus();
  await page.screenshot({
    path: "test-results/game-details.png",
    fullPage: true,
  });
  await page.setViewportSize({ width: 720, height: 600 });
  await expect(page.getByRole("button", { name: "Rescan" })).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBeTruthy();
  await page.screenshot({
    path: "test-results/game-details-small.png",
    fullPage: true,
  });
  await page.getByRole("button", { name: "Rescan" }).click();
  await page.getByRole("button", { name: "Jobs", exact: true }).click();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(
    page.getByText("Scan cancelled. Previous results were kept."),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "View Nebula Rift", exact: true })
    .first()
    .click();
  await expect(
    page.getByRole("cell", { name: "data/world.pak" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Remove from library" }).click();
  await expect(page.getByRole("dialog")).toContainText(
    "game files and saves will stay on disk",
  );
  await page.getByRole("button", { name: "Remove entry" }).click();
  await expect(
    page.getByRole("heading", { name: "Start with your first game." }),
  ).toBeVisible();
});

test("discovery imports selected games and prevents re-adding them", async ({
  page,
}) => {
  await native(page);
  await page.goto("/");
  await page.getByRole("button", { name: "Add games" }).first().click();
  await page
    .getByRole("button", { name: "Find installed games", exact: false })
    .click();
  await expect(page.getByRole("checkbox")).toBeChecked();
  await page.getByRole("button", { name: "Add 1 game", exact: true }).click();
  await page.getByRole("button", { name: "Add games" }).first().click();
  await page
    .getByRole("button", { name: "Find installed games", exact: false })
    .click();
  await expect(page.getByRole("checkbox")).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).not.toBeVisible();
  await page
    .getByRole("textbox", { name: "Search games" })
    .fill("does-not-exist");
  await expect(
    page.getByText("No games match", { exact: false }),
  ).toBeVisible();
});
