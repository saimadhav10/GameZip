import { useState } from "react";
import {
  ChevronRight,
  FolderOpen,
  Gamepad2,
  LoaderCircle,
  Plus,
} from "lucide-react";
import { api } from "./api";
import type { Candidate, Game } from "./types";
import { Modal } from "./Modal";

export function AddGames({
  games,
  close,
  added,
}: {
  games: Game[];
  close: () => void;
  added: () => Promise<void>;
}) {
  const [found, setFound] = useState<Candidate[] | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [warnings, setWarnings] = useState<string[]>([]);
  const existing = (path: string) =>
    games.some(
      (g) =>
        g.path.replace(/\//g, "\\").toLowerCase() ===
        path.replace(/\//g, "\\").toLowerCase(),
    );
  async function discover() {
    setBusy(true);
    setError("");
    try {
      const result = await api.discover();
      setFound(result.games);
      setWarnings(result.warnings);
      setSelected(
        new Set(
          result.games.filter((g) => !existing(g.path)).map((g) => g.path),
        ),
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function manual() {
    setBusy(true);
    setError("");
    try {
      const path = await api.pickFolder();
      if (typeof path === "string") {
        await api.add({ name: "", path, launcher: "Manual" });
        await added();
        close();
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function importGames() {
    setBusy(true);
    setError("");
    const failures: string[] = [];
    for (const game of found ?? []) {
      if (!selected.has(game.path)) continue;
      try {
        await api.add(game);
        setSelected((previous) => {
          const next = new Set(previous);
          next.delete(game.path);
          return next;
        });
      } catch (e) {
        failures.push(`${game.name}: ${String(e)}`);
      }
    }
    try {
      await added();
    } catch (e) {
      failures.push(String(e));
    }
    setBusy(false);
    if (failures.length) setError(failures.join("\n"));
    else close();
  }
  return (
    <Modal title="Add to your library" close={close} busy={busy}>
      <p className="muted">
        Find installed Steam and Epic games, or choose a game folder.
      </p>
      <div className="add-options">
        <button onClick={discover} disabled={busy}>
          <Gamepad2 />
          <span>
            Find installed games<small>Steam & Epic Games</small>
          </span>
          {busy ? (
            <LoaderCircle className="spin" size={18} />
          ) : (
            <ChevronRight size={18} />
          )}
        </button>
        <button onClick={manual} disabled={busy}>
          <FolderOpen />
          <span>
            Choose a folder<small>Any local installation</small>
          </span>
          <ChevronRight size={18} />
        </button>
      </div>
      {error && (
        <div className="alert" role="alert">
          {error}
        </div>
      )}
      {found && (
        <div className="discovery-list">
          {found.length === 0 && (
            <p className="empty-hint">
              No installed games found. You can still add a folder.
            </p>
          )}
          {found.map((game) => (
            <label key={game.path} className="discovery-item">
              <input
                type="checkbox"
                disabled={busy || existing(game.path)}
                checked={selected.has(game.path) && !existing(game.path)}
                onChange={(e) =>
                  setSelected((previous) => {
                    const next = new Set(previous);
                    if (e.target.checked) next.add(game.path);
                    else next.delete(game.path);
                    return next;
                  })
                }
              />
              <span>
                <strong>{game.name}</strong>
                <small title={game.path}>{game.path}</small>
              </span>
              <span className="tag">
                {existing(game.path) ? "Added" : game.launcher}
              </span>
            </label>
          ))}
        </div>
      )}
      {warnings.length > 0 && (
        <details className="disclosure">
          <summary>Some library information could not be read</summary>
          {warnings.map((warning, i) => (
            <p key={i}>{warning}</p>
          ))}
        </details>
      )}
      <div className="modal-footer">
        <button className="secondary" disabled={busy} onClick={close}>
          Cancel
        </button>
        {found && found.length > 0 && (
          <button
            className="primary"
            disabled={busy || selected.size === 0}
            onClick={importGames}
          >
            Add {selected.size} {selected.size === 1 ? "game" : "games"}
            <Plus size={17} />
          </button>
        )}
      </div>
    </Modal>
  );
}
