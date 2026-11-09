import { useEffect, useState } from "react";
import { api, bytes, dateTime, desktop } from "./api";
import { Modal } from "./Modal";
import type {
  BackupInventory,
  Game,
  VideoCandidate,
  VideoSettings,
} from "./types";

export function Videos({
  game,
  close,
  settings,
  started,
}: {
  game: Game;
  close: () => void;
  settings: () => void;
  started: () => Promise<void>;
}) {
  const [files, setFiles] = useState<VideoCandidate[]>([]);
  const [loading, setLoading] = useState(true);
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    let alive = true;
    Promise.all([api.videos(game.id), api.videoSettings()])
      .then(([items, prefs]) => {
        if (alive) {
          setFiles(items);
          setReady(!!prefs.toolsDirectory);
        }
      })
      .catch((e) => {
        if (alive) setError(String(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [game.id]);
  async function preview(path: string) {
    setBusy(true);
    setError("");
    try {
      await api.previewVideo(game.id, path);
      await started();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <Modal title="Video previews" close={close} busy={busy}>
      <p className="muted">
        Create a separate 1080p copy. Your game keeps its original video.
      </p>
      {error && (
        <p role="alert" className="alert">
          {error}
        </p>
      )}
      {loading ? (
        <p role="status">Looking for videos…</p>
      ) : (
        <>
          {!ready && (
            <p className="notice">
              Choose your FFmpeg tools in Settings first.{" "}
              <button className="text-button" onClick={settings}>
                Set up tools
              </button>
            </p>
          )}
          {files.length === 0 ? (
            <p className="empty-hint">
              No standalone videos found. Videos inside game archives cannot be
              previewed.
            </p>
          ) : (
            <div className="video-list">
              {files.map((file) => (
                <div className="video-row" key={file.path}>
                  <div>
                    <strong>{file.path}</strong>
                    <small>
                      {bytes(file.bytes)} ·{" "}
                      {file.previewAvailable
                        ? "MP4 · compatibility checked before encoding"
                        : "Format not supported"}
                    </small>
                  </div>
                  {file.previewAvailable && (
                    <button
                      className="secondary"
                      disabled={busy || !ready}
                      onClick={() => preview(file.path)}
                    >
                      Create 1080p preview
                    </button>
                  )}
                </div>
              ))}
              {files.length >= 200 && (
                <p className="muted">
                  Showing the first 200 video files found.
                </p>
              )}
            </div>
          )}
        </>
      )}
      <details className="video-help">
        <summary>Supported previews</summary>
        <p>
          Standalone 3840 × 2160 H.264 MP4, standard colour and constant frame
          rate. Audio is copied. Other profiles are rejected. Previews use
          additional disk space and are saved locally; game replacement remains
          unavailable until in-game playback is validated.
        </p>
      </details>
    </Modal>
  );
}

export function VideoPreferences() {
  const [value, setValue] = useState<VideoSettings | null>(null);
  const [inventory, setInventory] = useState<BackupInventory | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [status, setStatus] = useState("");
  useEffect(() => {
    if (!desktop) return;
    let alive = true;
    api
      .videoSettings()
      .then((v) => {
        if (alive) setValue(v);
      })
      .catch((e) => {
        if (alive) setError(String(e));
      });
    return () => {
      alive = false;
    };
  }, []);
  async function run(task: () => Promise<void>) {
    setBusy(true);
    setError("");
    setStatus("");
    try {
      await task();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function choose(field: "toolsDirectory" | "backupDirectory") {
    const folder = await api.pickVideoFolder(
      field === "toolsDirectory"
        ? "Choose FFmpeg bin folder"
        : "Choose a dedicated backup folder outside your games",
    );
    if (typeof folder === "string" && value)
      setValue({ ...value, [field]: folder });
  }
  return (
    <section className="panel video-settings">
      <details>
        <summary>Videos & backups</summary>
        <p className="muted">
          Preview tools and local recovery copies. Game-video replacement is not
          enabled yet.
        </p>
        {error && (
          <p className="alert" role="alert">
            {error}
          </p>
        )}
        {value ? (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void run(async () => {
                setValue(await api.saveVideoSettings(value));
                setStatus("Video settings saved.");
              });
            }}
          >
            <div className="video-row">
              <div>
                <strong>FFmpeg tools</strong>
                <small>{value.toolsDirectory ?? "Not configured"}</small>
              </div>
              <button
                type="button"
                className="secondary"
                disabled={busy}
                onClick={() => run(() => choose("toolsDirectory"))}
              >
                Choose tools folder
              </button>
            </div>
            <p className="muted small-note">
              Use a trusted FFmpeg build containing ffmpeg.exe and ffprobe.exe.
              Tools run locally and are not bundled.
            </p>
            <div className="video-row">
              <div>
                <strong>Original backups</strong>
                <small>
                  {value.backupDirectory ?? "Choose a dedicated local folder"}
                </small>
              </div>
              <button
                type="button"
                className="secondary"
                disabled={busy}
                onClick={() => run(() => choose("backupDirectory"))}
              >
                Choose backup folder
              </button>
            </div>
            <label className="retention">
              Keep originals for{" "}
              <input
                type="number"
                min="7"
                max="4294967295"
                step="1"
                required
                aria-label="Backup retention days"
                value={value.retentionDays}
                disabled={busy}
                onChange={(e) =>
                  setValue({ ...value, retentionDays: e.target.valueAsNumber })
                }
              />{" "}
              days
            </label>
            <p className="muted">
              Minimum seven days after a successful conversion. Expired backups
              are deleted automatically while GameZip is open. Incomplete
              recovery copies are kept.
            </p>
            <div className="video-controls">
              <button className="primary" disabled={busy}>
                Save video settings
              </button>
              <button
                type="button"
                className="secondary"
                disabled={busy}
                onClick={() =>
                  run(async () => {
                    setInventory(await api.backups());
                  })
                }
              >
                View backups
              </button>
            </div>
          </form>
        ) : (
          <p className="muted">
            {desktop
              ? "Loading video settings…"
              : "Available in the desktop app."}
          </p>
        )}
        {status && <p role="status">{status}</p>}
        {inventory && (
          <div className="video-list">
            {inventory.warnings.map((w) => (
              <p className="alert" key={w}>
                {w}
              </p>
            ))}
            {!inventory.backups.some((b) => b.record.state !== "Expired") && (
              <p className="muted">No retained video backups.</p>
            )}
            {inventory.backups
              .filter((b) => b.record.state !== "Expired")
              .map(({ vault, record }) => (
                <div className="video-row" key={`${vault}:${record.id}`}>
                  <div>
                    <strong>{record.original_path}</strong>
                    <small>
                      {bytes(record.bytes)} ·{" "}
                      {record.expires_at && record.state === "Completed"
                        ? `Expires ${dateTime(record.expires_at)}`
                        : "Kept for recovery"}
                    </small>
                  </div>
                  {["Ready", "Completed", "Restoring"].includes(
                    record.state,
                  ) && (
                    <button
                      className="secondary"
                      disabled={busy}
                      onClick={() =>
                        run(async () => {
                          const destination = await api.pickExport();
                          if (!destination) return;
                          await api.exportBackup(vault, record.id, destination);
                          setStatus("Original exported. Game files were kept.");
                        })
                      }
                    >
                      Export original
                    </button>
                  )}
                </div>
              ))}
          </div>
        )}
      </details>
    </section>
  );
}
