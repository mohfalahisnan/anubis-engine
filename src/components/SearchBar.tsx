import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { Compass, Loader2, Search, Target } from "lucide-react";
import type { QueryResult } from "./GraphVisualizer";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { cn } from "../lib/utils";
import { useWorkdir } from "../contexts/WorkdirContext";

type Props = {
  selectedChunkId?: string | null;
  /// Fired when a user clicks a result card — highlight only, no refetch.
  onSelectResult: (result: QueryResult) => void;
  /// Fired after a query returns. App.tsx uses this to load the multi-seed
  /// graph constellation centered on the hits.
  onResults?: (results: QueryResult[], depth: 0 | 1 | 2) => void;
};

type SearchDepth = 0 | 1 | 2;

const DEPTH_PRESETS: {
  value: SearchDepth;
  label: string;
  hint: string;
  Icon: React.ComponentType<{ className?: string }>;
}[] = [
  {
    value: 0,
    label: "Precise",
    hint: "Direct text + vector matches only",
    Icon: Target,
  },
  {
    value: 1,
    label: "Default",
    hint: "Include 1-hop graph neighbors",
    Icon: Search,
  },
  {
    value: 2,
    label: "Discovery",
    hint: "2-hop expansion (broader context)",
    Icon: Compass,
  },
];

export default function SearchBar({
  selectedChunkId = null,
  onSelectResult,
  onResults,
}: Props) {
  const { activeWorkdir } = useWorkdir();
  const [queryText, setQueryText] = useState("");
  const [results, setResults] = useState<QueryResult[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [depth, setDepth] = useState<SearchDepth>(1);

  async function submit() {
    if (!queryText.trim() || !activeWorkdir) return;
    setBusy(true);
    setError(null);
    try {
      const nextResults = await invoke<QueryResult[]>("query", {
        workdir: activeWorkdir,
        q: queryText.trim(),
        limit: 10,
        depth,
      });
      setResults(nextResults);
      if (nextResults.length > 0) {
        onResults?.(nextResults, depth);
      }
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="relative flex gap-2">
        <div className="relative flex-1">
          <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-[var(--color-muted-foreground)]" />
          <Input
            value={queryText}
            onChange={(event) => setQueryText(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") submit();
            }}
            placeholder={
              activeWorkdir
                ? "Search — BM25 + vector + entity + graph"
                : "Pick a workdir to enable search"
            }
            disabled={!activeWorkdir}
            className="h-10 pl-9"
          />
        </div>
        <Button
          onClick={submit}
          disabled={busy || !queryText.trim() || !activeWorkdir}
          size="lg"
        >
          {busy ? (
            <>
              <Loader2 className="size-4 animate-spin" /> Searching
            </>
          ) : (
            <>
              <Search className="size-4" /> Search
            </>
          )}
        </Button>
      </div>

      <div className="flex items-center gap-1 text-[10px]">
        <span className="mr-1 uppercase tracking-wider text-[var(--color-muted-foreground)]">
          Depth
        </span>
        {DEPTH_PRESETS.map(({ value, label, hint, Icon }) => (
          <button
            key={value}
            type="button"
            onClick={() => setDepth(value)}
            title={hint}
            className={cn(
              "inline-flex items-center gap-1 rounded-md border px-2 py-1 transition-colors",
              depth === value
                ? "border-[var(--color-primary)] bg-[var(--color-primary)]/10 text-[var(--color-primary)]"
                : "border-[var(--color-border)] text-[var(--color-muted-foreground)] hover:bg-[var(--color-accent)]/60",
            )}
          >
            <Icon className="size-3" />
            <span className="font-medium">{label}</span>
            <span className="font-mono opacity-60">·{value}</span>
          </button>
        ))}
      </div>

      {results.length > 0 ? (
        <div className="flex max-h-44 gap-2 overflow-x-auto pb-1">
          {results.map((result) => (
            <button
              key={result.chunk_id}
              onClick={() => onSelectResult(result)}
              className={cn(
                "flex w-64 shrink-0 flex-col gap-1 rounded-lg border p-3 text-left transition-colors",
                result.chunk_id === selectedChunkId
                  ? "border-[var(--color-primary)] bg-[var(--color-accent)]"
                  : "border-[var(--color-border)] bg-[var(--color-card)] hover:bg-[var(--color-accent)]/60",
              )}
            >
              <div className="flex items-center justify-between gap-2">
                <span className="truncate text-xs font-semibold">
                  {result.filename}
                </span>
                <span className="rounded-md bg-[var(--color-primary)] px-1.5 py-0.5 text-[10px] font-mono text-[var(--color-primary-foreground)]">
                  {Math.round(result.score * 100)}%
                </span>
              </div>
              <p className="line-clamp-3 text-[11px] text-[var(--color-muted-foreground)]">
                {result.content}
              </p>
              <div
                className="flex gap-1 text-[9px] font-mono text-[var(--color-muted-foreground)]"
                title="V=Vector  B=BM25  G=Graph expansion  E=Entity  C=Centrality (tie-breaker, gated)"
              >
                <span>V {Math.round(result.score_vec * 100)}</span>
                <span>B {Math.round(result.score_bm25 * 100)}</span>
                <span>G {Math.round(result.score_graph * 100)}</span>
                <span>E {Math.round((result.score_entity ?? 0) * 100)}</span>
                <span>C {Math.round((result.score_centrality ?? 0) * 100)}</span>
              </div>
            </button>
          ))}
        </div>
      ) : null}

      {error ? (
        <div className="rounded-md border border-[var(--color-destructive)]/30 bg-[var(--color-destructive)]/10 px-3 py-2 text-xs text-[var(--color-destructive)]">
          {error}
        </div>
      ) : null}
    </div>
  );
}
