import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  FolderOpen,
  Loader2,
  Play,
  RefreshCw,
  Trash2,
} from "lucide-react";
import { Button } from "./ui/button";
import { Progress } from "./ui/progress";
import { Badge } from "./ui/badge";

type Stats = {
  documents?: number;
  chunks?: number;
  graph_edges?: number;
  entities?: number;
  edges_by_type?: Record<string, number>;
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
  onCleared: () => void;
};

export default function IndexStatus({ onIndexed, onCleared }: Props) {
  const [path, setPath] = useState("");
  const [stats, setStats] = useState<Stats>({});
  const [progress, setProgress] = useState<Progress | null>(null);
  const [busy, setBusy] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function loadStats() {
    try {
      const nextStats = await invoke<Stats>("get_index_stats");
      setStats(nextStats);
    } catch (reason) {
      setError(String(reason));
    }
  }

  useEffect(() => {
    loadStats();
    const unlisten = listen<Progress>("index-progress", (event) => {
      setProgress(event.payload);
      if (event.payload.status === "done") {
        setBusy(false);
        loadStats().then(onIndexed);
      }
      if (event.payload.status === "error") {
        setBusy(false);
        setError(event.payload.errors.join("\n") || "Indexing failed");
      }
    });
    return () => {
      unlisten.then((dispose) => dispose()).catch(() => undefined);
    };
  }, [onIndexed]);

  async function pickFolder() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select knowledge folder",
      });
      if (typeof selected === "string") {
        setPath(selected);
      }
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function indexFolder() {
    if (!path.trim()) return;
    setBusy(true);
    setError(null);
    setProgress({ total: 0, done: 0, current: "", status: "running", errors: [] });
    try {
      await invoke("index_folder", { path: path.trim() });
      await loadStats();
      onIndexed();
    } catch (reason) {
      setBusy(false);
      setError(String(reason));
    }
  }

  async function clearIndex() {
    if (clearing) return;
    const confirmed = window.confirm(
      "Clear all indexed data? This removes every document, chunk, vector, and graph edge. Source files stay untouched.",
    );
    if (!confirmed) return;
    setClearing(true);
    setError(null);
    try {
      await invoke("reset_index");
      setProgress(null);
      await loadStats();
      onCleared();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setClearing(false);
    }
  }

  const hasIndex = (stats.documents ?? 0) > 0;
  const pct = progress && progress.total > 0
    ? Math.round((progress.done / progress.total) * 100)
    : 0;

  return (
    <div className="flex flex-col gap-4">
      <div className="grid grid-cols-3 gap-2">
        <Stat label="Docs" value={stats.documents ?? 0} />
        <Stat label="Chunks" value={stats.chunks ?? 0} />
        <Stat label="Edges (DB)" value={stats.graph_edges ?? 0} />
      </div>

      {stats.edges_by_type && Object.keys(stats.edges_by_type).length > 0 ? (
        <div className="space-y-1.5">
          <div className="text-[10px] uppercase tracking-wider text-[var(--color-muted-foreground)]">
            Edges by type
          </div>
          <div className="flex flex-wrap gap-1.5">
            {Object.entries(stats.edges_by_type)
              .sort((a, b) => b[1] - a[1])
              .map(([type, count]) => (
                <span
                  key={type}
                  className="inline-flex items-center gap-1 rounded-md border border-[var(--color-border)] bg-[var(--color-card)] px-1.5 py-0.5 text-[10px]"
                >
                  <span className="text-[var(--color-muted-foreground)]">
                    {type}
                  </span>
                  <span className="font-mono font-semibold tabular-nums">
                    {count.toLocaleString()}
                  </span>
                </span>
              ))}
            {stats.entities ? (
              <span className="inline-flex items-center gap-1 rounded-md border border-[var(--color-border)] bg-[var(--color-card)] px-1.5 py-0.5 text-[10px]">
                <span className="text-[var(--color-muted-foreground)]">
                  entities
                </span>
                <span className="font-mono font-semibold tabular-nums">
                  {stats.entities.toLocaleString()}
                </span>
              </span>
            ) : null}
          </div>
        </div>
      ) : null}

      <div className="space-y-2">
        <label className="text-xs font-medium text-[var(--color-muted-foreground)]">
          Knowledge folder
        </label>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={pickFolder}
            className="flex-1 flex items-center gap-2 rounded-md border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-left text-xs transition-colors hover:bg-[var(--color-accent)]"
          >
            <FolderOpen className="size-4 shrink-0 text-[var(--color-muted-foreground)]" />
            <span className="truncate">
              {path || (
                <span className="text-[var(--color-muted-foreground)]">
                  Choose folder…
                </span>
              )}
            </span>
          </button>
        </div>

        <div className="flex gap-2">
          <Button
            onClick={indexFolder}
            disabled={busy || !path.trim()}
            className="flex-1"
            size="sm"
          >
            {busy ? (
              <>
                <Loader2 className="size-4 animate-spin" /> Indexing
              </>
            ) : (
              <>
                {hasIndex ? (
                  <RefreshCw className="size-4" />
                ) : (
                  <Play className="size-4" />
                )}
                {hasIndex ? "Reindex" : "Index"}
              </>
            )}
          </Button>
          <Button
            onClick={clearIndex}
            disabled={clearing || busy || !hasIndex}
            variant="outline"
            size="sm"
            title="Clear all indexed data"
          >
            {clearing ? (
              <Loader2 className="size-4 animate-spin" />
            ) : (
              <Trash2 className="size-4" />
            )}
            Clear
          </Button>
        </div>
      </div>

      {progress && progress.status !== "idle" ? (
        <div className="space-y-2 rounded-lg border border-[var(--color-border)] bg-[var(--color-muted)]/40 p-3">
          <div className="flex items-center justify-between text-xs">
            <div className="flex items-center gap-2">
              {progress.status === "running" ? (
                <Loader2 className="size-3.5 animate-spin text-[var(--color-primary)]" />
              ) : progress.status === "done" ? (
                <CheckCircle2 className="size-3.5 text-[var(--color-success)]" />
              ) : (
                <AlertTriangle className="size-3.5 text-[var(--color-destructive)]" />
              )}
              <span className="font-medium capitalize">{progress.status}</span>
            </div>
            <span className="font-mono text-[var(--color-muted-foreground)]">
              {progress.done}/{progress.total}
            </span>
          </div>
          <Progress value={pct} />
          {progress.current ? (
            <p className="truncate text-[11px] text-[var(--color-muted-foreground)]">
              {progress.current}
            </p>
          ) : null}
        </div>
      ) : null}

      {error ? (
        <div className="flex items-start gap-2 rounded-md border border-[var(--color-destructive)]/30 bg-[var(--color-destructive)]/10 p-2 text-xs text-[var(--color-destructive)]">
          <AlertTriangle className="size-4 shrink-0" />
          <span className="break-all">{error}</span>
        </div>
      ) : null}
    </div>
  );
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] p-3">
      <div className="text-[10px] uppercase tracking-wider text-[var(--color-muted-foreground)]">
        {label}
      </div>
      <div className="mt-1 text-xl font-semibold tabular-nums">
        {value.toLocaleString()}
      </div>
    </div>
  );
}

export function StatusBadge({ status }: { status: string }) {
  const variant =
    status === "indexed"
      ? "success"
      : status === "error"
        ? "destructive"
        : "secondary";
  return <Badge variant={variant as any}>{status}</Badge>;
}
