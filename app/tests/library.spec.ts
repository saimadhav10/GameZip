import { expect, test, type Page } from "@playwright/test";

// Only the native boundary is mocked. Test data is never bundled into the app.
async function native(page: Page) {
  await page.addInitScript(() => {
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
          name: "Archives",
          files: 20,
          logicalBytes: 1610612736,
          allocatedBytes: 1073741824,
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
          };
        if (cmd === "list_jobs") {
          if (jobs[0]?.state === "running" && Date.now() >= finishAt) {
            jobs[0].state = "completed";
            jobs[0].message = "Scan complete";
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
          };
          jobs.unshift(job);
          return job;
        }
        if (cmd === "cancel_scan") {
          jobs[0].state = "cancelled";
          jobs[0].message = "Scan cancelled. Previous results were kept.";
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
  });
}

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
