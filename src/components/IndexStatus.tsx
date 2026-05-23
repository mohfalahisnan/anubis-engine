import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  CircleStop,
  FolderOpen,
  Loader2,
  Mic2,
  MicOff,
  Play,
  RefreshCw,
  Trash2,
} from "lucide-react";
import { Button } from "./ui/button";
import { Progress } from "./ui/progress";
import { Badge } from "./ui/badge";
import { cn } from "../lib/utils";
import { useWorkdir } from "../contexts/WorkdirContext";

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
  status: "idle" | "running" | "done" | "error" | "cancelled";
  errors: string[];
  stage?: "parsing" | "embedding" | "writing" | "linking";
  workdir_id?: string | null;
};

type PreprocessProgress = {
  total: number;
  done: number;
  current: string;
  status: "idle" | "running" | "done" | "error" | "cancelled";
  errors: string[];
  kind?: "video" | "audio" | "image" | "pdf";
  stage?: "transcribing" | "ocr" | "cachedskipped" | "cached_skipped";
  workdir_id?: string | null;
};

type Props = {
  onIndexed: () => void;
  onCleared: () => void;
};

type Settings = {
  transcription_enabled: boolean;
};

export default function IndexStatus({ onIndexed, onCleared }: Props) {
  const { activeWorkdir, activeWorkdirId, refreshKnownWorkdirs } = useWorkdir();
  const [path, setPath] = useState("");
  const [stats, setStats] = useState<Stats>({});
  const [progress, setProgress] = useState<Progress | null>(null);
  const [preprocessProgress, setPreprocessProgress] =
    useState<PreprocessProgress | null>(null);
  const [busy, setBusy] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [transcribeEnabled, setTranscribeEnabled] = useState(true);
  const [savingTranscribe, setSavingTranscribe] = useState(false);

  async function loadStats() {
    if (!activeWorkdir) {
      setStats({});
      return;
    }
    try {
      const nextStats = await invoke<Stats>("get_index_stats", {
        workdir: activeWorkdir,
      });
      setStats(nextStats);
      setError(null);
    } catch (reason) {
      const message = String(reason);
      // Engine bootstrap is asynchronous — suppress the noisy banner while
      // we wait for first-run setup (downloads, migration) to finish. The
      // ModelDownloadBanner already shows what the app is doing.
      if (!message.toLowerCase().includes("still initialising")) {
        setError(message);
      }
    }
  }

  // Poll engine_ready until the backend is up, then load stats. The bootstrap
  // window can be many seconds on first run (model downloads).
  useEffect(() => {
    let cancelled = false;
    async function waitForReady() {
      while (!cancelled) {
        try {
          const ready = await invoke<boolean>("engine_ready");
          if (ready) {
            await loadStats();
            await loadSettings();
            return;
          }
        } catch {
          // Command not yet registered — keep trying.
        }
        await new Promise((resolve) => setTimeout(resolve, 700));
      }
    }
    void waitForReady();
    return () => {
      cancelled = true;
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeWorkdir]);

  async function loadSettings() {
    try {
      const next = await invoke<Settings>("get_settings");
      setTranscribeEnabled(!!next.transcription_enabled);
    } catch {
      // Optional — surface no error if the backend is older than this command.
    }
  }

  async function toggleTranscribe() {
    if (savingTranscribe) return;
    const next = !transcribeEnabled;
    setTranscribeEnabled(next); // optimistic — feels snappier
    setSavingTranscribe(true);
    try {
      await invoke("set_transcription_enabled", { enabled: next });
    } catch (reason) {
      setTranscribeEnabled(!next); // rollback
      setError(String(reason));
    } finally {
      setSavingTranscribe(false);
    }
  }

  useEffect(() => {
    const unlisten = listen<Progress>("index-progress", (event) => {
      // Drop events from other workdirs so concurrent indexing doesn't
      // mix progress bars.
      if (
        event.payload.workdir_id &&
        activeWorkdirId &&
        event.payload.workdir_id !== activeWorkdirId
      ) {
        return;
      }
      setProgress(event.payload);
      if (event.payload.status === "running") {
        setPreprocessProgress(null);
      }
      if (event.payload.status === "done") {
        setBusy(false);
        loadStats().then(onIndexed);
      }
      if (event.payload.status === "error") {
        setBusy(false);
        setError(event.payload.errors.join("\n") || "Indexing failed");
      }
      if (event.payload.status === "cancelled") {
        setBusy(false);
        loadStats().then(onIndexed);
      }
    });
    return () => {
      unlisten.then((dispose) => dispose()).catch(() => undefined);
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [onIndexed, activeWorkdirId]);

  useEffect(() => {
    const unlisten = listen<PreprocessProgress>("preprocess-progress", (event) => {
      if (
        event.payload.workdir_id &&
        activeWorkdirId &&
        event.payload.workdir_id !== activeWorkdirId
      ) {
        return;
      }
      setPreprocessProgress(event.payload);
      if (event.payload.status === "cancelled") {
        setBusy(false);
        loadStats().then(onIndexed);
      }
    });
    return () => {
      unlisten.then((dispose) => dispose()).catch(() => undefined);
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [onIndexed, activeWorkdirId]);

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
    if (!path.trim() || !activeWorkdir) return;
    setBusy(true);
    setError(null);
    setPreprocessProgress(null);
    setProgress(null);
    try {
      await invoke("index_folder", { workdir: activeWorkdir, path: path.trim() });
      setBusy(false);
      await loadStats();
      // Pick up the newly-registered storage dir in the workdir picker.
      void refreshKnownWorkdirs();
      onIndexed();
    } catch (reason) {
      setBusy(false);
      setError(String(reason));
    }
  }

  async function cancelIndexing() {
    if (!busy || !activeWorkdir) return;
    try {
      await invoke("cancel_indexing", { workdir: activeWorkdir });
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function clearIndex() {
    if (clearing || !activeWorkdir) return;
    const confirmed = window.confirm(
      "Clear all indexed data? This removes every document, chunk, vector, and graph edge. Source files stay untouched.",
    );
    if (!confirmed) return;
    setClearing(true);
    setError(null);
    try {
      await invoke("reset_index", { workdir: activeWorkdir });
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
  const preprocessPct = preprocessProgress && preprocessProgress.total > 0
    ? Math.round((preprocessProgress.done / preprocessProgress.total) * 100)
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

        <button
          type="button"
          onClick={toggleTranscribe}
          disabled={savingTranscribe}
          className="flex w-full items-center gap-2 rounded-md border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-left text-xs transition-colors hover:bg-[var(--color-accent)] disabled:opacity-60"
          title={
            transcribeEnabled
              ? "Click to skip Whisper transcription for video & audio files"
              : "Click to enable Whisper transcription for video & audio files"
          }
        >
          {transcribeEnabled ? (
            <Mic2 className="size-4 shrink-0 text-[var(--color-primary)]" />
          ) : (
            <MicOff className="size-4 shrink-0 text-[var(--color-muted-foreground)]" />
          )}
          <div className="flex-1">
            <div className="font-medium">
              Transcribe video &amp; audio
            </div>
            <div className="text-[10px] text-[var(--color-muted-foreground)]">
              {transcribeEnabled
                ? "On — runs Whisper on indexed media"
                : "Off — media files indexed as metadata only"}
            </div>
          </div>
          <span
            className={cn(
              "flex h-5 w-9 items-center rounded-full border transition-colors",
              transcribeEnabled
                ? "border-[var(--color-primary)]/40 bg-[var(--color-primary)]/30"
                : "border-[var(--color-border)] bg-[var(--color-muted)]/60",
            )}
          >
            <span
              className={cn(
                "h-4 w-4 rounded-full bg-[var(--color-card)] shadow-sm transition-transform",
                transcribeEnabled ? "translate-x-4" : "translate-x-0.5",
              )}
            />
          </span>
        </button>

        <div className="flex gap-2">
          <Button
            onClick={indexFolder}
            disabled={busy || !path.trim() || !activeWorkdir}
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
            disabled={clearing || busy || !hasIndex || !activeWorkdir}
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
          <Button
            onClick={cancelIndexing}
            disabled={!busy}
            variant="outline"
            size="sm"
            title="Stop indexing after the current step"
          >
            <CircleStop className="size-4" />
            Cancel
          </Button>
        </div>
      </div>

      {preprocessProgress && preprocessProgress.status !== "idle" ? (
        <ProgressPanel
          label="Preprocessing"
          progress={preprocessProgress}
          value={preprocessPct}
          detail={preprocessDetail(preprocessProgress)}
        />
      ) : null}

      {progress && progress.status !== "idle" ? (
        <ProgressPanel
          label="Indexing"
          progress={progress}
          value={pct}
          detail={indexDetail(progress)}
        />
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

function ProgressPanel({
  label,
  progress,
  value,
  detail,
}: {
  label: string;
  progress: Pick<Progress, "total" | "done" | "current" | "status">;
  value: number;
  detail?: string;
}) {
  return (
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
          <span className="font-medium">{label}</span>
          <span className="text-[var(--color-muted-foreground)]">
            {statusLabel(progress.status)}
          </span>
        </div>
        <span className="font-mono text-[var(--color-muted-foreground)]">
          {progress.done}/{progress.total}
        </span>
      </div>
      <Progress value={value} />
      {detail || progress.current ? (
        <p className="truncate text-[11px] text-[var(--color-muted-foreground)]">
          {[detail, progress.current].filter(Boolean).join(" - ")}
        </p>
      ) : null}
    </div>
  );
}

function statusLabel(status: Progress["status"]) {
  if (status === "done") return "done";
  if (status === "cancelled") return "cancelled";
  if (status === "error") return "failed";
  return "running";
}

function indexDetail(progress: Progress) {
  const labels: Record<NonNullable<Progress["stage"]>, string> = {
    parsing: "parsing",
    embedding: "embedding",
    writing: "writing",
    linking: "linking",
  };
  return progress.stage ? labels[progress.stage] : undefined;
}

function preprocessDetail(progress: PreprocessProgress) {
  if (progress.stage === "cachedskipped" || progress.stage === "cached_skipped") {
    return "cached";
  }
  if (progress.stage === "transcribing") return "transcribing";
  if (progress.stage === "ocr") return "reading image text";
  return progress.kind;
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
