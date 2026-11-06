import { useEffect, useState } from "react";
import { CircleAlert, LoaderCircle } from "lucide-react";
import { api, bytes } from "./api";
import { Modal } from "./Modal";
import type { CompressionPreview, Game } from "./types";

export function CompressionDialog({
  game,
  operation,
  close,
  started,
}: {
  game: Game;
  operation: "compress" | "decompress";
  close: () => void;
  started: () => Promise<void>;
}) {
  const [preview, setPreview] = useState<CompressionPreview | null>(null);
  const [error, setError] = useState("");
  const [starting, setStarting] = useState(false);
  const [retry, setRetry] = useState(0);
  const [strength, setStrength] = useState<"fast" | "balanced" | "maximum">(
    "balanced",
  );
  const verb = operation === "compress" ? "Compress" : "Decompress";
  useEffect(() => {
    let alive = true;
    setPreview(null);
    setError("");
    api
      .prepareCompression(game.id, operation, strength)
      .then((result) => {
        if (alive) setPreview(result);
      })
      .catch((error) => {
        if (alive) setError(String(error));
      });
    return () => {
      alive = false;
    };
  }, [game.id, operation, retry, strength]);
  const enoughSpace = preview && preview.freeBytes >= preview.requiredFreeBytes;
  return (
    <Modal title={`${verb} ${game.name}?`} close={close} busy={starting}>
      {operation === "compress" && (
        <div className="segmented" aria-label="Compression strength">
          {(["fast", "balanced", "maximum"] as const).map((value) => (
            <button
              key={value}
              disabled={starting}
              aria-pressed={strength === value}
              onClick={() => {
                if (value === strength) return;
                setPreview(null);
                setStrength(value);
              }}
            >
              {value[0].toUpperCase() + value.slice(1)}
            </button>
          ))}
        </div>
      )}
      {error && (
        <div className="alert" role="alert">
          <CircleAlert size={18} />
          <span>{error}</span>
        </div>
      )}
      {!preview && !error && (
        <p className="compression-loading" role="status">
          <LoaderCircle className="spin" size={18} /> Checking files and
          available space…
        </p>
      )}
      {preview && (
        <>
          <div className="compression-summary">
            <strong>{preview.files.toLocaleString()} files</strong>
            <span>{bytes(preview.allocatedBytes)} on disk</span>
          </div>
          <p className="muted">
            {operation === "compress"
              ? `${strength === "fast" ? "Faster compression, usually smaller savings." : strength === "balanced" ? "A balance of compression speed and savings." : "Maximum compression, with more processing time."} Actual savings appear after the job.`
              : "Remove compression and return these files to their uncompressed size."}
          </p>
          {preview.files === 0 ? (
            <p className="notice">No eligible files need this operation.</p>
          ) : (
            !enoughSpace && (
              <p className="alert">
                This operation needs {bytes(preview.requiredFreeBytes)} free.
                Only {bytes(preview.freeBytes)} is available.
              </p>
            )
          )}
          <p className="small-note">
            Keep the game closed. If it starts, GameZip stops after the current
            file. Cancel does the same.
          </p>
          <details className="disclosure">
            <summary>Details</summary>
            <p>
              {preview.skipped.toLocaleString()} files or folders excluded or
              already in the requested state. Shared files, links, and
              unsupported files are skipped.
            </p>
            <p>
              Completed files keep their new compression state if you stop. You
              can {operation === "compress" ? "decompress" : "compress"} them
              later.
            </p>
            {operation === "compress" && (
              <p>
                Already compressed files are skipped. Decompress first if you
                want to change their compression strength.
              </p>
            )}
          </details>
        </>
      )}
      <div className="modal-footer">
        <button className="secondary" disabled={starting} onClick={close}>
          Close
        </button>
        {error ? (
          <button
            className="primary"
            disabled={starting}
            onClick={() => setRetry((value) => value + 1)}
          >
            Check again
          </button>
        ) : (
          <button
            className="primary"
            disabled={!preview?.files || !enoughSpace || starting}
            onClick={async () => {
              if (!preview) return;
              setStarting(true);
              try {
                await api.compress(preview.token);
                await started();
                close();
              } catch (error) {
                setError(String(error));
              } finally {
                setStarting(false);
              }
            }}
          >
            {starting
              ? "Starting…"
              : `Start ${operation === "compress" ? "compression" : "decompression"}`}
          </button>
        )}
      </div>
    </Modal>
  );
}
