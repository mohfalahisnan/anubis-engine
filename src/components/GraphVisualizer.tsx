import ForceGraph2D from "react-force-graph-2d";
import type { ForceGraphMethods } from "react-force-graph-2d";
import { useEffect, useMemo, useRef, useState } from "react";
import { Crosshair, Globe2, Loader2, Network, ScanSearch } from "lucide-react";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";
import { cn } from "../lib/utils";

export type QueryResult = {
  chunk_id: string;
  doc_id: string;
  content: string;
  filename: string;
  page?: number | null;
  score: number;
  score_bm25: number;
  score_vec: number;
  score_graph: number;
  score_entity: number;
  score_centrality: number;
};

export type GraphOverviewNode = {
  chunk_id: string;
  doc_id: string;
  content: string;
  filename: string;
  page?: number | null;
  degree: number;
};

export type GraphOverviewEdge = {
  src_chunk: string;
  dst_chunk: string;
  weight: number;
  edge_type: string;
};

export type GraphOverviewPayload = {
  nodes: GraphOverviewNode[];
  edges: GraphOverviewEdge[];
};

export type GraphNode = {
  id: string;
  chunkId: string;
  docId: string;
  label: string;
  filename: string;
  content: string;
  page?: number | null;
  score: number;
  scoreBm25: number;
  scoreVec: number;
  scoreGraph: number;
  scoreEntity: number;
  scoreCentrality: number;
  degree: number;
  type:
    | "center"
    | "neighbor"
    | "global"
    | "search_result"
    | "search_neighbor";
  searchRank?: number | null;
};

export type GraphLink = {
  source: string;
  target: string;
  weight: number;
  type: "graph_neighbor" | "global" | "search";
};

export type GraphData = {
  nodes: GraphNode[];
  links: GraphLink[];
};

type Mode = "global" | "focus" | "search";

type GraphVisualizerProps = {
  data: GraphData;
  mode: Mode;
  relationDepth?: number;
  loading?: boolean;
  error?: string | null;
  selectedNodeId?: string | null;
  onSelectNode?: (node: GraphNode) => void;
  onRefocusNode?: (node: GraphNode) => void;
  onReturnToGlobal?: () => void;
  onRelationDepthChange?: (depth: number) => void;
};

const MAX_VISIBLE_NODES = 160;
const MAX_VISIBLE_LINKS = 520;
const GRAPH_BACKGROUND = "#0b0d12";
const OBSIDIAN_LINK_GLOBAL = "rgba(144, 154, 180, 0.18)";
const OBSIDIAN_LINK_FOCUS = "rgba(138, 163, 255, 0.26)";
const OBSIDIAN_LINK_SEARCH = "rgba(245, 200, 110, 0.45)";

export function focusToGraphData(
  center: QueryResult | null,
  neighbors: QueryResult[],
): GraphData {
  if (!center) return { nodes: [], links: [] };

  const centerNode: GraphNode = {
    id: center.chunk_id,
    chunkId: center.chunk_id,
    docId: center.doc_id,
    label: center.filename,
    filename: center.filename,
    content: center.content,
    page: center.page,
    score: center.score,
    scoreBm25: center.score_bm25,
    scoreVec: center.score_vec,
    scoreGraph: center.score_graph,
    scoreEntity: center.score_entity ?? 0,
    scoreCentrality: center.score_centrality ?? 0,
    degree: 0,
    type: "center",
  };

  const seen = new Set([center.chunk_id]);
  const neighborNodes: GraphNode[] = neighbors
    .filter((item) => item.chunk_id !== center.chunk_id)
    .sort((a, b) => b.score - a.score)
    .filter((item) => {
      if (seen.has(item.chunk_id)) return false;
      seen.add(item.chunk_id);
      return true;
    })
    .slice(0, MAX_VISIBLE_NODES - 1)
    .map((item) => ({
      id: item.chunk_id,
      chunkId: item.chunk_id,
      docId: item.doc_id,
      label: item.filename,
      filename: item.filename,
      content: item.content,
      page: item.page,
      score: item.score,
      scoreBm25: item.score_bm25,
      scoreVec: item.score_vec,
      scoreGraph: item.score_graph,
      scoreEntity: item.score_entity ?? 0,
      scoreCentrality: item.score_centrality ?? 0,
      degree: 0,
      type: "neighbor",
    }));

  const links: GraphLink[] = neighborNodes
    .slice(0, MAX_VISIBLE_LINKS)
    .map((node) => ({
      source: center.chunk_id,
      target: node.id,
      weight: node.score,
      type: "graph_neighbor",
    }));

  return { nodes: [centerNode, ...neighborNodes], links };
}

export function overviewToGraphData(payload: GraphOverviewPayload): GraphData {
  const visibleNodes = payload.nodes.slice(0, MAX_VISIBLE_NODES);
  const maxDegree = visibleNodes.reduce(
    (acc, node) => Math.max(acc, node.degree),
    1,
  );
  const allowed = new Set(visibleNodes.map((n) => n.chunk_id));

  const nodes: GraphNode[] = visibleNodes.map((node) => ({
    id: node.chunk_id,
    chunkId: node.chunk_id,
    docId: node.doc_id,
    label: node.filename,
    filename: node.filename,
    content: node.content,
    page: node.page,
    score: maxDegree > 0 ? node.degree / maxDegree : 0,
    scoreBm25: 0,
    scoreVec: 0,
    scoreGraph: maxDegree > 0 ? node.degree / maxDegree : 0,
    scoreEntity: 0,
    scoreCentrality: 0,
    degree: node.degree,
    type: "global",
  }));

  const links: GraphLink[] = payload.edges
    .filter((edge) => allowed.has(edge.src_chunk) && allowed.has(edge.dst_chunk))
    .slice(0, MAX_VISIBLE_LINKS)
    .map((edge) => ({
      source: edge.src_chunk,
      target: edge.dst_chunk,
      weight: edge.weight,
      type: "global",
    }));

  return { nodes, links };
}

/// Builds a graph constellation around a set of search hits. Hits become
/// `search_result` nodes (ranked & sized by score). Their shared neighborhood
/// (fetched server-side via get_search_neighborhood) becomes `search_neighbor`
/// nodes. Links inside the constellation get tagged `search` so the renderer
/// can give them a brighter tint.
export function searchToGraphData(
  results: QueryResult[],
  payload: GraphOverviewPayload,
): GraphData {
  if (results.length === 0) return { nodes: [], links: [] };

  const ranked = new Map<string, { result: QueryResult; rank: number }>();
  results.forEach((result, idx) => {
    if (!ranked.has(result.chunk_id)) {
      ranked.set(result.chunk_id, { result, rank: idx });
    }
  });

  const visibleNodes = payload.nodes.slice(0, MAX_VISIBLE_NODES);
  const maxDegree = visibleNodes.reduce(
    (acc, node) => Math.max(acc, node.degree),
    1,
  );

  // Map overview nodes → GraphNode, tagging search hits.
  const byId = new Map<string, GraphNode>();
  for (const node of visibleNodes) {
    const hit = ranked.get(node.chunk_id);
    if (hit) {
      const r = hit.result;
      byId.set(node.chunk_id, {
        id: node.chunk_id,
        chunkId: node.chunk_id,
        docId: node.doc_id,
        label: node.filename,
        filename: node.filename,
        content: node.content,
        page: node.page,
        score: r.score,
        scoreBm25: r.score_bm25,
        scoreVec: r.score_vec,
        scoreGraph: r.score_graph,
        scoreEntity: r.score_entity ?? 0,
        scoreCentrality: r.score_centrality ?? 0,
        degree: node.degree,
        type: "search_result",
        searchRank: hit.rank,
      });
    } else {
      const relative = maxDegree > 0 ? node.degree / maxDegree : 0;
      byId.set(node.chunk_id, {
        id: node.chunk_id,
        chunkId: node.chunk_id,
        docId: node.doc_id,
        label: node.filename,
        filename: node.filename,
        content: node.content,
        page: node.page,
        score: relative,
        scoreBm25: 0,
        scoreVec: 0,
        scoreGraph: relative,
        scoreEntity: 0,
        scoreCentrality: 0,
        degree: node.degree,
        type: "search_neighbor",
        searchRank: null,
      });
    }
  }

  // Safety: if a top-N hit isn't in the neighborhood payload (e.g. isolated
  // chunk with no edges), still surface it as an isolated result node.
  for (const [chunkId, { result, rank }] of ranked) {
    if (!byId.has(chunkId)) {
      byId.set(chunkId, {
        id: chunkId,
        chunkId,
        docId: result.doc_id,
        label: result.filename,
        filename: result.filename,
        content: result.content,
        page: result.page,
        score: result.score,
        scoreBm25: result.score_bm25,
        scoreVec: result.score_vec,
        scoreGraph: result.score_graph,
        scoreEntity: result.score_entity ?? 0,
        scoreCentrality: result.score_centrality ?? 0,
        degree: 0,
        type: "search_result",
        searchRank: rank,
      });
    }
  }

  const nodes = Array.from(byId.values());
  const allowed = new Set(nodes.map((n) => n.id));
  const links: GraphLink[] = payload.edges
    .filter((edge) => allowed.has(edge.src_chunk) && allowed.has(edge.dst_chunk))
    .slice(0, MAX_VISIBLE_LINKS)
    .map((edge) => {
      const bothHits =
        ranked.has(edge.src_chunk) && ranked.has(edge.dst_chunk);
      return {
        source: edge.src_chunk,
        target: edge.dst_chunk,
        weight: edge.weight,
        type: bothHits ? "search" : "graph_neighbor",
      };
    });

  return { nodes, links };
}

export function neighborhoodToGraphData(
  center: QueryResult,
  payload: GraphOverviewPayload,
): GraphData {
  const visibleNodes = payload.nodes.slice(0, MAX_VISIBLE_NODES);
  const maxDegree = visibleNodes.reduce(
    (acc, node) => Math.max(acc, node.degree),
    1,
  );
  const allowed = new Set(visibleNodes.map((n) => n.chunk_id));

  const nodes: GraphNode[] = visibleNodes.map((node) => {
    const isCenter = node.chunk_id === center.chunk_id;
    const relativeScore = maxDegree > 0 ? node.degree / maxDegree : 0;
    return {
      id: node.chunk_id,
      chunkId: node.chunk_id,
      docId: node.doc_id,
      label: node.filename,
      filename: node.filename,
      content: node.content,
      page: node.page,
      score: isCenter ? center.score : relativeScore,
      scoreBm25: isCenter ? center.score_bm25 : 0,
      scoreVec: isCenter ? center.score_vec : 0,
      scoreGraph: isCenter ? center.score_graph : relativeScore,
      scoreEntity: isCenter ? (center.score_entity ?? 0) : 0,
      scoreCentrality: isCenter ? (center.score_centrality ?? 0) : 0,
      degree: node.degree,
      type: isCenter ? "center" : "neighbor",
    };
  });

  const links: GraphLink[] = payload.edges
    .filter((edge) => allowed.has(edge.src_chunk) && allowed.has(edge.dst_chunk))
    .slice(0, MAX_VISIBLE_LINKS)
    .map((edge) => ({
      source: edge.src_chunk,
      target: edge.dst_chunk,
      weight: edge.weight,
      type: "graph_neighbor",
    }));

  return { nodes, links };
}

// Stable color from doc id (HSL).
function colorForDoc(docId: string): string {
  let hash = 0;
  for (let i = 0; i < docId.length; i++) {
    hash = (hash * 31 + docId.charCodeAt(i)) | 0;
  }
  const hue = Math.abs(hash) % 360;
  return `hsl(${hue}, 62%, 66%)`;
}

function nodeDisplayValue(node: GraphNode, mode: Mode): number {
  if (node.type === "center") return 8;
  if (node.type === "search_result") return Math.max(5, 4 + node.score * 4);
  if (node.type === "search_neighbor") {
    return Math.min(4.5, 1.6 + Math.sqrt(node.degree) * 0.24);
  }
  if (mode === "global") return Math.min(6, 1.8 + Math.sqrt(node.degree) * 0.28);
  return Math.max(2.2, Math.min(5, 2.4 + node.score * 2.2));
}

function nodeRadius(node: GraphNode, mode: Mode, selected: boolean): number {
  if (selected) return mode === "global" ? 4.8 : 6.2;
  if (node.type === "center") return 5.8;
  if (node.type === "search_result") {
    return Math.max(4.4, Math.min(6.2, 3.6 + node.score * 3.4));
  }
  if (node.type === "search_neighbor") {
    return Math.min(4.0, 2.0 + Math.sqrt(node.degree) * 0.2);
  }
  if (mode === "global") return Math.min(4.6, 2.2 + Math.sqrt(node.degree) * 0.22);
  return Math.max(2.6, Math.min(4.6, 2.6 + node.score * 1.8));
}

/// Gold/amber palette for search-result nodes — top hit is brightest, others
/// fade with rank. Keeps the result constellation visually distinct from
/// graph-only neighbors (which stay on the per-doc HSL palette).
function colorForSearchResult(rank: number | null | undefined): string {
  const r = rank ?? 0;
  // Hue 42 = warm amber; lightness drops slightly as rank increases.
  const lightness = Math.max(56, 74 - r * 1.8);
  const saturation = Math.max(72, 92 - r * 1.4);
  return `hsl(42, ${saturation}%, ${lightness}%)`;
}

function colorForNode(node: GraphNode, mode: Mode, selectedId: string | null): string {
  if (node.id === selectedId) return "#d7dce8";
  if (node.type === "center") return "#8ea4ff";
  if (node.type === "neighbor") return "#9aa7ff";
  if (node.type === "search_result") return colorForSearchResult(node.searchRank);
  // search_neighbor or global → doc color, but search_neighbor is dimmed.
  const base = colorForDoc(node.docId);
  return mode === "search" ? dimColor(base, 0.62) : base;
}

function dimColor(hsl: string, factor: number): string {
  // Cheap HSL dim: pull lightness toward 50 then multiply alpha-equivalent.
  const match = hsl.match(/hsl\((\d+(?:\.\d+)?),\s*(\d+(?:\.\d+)?)%,\s*(\d+(?:\.\d+)?)%\)/);
  if (!match) return hsl;
  const h = match[1];
  const s = Math.max(20, Number(match[2]) * factor);
  const l = Math.max(28, Number(match[3]) * factor);
  return `hsl(${h}, ${s}%, ${l}%)`;
}

function linkDistance(link: GraphLink, mode: Mode): number {
  const weight = Number.isFinite(link.weight) ? link.weight : 0;
  if (mode === "global") return 92 + (1 - weight) * 42;
  return 118 + (1 - weight) * 54;
}

function linkStrength(link: GraphLink, mode: Mode): number {
  const weight = Number.isFinite(link.weight) ? link.weight : 0;
  if (mode === "global") return 0.11 + weight * 0.12;
  return 0.08 + weight * 0.1;
}

export default function GraphVisualizer({
  data,
  mode,
  relationDepth = 2,
  loading = false,
  error = null,
  selectedNodeId = null,
  onSelectNode,
  onRefocusNode,
  onReturnToGlobal,
  onRelationDepthChange,
}: GraphVisualizerProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const graphRef =
    useRef<ForceGraphMethods<GraphNode, GraphLink> | undefined>(undefined);
  const [internalSelectedId, setInternalSelectedId] = useState<string | null>(
    selectedNodeId,
  );
  const [size, setSize] = useState({ width: 720, height: 440 });

  useEffect(() => {
    setInternalSelectedId(selectedNodeId);
  }, [selectedNodeId]);

  useEffect(() => {
    if (mode === "focus") {
      const center = data.nodes.find((n) => n.type === "center");
      if (center) setInternalSelectedId(center.id);
    } else if (mode === "search" && !selectedNodeId) {
      // Default-select the top-ranked search hit so the inspector panel
      // shows something useful without a click.
      const topHit = [...data.nodes]
        .filter((n) => n.type === "search_result")
        .sort(
          (a, b) =>
            (a.searchRank ?? Number.POSITIVE_INFINITY) -
            (b.searchRank ?? Number.POSITIVE_INFINITY),
        )[0];
      if (topHit) setInternalSelectedId(topHit.id);
    }
  }, [data, mode, selectedNodeId]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      setSize({
        width: Math.max(320, Math.floor(width)),
        height: Math.max(320, Math.floor(height)),
      });
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, []);

  const selectedNode = useMemo(
    () => data.nodes.find((n) => n.id === internalSelectedId) ?? null,
    [data.nodes, internalSelectedId],
  );
  const hasData = data.nodes.length > 0;

  useEffect(() => {
    const graph = graphRef.current;
    if (!graph || !hasData) return;

    const chargeForce = graph.d3Force("charge");
    chargeForce?.strength?.(mode === "global" ? -185 : -230);
    chargeForce?.distanceMax?.(mode === "global" ? 520 : 420);

    const linkForce = graph.d3Force("link");
    linkForce?.distance?.((rawLink: GraphLink) => linkDistance(rawLink, mode));
    linkForce?.strength?.((rawLink: GraphLink) => linkStrength(rawLink, mode));

    graph.d3ReheatSimulation();
    window.setTimeout(() => {
      graph.zoomToFit(520, 72);
    }, 180);
  }, [data, hasData, mode, size.height, size.width]);

  function selectNode(node: GraphNode) {
    setInternalSelectedId(node.id);
    onSelectNode?.(node);
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          {mode === "global" ? (
            <Globe2 className="size-4 text-[var(--color-primary)]" />
          ) : mode === "search" ? (
            <ScanSearch className="size-4 text-[#f4c074]" />
          ) : (
            <Network className="size-4 text-[var(--color-primary)]" />
          )}
          <h2 className="text-sm font-semibold">
            {mode === "global"
              ? "Global Knowledge Graph"
              : mode === "search"
                ? "Search Constellation"
                : "Focused Graph"}
          </h2>
          {hasData ? (
            <Badge variant="secondary">
              {data.nodes.length} nodes · {data.links.length} edges
            </Badge>
          ) : null}
        </div>
        <div className="flex items-center gap-2">
          {mode === "focus" && onRelationDepthChange ? (
            <div className="flex items-center gap-1 rounded-md border border-[var(--color-border)] bg-[var(--color-card)] p-1">
              {[1, 2, 3].map((depth) => (
                <Button
                  key={depth}
                  variant={relationDepth === depth ? "default" : "ghost"}
                  size="sm"
                  className="h-6 px-2 text-[11px]"
                  onClick={() => onRelationDepthChange(depth)}
                >
                  {depth} hop
                </Button>
              ))}
            </div>
          ) : null}
          {mode !== "global" && onReturnToGlobal ? (
            <Button variant="outline" size="sm" onClick={onReturnToGlobal}>
              <Globe2 className="size-3.5" /> Back to global
            </Button>
          ) : null}
          {selectedNode ? (
            <Button
              variant="outline"
              size="sm"
              onClick={() => onRefocusNode?.(selectedNode)}
            >
              <Crosshair className="size-3.5" /> Refocus
            </Button>
          ) : null}
        </div>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 lg:grid-cols-[minmax(0,1fr)_320px]">
        <div
          ref={containerRef}
          className={cn(
            "relative min-h-[420px] overflow-hidden rounded-xl border border-[var(--color-border)] bg-[#0b0d12] shadow-inner",
          )}
        >
          {loading ? (
            <div className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-2 bg-[#0b0d12]/80 text-white/80">
              <Loader2 className="size-6 animate-spin" />
              <span className="text-xs">Loading graph…</span>
            </div>
          ) : null}

          {error ? (
            <div className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-2 bg-[#0b0d12]/90 px-6 text-center text-white/80">
              <span className="text-sm font-medium">Unable to load graph</span>
              <span className="text-xs text-white/60">{error}</span>
            </div>
          ) : null}

          {!loading && !error && !hasData ? (
            <div className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-3 bg-[#0b0d12] px-6 text-center text-white/70">
              <ScanSearch className="size-10 text-white/30" />
              <div>
                <p className="text-sm font-medium text-white/90">
                  No graph yet
                </p>
                <p className="mt-1 text-xs text-white/50">
                  Pick a folder and run indexing — every chunk and edge will
                  appear here.
                </p>
              </div>
            </div>
          ) : null}

          {hasData ? (
            <ForceGraph2D
              ref={graphRef}
              graphData={data}
              width={size.width}
              height={size.height}
              backgroundColor={GRAPH_BACKGROUND}
              nodeRelSize={2}
              nodeVal={(rawNode) => nodeDisplayValue(rawNode as GraphNode, mode)}
              nodeColor={(rawNode) =>
                colorForNode(rawNode as GraphNode, mode, internalSelectedId)
              }
              nodeCanvasObjectMode={() => "replace"}
              nodeCanvasObject={(rawNode, ctx, globalScale) => {
                const node = rawNode as GraphNode & { x?: number; y?: number };
                if (typeof node.x !== "number" || typeof node.y !== "number") {
                  return;
                }

                const selected = node.id === internalSelectedId;
                const radius = nodeRadius(node, mode, selected);
                const fill = colorForNode(node, mode, internalSelectedId);
                const isSearchHit = node.type === "search_result";

                // Halo for highlighted nodes: center / selected / top search hits.
                const halo =
                  selected ||
                  node.type === "center" ||
                  (isSearchHit && (node.searchRank ?? 99) <= 2);
                if (halo) {
                  ctx.beginPath();
                  ctx.arc(node.x, node.y, radius + 5, 0, Math.PI * 2);
                  ctx.fillStyle = isSearchHit
                    ? "rgba(245, 200, 110, 0.22)"
                    : node.type === "center"
                      ? "rgba(142, 164, 255, 0.16)"
                      : "rgba(215, 220, 232, 0.14)";
                  ctx.fill();
                }

                ctx.beginPath();
                ctx.arc(node.x, node.y, radius, 0, Math.PI * 2);
                ctx.fillStyle = fill;
                ctx.fill();
                ctx.lineWidth = selected
                  ? 1.4 / globalScale
                  : isSearchHit
                    ? 1.1 / globalScale
                    : 0.75 / globalScale;
                ctx.strokeStyle = selected
                  ? "rgba(255, 255, 255, 0.95)"
                  : isSearchHit
                    ? "rgba(255, 220, 150, 0.85)"
                    : "rgba(255, 255, 255, 0.28)";
                ctx.stroke();

                // Rank badge on top-3 search hits — tiny number on the upper-right.
                if (isSearchHit && (node.searchRank ?? 99) <= 2 && globalScale > 0.7) {
                  const badgeR = Math.max(5, 6 / globalScale);
                  ctx.beginPath();
                  ctx.arc(
                    node.x + radius * 0.85,
                    node.y - radius * 0.85,
                    badgeR,
                    0,
                    Math.PI * 2,
                  );
                  ctx.fillStyle = "#1e1410";
                  ctx.fill();
                  ctx.lineWidth = 1 / globalScale;
                  ctx.strokeStyle = "rgba(245, 200, 110, 0.9)";
                  ctx.stroke();
                  ctx.fillStyle = "rgba(245, 215, 140, 0.95)";
                  ctx.font = `bold ${Math.max(8, 10 / globalScale)}px Inter, ui-sans-serif, system-ui`;
                  ctx.textAlign = "center";
                  ctx.textBaseline = "middle";
                  ctx.fillText(
                    String((node.searchRank ?? 0) + 1),
                    node.x + radius * 0.85,
                    node.y - radius * 0.85,
                  );
                }

                const shouldLabel =
                  selected ||
                  node.type === "center" ||
                  isSearchHit ||
                  (mode === "global" && node.degree >= 6 && globalScale > 1.05);
                if (!shouldLabel) return;

                const fontSize = Math.max(9, 11 / globalScale);
                ctx.font = `${fontSize}px Inter, ui-sans-serif, system-ui`;
                ctx.textAlign = "left";
                ctx.textBaseline = "middle";
                ctx.fillStyle = selected
                  ? "rgba(241, 245, 249, 0.96)"
                  : "rgba(209, 216, 232, 0.78)";
                ctx.fillText(
                  truncateText(node.filename, 28),
                  node.x + radius + 5,
                  node.y,
                );
              }}
              nodePointerAreaPaint={(rawNode, color, ctx) => {
                const node = rawNode as GraphNode & { x?: number; y?: number };
                if (typeof node.x !== "number" || typeof node.y !== "number") {
                  return;
                }
                ctx.fillStyle = color;
                ctx.beginPath();
                ctx.arc(node.x, node.y, nodeRadius(node, mode, false) + 6, 0, Math.PI * 2);
                ctx.fill();
              }}
              nodeLabel={(rawNode) => {
                const node = rawNode as GraphNode;
                const page = node.page ? `\nPage: ${node.page}` : "";
                const degreeLine =
                  mode === "global" ? `\nDegree: ${node.degree}` : "";
                const preview = truncateText(node.content, 180);
                return `${node.filename}${page}${degreeLine}\n${preview}`;
              }}
              linkWidth={(rawLink) =>
                Math.max(0.25, (rawLink as GraphLink).weight * 1.05)
              }
              linkColor={(rawLink) => {
                const link = rawLink as GraphLink;
                if (link.type === "search") return OBSIDIAN_LINK_SEARCH;
                if (mode === "global") return OBSIDIAN_LINK_GLOBAL;
                return OBSIDIAN_LINK_FOCUS;
              }}
              linkDirectionalParticles={0}
              linkDirectionalParticleWidth={(rawLink) =>
                Math.max(1, (rawLink as GraphLink).weight * 2)
              }
              warmupTicks={mode === "global" ? 80 : 60}
              cooldownTicks={mode === "global" ? 240 : 180}
              d3AlphaDecay={0.022}
              d3VelocityDecay={0.58}
              minZoom={0.28}
              maxZoom={5}
              onNodeClick={(rawNode) => selectNode(rawNode as GraphNode)}
              onNodeRightClick={(rawNode, event) => {
                event.preventDefault();
                onRefocusNode?.(rawNode as GraphNode);
              }}
            />
          ) : null}
        </div>

        <SelectedNodePanel
          node={selectedNode}
          mode={mode}
          onRefocusNode={onRefocusNode}
        />
      </div>
    </div>
  );
}

function SelectedNodePanel({
  node,
  mode,
  onRefocusNode,
}: {
  node: GraphNode | null;
  mode: Mode;
  onRefocusNode?: (node: GraphNode) => void;
}) {
  if (!node) {
    return (
      <aside className="flex min-h-[200px] items-center justify-center rounded-xl border border-dashed border-[var(--color-border)] p-6 text-center text-xs text-[var(--color-muted-foreground)]">
        Click any node to inspect its chunk.
      </aside>
    );
  }

  return (
    <aside className="flex flex-col gap-4 rounded-xl border border-[var(--color-border)] bg-[var(--color-card)] p-4">
      <div>
        <div className="mb-1 flex items-center gap-2">
          <Badge
            variant={
              node.type === "center" || node.type === "search_result"
                ? "default"
                : "secondary"
            }
          >
            {node.type === "center"
              ? "center"
              : node.type === "neighbor"
                ? "neighbor"
                : node.type === "search_result"
                  ? `hit #${(node.searchRank ?? 0) + 1}`
                  : node.type === "search_neighbor"
                    ? "neighbor"
                    : "node"}
          </Badge>
          {node.page ? (
            <span className="text-[10px] text-[var(--color-muted-foreground)]">
              page {node.page}
            </span>
          ) : null}
        </div>
        <h3 className="break-words text-sm font-semibold leading-tight">
          {node.filename}
        </h3>
      </div>

      {mode === "global" ||
      (mode === "search" && node.type === "search_neighbor") ? (
        <div className="grid grid-cols-2 gap-2">
          <Cell label="Degree" value={node.degree.toString()} highlight />
          <Cell
            label="Relative"
            value={`${Math.round(node.score * 100)}%`}
          />
        </div>
      ) : (
        <div className="grid grid-cols-2 gap-2">
          <Cell
            label="Final"
            value={`${Math.round(node.score * 100)}%`}
            highlight
          />
          <Cell label="Vector" value={`${Math.round(node.scoreVec * 100)}%`} />
          <Cell label="BM25" value={`${Math.round(node.scoreBm25 * 100)}%`} />
          <Cell
            label="Graph"
            value={`${Math.round(node.scoreGraph * 100)}%`}
          />
          <Cell
            label="Entity"
            value={`${Math.round((node.scoreEntity ?? 0) * 100)}%`}
          />
          <Cell
            label="Centrality"
            value={`${Math.round((node.scoreCentrality ?? 0) * 100)}%`}
          />
        </div>
      )}

      <p className="max-h-48 overflow-y-auto rounded-md bg-[var(--color-muted)]/40 p-3 text-xs leading-relaxed text-[var(--color-foreground)]/80">
        {truncateText(node.content, 720)}
      </p>

      <Button
        variant="outline"
        size="sm"
        className="w-full"
        onClick={() => onRefocusNode?.(node)}
      >
        <Crosshair className="size-3.5" /> Focus this chunk
      </Button>
    </aside>
  );
}

function Cell({
  label,
  value,
  highlight,
}: {
  label: string;
  value: string;
  highlight?: boolean;
}) {
  return (
    <div
      className={cn(
        "rounded-md border p-2",
        highlight
          ? "border-[var(--color-primary)]/40 bg-[var(--color-primary)]/5"
          : "border-[var(--color-border)] bg-[var(--color-card)]",
      )}
    >
      <div className="text-[10px] uppercase tracking-wider text-[var(--color-muted-foreground)]">
        {label}
      </div>
      <div className="mt-0.5 font-mono text-sm font-semibold tabular-nums">
        {value}
      </div>
    </div>
  );
}

function truncateText(value: string, maxLength: number) {
  const compact = value.replace(/\s+/g, " ").trim();
  if (compact.length <= maxLength) return compact;
  return `${compact.slice(0, maxLength - 1)}…`;
}
