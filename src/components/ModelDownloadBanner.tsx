import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { CheckCircle2, Download, Loader2, TriangleAlert } from "lucide-react";
import { Progress } from "./ui/progress";

type Status = "starting" | "downloading" | "ready" | "error";

type ModelDownloadEvent = {
  id: string;
  label: string;
  status: Status;
  bytes_done?: number;
  bytes_total?: number;
  message?: string;
};

type EntryState = ModelDownloadEvent & {
  /// Epoch ms when we first saw this entry — used to keep "ready" rows on
  /// screen briefly before they disappear.
  firstSeenAt: number;
  /// Epoch ms when the entry transitioned to a terminal status.
  finishedAt?: number;
};

const STICKY_READY_MS = 3000;

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

export default function ModelDownloadBanner() {
  const [entries, setEntries] = useState<Record<string, EntryState>>({});

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let mounted = true;

    listen<ModelDownloadEvent>("model-download", (event) => {
      if (!mounted) return;
      const payload = event.payload;
      setEntries((current) => {
        const previous = current[payload.id];
        const now = Date.now();
        const next: EntryState = {
          ...payload,
          firstSeenAt: previous?.firstSeenAt ?? now,
          finishedAt:
            payload.status === "ready" || payload.status === "error"
              ? now
              : undefined,
        };
        return { ...current, [payload.id]: next };
      });
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {
        /* event listener failed — banner just stays empty */
      });

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  // Sweep finished entries off the banner after STICKY_READY_MS so the user
  // sees the "ready" tick briefly without it lingering forever.
  useEffect(() => {
    const interval = window.setInterval(() => {
      setEntries((current) => {
        const now = Date.now();
        let changed = false;
        const next: Record<string, EntryState> = {};
        for (const [id, entry] of Object.entries(current)) {
          if (
            entry.status === "ready" &&
            entry.finishedAt &&
            now - entry.finishedAt > STICKY_READY_MS
          ) {
            changed = true;
            continue;
          }
          next[id] = entry;
        }
        return changed ? next : current;
      });
    }, 1000);
    return () => window.clearInterval(interval);
  }, []);

  const visible = Object.values(entries).sort(
    (a, b) => a.firstSeenAt - b.firstSeenAt,
  );

  if (visible.length === 0) return null;

  return (
    <div className="pointer-events-none fixed inset-x-0 top-0 z-50 flex justify-center px-4 pt-3">
      <div className="pointer-events-auto w-full max-w-2xl space-y-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-card)]/95 p-3 shadow-xl backdrop-blur">
        {visible.map((entry) => (
          <ModelRow key={entry.id} entry={entry} />
        ))}
      </div>
    </div>
  );
}

function ModelRow({ entry }: { entry: EntryState }) {
  const pct =
    entry.bytes_total && entry.bytes_total > 0 && entry.bytes_done != null
      ? Math.min(100, Math.round((entry.bytes_done / entry.bytes_total) * 100))
      : null;

  const icon = (() => {
    switch (entry.status) {
      case "ready":
        return <CheckCircle2 className="size-4 text-emerald-500" />;
      case "error":
        return <TriangleAlert className="size-4 text-red-500" />;
      case "downloading":
        return <Download className="size-4 text-[var(--color-primary)]" />;
      default:
        return (
          <Loader2 className="size-4 animate-spin text-[var(--color-primary)]" />
        );
    }
  })();

  const subtitle = (() => {
    if (entry.status === "error") return entry.message ?? "Download failed";
    if (entry.status === "ready") return "Ready";
    if (entry.status === "downloading") {
      if (pct != null) {
        const done = formatBytes(entry.bytes_done ?? 0);
        const total = formatBytes(entry.bytes_total ?? 0);
        return `${done} / ${total} · ${pct}%`;
      }
      if (entry.bytes_done != null) {
        return `${formatBytes(entry.bytes_done)} downloaded`;
      }
      return "Downloading…";
    }
    return "Starting…";
  })();

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2">
        {icon}
        <span className="flex-1 truncate text-sm font-medium">
          {entry.label}
        </span>
        <span className="text-[11px] text-[var(--color-muted-foreground)]">
          {subtitle}
        </span>
      </div>
      {entry.status === "downloading" || entry.status === "starting" ? (
        <Progress value={pct ?? undefined} indeterminate={pct == null} />
      ) : null}
    </div>
  );
}
