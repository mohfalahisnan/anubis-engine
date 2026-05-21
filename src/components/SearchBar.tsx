import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import type { QueryResult } from "./GraphVisualizer";

type Props = {
  selectedChunkId?: string | null;
  onSelectResult: (result: QueryResult) => void;
};

export default function SearchBar({ selectedChunkId = null, onSelectResult }: Props) {
  const [queryText, setQueryText] = useState("");
  const [results, setResults] = useState<QueryResult[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    if (!queryText.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const nextResults = await invoke<QueryResult[]>("query", {
        q: queryText.trim(),
        limit: 10,
      });
      setResults(nextResults);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="search-panel">
      <div className="search-row">
        <input
          value={queryText}
          onChange={(event) => setQueryText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") submit();
          }}
          placeholder="Search indexed knowledge"
        />
        <button onClick={submit} disabled={busy || !queryText.trim()}>
          Search
        </button>
      </div>
      <div className="results-list">
        {results.map((result) => (
          <button
            key={result.chunk_id}
            className={
              result.chunk_id === selectedChunkId ? "result-row selected" : "result-row"
            }
            onClick={() => onSelectResult(result)}
          >
            <span>{result.filename}</span>
            <p>{result.content}</p>
            <small>{Math.round(result.score * 100)}%</small>
          </button>
        ))}
      </div>
      {error ? <div className="error-line">{error}</div> : null}
    </section>
  );
}
