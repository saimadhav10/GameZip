import { useEffect, useState } from "react";
import { api, desktop } from "./api";
import { Modal } from "./Modal";
import type { UpdateStatus } from "./types";

export function Updates({
  settings,
  busy,
}: {
  settings: boolean;
  busy: boolean;
}) {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [working, setWorking] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    if (!desktop) return;
    let alive = true;
    const refresh = () =>
      api
        .updateStatus()
        .then((value) => {
          if (alive) setStatus(value);
        })
        .catch(() => {});
    void refresh();
    const timer = setInterval(refresh, 30_000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);
  async function check() {
    setWorking(true);
    setError("");
    try {
      setStatus(await api.checkUpdate());
    } catch (e) {
      setError(String(e));
    } finally {
      setWorking(false);
    }
  }
  async function install() {
    if (!status?.version) return;
    setWorking(true);
    setError("");
    try {
      await api.installUpdate(status.version);
      setStatus(await api.updateStatus());
    } catch (e) {
      setError(String(e));
      setStatus(await api.updateStatus());
    } finally {
      setWorking(false);
    }
  }
  const available = status?.state === "available";
  return (
    <>
      {(settings || available) && (
        <section className="panel update-panel" aria-label="App updates">
          <div>
            <strong>
              {available
                ? `GameZip ${status.version} is available`
                : "App updates"}
            </strong>
            <p className="muted">
              {status?.message ?? "Checking update status…"}
            </p>
          </div>
          {available ? (
            <button
              className="secondary"
              disabled={busy || working}
              onClick={() => setConfirm(true)}
            >
              Review update
            </button>
          ) : (
            <button
              className="secondary"
              disabled={
                !desktop ||
                working ||
                status?.state === "checking" ||
                status?.state === "installing"
              }
              onClick={check}
            >
              {working || status?.state === "checking"
                ? "Checking…"
                : "Check for updates"}
            </button>
          )}
          {available && busy && (
            <small className="muted">
              Finish or cancel the current job first.
            </small>
          )}
          {settings && (
            <small className="muted">
              Checks at startup and every six hours. Installation is your
              choice.
            </small>
          )}
          {error && !confirm && (
            <p role="alert" className="alert">
              {error}
            </p>
          )}
        </section>
      )}
      {confirm && (
        <Modal
          title={`Install GameZip ${status?.version ?? "update"}?`}
          close={() => setConfirm(false)}
          busy={working}
        >
          <p>
            The update will be downloaded, its signature verified, and GameZip
            restarted.
          </p>
          {error && (
            <p className="alert" role="alert">
              {error}
            </p>
          )}
          {working && (
            <p role="status">Downloading and verifying the update…</p>
          )}
          <div className="modal-footer">
            <button
              className="secondary"
              disabled={working}
              onClick={() => setConfirm(false)}
            >
              Later
            </button>
            <button
              className="primary"
              disabled={busy || working || !available}
              onClick={install}
            >
              Install and restart
            </button>
          </div>
        </Modal>
      )}
    </>
  );
}
