# Graph Visualizer Spec — Anubis OS

## 1. Purpose

Build a frontend-only graph visualization component for Anubis OS that presents search results and chunk relationships in an Obsidian-style focused graph.

This component does **not** implement embedding, indexing, graph construction, or storage. Those are already handled by the Rust/Tauri backend. The visualizer consumes existing Tauri commands and renders graph relationships in the React + TypeScript frontend.

Primary file:

```txt
src/components/GraphVisualizer.tsx
```

Related frontend files:

```txt
src/components/KnowledgeBrowser.tsx
src/components/SearchBar.tsx
src/components/IndexStatus.tsx
```

## 2. Goals

- Render a focused, interactive knowledge graph.
- Use query results and chunk neighbors from existing Tauri commands.
- Provide an Obsidian-like graph experience: nodes, edges, zoom, pan, drag, hover, click, and refocus.
- Avoid rendering the entire database graph by default.
- Keep the UI fast and understandable even with many indexed chunks.
- Support offline desktop usage inside the Tauri v2 app.

## 3. Non-goals

The graph visualizer must **not**:

- Build embeddings.
- Compute semantic similarity.
- Parse documents.
- Access SQLite directly.
- Build graph edges itself.
- Replace the Rust graph engine.
- Render every indexed chunk by default.
- Add Python, external services, or runtime dependencies outside the frontend package.

## 4. Recommended Library

Use:

```bash
npm install react-force-graph
```

Render with:

```tsx
import ForceGraph2D from "react-force-graph-2d";
```

Rationale:

- Closest fit for Obsidian-style force-directed graph visualization.
- Works well with React + TypeScript.
- Supports zoom, pan, drag, hover labels, click handlers, and dynamic graph data.
- Suitable for a focused local graph around selected chunks.
- Does not require backend changes.

## 5. Backend Commands Used

The visualizer should use the existing Tauri command contracts from the app spec.

### 5.1 Search command

```ts
invoke("query", { q: string, limit?: number })
```

Returns:

```ts
type QueryResult = {
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
```

Usage:

- Called by `SearchBar` or `KnowledgeBrowser`.
- Produces initial search results.
- User selects one result as the graph center.

### 5.2 Chunk neighbors command

```ts
invoke("get_chunk_neighbors", { chunkId: string, depth?: number })
```

Returns:

```ts
QueryResult[]
```

Usage:

- Called when a user selects a chunk.
- Provides the graph neighborhood around the selected chunk.
- Default depth should be `1` or `2`.

### 5.3 Optional document chunks command

```ts
invoke("get_doc_chunks", { docId: string })
```

Returns:

```ts
Chunk[]
```

Usage:

- Optional expansion when the user wants to inspect all chunks from the same document.
- Should not be called automatically for every node.

## 6. UX Model

The graph should be **focused**, not global.

Primary interaction flow:

```txt
User searches
  ↓
query(q, limit)
  ↓
Search results appear
  ↓
User clicks a result
  ↓
get_chunk_neighbors(chunk_id, depth = 2)
  ↓
GraphVisualizer renders selected chunk + neighbors
  ↓
User clicks another node
  ↓
Side panel updates
  ↓
User double-clicks node
  ↓
Graph refocuses around that node
```

## 7. Layout

Recommended screen layout:

```txt
┌──────────────────────────────────────────────────────┐
│ IndexStatus                                          │
├──────────────────────────────────────────────────────┤
│ SearchBar                                            │
├───────────────────────┬──────────────────────────────┤
│ Search Results        │ GraphVisualizer              │
│                       │                              │
│ - result 1            │       focused graph           │
│ - result 2            │                              │
│ - result 3            │                              │
├───────────────────────┴──────────────────────────────┤
│ Selected chunk preview / document metadata            │
└──────────────────────────────────────────────────────┘
```

`GraphVisualizer` should own only the graph canvas and graph-specific interactions. Search state and result list may live in `KnowledgeBrowser`.

## 8. Data Types

### 8.1 Backend result type

```ts
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
```

### 8.2 Graph node type

```ts
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
```

### 8.3 Graph link type

```ts
export type GraphLink = {
  source: string;
  target: string;
  weight: number;
  type: "graph_neighbor" | "search_result";
};
```

### 8.4 Graph data type

```ts
export type GraphData = {
  nodes: GraphNode[];
  links: GraphLink[];
};
```

## 9. Data Transformation

The backend returns a list of `QueryResult` objects, not a frontend graph object. The visualizer must transform query results into nodes and links.

### 9.1 Center node

The selected chunk becomes the center node.

```ts
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
```

### 9.2 Neighbor nodes

Every returned neighbor becomes a `neighbor` node.

```ts
const neighborNodes = neighbors
  .filter((item) => item.chunk_id !== center.chunk_id)
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
    type: "neighbor" as const,
  }));
```

### 9.3 Links

Because `get_chunk_neighbors` returns neighbor results without raw edge rows, the frontend should create visual links from the center node to each neighbor.

```ts
const links = neighborNodes.map((node) => ({
  source: center.chunk_id,
  target: node.id,
  weight: node.score,
  type: "graph_neighbor" as const,
}));
```

If the backend later returns explicit edge weights and edge types, replace `node.score` with the returned edge weight.

## 10. Component API

```tsx
type GraphVisualizerProps = {
  center: QueryResult | null;
  neighbors: QueryResult[];
  loading?: boolean;
  error?: string | null;
  onSelectNode?: (node: GraphNode) => void;
  onRefocusNode?: (node: GraphNode) => void;
};
```

Expected behavior:

- `center === null`: show empty state.
- `loading === true`: show graph loading state.
- `error`: show graph error state.
- `onSelectNode`: called on single click.
- `onRefocusNode`: called on double click.

## 11. Visual Encoding

| Visual property | Meaning |
|---|---|
| Center node size | Larger than all other nodes |
| Neighbor node size | Based on hybrid score |
| Link thickness | Based on score or edge weight |
| Node label | Filename by default |
| Hover label | Filename, score, page, short content preview |
| Node color | Based on node type |
| Link particles | Optional subtle direction/relationship cue |

Recommended defaults:

```ts
nodeVal={(node) => node.type === "center" ? 14 : Math.max(4, node.score * 10)}
linkWidth={(link) => Math.max(1, link.weight * 4)}
nodeLabel={(node) => `${node.filename}\nScore: ${node.score.toFixed(2)}`}
```

## 12. Interaction Requirements

### 12.1 Click node

Single click should:

- Mark node as selected.
- Update the selected chunk preview panel.
- Show filename, page, score breakdown, and content preview.

### 12.2 Double click node

Double click should:

- Call `onRefocusNode(node)`.
- Parent component should call `get_chunk_neighbors(node.chunkId, depth)`.
- Graph should rerender with the clicked node as center.

### 12.3 Hover node

Hover should show:

- Filename.
- Page number if available.
- Final score.
- Short content preview.

### 12.4 Empty state

When no center node is selected:

```txt
Search your knowledge base and select a result to view its graph.
```

### 12.5 Error state

When graph loading fails:

```txt
Unable to load graph neighbors.
```

Also show the error message in a smaller muted area.

## 13. Performance Requirements

The visualizer should cap visible graph size.

Recommended limits:

```ts
const MAX_VISIBLE_NODES = 80;
const MAX_VISIBLE_LINKS = 120;
```

Rules:

- Sort neighbors by `score` descending.
- Render only the top `MAX_VISIBLE_NODES - 1` neighbors.
- Do not render the entire graph database.
- Default graph depth should be `1` for speed, `2` for richer exploration.
- Avoid expensive recalculation on every render; use `useMemo` for graph transformation.

## 14. Accessibility Requirements

- Provide a non-canvas selected-node detail panel.
- Ensure graph actions are also available through search results and side panel buttons.
- Do not rely only on color to communicate meaning.
- Provide visible labels or tooltips for important graph states.

## 15. Suggested Implementation

```tsx
import { useMemo } from "react";
import ForceGraph2D from "react-force-graph-2d";

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

function toGraphData(center: QueryResult | null, neighbors: QueryResult[]): GraphData {
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

  const neighborNodes: GraphNode[] = neighbors
    .filter((item) => item.chunk_id !== center.chunk_id)
    .sort((a, b) => b.score - a.score)
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

  const links: GraphLink[] = neighborNodes.map((node) => ({
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

export function GraphVisualizer({
  center,
  neighbors,
  loading = false,
  error = null,
  onSelectNode,
  onRefocusNode,
}: GraphVisualizerProps) {
  const graphData = useMemo(
    () => toGraphData(center, neighbors),
    [center, neighbors],
  );

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center rounded-2xl border bg-zinc-950 text-sm text-zinc-400">
        Loading graph...
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-full flex-col items-center justify-center rounded-2xl border bg-zinc-950 text-sm text-zinc-400">
        <div>Unable to load graph neighbors.</div>
        <div className="mt-2 max-w-md text-xs text-zinc-500">{error}</div>
      </div>
    );
  }

  if (!center) {
    return (
      <div className="flex h-full items-center justify-center rounded-2xl border bg-zinc-950 text-sm text-zinc-400">
        Search your knowledge base and select a result to view its graph.
      </div>
    );
  }

  return (
    <div className="h-full w-full rounded-2xl border bg-zinc-950">
      <ForceGraph2D
        graphData={graphData}
        backgroundColor="#09090b"
        nodeLabel={(node) => {
          const graphNode = node as GraphNode;
          const page = graphNode.page ? `Page: ${graphNode.page}\n` : "";
          return `${graphNode.filename}\n${page}Score: ${graphNode.score.toFixed(2)}`;
        }}
        nodeRelSize={6}
        nodeVal={(node) => {
          const graphNode = node as GraphNode;
          return graphNode.type === "center" ? 14 : Math.max(4, graphNode.score * 10);
        }}
        nodeAutoColorBy="type"
        linkWidth={(link) => {
          const graphLink = link as GraphLink;
          return Math.max(1, graphLink.weight * 4);
        }}
        linkDirectionalParticles={1}
        linkDirectionalParticleWidth={(link) => {
          const graphLink = link as GraphLink;
          return Math.max(1, graphLink.weight * 2);
        }}
        onNodeClick={(node) => onSelectNode?.(node as GraphNode)}
        onNodeRightClick={(node) => onRefocusNode?.(node as GraphNode)}
      />
    </div>
  );
}
```

Note: `react-force-graph-2d` does not provide a dedicated double-click prop in all versions. If double-click is needed, implement it in the parent using a click timestamp, or use right-click/context menu as the refocus shortcut.

## 16. Parent Component Responsibilities

`KnowledgeBrowser.tsx` should own:

- Search query state.
- Search results state.
- Selected result state.
- Neighbor loading state.
- Selected graph node state.
- Calls to Tauri `invoke`.

Example parent flow:

```tsx
async function handleSelectResult(result: QueryResult) {
  setCenter(result);
  setSelectedNode(null);
  setGraphLoading(true);
  setGraphError(null);

  try {
    const nextNeighbors = await invoke<QueryResult[]>("get_chunk_neighbors", {
      chunkId: result.chunk_id,
      depth: 2,
    });

    setNeighbors(nextNeighbors);
  } catch (error) {
    setGraphError(error instanceof Error ? error.message : String(error));
  } finally {
    setGraphLoading(false);
  }
}
```

## 17. Selected Node Detail Panel

When a node is selected, show:

```txt
Filename
Page number, if any
Final score
BM25 score
Vector score
Graph score
Content preview
Actions:
- Refocus graph here
- Show all chunks from this document
- Open source document, if path is later exposed
```

Current `QueryResult` does not expose document path. If opening the source file is needed, update the backend result later to include `path` from the `documents` table.

## 18. Future Backend Improvements

The current frontend can work with `QueryResult[]`, but a richer graph API would improve accuracy.

Potential future command:

```rust
#[tauri::command]
pub async fn get_graph_neighborhood(
    chunk_id: String,
    depth: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<GraphNeighborhood, String>
```

Suggested response:

```ts
type GraphNeighborhood = {
  nodes: Array<{
    chunk_id: string;
    doc_id: string;
    filename: string;
    content: string;
    page?: number | null;
    score?: number;
  }>;
  edges: Array<{
    source: string;
    target: string;
    weight: number;
    edge_type: "semantic" | "shared_entity" | "same_doc";
  }>;
};
```

Benefits:

- Frontend can render real edges instead of inferred center-to-neighbor links.
- Edge type can be visualized.
- Same-document, semantic, and shared-entity links can look different.
- Depth traversal becomes more accurate.

## 19. Acceptance Criteria

- [ ] `GraphVisualizer.tsx` renders without backend changes.
- [ ] Empty state appears before node selection.
- [ ] Loading state appears while neighbors are fetched.
- [ ] Error state appears if neighbor loading fails.
- [ ] Selecting a search result renders a focused graph.
- [ ] Center node is visually larger than neighbor nodes.
- [ ] Link width reflects score or edge weight.
- [ ] Clicking a node updates selected-node details.
- [ ] Refocusing on a node reloads neighbors through `get_chunk_neighbors`.
- [ ] Graph does not render more than 80 nodes by default.
- [ ] Graph UI works offline inside the Tauri desktop app.

## 20. Implementation Order

```txt
Step 1: Install react-force-graph
Step 2: Create GraphVisualizer.tsx with static mock data
Step 3: Add QueryResult, GraphNode, GraphLink, GraphData types
Step 4: Implement toGraphData(center, neighbors)
Step 5: Wire GraphVisualizer into KnowledgeBrowser.tsx
Step 6: On search result click, call get_chunk_neighbors
Step 7: Add selected node detail panel
Step 8: Add refocus action
Step 9: Add node cap and score sorting
Step 10: Polish empty/loading/error states
```

## 21. Final Recommendation

Use `react-force-graph-2d` for the first version. Render a focused graph around the selected chunk using `get_chunk_neighbors`. Do not attempt a full global graph until the backend exposes a dedicated graph neighborhood response with explicit nodes and edges.
