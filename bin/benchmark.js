#!/usr/bin/env node

const { spawn } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const readline = require("node:readline");

const ONE_BY_ONE_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAFgwJ/lErp8gAAAABJRU5ErkJggg==",
  "base64",
);

const MODULES = [
  ["shipping_module.md", "Shipping Service", "thermal printer printhead replacement", "SHIP-NODE-SURYA"],
  ["billing_module.md", "Billing Service", "payment reconciliation ledger", "BILL-CLOSE-224"],
  ["inventory_module.md", "Inventory Service", "warehouse slotting inventory reserve", "INV-SLOT-090"],
  ["routing_module.md", "Routing Service", "barcode route optimization dispatch", "ROUTE-KAPPA-118"],
  ["returns_module.md", "Returns Service", "refund inspection quarantine", "RET-QUEUE-041"],
  ["notifications_module.md", "Notifications Service", "email webhook retry", "NOTIFY-EDGE-077"],
  ["auth_module.md", "Auth Service", "session refresh token", "AUTH-GATE-302"],
  ["catalog_module.md", "Catalog Service", "product variant enrichment", "CAT-MERGE-615"],
  ["orders_module.md", "Orders Service", "order allocation promise", "ORDER-PROMISE-026"],
  ["fulfillment_module.md", "Fulfillment Service", "pick pack wave", "FULFILL-WAVE-404"],
  ["analytics_module.md", "Analytics Service", "forecast dashboard anomaly", "ANALYTICS-CUBE-512"],
  ["search_module.md", "Search Service", "semantic ranking facet", "SEARCH-FACET-931"],
  ["support_module.md", "Support Service", "case escalation SLA", "SUPPORT-SLA-733"],
  ["mobile_module.md", "Mobile Service", "offline scan sync", "MOBILE-SCAN-188"],
  ["compliance_module.md", "Compliance Service", "retention policy audit", "COMP-AUDIT-640"],
];

const QUERY_CASES = [
  {
    label: "printer replacement",
    query: "thermal printer printhead replacement",
    relevantFiles: ["shipping_module.md"],
    k: 3,
    mustIncludeTopK: ["shipping_module.md"],
    category: "accuracy",
  },
  {
    label: "atlas incident anchor",
    query: "INC-2026-ATLAS-014",
    relevantFiles: ["syslog_03.txt", "shipping_module.md"],
    k: 5,
    mustIncludeTopK: ["syslog_03.txt", "shipping_module.md"],
    category: "graph",
  },
  {
    label: "active module listing",
    query: "active module listing",
    relevantFiles: ["shipping_module.md", "billing_module.md", "inventory_module.md"],
    k: 5,
    category: "downrank",
  },
  {
    label: "audit item 42",
    query: "audit log item 42",
    relevantFiles: ["inventory_audit.json"],
    k: 5,
    mustIncludeTopK: ["inventory_audit.json"],
    category: "json",
  },
  {
    label: "payment ledger",
    query: "payment reconciliation ledger",
    relevantFiles: ["billing_module.md"],
    k: 5,
    mustIncludeTopK: ["billing_module.md"],
    category: "accuracy",
  },
  {
    label: "route optimization",
    query: "barcode route optimization dispatch",
    relevantFiles: ["routing_module.md"],
    k: 5,
    mustIncludeTopK: ["routing_module.md"],
    category: "accuracy",
  },
  {
    label: "slotting reserve",
    query: "warehouse slotting inventory reserve",
    relevantFiles: ["inventory_module.md"],
    k: 5,
    mustIncludeTopK: ["inventory_module.md"],
    category: "accuracy",
  },
  {
    label: "webhook retry",
    query: "email webhook retry notification",
    relevantFiles: ["notifications_module.md"],
    k: 5,
    mustIncludeTopK: ["notifications_module.md"],
    category: "accuracy",
  },
  {
    label: "session token",
    query: "session refresh token auth gate",
    relevantFiles: ["auth_module.md"],
    k: 5,
    mustIncludeTopK: ["auth_module.md"],
    category: "accuracy",
  },
  {
    label: "pick pack wave",
    query: "pick pack wave fulfillment",
    relevantFiles: ["fulfillment_module.md"],
    k: 5,
    mustIncludeTopK: ["fulfillment_module.md"],
    category: "accuracy",
  },
  {
    label: "offline scan",
    query: "offline scan sync mobile",
    relevantFiles: ["mobile_module.md"],
    k: 5,
    mustIncludeTopK: ["mobile_module.md"],
    category: "accuracy",
  },
  {
    label: "alert error",
    query: "ALERT conveyor thermal threshold",
    relevantFiles: ["syslog_07.txt"],
    k: 5,
    category: "accuracy",
  },
  {
    label: "invoice approval",
    query: "VID-APPROVAL-005 invoice approval",
    relevantFiles: ["img_invoice_02.png"],
    k: 5,
    category: "graph",
  },
  {
    label: "surya node",
    query: "SHIP-NODE-SURYA cooling lane",
    relevantFiles: ["shipping_module.md", "inventory_audit.json"],
    k: 5,
    category: "graph",
  },
  {
    label: "retention audit",
    query: "retention policy audit compliance",
    relevantFiles: ["compliance_module.md"],
    k: 5,
    mustIncludeTopK: ["compliance_module.md"],
    category: "accuracy",
  },
];

function generateDataset(rootDir, options = {}) {
  const scale = options.scale === "full" ? "full" : "quick";
  fs.rmSync(rootDir, { recursive: true, force: true });
  fs.mkdirSync(rootDir, { recursive: true });

  const sourceFiles = [];
  const mediaFiles = [];
  const freshSidecars = [];
  const staleOrMissingSidecars = [];

  function writeTextFile(name, text) {
    const file = path.join(rootDir, name);
    fs.writeFileSync(file, text, "utf8");
    sourceFiles.push(file);
    return file;
  }

  writeTextFile("inventory_audit.json", JSON.stringify(buildInventoryAudit(scale), null, 2));
  writeTextFile("activity_log.csv", buildActivityLogCsv(scale));

  for (const [filename, title, phrase, anchor] of MODULES) {
    writeTextFile(filename, moduleMarkdown(title, phrase, anchor, filename, scale));
  }

  for (let i = 1; i <= 20; i += 1) {
    const name = `syslog_${String(i).padStart(2, "0")}.txt`;
    writeTextFile(name, syslogText(i));
  }

  for (let i = 1; i <= 8; i += 1) {
    const filename = `img_invoice_${String(i).padStart(2, "0")}.png`;
    const file = path.join(rootDir, filename);
    fs.writeFileSync(file, ONE_BY_ONE_PNG);
    sourceFiles.push(file);
    mediaFiles.push(file);

    const sidecar = sidecarPath(file);
    if (i <= 4) {
      const text =
        i === 2
          ? `${"Invoice OCR: VID-APPROVAL-005 invoice approval confirms dock payment evidence. ".repeat(8)}`
          : `Invoice OCR: cached invoice ${i} references dock receipt INV-CACHED-${String(i).padStart(3, "0")}.`;
      fs.writeFileSync(sidecar, text, "utf8");
      setRelativeMtime(file, sidecar, 10);
      freshSidecars.push(sidecar);
    } else if (i <= 6) {
      fs.writeFileSync(sidecar, `Stale OCR sidecar for invoice ${i}.`, "utf8");
      setRelativeMtime(file, sidecar, -10);
      staleOrMissingSidecars.push(sidecar);
    } else {
      staleOrMissingSidecars.push(sidecar);
    }
  }

  for (let i = 1; i <= 5; i += 1) {
    const filename = `video_record_${String(i).padStart(2, "0")}.mp4`;
    const file = path.join(rootDir, filename);
    fs.writeFileSync(file, Buffer.from(`mock mp4 ${i}\n`));
    sourceFiles.push(file);
    mediaFiles.push(file);

    const sidecar = sidecarPath(file);
    fs.writeFileSync(
      sidecar,
      `Whisper transcript ${i}: warehouse dispatch note VID-TRANSCRIPT-${String(i).padStart(3, "0")} with package movement timing.`,
      "utf8",
    );
    setRelativeMtime(file, sidecar, 10);
    freshSidecars.push(sidecar);
  }

  writeTextFile("readme_master.md", readmeMaster());
  writeTextFile("manifest.json", JSON.stringify({ files: sourceFiles.map((file) => path.basename(file)) }, null, 2));

  return {
    rootDir,
    scale,
    sourceFiles,
    mediaFiles,
    freshSidecars,
    staleOrMissingSidecars,
    preprocessPlan: {
      total: mediaFiles.length,
      cacheHits: freshSidecars.length,
      expectedRuns: staleOrMissingSidecars.length,
    },
    totalBytes: sourceFiles.reduce((sum, file) => sum + fs.statSync(file).size, 0),
  };
}

function buildInventoryAudit(scale) {
  const noteRepeat = scale === "full" ? 180 : 18;
  return {
    generated_at: "2026-05-23T00:00:00Z",
    corpus: "anubis benchmark inventory audit",
    records: Array.from({ length: 60 }, (_, i) => {
      const node = i === 42 ? "SHIP-NODE-SURYA" : `NODE-${String(i).padStart(3, "0")}`;
      return {
        audit_id: `AUDIT-${String(i).padStart(4, "0")}`,
        audit_query_token: `audit log item ${i}`,
        component: i === 42 ? "thermal cooling lane replacement queue" : `component-${i % 9}`,
        node,
        device_status: i % 4 === 0 ? "attention" : "nominal",
        remediation: i === 42 ? "Inspect SHIP-NODE-SURYA printhead staging sensor." : "Continue monitoring.",
        notes: "inventory telemetry payload ".repeat(noteRepeat),
      };
    }),
  };
}

function buildActivityLogCsv(scale) {
  const rowCount = scale === "full" ? 5000 : 500;
  const rows = ["log_id,node,severity,message,timestamp"];
  for (let i = 0; i < rowCount; i += 1) {
    const node = i % 41 === 0 ? "SHIP-NODE-SURYA" : `NODE-${String(i % 200).padStart(3, "0")}`;
    const severity = i % 37 === 0 ? "WARN" : "INFO";
    const message = i % 137 === 0 ? "thermal threshold drift observed" : "regular heartbeat and package scan";
    rows.push(`LOG-${String(i).padStart(5, "0")},${node},${severity},${message},2026-05-23T${String(i % 24).padStart(2, "0")}:00:00Z`);
  }
  return rows.join("\n");
}

function moduleMarkdown(title, phrase, anchor, filename, scale) {
  const detailRepeat = scale === "full" ? 45 : 10;
  const atlasNote =
    filename === "shipping_module.md"
      ? "\nIncident resolution: INC-2026-ATLAS-014 was resolved by replacing the thermal printer printhead and recalibrating SHIP-NODE-SURYA cooling lane labels.\n"
      : "";
  return `# ${title}

Active module listing: ${title} owns the ${phrase} workflow.

Primary operating phrase: ${phrase}.
Operational anchor: ${anchor}.
${atlasNote}
Runbook:
- Validate queue depth and recent deployment health.
- Compare device telemetry with activity_log.csv before escalation.
- Record outcome in the active module listing for shift handoff.

Detail:
${(`${phrase} ${anchor} active module listing operational diagnostic paragraph. `).repeat(detailRepeat)}
`;
}

function syslogText(i) {
  if (i === 3) {
    return [
      "2026-05-23T03:14:00Z ERROR atlas gateway detected INC-2026-ATLAS-014.",
      "Thermal printer printhead replacement required before shipping labels can clear.",
      "Escalate to shipping_module.md for the resolution runbook.",
    ].join("\n");
  }
  if (i === 7) {
    return [
      "2026-05-23T07:18:00Z ALERT conveyor thermal threshold exceeded.",
      "Cooling lane reported sustained heat and operator acknowledgement delay.",
      "Recommended action: inspect conveyor thermal threshold guard.",
    ].join("\n");
  }
  return [
    `2026-05-23T${String(i).padStart(2, "0")}:00:00Z INFO node syslog ${i} heartbeat normal.`,
    `Trace token SYSLOG-${String(i).padStart(2, "0")} package scanner latency nominal.`,
    "No active incident; retain log for baseline retrieval quality.",
  ].join("\n");
}

function readmeMaster() {
  return `# Benchmark Master README

This reference document is an active module listing that names every service and anchor.
It intentionally repeats terms from shipping_module.md, billing_module.md, and inventory_module.md
so the benchmark can verify reference down-ranking.

Reference anchors:
- VID-APPROVAL-005 appears here as a manifest-style cross-reference.
- INC-2026-ATLAS-014 is listed here but the resolution belongs in shipping_module.md.
- SHIP-NODE-SURYA appears here as a reference pointer, not primary evidence.
`;
}

function sidecarPath(sourceFile) {
  const parsed = path.parse(sourceFile);
  return path.join(parsed.dir, `${parsed.name}.anubis.txt`);
}

function setRelativeMtime(sourceFile, sidecarFile, deltaSeconds) {
  const sourceTime = new Date(Date.now() - 60_000);
  const sidecarTime = new Date(sourceTime.getTime() + deltaSeconds * 1000);
  fs.utimesSync(sourceFile, sourceTime, sourceTime);
  fs.utimesSync(sidecarFile, sidecarTime, sidecarTime);
}

function percentile(values, p) {
  if (!values.length) {
    return 0;
  }
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.ceil((p / 100) * sorted.length) - 1;
  return sorted[Math.max(0, Math.min(sorted.length - 1, index))];
}

function calculateAqi({ averageRecallAt5, p95LatencyMs }) {
  const latencyScore = Math.max(0, 100 - p95LatencyMs / 5);
  return Math.max(0, Math.min(100, round1(0.7 * (averageRecallAt5 * 100) + 0.3 * latencyScore)));
}

function classifyQuery(recall, precision) {
  if (recall === 0) return "critical_fail";
  if (recall >= 0.85 && precision >= 0.45) return "strong_pass";
  if (recall >= 0.75 && precision >= 0.35) return "pass";
  if (recall >= 0.60 && precision >= 0.20) return "weak_pass";
  return "fail";
}

function evaluateSearchCase(testCase, results) {
  const k = testCase.k || 5;
  const topK = results.slice(0, k);
  const topFilenames = topK.map((result) => result.filename);
  const relevant = new Set(testCase.relevantFiles || []);
  const returnedRelevant = new Set(topFilenames.filter((filename) => relevant.has(filename)));
  const required = testCase.mustIncludeTopK || [];
  const missingRequired = required.filter((filename) => !topFilenames.includes(filename));
  const recallAtK = relevant.size === 0 ? 1 : returnedRelevant.size / relevant.size;
  const precisionAtK = returnedRelevant.size / k;
  const hasRelevantHit = relevant.size === 0 || returnedRelevant.size > 0;
  const ranking = rankingMetrics(results, relevant);
  const queryStatus = classifyQuery(recallAtK, precisionAtK);

  return {
    label: testCase.label,
    query: testCase.query,
    recallAtK: round2(recallAtK),
    precisionAtK: round2(precisionAtK),
    ...ranking,
    queryStatus,
    status: missingRequired.length === 0 && hasRelevantHit ? "PASS" : "FAIL",
    missingRequired,
    topFilenames,
  };
}

function rankingMetrics(results, relevant) {
  const relevantSet = relevant instanceof Set ? relevant : new Set(relevant || []);
  const top5 = results.slice(0, 5).map((result) => result.filename);
  const top10 = results.slice(0, 10).map((result) => result.filename);
  const recallAt5 = recallAt(top5, relevantSet);
  const recallAt10 = recallAt(top10, relevantSet);
  const precisionAt5 = precisionAt(top5, relevantSet, 5);
  const precisionAt10 = precisionAt(top10, relevantSet, 10);
  const firstRelevantIndex = top10.findIndex((filename) => relevantSet.has(filename));

  return {
    recallAt5: round2(recallAt5),
    recallAt10: round2(recallAt10),
    precisionAt5: round2(precisionAt5),
    precisionAt10: round2(precisionAt10),
    top1Accuracy: top10[0] && relevantSet.has(top10[0]) ? 1 : 0,
    top3Accuracy: top10.slice(0, 3).some((filename) => relevantSet.has(filename)) ? 1 : 0,
    mrrAt10: round2(firstRelevantIndex === -1 ? 0 : 1 / (firstRelevantIndex + 1)),
    ndcgAt10: round2(ndcgAt(top10, relevantSet, 10)),
  };
}

function recallAt(filenames, relevantSet) {
  if (relevantSet.size === 0) {
    return 1;
  }
  return new Set(filenames.filter((filename) => relevantSet.has(filename))).size / relevantSet.size;
}

function precisionAt(filenames, relevantSet, k) {
  if (k === 0) {
    return 0;
  }
  return filenames.filter((filename) => relevantSet.has(filename)).length / k;
}

function ndcgAt(filenames, relevantSet, k) {
  if (relevantSet.size === 0) {
    return 1;
  }
  const seenRelevant = new Set();
  const dcg = filenames.slice(0, k).reduce((sum, filename, index) => {
    if (!relevantSet.has(filename) || seenRelevant.has(filename)) {
      return sum;
    }
    seenRelevant.add(filename);
    return sum + 1 / Math.log2(index + 2);
  }, 0);
  const idealHits = Math.min(relevantSet.size, k);
  const idcg = Array.from({ length: idealHits }, (_, index) => 1 / Math.log2(index + 2)).reduce(
    (sum, value) => sum + value,
    0,
  );
  return idcg === 0 ? 0 : dcg / idcg;
}

function debugSearchResults(results, debug = {}) {
  if (!debug.includeScoreBreakdown) {
    return undefined;
  }
  const limit = Math.max(0, debug.includeTopResults || 5);
  return results.slice(0, limit).map((result) => ({
    resultId: result.chunk_id || result.id || `${result.doc_id || "unknown"}:${result.filename || "unknown"}`,
    documentId: result.doc_id,
    chunkId: result.chunk_id,
    title: result.title || result.filename,
    sourcePath: result.path,
    scoreBreakdown: cleanObject({
      vector: numberOrUndefined(result.score_vec),
      bm25: numberOrUndefined(result.score_bm25),
      graph: numberOrUndefined(result.score_graph),
      entity: numberOrUndefined(result.score_entity),
      sourceQuality: numberOrUndefined(result.score_centrality),
      final: numberOrUndefined(result.score) || 0,
    }),
  }));
}

function buildPrecisionDiagnostic({ testCase, report, results }) {
  const relevant = new Set(testCase.relevantFiles || []);
  const falsePositives = candidateSummaries(results.slice(0, 10))
    .filter((candidate) => !relevant.has(candidate.filename))
    .map((candidate) => ({
      ...candidate,
      profile: falsePositiveProfile(candidate.scoreBreakdown || {}),
    }));
  const falsePositiveProfiles = falsePositives.reduce((counts, candidate) => {
    counts[candidate.profile] = (counts[candidate.profile] || 0) + 1;
    return counts;
  }, {});

  return {
    label: testCase.label,
    query: testCase.query,
    precisionAt10: report.precisionAt10,
    relevantFiles: Array.from(relevant),
    falsePositiveCount: falsePositives.length,
    falsePositiveProfiles,
    falsePositives,
  };
}

function falsePositiveProfile(score = {}) {
  const vector = Number(score.vector || 0);
  const bm25 = Number(score.bm25 || 0);
  const graph = Number(score.graph || 0);
  const entity = Number(score.entity || 0);

  if (graph >= 0.5) return "graph_assisted";
  if (vector > 0 && bm25 === 0 && graph === 0 && entity === 0) return "vector_only";
  if (bm25 > 0 || entity > 0) return "lexical_entity";
  return "other";
}

function graphMetricsFromStats(stats = {}) {
  const totalNodes = Number(stats.chunks || 0);
  const totalEdges = Number(stats.graph_edges || 0);
  const edgesByType = stats.edges_by_type || {};
  const evidenceEdges = ["shared_anchor", "shared_entity", "manifest_overlap"].reduce(
    (sum, key) => sum + Number(edgesByType[key] || 0),
    0,
  );
  const visibleEdges = evidenceEdges;
  return {
    totalNodes,
    totalEdges,
    edgesPerChunk: totalNodes === 0 ? 0 : round2(totalEdges / totalNodes),
    candidateEdges: totalEdges,
    visibleEdges,
    visibleEdgesPerNode: totalNodes === 0 ? 0 : round2(visibleEdges / totalNodes),
    weakEdgeRatio: null,
    duplicateEdgeRatio: null,
    edgeEvidenceCoverage: visibleEdges === 0 ? 1 : round2(evidenceEdges / visibleEdges),
    edgesByType,
  };
}

function indexingPhaseTimings(totalMs) {
  return {
    discoveryMs: null,
    cacheCheckMs: null,
    textExtractionMs: null,
    ocrMs: null,
    chunkingMs: null,
    embeddingMs: null,
    edgeGenerationMs: null,
    dbWriteMs: null,
    totalMs: Math.round(totalMs),
  };
}

function queryStatusCounts(searchReports) {
  const counts = {
    strong_pass: 0,
    pass: 0,
    weak_pass: 0,
    fail: 0,
    critical_fail: 0,
  };
  for (const report of searchReports) {
    counts[report.queryStatus] = (counts[report.queryStatus] || 0) + 1;
  }
  return counts;
}

function criticalFailureCount(searchReports) {
  return searchReports.filter((item) => {
    return item.queryStatus === "critical_fail" && item.category !== "downrank";
  }).length;
}

function buildCriticalFailureDiagnostic({
  testCase,
  report,
  docs,
  chunksByDocId,
  diagnosticResults,
  aliases = [],
}) {
  const docsById = new Map(docs.map((doc) => [doc.id, doc]));
  const enrichedDiagnosticResults = diagnosticResults.map((result) => ({
    ...result,
    path: result.path || docsById.get(result.doc_id)?.path || null,
  }));
  const relevantFiles = new Set(testCase.relevantFiles || []);
  const expectedDocs = docs.filter((doc) => relevantFiles.has(doc.filename));
  const expectedChunks = expectedDocs.flatMap((doc) => {
    return (chunksByDocId.get(doc.id) || []).map((chunk) => ({ ...chunk, document: doc }));
  });
  const expectedFilenames = new Set(expectedDocs.map((doc) => doc.filename));
  const exactTerms = queryTerms(testCase.query);
  const aliasList = Array.isArray(aliases) ? aliases : [];
  const expectedChunksContainExactQueryTerms = expectedChunks.some((chunk) =>
    contentContainsTerms(chunk.content, exactTerms),
  );
  const aliasMatches = aliasList.filter((alias) =>
    expectedChunks.some((chunk) => contentContainsTerms(chunk.content, queryTerms(alias))),
  );
  const expectedChunksMatchAliasTerms = aliasList.length ? aliasMatches.length > 0 : null;

  const topBm25Candidates = topCandidatesByScore(enrichedDiagnosticResults, "score_bm25");
  const topVectorCandidates = topCandidatesByScore(enrichedDiagnosticResults, "score_vec");
  const topGraphCandidates = topCandidatesByScore(enrichedDiagnosticResults, "score_graph");
  const finalMergedTopResults = candidateSummaries(enrichedDiagnosticResults.slice(0, 10));

  const foundInBm25Candidates = containsExpected(topBm25Candidates, expectedFilenames);
  const foundInVectorCandidates = containsExpected(topVectorCandidates, expectedFilenames);
  const foundInGraphCandidates = containsExpected(topGraphCandidates, expectedFilenames);
  const foundInMergedCandidates = enrichedDiagnosticResults.some((result) => expectedFilenames.has(result.filename));
  const foundInFinalTopK = (report.topFilenames || []).some((filename) => expectedFilenames.has(filename));
  const droppedAtStage = inferDroppedAtStage({
    expectedDocumentsIndexed: expectedDocs.length === relevantFiles.size,
    expectedChunksIndexed: expectedChunks.length > 0,
    expectedChunksContainExactQueryTerms,
    expectedChunksMatchAliasTerms,
    foundInMergedCandidates,
    foundInFinalTopK,
  });

  const notes = [
    "Raw BM25/vector/graph candidate pools are not exposed by the current MCP API; candidate lists are component-sorted views of the debug diagnostic search result set.",
  ];
  if (!aliasList.length) {
    notes.push("No alias terms configured for this query.");
  }

  return {
    query: testCase.query,
    expectedEvidence: expectedDocs.map((doc) => {
      const chunks = chunksByDocId.get(doc.id) || [];
      return {
        filename: doc.filename,
        documentId: doc.id,
        sourcePath: doc.path || null,
        chunkIds: chunks.map((chunk) => chunk.id),
        exactQueryTermsFound: chunks.some((chunk) => contentContainsTerms(chunk.content, exactTerms)),
        aliasTermsFound: aliasList.length
          ? aliasList.filter((alias) =>
              chunks.some((chunk) => contentContainsTerms(chunk.content, queryTerms(alias))),
            )
          : null,
      };
    }),
    expectedDocumentsIndexed: expectedDocs.length === relevantFiles.size,
    expectedChunksIndexed: expectedChunks.length > 0,
    expectedChunksContainExactQueryTerms,
    expectedChunksMatchAliasTerms,
    foundInBm25Candidates,
    foundInVectorCandidates,
    foundInGraphCandidates,
    foundInMergedCandidates,
    foundInFinalTopK,
    droppedAtStage,
    likelyCause: likelyCauseForDrop(droppedAtStage, {
      foundInVectorCandidates,
      foundInBm25Candidates,
      foundInGraphCandidates,
      expectedChunksContainExactQueryTerms,
    }),
    topBm25Candidates,
    topVectorCandidates,
    topGraphCandidates,
    finalMergedTopResults,
    scoreBreakdownForTopCandidates: finalMergedTopResults,
    filtersDownrankRulesApplied: [
      "reference_doc_text_signal_multiplier",
      "low_signal_chunk_text_multiplier",
      "centrality_relevance_gate",
      "max_results_per_doc_diversification",
    ],
    expectedSourcePaths: expectedDocs.map((doc) => doc.path || null),
    retrievedSourcePaths: finalMergedTopResults.map((result) => result.sourcePath || null),
    notes,
  };
}

function queryTerms(text) {
  return Array.from(
    new Set(
      String(text || "")
        .toLowerCase()
        .split(/[^a-z0-9]+/)
        .map((term) => term.trim())
        .filter(Boolean),
    ),
  );
}

function contentContainsTerms(content, terms) {
  const normalized = String(content || "").toLowerCase();
  return terms.every((term) => normalized.includes(term));
}

function topCandidatesByScore(results, scoreKey, limit = 10) {
  return candidateSummaries(
    results
      .filter((result) => Number(result[scoreKey] || 0) > 0)
      .slice()
      .sort((a, b) => Number(b[scoreKey] || 0) - Number(a[scoreKey] || 0))
      .slice(0, limit),
  );
}

function candidateSummaries(results) {
  return results.map((result) => ({
    resultId: result.chunk_id || result.id || `${result.doc_id || "unknown"}:${result.filename || "unknown"}`,
    documentId: result.doc_id,
    chunkId: result.chunk_id,
    filename: result.filename,
    title: result.title || result.filename,
    sourcePath: result.path || null,
    scoreBreakdown: cleanObject({
      vector: numberOrUndefined(result.score_vec),
      bm25: numberOrUndefined(result.score_bm25),
      graph: numberOrUndefined(result.score_graph),
      entity: numberOrUndefined(result.score_entity),
      sourceQuality: numberOrUndefined(result.score_centrality),
      final: numberOrUndefined(result.score) || 0,
    }),
  }));
}

function containsExpected(candidates, expectedFilenames) {
  return candidates.some((candidate) => expectedFilenames.has(candidate.filename));
}

function inferDroppedAtStage({
  expectedDocumentsIndexed,
  expectedChunksIndexed,
  expectedChunksContainExactQueryTerms,
  expectedChunksMatchAliasTerms,
  foundInMergedCandidates,
  foundInFinalTopK,
}) {
  if (!expectedDocumentsIndexed) return "document_indexing";
  if (!expectedChunksIndexed) return "chunking";
  if (!expectedChunksContainExactQueryTerms && expectedChunksMatchAliasTerms !== true) {
    return "content_or_vocabulary";
  }
  if (!foundInMergedCandidates) return "candidate_generation";
  if (!foundInFinalTopK) return "final_ranking";
  return "none";
}

function likelyCauseForDrop(stage, signals) {
  if (stage === "document_indexing" || stage === "chunking") return "indexing_gap";
  if (stage === "content_or_vocabulary") return "vocabulary_or_extraction_gap";
  if (stage === "candidate_generation") return "candidate_generation_gap";
  if (stage === "final_ranking") {
    if (signals.foundInVectorCandidates && !signals.foundInBm25Candidates) {
      return "ranking_or_vocabulary_mismatch";
    }
    return "ranking_or_diversification";
  }
  return "none";
}

function decideExperiment(before, after) {
  if (after.permissionLeakage > 0) return "revert";
  if (after.criticalFailures > before.criticalFailures) return "revert";
  if (after.recallAt10 < before.recallAt10 - 0.03) return "revert";
  if (after.p95LatencyMs > 500) return "revert";

  const precisionGain = after.precisionAt10 - before.precisionAt10;
  const aqiGain = after.aqi - before.aqi;
  const evidenceGain = Number(after.edgeEvidenceCoverage || 0) - Number(before.edgeEvidenceCoverage || 0);

  if (
    precisionGain >= 0.05 ||
    aqiGain >= 2 ||
    evidenceGain >= 0.10 ||
    (Number.isFinite(after.visibleEdgesPerNode) && after.visibleEdgesPerNode <= 15)
  ) return "keep";
  return "needs_more_data";
}

async function runBenchmark(options = {}) {
  const repoRoot = options.repoRoot || path.resolve(__dirname, "..");
  const scratchRoot = options.dataDir
    ? path.resolve(options.dataDir)
    : path.join(repoRoot, "scratch", "temp_benchmark_data");
  const runRoot = fs.mkdtempSync(path.join(os.tmpdir(), "anubis-benchmark-run-"));
  const dbPath = path.join(runRoot, "anubis-benchmark.db");
  const ftsPath = path.join(runRoot, "fts_index");
  const workdirsRoot = path.join(runRoot, "workdirs");

  let client;
  try {
    const dataset = generateDataset(scratchRoot, { scale: options.scale });
    const workdir = resolveBenchmarkWorkdir(options.workdir, dataset);
    if (options.generateOnly) {
      return {
        dataset,
        report: null,
        summary: { generatedOnly: true, workdir },
      };
    }

    const binPath = resolveEngineBinary(options.bin, repoRoot);
    const modelCacheDir = defaultAppModelCacheDir(repoRoot);
    client = new JsonRpcClient(binPath, {
      ANUBIS_DB_PATH: dbPath,
      ANUBIS_FTS_PATH: ftsPath,
      ANUBIS_EMBED_MODELS_DIR: modelCacheDir,
      ANUBIS_OCR_MODELS_DIR: modelCacheDir,
    });

    const bootstrapStart = nowMs();
    await client.start();
    await client.request("initialize", {
      protocolVersion: "2025-06-18",
      capabilities: {},
      clientInfo: { name: "anubis-benchmark", version: "1.0.0" },
    });
    await client.notify("notifications/initialized", {});
    const bootstrapMs = nowMs() - bootstrapStart;

    const indexStart = nowMs();
    await callTool(client, "anubis_index_folder", withWorkdir(workdir, { path: dataset.rootDir }));
    const indexMs = nowMs() - indexStart;

    const stats = await callTool(client, "anubis_get_index_stats", withWorkdir(workdir));
    const docs = await callTool(client, "anubis_list_documents", withWorkdir(workdir));

    const searchReports = [];
    const queryLatencies = [];
    const resultCache = new Map();
    const debug = options.debug || {};

    for (const testCase of QUERY_CASES) {
      const queryStart = nowMs();
      const results = await callTool(client, "anubis_search", {
        workdir,
        q: testCase.query,
        limit: 10,
        depth: 2,
      });
      const elapsed = nowMs() - queryStart;
      queryLatencies.push(elapsed);
      resultCache.set(testCase.label, results);
      const report = evaluateSearchCase(testCase, results);
      searchReports.push({
        ...report,
        debugTopResults: debugSearchResults(results, debug),
        latencyMs: Math.round(elapsed),
        category: testCase.category,
      });
    }

    const graphCheck = await evaluateGraphCheck(client, resultCache.get("atlas incident anchor") || [], workdir);
    const downrankCheck = evaluateDownrank(resultCache.get("active module listing") || []);
    const jsonCheck = await evaluateJsonChunking(client, docs, workdir);
    const criticalFailureDiagnostics = debug.includeCriticalFailureDiagnostics
      ? await buildCriticalFailureDiagnostics(client, {
          workdir,
          docs,
          searchReports,
          resultCache,
          queryCases: QUERY_CASES,
          aliases: debug.aliases || {},
          candidateLimit: debug.includeDiagnosticCandidates || 50,
        })
      : [];
    const precisionDiagnostics = debug.includePrecisionDiagnostics
      ? searchReports.map((report) => {
          const testCase = QUERY_CASES.find((item) => item.label === report.label);
          return buildPrecisionDiagnostic({
            testCase,
            report,
            results: resultCache.get(report.label) || [],
          });
        })
      : [];

    const averageRecallAtK =
      searchReports.reduce((sum, item) => sum + item.recallAtK, 0) / searchReports.length;
    const averagePrecisionAtK =
      searchReports.reduce((sum, item) => sum + item.precisionAtK, 0) / searchReports.length;
    const averageRecallAt5 =
      searchReports.reduce((sum, item) => sum + item.recallAt5, 0) / searchReports.length;
    const averagePrecisionAt5 =
      searchReports.reduce((sum, item) => sum + item.precisionAt5, 0) / searchReports.length;
    const averageRecallAt10 =
      searchReports.reduce((sum, item) => sum + item.recallAt10, 0) / searchReports.length;
    const averagePrecisionAt10 =
      searchReports.reduce((sum, item) => sum + item.precisionAt10, 0) / searchReports.length;
    const averageTop1Accuracy =
      searchReports.reduce((sum, item) => sum + item.top1Accuracy, 0) / searchReports.length;
    const averageTop3Accuracy =
      searchReports.reduce((sum, item) => sum + item.top3Accuracy, 0) / searchReports.length;
    const averageMrrAt10 =
      searchReports.reduce((sum, item) => sum + item.mrrAt10, 0) / searchReports.length;
    const averageNdcgAt10 =
      searchReports.reduce((sum, item) => sum + item.ndcgAt10, 0) / searchReports.length;
    const p50LatencyMs = percentile(queryLatencies, 50);
    const p95LatencyMs = percentile(queryLatencies, 95);
    const p99LatencyMs = percentile(queryLatencies, 99);
    const avgLatencyMs =
      queryLatencies.reduce((sum, value) => sum + value, 0) / Math.max(1, queryLatencies.length);

    const benchmarkStorageDir = resolveBenchmarkStorageDir(workdirsRoot);
    const actualDbPath = benchmarkStorageDir ? path.join(benchmarkStorageDir, "anubis.db") : dbPath;
    const dbSizeBytes = fileSizeIfExists(actualDbPath);
    const graphMetrics = graphMetricsFromStats(stats);
    const summary = {
      bootstrapMs: Math.round(bootstrapMs),
      workdir,
      workdirsRoot,
      dbPath: actualDbPath,
      dbSizeBytes,
      preprocess: dataset.preprocessPlan,
      indexMs: Math.round(indexMs),
      indexingPhases: indexingPhaseTimings(indexMs),
      throughputFilesPerSec: round1(dataset.sourceFiles.length / Math.max(indexMs / 1000, 0.001)),
      throughputKbPerSec: round1(dataset.totalBytes / 1024 / Math.max(indexMs / 1000, 0.001)),
      stats,
      graphMetrics,
      queryLatency: {
        averageMs: Math.round(avgLatencyMs),
        p50Ms: Math.round(p50LatencyMs),
        p95Ms: Math.round(p95LatencyMs),
        p99Ms: Math.round(p99LatencyMs),
      },
      averageRecallAtK: round2(averageRecallAtK),
      averagePrecisionAtK: round2(averagePrecisionAtK),
      averageRecallAt5: round2(averageRecallAt5),
      averagePrecisionAt5: round2(averagePrecisionAt5),
      averageRecallAt10: round2(averageRecallAt10),
      averagePrecisionAt10: round2(averagePrecisionAt10),
      top1Accuracy: round2(averageTop1Accuracy),
      top3Accuracy: round2(averageTop3Accuracy),
      mrrAt10: round2(averageMrrAt10),
      ndcgAt10: round2(averageNdcgAt10),
      queryStatusCounts: queryStatusCounts(searchReports),
      criticalFailures: criticalFailureCount(searchReports),
      permissionLeakage: 0,
      aqi: calculateAqi({ averageRecallAt5: averageRecallAtK, p95LatencyMs }),
      graphCheck,
      downrankCheck,
      jsonCheck,
      criticalFailureDiagnostics,
      precisionDiagnostics,
      searchReports,
      sourceFileCount: dataset.sourceFiles.length,
      scale: dataset.scale,
    };

    return {
      dataset,
      summary,
      report: formatReport(summary),
    };
  } finally {
    if (client) {
      await client.close();
    }
    if (!options.keepData) {
      await rmDirWithRetry(scratchRoot);
    }
    await rmDirWithRetry(runRoot);
  }
}

async function evaluateGraphCheck(client, anchorResults, workdir) {
  const seed = anchorResults.find((result) => /syslog_03\.txt|shipping_module\.md/.test(result.filename));
  if (!seed) {
    return { status: "FAIL", reason: "anchor query did not return a syslog/shipping seed" };
  }

  const neighborhood = await callTool(client, "anubis_get_graph_neighborhood", {
    workdir,
    chunk_id: seed.chunk_id,
    depth: 2,
    limit: 160,
  });
  const nodesById = new Map((neighborhood.nodes || []).map((node) => [node.chunk_id, node]));
  const edge = (neighborhood.edges || []).find((candidate) => {
    if (candidate.edge_type !== "shared_anchor" || candidate.edge_reason !== "anchor:INC-2026-ATLAS-014") {
      return false;
    }
    const src = nodesById.get(candidate.src_chunk);
    const dst = nodesById.get(candidate.dst_chunk);
    const filenames = [src && src.filename, dst && dst.filename];
    return filenames.includes("syslog_03.txt") && filenames.includes("shipping_module.md");
  });

  if (!edge) {
    return { status: "FAIL", reason: "missing shared_anchor edge for INC-2026-ATLAS-014" };
  }
  if (!edge.evidence || !edge.evidence.src_span || !edge.evidence.dst_span) {
    return { status: "FAIL", reason: "shared_anchor edge lacks citation spans" };
  }
  return { status: "PASS", edgeReason: edge.edge_reason };
}

async function buildCriticalFailureDiagnostics(client, options) {
  const docs = options.docs.documents || [];
  const diagnostics = [];
  for (const report of options.searchReports) {
    if (report.queryStatus !== "critical_fail" || report.category === "downrank") {
      continue;
    }
    const testCase = options.queryCases.find((item) => item.label === report.label);
    if (!testCase) {
      continue;
    }
    const relevant = new Set(testCase.relevantFiles || []);
    const expectedDocs = docs.filter((doc) => relevant.has(doc.filename));
    const chunksByDocId = new Map();
    for (const doc of expectedDocs) {
      const chunkResult = await callTool(client, "anubis_get_doc_chunks", {
        workdir: options.workdir,
        doc_id: doc.id,
      });
      chunksByDocId.set(doc.id, chunkResult.chunks || []);
    }
    const diagnosticResults = await callTool(client, "anubis_search", {
      workdir: options.workdir,
      q: testCase.query,
      limit: Math.min(Math.max(options.candidateLimit || 50, 10), 50),
      depth: 2,
    });
    diagnostics.push(
      buildCriticalFailureDiagnostic({
        testCase,
        report,
        docs,
        chunksByDocId,
        diagnosticResults,
        aliases: aliasesForQuery(options.aliases, testCase),
      }),
    );
  }
  return diagnostics;
}

function aliasesForQuery(aliasMap, testCase) {
  if (!aliasMap || typeof aliasMap !== "object") {
    return [];
  }
  return aliasMap[testCase.label] || aliasMap[testCase.query] || [];
}

function evaluateDownrank(results) {
  const readmeIndex = results.findIndex((result) => result.filename === "readme_master.md");
  const contentIndex = results.findIndex((result) => result.filename && result.filename.endsWith("_module.md"));
  if (contentIndex === -1) {
    return { status: "FAIL", reason: "no content module returned" };
  }
  if (readmeIndex === -1) {
    return { status: "PASS", reason: "reference document did not enter top results" };
  }
  return contentIndex < readmeIndex
    ? { status: "PASS", reason: "content module ranked above reference document" }
    : { status: "FAIL", reason: "reference document ranked above content module" };
}

async function evaluateJsonChunking(client, docsResult, workdir) {
  const docs = docsResult.documents || [];
  const doc = docs.find((item) => item.filename === "inventory_audit.json");
  if (!doc) {
    return { status: "FAIL", reason: "inventory_audit.json was not indexed" };
  }
  const chunkResult = await callTool(client, "anubis_get_doc_chunks", { workdir, doc_id: doc.id });
  const chunks = chunkResult.chunks || [];
  const item42 = chunks.find((chunk) => chunk.content.includes("audit log item 42"));
  if (!item42) {
    return { status: "FAIL", reason: "audit log item 42 did not appear in any chunk" };
  }
  if (chunks.length <= 1) {
    return { status: "FAIL", reason: "inventory JSON was not split into multiple chunks" };
  }
  return { status: "PASS", chunks: chunks.length, page: item42.page };
}

function formatReport(summary) {
  const line = "=".repeat(72);
  const queryLine = "-".repeat(70);
  const rows = summary.searchReports
    .map((item, index) => {
      const name = `${index + 1}. ${item.label}`.padEnd(30);
      const recall = item.recallAt10.toFixed(2).padEnd(10);
      const precision = item.precisionAt10.toFixed(2).padEnd(9);
      const status = item.queryStatus.padEnd(14);
      return `  ${name}${recall}${precision}${status}${String(item.latencyMs).padStart(5)} ms`;
    })
    .join("\n");
  const debugRows = summary.searchReports
    .filter((item) => item.debugTopResults && item.debugTopResults.length)
    .map((item) => {
      const topRows = item.debugTopResults
        .map((result, index) => {
          const score = result.scoreBreakdown || {};
          return `    ${index + 1}. ${result.title || result.resultId} final=${fmtScore(score.final)} vec=${fmtScore(score.vector)} bm25=${fmtScore(score.bm25)} graph=${fmtScore(score.graph)} entity=${fmtScore(score.entity)} source=${fmtScore(score.sourceQuality)}`;
        })
        .join("\n");
      return `  ${item.label}\n${topRows}`;
    })
    .join("\n");
  const criticalFailureRows = (summary.criticalFailureDiagnostics || [])
    .map((diagnostic) => {
      const evidence = diagnostic.expectedEvidence
        .map((item) => {
          return `    - ${item.filename} doc=${item.documentId || "n/a"} chunks=${item.chunkIds.length} exactTerms=${item.exactQueryTermsFound} aliases=${Array.isArray(item.aliasTermsFound) ? item.aliasTermsFound.join(", ") || "none" : "n/a"} path=${item.sourcePath || "n/a"}`;
        })
        .join("\n");
      const bm25 = diagnostic.topBm25Candidates
        .slice(0, 5)
        .map((item, index) => `    ${index + 1}. ${item.filename} final=${fmtScore(item.scoreBreakdown.final)} bm25=${fmtScore(item.scoreBreakdown.bm25)} vec=${fmtScore(item.scoreBreakdown.vector)} graph=${fmtScore(item.scoreBreakdown.graph)} path=${item.sourcePath || "n/a"}`)
        .join("\n");
      const vector = diagnostic.topVectorCandidates
        .slice(0, 5)
        .map((item, index) => `    ${index + 1}. ${item.filename} final=${fmtScore(item.scoreBreakdown.final)} bm25=${fmtScore(item.scoreBreakdown.bm25)} vec=${fmtScore(item.scoreBreakdown.vector)} graph=${fmtScore(item.scoreBreakdown.graph)} path=${item.sourcePath || "n/a"}`)
        .join("\n");
      const graph = diagnostic.topGraphCandidates
        .slice(0, 5)
        .map((item, index) => `    ${index + 1}. ${item.filename} final=${fmtScore(item.scoreBreakdown.final)} bm25=${fmtScore(item.scoreBreakdown.bm25)} vec=${fmtScore(item.scoreBreakdown.vector)} graph=${fmtScore(item.scoreBreakdown.graph)} path=${item.sourcePath || "n/a"}`)
        .join("\n");
      const merged = diagnostic.finalMergedTopResults
        .slice(0, 5)
        .map((item, index) => `    ${index + 1}. ${item.filename} final=${fmtScore(item.scoreBreakdown.final)} bm25=${fmtScore(item.scoreBreakdown.bm25)} vec=${fmtScore(item.scoreBreakdown.vector)} graph=${fmtScore(item.scoreBreakdown.graph)} path=${item.sourcePath || "n/a"}`)
        .join("\n");
      return `  Query: ${diagnostic.query}
  Expected docs indexed : ${diagnostic.expectedDocumentsIndexed}
  Expected chunks indexed: ${diagnostic.expectedChunksIndexed}
  Exact query terms      : ${diagnostic.expectedChunksContainExactQueryTerms}
  Alias terms            : ${fmtUnavailable(diagnostic.expectedChunksMatchAliasTerms)}
  Found in BM25 view     : ${diagnostic.foundInBm25Candidates}
  Found in vector view   : ${diagnostic.foundInVectorCandidates}
  Found in graph view    : ${diagnostic.foundInGraphCandidates}
  Found in merged view   : ${diagnostic.foundInMergedCandidates}
  Found in final top K   : ${diagnostic.foundInFinalTopK}
  Dropped at stage       : ${diagnostic.droppedAtStage}
  Likely cause           : ${diagnostic.likelyCause}
  Expected evidence:
${evidence || "    n/a"}
  Top BM25 candidates:
${bm25 || "    n/a"}
  Top vector candidates:
${vector || "    n/a"}
  Top graph candidates:
${graph || "    n/a"}
  Final merged top results:
${merged || "    n/a"}
  Rules considered       : ${diagnostic.filtersDownrankRulesApplied.join(", ")}
  Notes                  : ${diagnostic.notes.join(" ")}`;
    })
    .join("\n\n");

  return `${line}
                      ANUBIS ENGINE BENCHMARK REPORT
${line}
[SYSTEM INFO]
  Corpus Scale     : ${summary.scale}
  Bootstrap Time   : ${summary.bootstrapMs} ms
  Database Path    : ${summary.dbPath}
  Database Size    : ${formatBytes(summary.dbSizeBytes)}

[INDEXING PERFORMANCE]
  Pre-pass Plan    : ${summary.preprocess.total} files checked, ${summary.preprocess.cacheHits} cache hits, ${summary.preprocess.expectedRuns} expected OCR runs
  Index Pass Time  : ${(summary.indexMs / 1000).toFixed(2)} s
  Throughput       : ${summary.throughputFilesPerSec} files/sec (${summary.throughputKbPerSec} KB/s)
  Total Row Counts : Documents: ${summary.stats.documents || 0} | Chunks: ${summary.stats.chunks || 0} | Edges: ${summary.stats.graph_edges || 0}
  Phase Timings    : discovery ${fmtMs(summary.indexingPhases.discoveryMs)} | cache ${fmtMs(summary.indexingPhases.cacheCheckMs)} | text ${fmtMs(summary.indexingPhases.textExtractionMs)} | OCR ${fmtMs(summary.indexingPhases.ocrMs)} | chunking ${fmtMs(summary.indexingPhases.chunkingMs)} | embedding ${fmtMs(summary.indexingPhases.embeddingMs)} | edges ${fmtMs(summary.indexingPhases.edgeGenerationMs)} | db ${fmtMs(summary.indexingPhases.dbWriteMs)} | total ${fmtMs(summary.indexingPhases.totalMs)}

[GRAPH QUALITY]
  Total Nodes      : ${summary.graphMetrics.totalNodes}
  Total Edges      : ${summary.graphMetrics.totalEdges}
  Edges/Chunk      : ${summary.graphMetrics.edgesPerChunk}
  Candidate Edges  : ${summary.graphMetrics.candidateEdges}
  Visible Edges    : ${fmtUnavailable(summary.graphMetrics.visibleEdges)}
  Visible/Node     : ${fmtUnavailable(summary.graphMetrics.visibleEdgesPerNode)}
  Weak Edge Ratio  : ${fmtUnavailable(summary.graphMetrics.weakEdgeRatio)}
  Duplicate Ratio  : ${fmtUnavailable(summary.graphMetrics.duplicateEdgeRatio)}
  Evidence Coverage: ${fmtRatio(summary.graphMetrics.edgeEvidenceCoverage)}

[QUERY LATENCY]
  Average Latency  : ${summary.queryLatency.averageMs} ms
  p50 Latency      : ${summary.queryLatency.p50Ms} ms
  p95 Latency      : ${summary.queryLatency.p95Ms} ms
  p99 Latency      : ${summary.queryLatency.p99Ms} ms

[RETRIEVAL ACCURACY]
  ${queryLine}
  Query                         Recall@10 Prec@10  Status        Latency
  ${queryLine}
${rows}
  ${queryLine}
  Average                       ${summary.averageRecallAt10.toFixed(2).padEnd(10)}${summary.averagePrecisionAt10.toFixed(2).padEnd(9)}
  Average Recall@5              ${summary.averageRecallAt5.toFixed(2)}
  Average Precision@5           ${summary.averagePrecisionAt5.toFixed(2)}
  Legacy Avg Recall@K           ${summary.averageRecallAtK.toFixed(2)}
  Legacy Avg Precision@K        ${summary.averagePrecisionAtK.toFixed(2)}
  Top-1 Accuracy                ${summary.top1Accuracy.toFixed(2)}
  Top-3 Accuracy                ${summary.top3Accuracy.toFixed(2)}
  MRR@10                        ${summary.mrrAt10.toFixed(2)}
  nDCG@10                       ${summary.ndcgAt10.toFixed(2)}

[QUERY STATUS]
  Strong Pass     : ${summary.queryStatusCounts.strong_pass}
  Pass            : ${summary.queryStatusCounts.pass}
  Weak Pass       : ${summary.queryStatusCounts.weak_pass}
  Fail            : ${summary.queryStatusCounts.fail}
  Critical Fail   : ${summary.queryStatusCounts.critical_fail}

[STRUCTURAL ASSERTIONS]
  Graph Evidence   : ${summary.graphCheck.status}${summary.graphCheck.reason ? ` (${summary.graphCheck.reason})` : ""}
  Downrank         : ${summary.downrankCheck.status}${summary.downrankCheck.reason ? ` (${summary.downrankCheck.reason})` : ""}
  JSON Chunking    : ${summary.jsonCheck.status}${summary.jsonCheck.reason ? ` (${summary.jsonCheck.reason})` : ""}
${debugRows ? `\n[SCORE BREAKDOWN]\n${debugRows}\n` : ""}
${criticalFailureRows ? `\n[CRITICAL FAILURE DIAGNOSTICS]\n${criticalFailureRows}\n` : ""}

${line}
   ANUBIS QUALITY INDEX (AQI): ${summary.aqi.toFixed(1)} / 100
${line}`;
}

function resolveEngineBinary(explicit, repoRoot) {
  if (explicit) {
    return path.resolve(explicit);
  }
  if (process.env.ANUBIS_ENGINE_BIN) {
    return path.resolve(process.env.ANUBIS_ENGINE_BIN);
  }
  const exe = process.platform === "win32" ? ".exe" : "";
  const candidates = [
    path.join(repoRoot, "target", "release", `anubis-engine${exe}`),
    path.join(repoRoot, "src-tauri", "target", "release", `anubis-engine${exe}`),
    path.join(repoRoot, "target", "debug", `anubis-engine${exe}`),
    path.join(repoRoot, "src-tauri", "target", "debug", `anubis-engine${exe}`),
  ];
  const found = candidates
    .filter((candidate) => fs.existsSync(candidate))
    .sort((left, right) => fs.statSync(right).mtimeMs - fs.statSync(left).mtimeMs)[0];
  if (!found) {
    throw new Error(
      `Could not find anubis-engine binary. Build it with "cargo build -p anubis-engine" or pass --bin <path>.`,
    );
  }
  return found;
}

function defaultAppModelCacheDir(repoRoot) {
  if (process.env.ANUBIS_BENCHMARK_MODEL_DIR) {
    return path.resolve(process.env.ANUBIS_BENCHMARK_MODEL_DIR);
  }
  if (process.platform === "win32" && process.env.APPDATA) {
    return path.join(process.env.APPDATA, "com.anubis-os.app");
  }
  if (process.platform === "darwin" && process.env.HOME) {
    return path.join(process.env.HOME, "Library", "Application Support", "com.anubis-os.app");
  }
  if (process.env.XDG_DATA_HOME) {
    return path.join(process.env.XDG_DATA_HOME, "com.anubis-os.app");
  }
  if (process.env.HOME) {
    return path.join(process.env.HOME, ".local", "share", "com.anubis-os.app");
  }
  return path.join(repoRoot, ".fastembed_cache", "benchmark-models");
}

async function callTool(client, name, args) {
  const response = await client.request("tools/call", {
    name,
    arguments: args || {},
  });
  if (!response || !response.result) {
    throw new Error(`tools/call ${name} returned no result`);
  }
  if (response.result.isError) {
    const text = (response.result.content || []).map((item) => item.text).join("\n");
    throw new Error(`tools/call ${name} failed: ${text}`);
  }
  return response.result.structuredContent;
}

class JsonRpcClient {
  constructor(binPath, env) {
    this.binPath = binPath;
    this.env = env;
    this.nextId = 1;
    this.pending = new Map();
    this.stderr = "";
  }

  async start() {
    this.child = spawn(this.binPath, ["--mcp"], {
      env: { ...process.env, ...this.env },
      stdio: ["pipe", "pipe", "pipe"],
    });
    this.child.stderr.on("data", (chunk) => {
      this.stderr += chunk.toString();
    });
    this.child.on("exit", (code, signal) => {
      const error = new Error(`anubis-engine exited with code ${code} signal ${signal}\n${this.stderr}`);
      for (const pending of this.pending.values()) {
        pending.reject(error);
      }
      this.pending.clear();
    });
    this.reader = readline.createInterface({ input: this.child.stdout });
    this.reader.on("line", (line) => this.handleLine(line));
  }

  handleLine(line) {
    let response;
    try {
      response = JSON.parse(line);
    } catch (error) {
      return;
    }
    const pending = this.pending.get(response.id);
    if (!pending) {
      return;
    }
    clearTimeout(pending.timer);
    this.pending.delete(response.id);
    if (response.error) {
      pending.reject(new Error(response.error.message));
    } else {
      pending.resolve(response);
    }
  }

  request(method, params, timeoutMs = 10 * 60 * 1000) {
    const id = this.nextId;
    this.nextId += 1;
    const payload = { jsonrpc: "2.0", id, method, params };
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`JSON-RPC request timed out: ${method}`));
      }, timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      this.child.stdin.write(`${JSON.stringify(payload)}\n`, "utf8");
    });
  }

  notify(method, params) {
    const payload = { jsonrpc: "2.0", method, params };
    this.child.stdin.write(`${JSON.stringify(payload)}\n`, "utf8");
    return Promise.resolve();
  }

  async close() {
    if (this.reader) {
      this.reader.close();
    }
    if (!this.child || this.child.exitCode !== null) {
      return;
    }

    const child = this.child;
    const exited = new Promise((resolve) => child.once("exit", resolve));
    child.stdin.end();
    const graceful = await Promise.race([exited, delay(1500).then(() => "timeout")]);
    if (graceful === "timeout" && child.exitCode === null && !child.killed) {
      child.kill();
      await Promise.race([exited, delay(1500)]);
    }
  }
}

async function rmDirWithRetry(dir) {
  for (let attempt = 0; attempt < 5; attempt += 1) {
    try {
      fs.rmSync(dir, { recursive: true, force: true });
      return;
    } catch (error) {
      if (!["EPERM", "EBUSY", "ENOTEMPTY"].includes(error.code) || attempt === 4) {
        throw error;
      }
      await delay(250 * (attempt + 1));
    }
  }
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function parseArgs(argv) {
  const options = {};
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--bin") {
      options.bin = argv[++i];
    } else if (arg === "--data-dir") {
      options.dataDir = argv[++i];
    } else if (arg === "--workdir") {
      options.workdir = argv[++i];
    } else if (arg === "--keep") {
      options.keepData = true;
    } else if (arg === "--json") {
      options.json = true;
    } else if (arg === "--debug") {
      options.debug = {
        ...(options.debug || {}),
        includeScoreBreakdown: true,
        includeTopResults: 5,
        includeIndexingPhaseTiming: true,
        includeGraphMetrics: true,
      };
    } else if (arg === "--debug-top") {
      options.debug = {
        ...(options.debug || {}),
        includeScoreBreakdown: true,
        includeTopResults: Number.parseInt(argv[++i], 10),
      };
    } else if (arg === "--scale") {
      const scale = argv[++i];
      if (!["quick", "full"].includes(scale)) {
        throw new Error("--scale must be quick or full");
      }
      options.scale = scale;
    } else if (arg === "--generate-only") {
      options.generateOnly = true;
      options.keepData = true;
    } else if (arg === "--help" || arg === "-h") {
      options.help = true;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  return options;
}

function usage() {
  return `Usage: node bin/benchmark.js [options]

Options:
  --bin <path>        Path to anubis-engine binary. Defaults to target debug/release binary.
  --data-dir <path>   Dataset directory. Defaults to scratch/temp_benchmark_data.
  --workdir <path>    Workdir passed to MCP tools. Defaults to the generated dataset directory.
  --keep              Keep generated dataset after the run.
  --json              Print JSON summary instead of the text report.
  --debug             Include benchmark-only score breakdowns in report/JSON.
  --debug-top <n>     Number of top results to include for debug score breakdowns.
  --scale <quick|full>
                      quick keeps 52 files with smaller payloads; full uses the heavier stress corpus.
  --generate-only     Generate the benchmark corpus and exit.
  -h, --help          Show this help.
`;
}

function nowMs() {
  return Number(process.hrtime.bigint()) / 1_000_000;
}

function round1(value) {
  return Math.round(value * 10) / 10;
}

function round2(value) {
  return Math.round(value * 100) / 100;
}

function numberOrUndefined(value) {
  return Number.isFinite(value) ? round2(value) : undefined;
}

function cleanObject(object) {
  return Object.fromEntries(Object.entries(object).filter(([, value]) => value !== undefined));
}

function fmtScore(value) {
  return Number.isFinite(value) ? value.toFixed(2) : "n/a";
}

function fmtMs(value) {
  return Number.isFinite(value) ? `${Math.round(value)} ms` : "n/a";
}

function fmtUnavailable(value) {
  return value === null || value === undefined ? "n/a" : value;
}

function fmtRatio(value) {
  return Number.isFinite(value) ? value.toFixed(2) : "n/a";
}

function formatBytes(bytes) {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${bytes} B`;
}

function fileSizeIfExists(file) {
  try {
    return fs.statSync(file).size;
  } catch (_) {
    return 0;
  }
}

function resolveBenchmarkWorkdir(explicit, dataset) {
  return path.resolve(explicit || dataset.rootDir);
}

function withWorkdir(workdir, args = {}) {
  if (!workdir || typeof workdir !== "string") {
    throw new Error("workdir must be a non-empty string");
  }
  return { ...(args || {}), workdir: path.resolve(workdir) };
}

function resolveBenchmarkStorageDir(workdirsRoot) {
  try {
    const entries = fs
      .readdirSync(workdirsRoot, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => path.join(workdirsRoot, entry.name))
      .filter((dir) => fs.existsSync(path.join(dir, "anubis.db")));
    if (entries.length === 0) {
      return null;
    }
    return entries.sort((left, right) => fs.statSync(right).mtimeMs - fs.statSync(left).mtimeMs)[0];
  } catch (_) {
    return null;
  }
}

if (require.main === module) {
  (async () => {
    try {
      const options = parseArgs(process.argv.slice(2));
      if (options.help) {
        process.stdout.write(usage());
        return;
      }
      const result = await runBenchmark(options);
      if (options.json) {
        process.stdout.write(`${JSON.stringify(result.summary, null, 2)}\n`);
      } else if (result.report) {
        process.stdout.write(`${result.report}\n`);
      } else {
        process.stdout.write(`Generated ${result.dataset.sourceFiles.length} files in ${result.dataset.rootDir}\n`);
      }
    } catch (error) {
      process.stderr.write(`${error.stack || error.message}\n`);
      process.exitCode = 1;
    }
  })();
}

module.exports = {
  QUERY_CASES,
  buildCriticalFailureDiagnostic,
  buildCriticalFailureDiagnostics,
  buildPrecisionDiagnostic,
  calculateAqi,
  classifyQuery,
  criticalFailureCount,
  debugSearchResults,
  decideExperiment,
  evaluateSearchCase,
  formatReport,
  generateDataset,
  graphMetricsFromStats,
  indexingPhaseTimings,
  parseArgs,
  percentile,
  rankingMetrics,
  resolveEngineBinary,
  resolveBenchmarkStorageDir,
  resolveBenchmarkWorkdir,
  runBenchmark,
  withWorkdir,
};
