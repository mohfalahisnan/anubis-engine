import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { FolderSearch, Sparkles } from "lucide-react";
import GraphVisualizer, {
  focusToGraphData,
  GraphData,
  GraphNode,
  GraphOverviewPayload,
  neighborhoodToGraphData,
  overviewToGraphData,
  QueryResult,
  searchToGraphData,
} from "./components/GraphVisualizer";
import IndexStatus from "./components/IndexStatus";
import KnowledgeBrowser, { DocumentRow } from "./components/KnowledgeBrowser";
import SearchBar from "./components/SearchBar";
import { Separator } from "./components/ui/separator";

type Chunk = {
  id: string;
  doc_id: string;
  content: string;
  page?: number | null;
};

type Mode = "global" | "focus" | "search";

export default function App() {
  const [selectedDocument, setSelectedDocument] = useState<DocumentRow | null>(
    null,
  );
  const [mode, setMode] = useState<Mode>("global");
  const [graphData, setGraphData] = useState<GraphData>({ nodes: [], links: [] });
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [graphLoading, setGraphLoading] = useState(false);
  const [graphError, setGraphError] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const [hasIndex, setHasIndex] = useState(false);
  const [relationDepth, setRelationDepth] = useState(2);
  const [focusedResult, setFocusedResult] = useState<QueryResult | null>(null);
  const [searchResults, setSearchResults] = useState<QueryResult[]>([]);

  const loadGlobalGraph = useCallback(async () => {
    setGraphLoading(true);
    setGraphError(null);
    try {
      const payload = await invoke<GraphOverviewPayload>("get_graph_overview", {
        limit: 250,
      });
      setGraphData(overviewToGraphData(payload));
      setMode("global");
      setSelectedNodeId(null);
      setFocusedResult(null);
      setSearchResults([]);
      setHasIndex(payload.nodes.length > 0);
    } catch (reason) {
      setGraphError(String(reason));
    } finally {
      setGraphLoading(false);
    }
  }, []);

  const showSearchConstellation = useCallback(
    async (results: QueryResult[], depth: number) => {
      if (results.length === 0) return;
      setMode("search");
      setSearchResults(results);
      setFocusedResult(null);
      // The top hit becomes the selected node so the inspector panel populates
      // immediately. Frontend re-selects when the user clicks another card.
      setSelectedNodeId(results[0].chunk_id);
      setGraphLoading(true);
      setGraphError(null);
      try {
        const neighborhood = await invoke<GraphOverviewPayload>(
          "get_search_neighborhood",
          {
            chunkIds: results.map((r) => r.chunk_id),
            depth,
            limit: 200,
          },
        );
        setGraphData(searchToGraphData(results, neighborhood));
      } catch (reason) {
        setGraphError(String(reason));
      } finally {
        setGraphLoading(false);
      }
    },
    [],
  );

  const focusOnChunk = useCallback(
    async (result: QueryResult, depth = relationDepth) => {
      setMode("focus");
      setSelectedNodeId(result.chunk_id);
      setFocusedResult(result);
      setSearchResults([]);
      setGraphData(focusToGraphData(result, []));
      setGraphLoading(true);
      setGraphError(null);

      try {
        const neighborhood = await invoke<GraphOverviewPayload>(
          "get_graph_neighborhood",
          {
            chunkId: result.chunk_id,
            depth,
            limit: 160,
          },
        );
        setGraphData(neighborhoodToGraphData(result, neighborhood));
      } catch (reason) {
        setGraphError(String(reason));
      } finally {
        setGraphLoading(false);
      }
    },
    [relationDepth],
  );

  // Click on a search-result card: just highlight in the current search
  // constellation (no refetch, no mode change). Use the "Refocus" button or
  // double-click a graph node to drill into a single-chunk focus view.
  const highlightChunk = useCallback((result: QueryResult) => {
    setSelectedNodeId(result.chunk_id);
  }, []);

  useEffect(() => {
    loadGlobalGraph();
  }, [loadGlobalGraph, refreshKey]);

  function handleIndexed() {
    setRefreshKey((value) => value + 1);
  }

  function handleCleared() {
    setSelectedDocument(null);
    setSelectedNodeId(null);
    setFocusedResult(null);
    setSearchResults([]);
    setGraphData({ nodes: [], links: [] });
    setMode("global");
    setGraphError(null);
    setHasIndex(false);
    setRefreshKey((value) => value + 1);
  }

  function handleRefocusNode(node: GraphNode) {
    focusOnChunk({
      chunk_id: node.chunkId,
      doc_id: node.docId,
      content: node.content,
      filename: node.filename,
      page: node.page,
      score: node.score,
      score_bm25: node.scoreBm25,
      score_vec: node.scoreVec,
      score_graph: node.scoreGraph,
      score_entity: node.scoreEntity ?? 0,
      score_centrality: node.scoreCentrality ?? 0,
    });
  }

  function handleRelationDepthChange(depth: number) {
    setRelationDepth(depth);
    if (mode === "focus" && focusedResult) {
      void focusOnChunk(focusedResult, depth);
    } else if (mode === "search" && searchResults.length > 0) {
      void showSearchConstellation(searchResults, depth);
    }
  }

  async function handleSelectDocument(document: DocumentRow) {
    setSelectedDocument(document);
    try {
      const chunks = await invoke<Chunk[]>("get_doc_chunks", {
        docId: document.id,
      });
      const firstChunk = chunks[0];
      if (firstChunk) {
        await focusOnChunk({
          chunk_id: firstChunk.id,
          doc_id: firstChunk.doc_id,
          content: firstChunk.content,
          filename: document.filename,
          page: firstChunk.page ?? null,
          score: 1,
          score_bm25: 0,
          score_vec: 1,
          score_graph: 0,
          score_entity: 0,
          score_centrality: 0,
        });
      }
    } catch (reason) {
      setGraphError(String(reason));
    }
  }

  return (
    <div className="flex h-full min-h-screen w-full">
      <aside className="flex w-[320px] shrink-0 flex-col gap-4 border-r border-[var(--color-border)] bg-[var(--color-card)]/40 p-5 backdrop-blur">
        <div className="flex items-center gap-2">
          <div className="flex size-9 items-center justify-center rounded-lg bg-[var(--color-primary)] text-[var(--color-primary-foreground)]">
            <Sparkles className="size-5" />
          </div>
          <div className="flex flex-col leading-tight">
            <span className="text-sm font-semibold">Anubis OS</span>
            <span className="text-[10px] uppercase tracking-wider text-[var(--color-muted-foreground)]">
              Knowledge Engine
            </span>
          </div>
        </div>

        <Separator />

        <IndexStatus onIndexed={handleIndexed} onCleared={handleCleared} />

        <Separator />

        <KnowledgeBrowser
          refreshKey={refreshKey}
          selectedId={selectedDocument?.id ?? null}
          onSelect={handleSelectDocument}
        />
      </aside>

      <main className="flex min-w-0 flex-1 flex-col gap-4 p-6">
        <SearchBar
          selectedChunkId={selectedNodeId}
          onSelectResult={highlightChunk}
          onResults={(results, depth) => {
            void showSearchConstellation(results, depth);
          }}
        />

        {hasIndex || graphData.nodes.length > 0 || graphLoading ? (
          <GraphVisualizer
            data={graphData}
            mode={mode}
            relationDepth={relationDepth}
            loading={graphLoading}
            error={graphError}
            selectedNodeId={selectedNodeId}
            onSelectNode={(node) => setSelectedNodeId(node.id)}
            onRefocusNode={handleRefocusNode}
            onReturnToGlobal={loadGlobalGraph}
            onRelationDepthChange={handleRelationDepthChange}
          />
        ) : (
          <EmptyState />
        )}
      </main>
    </div>
  );
}

function EmptyState() {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-4 rounded-xl border border-dashed border-[var(--color-border)] p-12 text-center">
      <div className="flex size-14 items-center justify-center rounded-2xl bg-[var(--color-accent)]">
        <FolderSearch className="size-7 text-[var(--color-primary)]" />
      </div>
      <div className="space-y-1">
        <h2 className="text-lg font-semibold">Index your knowledge base</h2>
        <p className="max-w-md text-sm text-[var(--color-muted-foreground)]">
          Pick a folder on the left — Anubis will parse, embed, and graph every
          document. The global knowledge graph will appear here once indexing
          finishes.
        </p>
      </div>
    </div>
  );
}
