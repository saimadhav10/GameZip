import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { AppInfo, Candidate, Discovery, Game, Job } from "./types";

export const desktop = isTauri();
export const api = {
  games: () => invoke<Game[]>("list_games"),
  jobs: () => invoke<Job[]>("list_jobs"),
  info: () => invoke<AppInfo>("app_info"),
  discover: () => invoke<Discovery>("discover_games"),
  add: (candidate: Candidate) => invoke<Game>("add_game", { ...candidate }),
  remove: (id: string) => invoke<void>("remove_game", { id }),
  scan: (id: string) => invoke<Job>("start_scan", { id }),
  cancel: (id: string) => invoke<void>("cancel_scan", { id }),
  openFolder: (id: string) => invoke<void>("open_game_folder", { id }),
  pickFolder: () =>
    open({
      directory: true,
      multiple: false,
      title: "Choose a game installation folder",
    }),
};

export function bytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  const level = Math.min(
    Math.floor(Math.log(value) / Math.log(1024)),
    units.length - 1,
  );
  return `${(value / 1024 ** level).toLocaleString(undefined, { maximumFractionDigits: level > 0 ? 1 : 0 })} ${units[level]}`;
}
export const activeJob = (job: Job) =>
  job.state === "running" || job.state === "stopping";
export const dateTime = (seconds: number) =>
  new Date(seconds * 1000).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
