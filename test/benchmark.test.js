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
