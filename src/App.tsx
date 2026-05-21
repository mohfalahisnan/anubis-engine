import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import GraphVisualizer, { GraphNode, QueryResult } from "./components/GraphVisualizer";
import IndexStatus from "./components/IndexStatus";
import KnowledgeBrowser, { DocumentRow } from "./components/KnowledgeBrowser";
import SearchBar from "./components/SearchBar";

export default function App() {
  const [selectedDocument, setSelectedDocument] = useState<DocumentRow | null>(null);
  const [selectedChunk, setSelectedChunk] = useState<QueryResult | null>(null);
  const [neighbors, setNeighbors] = useState<QueryResult[]>([]);
  const [graphLoading, setGraphLoading] = useState(false);
  const [graphError, setGraphError] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  async function loadGraphCenter(result: QueryResult) {
    setSelectedChunk(result);
    setNeighbors([]);
    setGraphLoading(true);
    setGraphError(null);

    try {
      const nextNeighbors = await invoke<QueryResult[]>("get_chunk_neighbors", {
        chunkId: result.chunk_id,
        depth: 2,
      });
      setNeighbors(nextNeighbors);
    } catch (reason) {
      setGraphError(String(reason));
    } finally {
      setGraphLoading(false);
    }
  }

  function handleRefocusNode(node: GraphNode) {
    loadGraphCenter({
      chunk_id: node.chunkId,
      doc_id: node.docId,
      content: node.content,
      filename: node.filename,
      page: node.page,
      score: node.score,
      score_bm25: node.scoreBm25,
      score_vec: node.scoreVec,
      score_graph: node.scoreGraph,
    });
  }

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand-block">
          <h1>Anubis OS</h1>
          <span>Knowledge Engine</span>
        </div>
        <IndexStatus onIndexed={() => setRefreshKey((value) => value + 1)} />
        <KnowledgeBrowser
          refreshKey={refreshKey}
          selectedId={selectedDocument?.id ?? null}
          onSelect={setSelectedDocument}
        />
      </aside>

      <section className="workspace">
        <SearchBar
          selectedChunkId={selectedChunk?.chunk_id ?? null}
          onSelectResult={loadGraphCenter}
        />
        <GraphVisualizer
          center={selectedChunk}
          neighbors={neighbors}
          loading={graphLoading}
          error={graphError}
          onRefocusNode={handleRefocusNode}
        />
      </section>
    </main>
  );
}
