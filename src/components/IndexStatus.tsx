import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

type Stats = {
  documents?: number;
  chunks?: number;
  graph_edges?: number;
  last_indexed?: string | null;
};

type Progress = {
  total: number;
  done: number;
  current: string;
  status: "idle" | "running" | "done" | "error";
  errors: string[];
};

type Props = {
  onIndexed: () => void;
};

export default function IndexStatus({ onIndexed }: Props) {
  const [path, setPath] = useState("");
  const [stats, setStats] = useState<Stats>({});
  const [progress, setProgress] = useState<Progress | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function loadStats() {
    const nextStats = await invoke<Stats>("get_index_stats");
    setStats(nextStats);
  }

  useEffect(() => {
    loadStats().catch((reason) => setError(String(reason)));
    const unlisten = listen<Progress>("index-progress", (event) => {
      setProgress(event.payload);
      if (event.payload.status === "done") {
        setBusy(false);
        loadStats().then(onIndexed).catch((reason) => setError(String(reason)));
      }
      if (event.payload.status === "error") {
        setBusy(false);
      }
    });
    return () => {
      unlisten.then((dispose) => dispose()).catch(() => undefined);
    };
  }, [onIndexed]);

  async function indexFolder() {
    if (!path.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await invoke("index_folder", { path: path.trim() });
      await loadStats();
      onIndexed();
    } catch (reason) {
      setBusy(false);
      setError(String(reason));
    }
  }

  return (
    <section className="panel">
      <div className="stats-grid">
        <Stat label="Docs" value={stats.documents ?? 0} />
        <Stat label="Chunks" value={stats.chunks ?? 0} />
        <Stat label="Edges" value={stats.graph_edges ?? 0} />
      </div>
      <div className="index-row">
        <input
          value={path}
          onChange={(event) => setPath(event.target.value)}
          placeholder="Folder path"
        />
        <button onClick={indexFolder} disabled={busy || !path.trim()}>
          {busy ? "Indexing" : "Index"}
        </button>
      </div>
      {progress ? (
        <div className="progress-line">
          {progress.status} {progress.done}/{progress.total} {progress.current}
        </div>
      ) : null}
      {error ? <div className="error-line">{error}</div> : null}
    </section>
  );
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div className="stat">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
