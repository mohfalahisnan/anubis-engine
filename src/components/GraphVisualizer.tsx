import ForceGraph2D from "react-force-graph-2d";
import { useEffect, useMemo, useRef, useState } from "react";

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
  type: "center" | "neighbor" | "search_result";
};

export type GraphLink = {
  source: string;
  target: string;
  weight: number;
  type: "graph_neighbor" | "search_result";
};

export type GraphData = {
  nodes: GraphNode[];
  links: GraphLink[];
};

type GraphVisualizerProps = {
  center: QueryResult | null;
  neighbors: QueryResult[];
  loading?: boolean;
  error?: string | null;
  onSelectNode?: (node: GraphNode) => void;
  onRefocusNode?: (node: GraphNode) => void;
};

const MAX_VISIBLE_NODES = 80;
const MAX_VISIBLE_LINKS = 120;
const DOUBLE_CLICK_MS = 320;

export function toGraphData(
  center: QueryResult | null,
  neighbors: QueryResult[],
): GraphData {
  if (!center) {
    return { nodes: [], links: [] };
  }

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
    type: "center",
  };

  const seen = new Set([center.chunk_id]);
  const neighborNodes: GraphNode[] = neighbors
    .filter((item) => item.chunk_id !== center.chunk_id)
    .sort((a, b) => b.score - a.score)
    .filter((item) => {
      if (seen.has(item.chunk_id)) {
        return false;
      }
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

  return {
    nodes: [centerNode, ...neighborNodes],
    links,
  };
}

export default function GraphVisualizer({
  center,
  neighbors,
  loading = false,
  error = null,
  onSelectNode,
  onRefocusNode,
}: GraphVisualizerProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const clickRef = useRef<{ id: string; at: number } | null>(null);
  const [selectedNode, setSelectedNode] = useState<GraphNode | null>(null);
  const [size, setSize] = useState({ width: 720, height: 440 });

  const graphData = useMemo(
    () => toGraphData(center, neighbors),
    [center, neighbors],
  );

  useEffect(() => {
    const nextCenter = graphData.nodes.find((node) => node.type === "center") ?? null;
    setSelectedNode(nextCenter);
  }, [graphData]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }

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

  function selectNode(node: GraphNode) {
    setSelectedNode(node);
    onSelectNode?.(node);
  }

  function handleNodeClick(rawNode: unknown) {
    const node = rawNode as GraphNode;
    const now = Date.now();
    const previous = clickRef.current;

    if (previous?.id === node.id && now - previous.at <= DOUBLE_CLICK_MS) {
      clickRef.current = null;
      onRefocusNode?.(node);
      return;
    }

    clickRef.current = { id: node.id, at: now };
    selectNode(node);
  }

  if (loading) {
    return (
      <section className="graph-panel graph-state">
        <span>Loading graph...</span>
      </section>
    );
  }

  if (error) {
    return (
      <section className="graph-panel graph-state">
        <span>Unable to load graph neighbors.</span>
        <small>{error}</small>
      </section>
    );
  }

  if (!center) {
    return (
      <section className="graph-panel graph-state">
        <span>Search your knowledge base and select a result to view its graph.</span>
      </section>
    );
  }

  return (
    <section className="graph-panel focused-graph">
      <div className="graph-header">
        <div>
          <h2>Focused Graph</h2>
          <small>
            {graphData.nodes.length} nodes · {graphData.links.length} links
          </small>
        </div>
        {selectedNode ? (
          <button
            className="secondary-button"
            onClick={() => onRefocusNode?.(selectedNode)}
          >
            Refocus
          </button>
        ) : null}
      </div>

      <div className="graph-layout">
        <div ref={containerRef} className="graph-canvas" aria-label="Focused chunk graph">
          <ForceGraph2D
            graphData={graphData}
            width={size.width}
            height={size.height}
            backgroundColor="#0d1117"
            nodeRelSize={6}
            nodeVal={(rawNode) => {
              const node = rawNode as GraphNode;
              return node.type === "center" ? 14 : Math.max(4, node.score * 10);
            }}
            nodeColor={(rawNode) => {
              const node = rawNode as GraphNode;
              if (node.type === "center") {
                return "#f2b84b";
              }
              if (node.id === selectedNode?.id) {
                return "#43c6ac";
              }
              return "#7aa2f7";
            }}
            nodeLabel={(rawNode) => {
              const node = rawNode as GraphNode;
              const page = node.page ? `\nPage: ${node.page}` : "";
              const preview = truncateText(node.content, 180);
              return `${node.filename}${page}\nScore: ${node.score.toFixed(2)}\n${preview}`;
            }}
            linkWidth={(rawLink) => {
              const link = rawLink as GraphLink;
              return Math.max(1, link.weight * 4);
            }}
            linkColor={() => "rgba(122, 162, 247, 0.42)"}
            linkDirectionalParticles={1}
            linkDirectionalParticleWidth={(rawLink) => {
              const link = rawLink as GraphLink;
              return Math.max(1, link.weight * 2);
            }}
            cooldownTicks={80}
            minZoom={0.35}
            maxZoom={4}
            onNodeClick={handleNodeClick}
            onNodeRightClick={(rawNode, event) => {
              event.preventDefault();
              onRefocusNode?.(rawNode as GraphNode);
            }}
          />
        </div>

        <SelectedNodePanel
          node={selectedNode}
          onRefocusNode={onRefocusNode}
        />
      </div>
    </section>
  );
}

function SelectedNodePanel({
  node,
  onRefocusNode,
}: {
  node: GraphNode | null;
  onRefocusNode?: (node: GraphNode) => void;
}) {
  if (!node) {
    return (
      <aside className="selected-node-panel">
        <span>Select a graph node to inspect its chunk.</span>
      </aside>
    );
  }

  return (
    <aside className="selected-node-panel">
      <div>
        <h3>{node.filename}</h3>
        <small>
          {node.page ? `Page ${node.page} · ` : ""}
          {node.type === "center" ? "Center chunk" : "Neighbor chunk"}
        </small>
      </div>

      <dl className="score-grid">
        <div>
          <dt>Final</dt>
          <dd>{formatScore(node.score)}</dd>
        </div>
        <div>
          <dt>BM25</dt>
          <dd>{formatScore(node.scoreBm25)}</dd>
        </div>
        <div>
          <dt>Vector</dt>
          <dd>{formatScore(node.scoreVec)}</dd>
        </div>
        <div>
          <dt>Graph</dt>
          <dd>{formatScore(node.scoreGraph)}</dd>
        </div>
      </dl>

      <p>{truncateText(node.content, 520)}</p>

      <button className="secondary-button full-width" onClick={() => onRefocusNode?.(node)}>
        Refocus graph here
      </button>
    </aside>
  );
}

function truncateText(value: string, maxLength: number) {
  const compact = value.replace(/\s+/g, " ").trim();
  if (compact.length <= maxLength) {
    return compact;
  }
  return `${compact.slice(0, maxLength - 1)}...`;
}

function formatScore(value: number) {
  return `${Math.round(value * 100)}%`;
}
