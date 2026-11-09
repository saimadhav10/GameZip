export interface Category {
  name: string;
  files: number;
  logicalBytes: number;
  allocatedBytes: number;
}
export interface LargeFile {
  path: string;
  category: string;
  logicalBytes: number;
  allocatedBytes: number | null;
}
export interface ScanReport {
  files: number;
  logicalBytes: number;
  allocatedBytes: number;
  allocationUnknown: number;
  skipped: number;
  duplicateLinks: number;
  issues: string[];
  categories: Category[];
  largestFiles: LargeFile[];
  completedAt: number;
}
export interface Game {
  id: string;
  name: string;
  path: string;
  launcher: string;
  missing: boolean;
  scan: ScanReport | null;
}
export interface Job {
  id: string;
  gameId: string;
  gameName: string;
  state: string;
  files: number;
  logicalBytes: number;
  message: string;
  startedAt: number;
  finishedAt: number | null;
  operation: "scan" | "compress" | "decompress" | "video_preview";
  outputPath?: string | null;
  totalFiles: number | null;
  beforeBytes: number;
  afterBytes: number;
}

export interface CompressionPreview {
  token: string;
  operation: "compress" | "decompress";
  files: number;
  skipped: number;
  allocatedBytes: number;
  freeBytes: number;
  requiredFreeBytes: number;
}
export interface Candidate {
  name: string;
  path: string;
  launcher: string;
}
export interface Discovery {
  games: Candidate[];
  warnings: string[];
}
export interface AppInfo {
  version: string;
  databasePath: string;
  performance: { quiet: boolean; idleOnly: boolean };
}

export interface VideoSettings {
  toolsDirectory: string | null;
  backupDirectory: string | null;
  retentionDays: number;
}
export interface VideoCandidate {
  path: string;
  bytes: number;
  previewAvailable: boolean;
}
export interface BackupInventory {
  backups: {
    vault: string;
    record: {
      id: string;
      original_path: string;
      bytes: number;
      expires_at: number | null;
      state:
        | "Preparing"
        | "Ready"
        | "Completed"
        | "Restoring"
        | "Deleting"
        | "Expired";
    };
  }[];
  warnings: string[];
}
