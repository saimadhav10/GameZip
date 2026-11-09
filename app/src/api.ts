import { invoke, isTauri } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  AppInfo,
  Candidate,
  Discovery,
  Game,
  Job,
  CompressionPreview,
  VideoSettings,
  VideoCandidate,
  BackupInventory,
} from "./types";

export const desktop = isTauri();
export const api = {
  videoSettings: () => invoke<VideoSettings>("video_settings"),
  saveVideoSettings: (value: VideoSettings) =>
    invoke<VideoSettings>("save_video_settings", { value }),
  videos: (id: string) => invoke<VideoCandidate[]>("video_candidates", { id }),
  previewVideo: (id: string, path: string) =>
    invoke<Job>("start_video_preview", { id, path }),
  openPreview: (id: string) => invoke<void>("open_job_output", { id }),
  backups: () => invoke<BackupInventory>("video_backups"),
  exportBackup: (vault: string, id: string, destination: string) =>
    invoke<void>("export_video_backup", { vault, id, destination }),
  pickVideoFolder: (title: string) =>
    open({ directory: true, multiple: false, title }),
  pickExport: () => save({ title: "Export original to a new file" }),
  games: () => invoke<Game[]>("list_games"),
  jobs: () => invoke<Job[]>("list_jobs"),
  info: () => invoke<AppInfo>("app_info"),
  performance: (value: AppInfo["performance"]) =>
    invoke<void>("set_performance", { value }),
  discover: () => invoke<Discovery>("discover_games"),
  add: (candidate: Candidate) => invoke<Game>("add_game", { ...candidate }),
  remove: (id: string) => invoke<void>("remove_game", { id }),
  scan: (id: string) => invoke<Job>("start_scan", { id }),
  prepareCompression: (
    id: string,
    operation: "compress" | "decompress",
    strength: "fast" | "balanced" | "maximum" = "maximum",
  ) =>
    invoke<CompressionPreview>("prepare_compression", {
      id,
      operation,
      strength,
    }),
  compress: (token: string) => invoke<Job>("start_compression", { token }),
  pause: (id: string, paused: boolean) =>
    invoke<void>("pause_job", { id, paused }),
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
  ["running", "stopping", "pausing", "paused", "waiting"].includes(job.state);
export const dateTime = (seconds: number) =>
  new Date(seconds * 1000).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
