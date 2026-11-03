import { useState } from "react";
import {
  ArrowLeft,
  CircleAlert,
  FolderOpen,
  HardDrive,
  RefreshCw,
  Search,
  Trash2,
} from "lucide-react";
import { bytes, dateTime } from "./api";
import type { Category, Game } from "./types";

const colors: Record<string, string> = {
  Archives: "#ff7a36",
  Videos: "#69ccee",
  Audio: "#b7a5ef",
  Images: "#72d99e",
  Programs: "#efc66b",
  Other: "#78818b",
};

function Distribution({
  categories,
  totalFiles,
}: {
  categories: Category[];
  totalFiles: number;
}) {
  const [mode, setMode] = useState<"disk" | "files">("disk");
  const total = categories.reduce(
    (sum, c) => sum + (mode === "disk" ? c.allocatedBytes : c.files),
    0,
  );
  let offset = 0;
  const segments = categories.map((category) => {
    const value = mode === "disk" ? category.allocatedBytes : category.files;
    const start = offset;
    offset += total ? (value / total) * 100 : 0;
    return `${colors[category.name]} ${start}% ${offset}%`;
  });
  return (
    <section className="panel distribution">
      <div className="panel-heading">
        <h2>What's taking space</h2>
        <div className="segmented" aria-label="Distribution measurement">
          <button
            aria-pressed={mode === "disk"}
            onClick={() => setMode("disk")}
          >
            Disk usage
          </button>
          <button
            aria-pressed={mode === "files"}
            onClick={() => setMode("files")}
          >
            File count
          </button>
        </div>
      </div>
      <div className="distribution-content">
        <div
          className="donut"
          role="img"
          aria-label={`File distribution by ${mode === "disk" ? "disk usage" : "file count"}; values listed alongside`}
          style={{
            background: total
              ? `conic-gradient(${segments.join(",")})`
              : "#30363b",
          }}
        >
          <div>
            <small>{mode === "disk" ? "ON DISK" : "FILES"}</small>
            <strong>
              {mode === "disk" ? bytes(total) : totalFiles.toLocaleString()}
            </strong>
          </div>
        </div>
        <div className="legend">
          {categories.map((category) => (
            <div key={category.name}>
              <i style={{ background: colors[category.name] }} />
              <span>{category.name}</span>
              <strong>
                {mode === "disk"
                  ? bytes(category.allocatedBytes)
                  : category.files.toLocaleString()}
              </strong>
            </div>
          ))}
          {categories.length === 0 && (
            <p className="muted">No files in this folder.</p>
          )}
        </div>
      </div>
    </section>
  );
}

export function GameDetails({
  game,
  busy,
  scan,
  back,
  openFolder,
  remove,
}: {
  game: Game;
  busy: boolean;
  scan: () => void;
  back: () => void;
  openFolder: () => void;
  remove: () => void;
}) {
  const [filter, setFilter] = useState("");
  const result = game.scan;
  const files =
    result?.largestFiles.filter((file) =>
      file.path.toLowerCase().includes(filter.toLowerCase()),
    ) ?? [];
  return (
    <>
      <button className="back-button" onClick={back}>
        <ArrowLeft size={16} /> Library
      </button>
      <div className="page-heading">
        <div>
          <div className="eyebrow">{game.launcher} installation</div>
          <h1>{game.name}</h1>
          <p className="path" title={game.path}>
            {game.path}
          </p>
        </div>
        <button
          className="primary"
          disabled={busy || game.missing}
          onClick={scan}
        >
          <RefreshCw size={17} />
          {result ? "Rescan" : "Analyse game"}
        </button>
      </div>
      {game.missing && (
        <div className="alert">
          <CircleAlert size={18} />
          This folder is unavailable. Reconnect the drive to scan it.
        </div>
      )}
      {result ? (
        <>
          <div className="detail-stats">
            <div>
              <span>
                Size on disk{result.allocationUnknown > 0 ? " (partial)" : ""}
              </span>
              <strong>{bytes(result.allocatedBytes)}</strong>
            </div>
            <div>
              <span>Logical size</span>
              <strong>{bytes(result.logicalBytes)}</strong>
            </div>
            <div>
              <span>Unique files</span>
              <strong>{result.files.toLocaleString()}</strong>
            </div>
          </div>
          {(result.skipped > 0 || result.allocationUnknown > 0) && (
            <div className="notice">
              <CircleAlert size={17} />
              <span>
                This scan is incomplete: {result.skipped} skipped items,{" "}
                {result.allocationUnknown} files with unknown disk usage.
                Details below.
              </span>
            </div>
          )}
          <Distribution
            categories={result.categories}
            totalFiles={result.files}
          />
          <section className="panel files-panel">
            <div className="panel-heading">
              <h2>
                Largest files{" "}
                <span className="count">{result.largestFiles.length}</span>
              </h2>
              <label className="search compact">
                <Search size={16} />
                <input
                  aria-label="Filter largest files"
                  placeholder="Find a file"
                  value={filter}
                  onChange={(e) => setFilter(e.target.value)}
                />
              </label>
            </div>
            <div className="table-scroll">
              <table>
                <thead>
                  <tr>
                    <th>File</th>
                    <th>Type</th>
                    <th>Logical size</th>
                    <th>On disk</th>
                  </tr>
                </thead>
                <tbody>
                  {files.map((file) => (
                    <tr key={file.path}>
                      <td title={file.path}>{file.path}</td>
                      <td>{file.category}</td>
                      <td>{bytes(file.logicalBytes)}</td>
                      <td>
                        {file.allocatedBytes === null
                          ? "Unavailable"
                          : bytes(file.allocatedBytes)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              {files.length === 0 && (
                <p className="empty-hint">No matching files.</p>
              )}
            </div>
          </section>
          <details className="disclosure">
            <summary>Scan details</summary>
            <p>
              Scanned {dateTime(result.completedAt)}. {result.duplicateLinks}{" "}
              duplicate hard links excluded. Archive contents are not inspected.
              Files can change after a scan.
            </p>
            {result.issues.map((issue, i) => (
              <p key={i} className="path">
                {issue}
              </p>
            ))}
            {result.skipped > result.issues.length && (
              <p>Only the first {result.issues.length} issues are listed.</p>
            )}
          </details>
        </>
      ) : (
        <div className="panel detail-empty">
          <HardDrive size={36} />
          <h2>See where your storage goes</h2>
          <p className="muted">
            Analyse this folder to see its disk usage and largest files.
          </p>
          <p className="small-note">
            Scanning reads your files without changing them.
          </p>
        </div>
      )}
      <div className="detail-footer">
        <button
          className="secondary"
          onClick={openFolder}
          disabled={game.missing}
        >
          <FolderOpen size={16} />
          Open folder
        </button>
        <button
          className="text-button danger-text"
          disabled={busy}
          onClick={remove}
        >
          <Trash2 size={15} />
          Remove from library
        </button>
      </div>
    </>
  );
}
