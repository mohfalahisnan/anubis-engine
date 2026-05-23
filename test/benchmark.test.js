const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

const benchmark = require("../bin/benchmark.js");

function tempDir() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "anubis-benchmark-test-"));
}

test("generateDataset creates the expected source corpus and anchors", () => {
  const root = tempDir();
  try {
    const dataset = benchmark.generateDataset(root);

    assert.equal(dataset.sourceFiles.length, 52);
    assert.equal(dataset.mediaFiles.length, 13);
    assert.equal(dataset.preprocessPlan.total, 13);
    assert.equal(dataset.preprocessPlan.cacheHits, 9);
    assert.equal(dataset.preprocessPlan.expectedRuns, 4);

    const shipping = fs.readFileSync(path.join(root, "shipping_module.md"), "utf8");
    assert.match(shipping, /thermal printer printhead replacement/);
    assert.match(shipping, /INC-2026-ATLAS-014/);
    assert.match(shipping, /SHIP-NODE-SURYA/);

    const syslog = fs.readFileSync(path.join(root, "syslog_03.txt"), "utf8");
    assert.match(syslog, /INC-2026-ATLAS-014/);

    const inventory = JSON.parse(fs.readFileSync(path.join(root, "inventory_audit.json"), "utf8"));
    assert.equal(inventory.records.length, 60);
    assert.equal(inventory.records[42].audit_query_token, "audit log item 42");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("percentile and AQI calculations are deterministic", () => {
  assert.equal(benchmark.percentile([10, 20, 30, 40, 50], 50), 30);
  assert.equal(benchmark.percentile([10, 20, 30, 40, 50], 95), 50);

  const aqi = benchmark.calculateAqi({
    averageRecallAt5: 0.9,
    p95LatencyMs: 50,
  });

  assert.equal(aqi, 90);
});

test("generateDataset supports quick and full payload scales", () => {
  const quickRoot = tempDir();
  const fullRoot = tempDir();
  try {
    const quick = benchmark.generateDataset(quickRoot, { scale: "quick" });
    const full = benchmark.generateDataset(fullRoot, { scale: "full" });

    assert.equal(quick.sourceFiles.length, 52);
    assert.equal(full.sourceFiles.length, 52);
    assert.ok(full.totalBytes > quick.totalBytes * 3);
  } finally {
    fs.rmSync(quickRoot, { recursive: true, force: true });
    fs.rmSync(fullRoot, { recursive: true, force: true });
  }
});

test("evaluateSearchCase reports recall, precision, and expected files", () => {
  const result = benchmark.evaluateSearchCase(
    {
      label: "anchor",
      query: "INC-2026-ATLAS-014",
      relevantFiles: ["syslog_03.txt", "shipping_module.md"],
      k: 5,
      mustIncludeTopK: ["syslog_03.txt", "shipping_module.md"],
    },
    [
      { filename: "syslog_03.txt" },
      { filename: "billing_module.md" },
      { filename: "shipping_module.md" },
    ],
  );

  assert.equal(result.recallAtK, 1);
  assert.equal(result.precisionAtK, 0.4);
  assert.equal(result.status, "PASS");
});

test("evaluateSearchCase caps recall and fails when no relevant file is returned", () => {
  const duplicates = benchmark.evaluateSearchCase(
    {
      label: "duplicates",
      query: "printer",
      relevantFiles: ["shipping_module.md"],
      k: 5,
    },
    [
      { filename: "shipping_module.md" },
      { filename: "shipping_module.md" },
      { filename: "shipping_module.md" },
    ],
  );

  assert.equal(duplicates.recallAtK, 1);
  assert.equal(duplicates.precisionAtK, 0.2);
  assert.equal(duplicates.status, "PASS");

  const miss = benchmark.evaluateSearchCase(
    {
      label: "miss",
      query: "invoice approval",
      relevantFiles: ["img_invoice_02.png"],
      k: 5,
    },
    [{ filename: "readme_master.md" }],
  );

  assert.equal(miss.recallAtK, 0);
  assert.equal(miss.status, "FAIL");
});

test("classifyQuery maps recall and precision to research statuses", () => {
  assert.equal(benchmark.classifyQuery(0, 1), "critical_fail");
  assert.equal(benchmark.classifyQuery(0.86, 0.45), "strong_pass");
  assert.equal(benchmark.classifyQuery(0.75, 0.35), "pass");
  assert.equal(benchmark.classifyQuery(0.6, 0.2), "weak_pass");
  assert.equal(benchmark.classifyQuery(0.59, 0.9), "fail");
});

test("evaluateSearchCase includes ranking metrics without changing legacy fields", () => {
  const result = benchmark.evaluateSearchCase(
    {
      label: "ranking",
      query: "printer",
      relevantFiles: ["shipping_module.md", "syslog_03.txt"],
      k: 5,
    },
    [
      { filename: "billing_module.md" },
      { filename: "shipping_module.md" },
      { filename: "readme_master.md" },
      { filename: "syslog_03.txt" },
      { filename: "catalog_module.md" },
    ],
  );

  assert.equal(result.recallAtK, 1);
  assert.equal(result.precisionAtK, 0.4);
  assert.equal(result.recallAt5, 1);
  assert.equal(result.recallAt10, 1);
  assert.equal(result.precisionAt5, 0.4);
  assert.equal(result.precisionAt10, 0.2);
  assert.equal(result.top1Accuracy, 0);
  assert.equal(result.top3Accuracy, 1);
  assert.equal(result.mrrAt10, 0.5);
  assert.equal(result.ndcgAt10, 0.65);
  assert.equal(result.queryStatus, "pass");
});

test("ranking metrics do not let duplicate relevant documents inflate nDCG", () => {
  const metrics = benchmark.rankingMetrics(
    [
      { filename: "shipping_module.md" },
      { filename: "shipping_module.md" },
      { filename: "shipping_module.md" },
      { filename: "billing_module.md" },
    ],
    new Set(["shipping_module.md"]),
  );

  assert.equal(metrics.recallAt10, 1);
  assert.equal(metrics.ndcgAt10, 1);
});

test("score breakdown debug output is opt-in", () => {
  const results = [
    {
      chunk_id: "chunk-1",
      doc_id: "doc-1",
      filename: "shipping_module.md",
      score: 0.75,
      score_vec: 0.8,
      score_bm25: 0.5,
      score_graph: 0.1,
      score_entity: 0.2,
      score_centrality: 0.05,
    },
  ];

  assert.equal(benchmark.debugSearchResults(results, { includeScoreBreakdown: false }), undefined);

  const debug = benchmark.debugSearchResults(results, {
    includeScoreBreakdown: true,
    includeTopResults: 1,
  });

  assert.deepEqual(debug[0].scoreBreakdown, {
    vector: 0.8,
    bm25: 0.5,
    graph: 0.1,
    entity: 0.2,
    sourceQuality: 0.05,
    final: 0.75,
  });
});

test("graph metrics report existing graph as candidate edges when visibility is not modeled", () => {
  const metrics = benchmark.graphMetricsFromStats({
    chunks: 4,
    graph_edges: 20,
    edges_by_type: {
      semantic: 10,
      shared_anchor: 4,
      same_doc: 6,
    },
  });

  assert.equal(metrics.totalNodes, 4);
  assert.equal(metrics.totalEdges, 20);
  assert.equal(metrics.candidateEdges, 20);
  assert.equal(metrics.visibleEdges, null);
  assert.equal(metrics.edgesPerChunk, 5);
  assert.equal(metrics.visibleEdgesPerNode, null);
  assert.equal(metrics.edgeEvidenceCoverage, 0.2);
});

test("critical failure count excludes downrank-only diagnostic probes", () => {
  assert.equal(
    benchmark.criticalFailureCount([
      { queryStatus: "critical_fail", category: "downrank" },
      { queryStatus: "critical_fail", category: "graph" },
      { queryStatus: "fail", category: "accuracy" },
    ]),
    1,
  );
});

test("critical failure diagnostics identify final ranking drops with indexed evidence", () => {
  const diagnostic = benchmark.buildCriticalFailureDiagnostic({
    testCase: {
      label: "invoice approval",
      query: "VID-APPROVAL-005 invoice approval",
      relevantFiles: ["img_invoice_02.png"],
      k: 5,
    },
    report: {
      topFilenames: ["readme_master.md", "video_record_05.mp4"],
    },
    docs: [
      {
        id: "doc-invoice",
        filename: "img_invoice_02.png",
        path: "D:\\bench\\img_invoice_02.png",
      },
    ],
    chunksByDocId: new Map([
      [
        "doc-invoice",
        [
          {
            id: "chunk-invoice",
            doc_id: "doc-invoice",
            content: "Invoice OCR: VID-APPROVAL-005 invoice approval confirms dock payment evidence.",
          },
        ],
      ],
    ]),
    diagnosticResults: [
      {
        filename: "readme_master.md",
        chunk_id: "chunk-readme",
        doc_id: "doc-readme",
        score: 0.6,
        score_bm25: 1,
        score_vec: 0.9,
        score_graph: 0,
        path: "D:\\bench\\readme_master.md",
      },
      {
        filename: "img_invoice_02.png",
        chunk_id: "chunk-invoice",
        doc_id: "doc-invoice",
        score: 0.4,
        score_bm25: 0,
        score_vec: 1,
        score_graph: 0,
        path: "D:\\bench\\img_invoice_02.png",
      },
    ],
    aliases: ["dock payment evidence"],
  });

  assert.equal(diagnostic.query, "VID-APPROVAL-005 invoice approval");
  assert.equal(diagnostic.expectedDocumentsIndexed, true);
  assert.equal(diagnostic.expectedChunksIndexed, true);
  assert.equal(diagnostic.expectedChunksContainExactQueryTerms, true);
  assert.equal(diagnostic.expectedChunksMatchAliasTerms, true);
  assert.equal(diagnostic.foundInVectorCandidates, true);
  assert.equal(diagnostic.foundInMergedCandidates, true);
  assert.equal(diagnostic.foundInFinalTopK, false);
  assert.equal(diagnostic.droppedAtStage, "final_ranking");
  assert.equal(diagnostic.likelyCause, "ranking_or_vocabulary_mismatch");
  assert.equal(diagnostic.expectedEvidence[0].documentId, "doc-invoice");
  assert.equal(diagnostic.finalMergedTopResults[0].sourcePath, "D:\\bench\\readme_master.md");
});

test("decideExperiment applies safety gates before improvement checks", () => {
  const before = {
    aqi: 69.6,
    recallAt10: 0.89,
    precisionAt10: 0.22,
    p95LatencyMs: 377,
    criticalFailures: 1,
    permissionLeakage: 0,
  };

  assert.equal(
    benchmark.decideExperiment(before, { ...before, precisionAt10: 0.28 }),
    "keep",
  );
  assert.equal(
    benchmark.decideExperiment(before, { ...before, aqi: 72 }),
    "keep",
  );
  assert.equal(
    benchmark.decideExperiment(before, { ...before, permissionLeakage: 1, precisionAt10: 0.4 }),
    "revert",
  );
  assert.equal(
    benchmark.decideExperiment(before, { ...before, recallAt10: 0.85 }),
    "revert",
  );
  assert.equal(
    benchmark.decideExperiment(before, { ...before, precisionAt10: 0.24 }),
    "needs_more_data",
  );
});
