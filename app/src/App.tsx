import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Activity,
  ArrowRight,
  Check,
  ChevronRight,
  CircleAlert,
  Folder,
  FolderOpen,
  Gamepad2,
  HardDrive,
  Layers3,
  LoaderCircle,
  Plus,
  Search,
  Settings2,
  ShieldCheck,
  X,
  Zap,
} from "lucide-react";
import { api, activeJob, bytes, dateTime, desktop } from "./api";
import { AddGames } from "./AddGames";
import { GameDetails } from "./GameDetails";
import { Modal } from "./Modal";
import { CompressionDialog } from "./CompressionDialog";
import { Videos, VideoPreferences } from "./Videos";
import { Updates } from "./Updates";
import type { AppInfo, Game, Job } from "./types";
import "./App.css";

type View = "library" | "jobs" | "settings";

function App() {
  const [view, setView] = useState<View>("library");
  const [games, setGames] = useState<Game[]>([]);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(desktop);
  const [error, setError] = useState("");
  const [adding, setAdding] = useState(false);
  const [removing, setRemoving] = useState<Game | null>(null);
  const [working, setWorking] = useState(false);
  const [videos, setVideos] = useState<Game | null>(null);
  const [compression, setCompression] = useState<{
    game: Game;
    operation: "compress" | "decompress";
  } | null>(null);
  const selected = games.find((game) => game.id === selectedId);
  const running = jobs.find(activeJob);
  const reload = useCallback(async () => {
    if (!desktop) return;
    const [nextGames, nextJobs, nextInfo] = await Promise.all([
      api.games(),
      api.jobs(),
      api.info(),
    ]);
    setGames(nextGames);
    setJobs(nextJobs);
    setInfo(nextInfo);
  }, []);
  useEffect(() => {
    reload()
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [reload]);
  useEffect(() => {
    if (!running) return;
    let alive = true;
    let pending = false;
    const timer = setInterval(async () => {
      if (pending) return;
      pending = true;
      try {
        const next = await api.jobs();
        if (!alive) return;
        // Read the completed scan before dropping the active job, so this effect's
        // cleanup cannot discard the final library refresh.
        if (!next.some(activeJob)) {
          const refreshed = await api.games();
          if (!alive) return;
          setGames(refreshed);
        }
        setJobs(next);
      } catch (e) {
        if (alive) setError(String(e));
      } finally {
        pending = false;
      }
    }, 1000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [running?.id]);
  async function action(task: () => Promise<unknown>) {
    setWorking(true);
    setError("");
    try {
      await task();
      await reload();
    } catch (e) {
      setError(String(e));
    } finally {
      setWorking(false);
    }
  }
  const scanned = games.filter((game) => game.scan);
  const disk = scanned.reduce(
    (sum, game) => sum + (game.scan?.allocatedBytes ?? 0),
    0,
  );
  const partial = scanned.some(
    (game) =>
      game.scan && (game.scan.skipped > 0 || game.scan.allocationUnknown > 0),
  );
  const filtered = useMemo(
    () =>
      games.filter((game) =>
        `${game.name} ${game.launcher}`
          .toLowerCase()
          .includes(query.toLowerCase()),
      ),
    [games, query],
  );
  function navigate(next: View) {
    setView(next);
    setSelectedId(null);
  }
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <a
          className="brand"
          aria-label="GameZip library"
          href="#"
          onClick={(e) => {
            e.preventDefault();
            navigate("library");
          }}
        >
          <span className="brand-mark">
            <Zap size={22} fill="currentColor" />
          </span>
          GameZip<span className="brand-dot">.</span>
        </a>
        <div className="sidebar-label">WORKSPACE</div>
        <nav aria-label="Main navigation">
          <button
            className={view === "library" ? "active" : ""}
            aria-label="Library"
            aria-current={view === "library" ? "page" : undefined}
            onClick={() => navigate("library")}
          >
            <Layers3 size={19} />
            <span>Library</span>
            <span className="nav-count">{games.length}</span>
          </button>
          <button
            className={view === "jobs" ? "active" : ""}
            aria-label="Jobs"
            aria-current={view === "jobs" ? "page" : undefined}
            onClick={() => navigate("jobs")}
          >
            <Activity size={19} />
            <span>Jobs</span>
            {running && <i className="activity-dot" />}
          </button>
          <button
            className={view === "settings" ? "active" : ""}
            aria-label="Settings"
            aria-current={view === "settings" ? "page" : undefined}
            onClick={() => navigate("settings")}
          >
            <Settings2 size={19} />
            <span>Settings</span>
          </button>
        </nav>
        <div className="sidebar-bottom">
          <ShieldCheck size={17} />
          <div>
            On your device<small>Your files stay yours.</small>
          </div>
        </div>
      </aside>
      <div className="workspace">
        <header className="topbar">
          <span>
            WORKSPACE <ChevronRight size={12} />{" "}
            <strong>
              {selected
                ? "Game details"
                : view[0].toUpperCase() + view.slice(1)}
            </strong>
          </span>
          <span className="local-status">
            <i /> Local storage
          </span>
        </header>
        <main>
          <Updates settings={view === "settings"} busy={working || !!running} />
          {!desktop && (
            <div className="notice">
              <CircleAlert size={17} />
              Desktop preview. Open GameZip on Windows to access your game
              folders.
            </div>
          )}
          {error && (
            <div className="alert" role="alert">
              <CircleAlert size={18} />
              <span>{error}</span>
              <button
                className="icon-button"
                aria-label="Dismiss error"
                onClick={() => setError("")}
              >
                <X size={17} />
              </button>
            </div>
          )}
          {running && view !== "jobs" && (
            <button className="active-banner" onClick={() => navigate("jobs")}>
              <LoaderCircle size={17} className="spin" />
              <span>
                {running.state === "paused"
                  ? "Paused"
                  : running.state === "pausing"
                    ? "Pausing"
                    : running.operation === "compress"
                      ? "Compressing"
                      : running.operation === "decompress"
                        ? "Decompressing"
                        : running.operation === "video_preview"
                          ? "Previewing video for"
                          : "Scanning"}{" "}
                <strong>{running.gameName}</strong>
              </span>
              <small>{running.files.toLocaleString()} files</small>
              <ArrowRight size={16} />
            </button>
          )}
          {loading ? (
            <div className="loading">
              <LoaderCircle className="spin" />
              Opening your library…
            </div>
          ) : view === "library" ? (
            selected ? (
              <GameDetails
                key={selected.id}
                game={selected}
                videos={() => setVideos(selected)}
                busy={working || !!running}
                scan={() => action(() => api.scan(selected.id))}
                back={() => setSelectedId(null)}
                openFolder={() => action(() => api.openFolder(selected.id))}
                remove={() => setRemoving(selected)}
                compress={(operation) =>
                  setCompression({ game: selected, operation })
                }
              />
            ) : (
              <>
                <div className="page-heading">
                  <div>
                    <div className="eyebrow">YOUR GAMES. LESS GUESSWORK.</div>
                    <h1>Your library, measured.</h1>
                    <p className="muted">
                      Find what's taking space. Make room for what's next.
                    </p>
                  </div>
                  <button
                    className="primary"
                    disabled={!desktop}
                    onClick={() => setAdding(true)}
                  >
                    <Plus size={18} />
                    Add games
                  </button>
                </div>
                <section className="overview" aria-label="Library summary">
                  <div>
                    <span>
                      <Gamepad2 size={16} />
                      Installed games
                    </span>
                    <strong>
                      {games.length}
                      <small>in your library</small>
                    </strong>
                  </div>
                  <div>
                    <span>
                      <HardDrive size={16} />
                      Measured disk usage{partial ? " · partial" : ""}
                    </span>
                    <strong>
                      {scanned.length ? bytes(disk) : "—"}
                      <small>
                        {scanned.length} of {games.length} analysed
                      </small>
                    </strong>
                  </div>
                  <div className="overview-note">
                    <span className="mini-symbol">
                      <Layers3 size={22} />
                    </span>
                    <p>
                      A clearer view of
                      <br />
                      <strong>your game storage.</strong>
                    </p>
                  </div>
                </section>
                <div className="library-toolbar">
                  <h2>
                    Installed games{" "}
                    <span className="count">{games.length}</span>
                  </h2>
                  <div>
                    <label className="search">
                      <Search size={17} />
                      <input
                        placeholder="Search games"
                        aria-label="Search games"
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                      />
                    </label>
                    <span className="sort-label">Largest first</span>
                  </div>
                </div>
                {games.length === 0 ? (
                  <section className="empty-library">
                    <div className="empty-art" aria-hidden="true">
                      <div className="art-tile back">
                        <Folder size={32} />
                      </div>
                      <div className="art-tile front">
                        <Gamepad2 size={45} />
                        <span className="art-plus">
                          <Plus size={17} />
                        </span>
                      </div>
                      <i />
                      <i />
                    </div>
                    <div className="eyebrow">
                      A LITTLE SPACE GOES A LONG WAY
                    </div>
                    <h2>Start with your first game.</h2>
                    <p>
                      Add an installed game to see its size,
                      <br />
                      file breakdown, and largest files.
                    </p>
                    <button
                      className="primary"
                      disabled={!desktop}
                      onClick={() => setAdding(true)}
                    >
                      <Plus size={17} />
                      Add games
                    </button>
                    <span className="small-note">
                      Steam, Epic, or any game folder
                    </span>
                  </section>
                ) : filtered.length === 0 ? (
                  <div className="panel empty-hint">
                    No games match “{query}”.{" "}
                    <button
                      className="text-button"
                      onClick={() => setQuery("")}
                    >
                      Clear search
                    </button>
                  </div>
                ) : (
                  <div className="game-grid">
                    {filtered.map((game, index) => (
                      <article className="game-card" key={game.id}>
                        <button
                          className="card-main"
                          onClick={() => setSelectedId(game.id)}
                        >
                          <div className={`game-cover tone-${index % 4}`}>
                            <span className="cover-lines" />
                            <span className="monogram">
                              {game.name
                                .split(/\s+/)
                                .map((s) => s[0])
                                .slice(0, 2)
                                .join("")}
                            </span>
                            <span className="launcher-tag">
                              {game.launcher}
                            </span>
                          </div>
                          <div className="game-content">
                            <h3>{game.name}</h3>
                            <div className="game-measure">
                              <strong>
                                {game.scan
                                  ? bytes(game.scan.allocatedBytes)
                                  : "Not analysed"}
                              </strong>
                              <span>
                                {game.scan ? "on disk" : "Ready to scan"}
                              </span>
                            </div>
                            <div
                              className={`game-status ${game.missing ? "warning" : game.scan ? "scanned" : ""}`}
                            >
                              <i />
                              {game.missing
                                ? "Folder unavailable"
                                : running?.gameId === game.id
                                  ? running.state === "paused"
                                    ? "Paused"
                                    : "Working"
                                  : game.scan
                                    ? game.scan.skipped ||
                                      game.scan.allocationUnknown
                                      ? "Review scan details"
                                      : "Analysed"
                                    : "Scan needed"}
                            </div>
                          </div>
                        </button>
                        <div className="card-footer">
                          <button onClick={() => setSelectedId(game.id)}>
                            View details
                            <ArrowRight size={15} />
                          </button>
                          <button
                            className="icon-button"
                            aria-label={`Open ${game.name} folder`}
                            disabled={game.missing || working}
                            onClick={() =>
                              action(() => api.openFolder(game.id))
                            }
                          >
                            <FolderOpen size={17} />
                          </button>
                        </div>
                      </article>
                    ))}
                  </div>
                )}
              </>
            )
          ) : view === "jobs" ? (
            <>
              <div className="page-heading">
                <div>
                  <div className="eyebrow">ACTIVITY</div>
                  <h1>A little work. More clarity.</h1>
                  <p className="muted">
                    Jobs and their results, all in one place.
                  </p>
                </div>
              </div>
              {jobs.length === 0 ? (
                <div className="panel detail-empty">
                  <Activity size={34} />
                  <h2>All quiet here.</h2>
                  <p className="muted">
                    Analyse a game to start your first scan.
                  </p>
                  <button
                    className="secondary"
                    onClick={() => navigate("library")}
                  >
                    Go to library
                    <ArrowRight size={16} />
                  </button>
                </div>
              ) : (
                <div className="job-list">
                  {jobs.map((job) => (
                    <article className="panel job-card" key={job.id}>
                      <div
                        className={`job-icon ${activeJob(job) ? "cyan" : ""}`}
                      >
                        {activeJob(job) ? (
                          <LoaderCircle className="spin" size={20} />
                        ) : job.state === "completed" ? (
                          <Check size={20} />
                        ) : (
                          <CircleAlert size={20} />
                        )}
                      </div>
                      <div className="job-body">
                        <div className="job-title">
                          <h3>{job.gameName}</h3>
                          <span
                            className={`tag ${activeJob(job) ? "cyan" : ""}`}
                          >
                            {job.state}
                          </span>
                        </div>
                        <p>{job.message}</p>
                        <small>
                          {job.files.toLocaleString()}
                          {job.totalFiles != null
                            ? ` of ${job.totalFiles.toLocaleString()}`
                            : ""}{" "}
                          files ·{" "}
                          {job.operation === "compress"
                            ? `${bytes(Math.max(0, job.beforeBytes - job.afterBytes))} saved on processed files`
                            : job.operation === "decompress"
                              ? `${bytes(Math.max(0, job.afterBytes - job.beforeBytes))} added on processed files`
                              : job.operation === "video_preview"
                                ? `${bytes(job.afterBytes)} preview · original kept`
                                : `${bytes(job.logicalBytes)} measured`}{" "}
                          · {dateTime(job.startedAt)}
                        </small>
                        {activeJob(job) && job.state !== "paused" && (
                          <div
                            className="indeterminate"
                            aria-label="Job in progress"
                          >
                            <span />
                          </div>
                        )}
                      </div>
                      {activeJob(job) &&
                        ["compress", "decompress"].includes(job.operation) &&
                        job.state !== "stopping" && (
                          <button
                            className="secondary"
                            disabled={working}
                            onClick={() =>
                              action(() =>
                                api.pause(
                                  job.id,
                                  !["paused", "pausing"].includes(job.state),
                                ),
                              )
                            }
                          >
                            {["paused", "pausing"].includes(job.state)
                              ? "Resume"
                              : "Pause"}
                          </button>
                        )}
                      {job.operation === "video_preview" &&
                        job.state === "completed" && (
                          <button
                            className="secondary"
                            disabled={working}
                            onClick={() =>
                              action(() => api.openPreview(job.id))
                            }
                          >
                            Open preview
                          </button>
                        )}
                      {activeJob(job) ? (
                        <button
                          className="secondary"
                          disabled={working || job.state === "stopping"}
                          onClick={() => action(() => api.cancel(job.id))}
                        >
                          {job.state === "stopping" ? "Stopping…" : "Cancel"}
                        </button>
                      ) : (
                        games.some((g) => g.id === job.gameId) && (
                          <button
                            className="icon-button"
                            aria-label={`View ${job.gameName}`}
                            onClick={() => {
                              setView("library");
                              setSelectedId(job.gameId);
                            }}
                          >
                            <ArrowRight size={19} />
                          </button>
                        )
                      )}
                    </article>
                  ))}
                </div>
              )}
            </>
          ) : (
            <>
              <div className="page-heading">
                <div>
                  <div className="eyebrow">YOUR WORKSPACE</div>
                  <h1>Simple by design.</h1>
                  <p className="muted">
                    Local storage. No cloud account required.
                  </p>
                </div>
              </div>
              <section className="panel settings-panel">
                <div className="setting-row">
                  <span className="setting-icon">
                    <Activity />
                  </span>
                  <div>
                    <h3>Quiet compression</h3>
                    <p>
                      Give your PC a break between files. Jobs take longer; a
                      large file can still keep the disk busy.
                    </p>
                  </div>
                  <input
                    type="checkbox"
                    aria-label="Quiet compression"
                    checked={info?.performance?.quiet ?? true}
                    disabled={working || !desktop || !info}
                    onChange={(e) =>
                      action(() =>
                        api.performance({
                          quiet: e.target.checked,
                          idleOnly: info?.performance?.idleOnly ?? false,
                        }),
                      )
                    }
                  />
                </div>
                <div className="setting-row">
                  <span className="setting-icon">
                    <Activity />
                  </span>
                  <div>
                    <h3>Idle-only compression</h3>
                    <p>
                      Start the next file after two minutes without keyboard or
                      mouse input. Returning finishes the current file, then
                      waits.
                    </p>
                  </div>
                  <input
                    type="checkbox"
                    aria-label="Idle-only compression"
                    checked={info?.performance?.idleOnly ?? false}
                    disabled={working || !desktop || !info}
                    onChange={(e) =>
                      action(() =>
                        api.performance({
                          quiet: info?.performance?.quiet ?? true,
                          idleOnly: e.target.checked,
                        }),
                      )
                    }
                  />
                </div>
                <div className="setting-row">
                  <span className="setting-icon">
                    <HardDrive />
                  </span>
                  <div>
                    <h3>Stored on this device</h3>
                    <p>Your library and scan history are saved locally.</p>
                    <code>
                      {info?.databasePath ?? "Available in the desktop app"}
                    </code>
                  </div>
                </div>
                <div className="setting-row">
                  <span className="setting-icon">
                    <Activity />
                  </span>
                  <div>
                    <h3>Keep going in the background</h3>
                    <p>
                      Closing the window keeps GameZip in the system tray. Use
                      Quit GameZip in the tray menu to finish the current file
                      and exit.
                    </p>
                  </div>
                  <span className="tag green">Enabled</span>
                </div>
                <div className="setting-row">
                  <span className="setting-icon">
                    <ShieldCheck />
                  </span>
                  <div>
                    <h3>Your game comes first</h3>
                    <p>
                      Compression checks for running game processes and stops
                      after the current file if the game starts. Scanning only
                      reads files.
                    </p>
                  </div>
                </div>
              </section>
              <VideoPreferences />
              <div className="settings-version">
                <span className="brand-small">
                  <Zap size={16} />
                  GameZip
                </span>
                <span>Version {info?.version ?? "0.1.0"}</span>
              </div>
            </>
          )}
          <footer className="workspace-footer">
            <span>MORE ROOM TO PLAY.</span>
            <span>
              <span className="status-dot" />
              All data stays on this device
            </span>
          </footer>
        </main>
      </div>
      {adding && (
        <AddGames games={games} close={() => setAdding(false)} added={reload} />
      )}
      {compression && (
        <CompressionDialog
          game={compression.game}
          operation={compression.operation}
          close={() => setCompression(null)}
          started={async () => {
            await reload();
            navigate("jobs");
          }}
        />
      )}
      {videos && (
        <Videos
          game={videos}
          close={() => setVideos(null)}
          settings={() => {
            setVideos(null);
            navigate("settings");
          }}
          started={async () => {
            setVideos(null);
            await reload();
            navigate("jobs");
          }}
        />
      )}
      {removing && (
        <Modal
          title="Remove from library?"
          close={() => setRemoving(null)}
          busy={working}
        >
          <p>
            Remove <strong>{removing.name}</strong> from GameZip? Its game files
            and saves will stay on disk.
          </p>
          <div className="modal-footer">
            <button
              className="secondary"
              disabled={working}
              onClick={() => setRemoving(null)}
            >
              Keep game
            </button>
            <button
              className="danger"
              disabled={working}
              onClick={() =>
                action(async () => {
                  await api.remove(removing.id);
                  setRemoving(null);
                  setSelectedId(null);
                })
              }
            >
              Remove entry
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}

export default App;
