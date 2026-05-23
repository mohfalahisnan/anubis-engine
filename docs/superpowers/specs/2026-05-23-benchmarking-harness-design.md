# Benchmarking Harness for Anubis: Accuracy, Speed, and Quality

Status: proposed
Date: 2026-05-23

## 1. Problem & Goals

To evaluate the quality of the Anubis Knowledge Engine, we need a reliable, repeatable, and automated way to measure:
1. **Speed/Performance**: Indexing throughput (files/sec, MB/sec), preprocessing times, and search query latency.
2. **Retrieval Quality/Accuracy**: Recall and precision under different query scenarios, correct down-ranking of reference documents (manifests/READMEs), and verification of graph-based neighbor expansion and citation evidence.

Currently, verification is limited to discrete Rust unit tests. This benchmarking harness provides a system-level evaluation suite to profile the engine's quality before and after changes.

---

## 2. Architecture & Data Flow

The benchmark runs as a standalone Node.js CLI script (`benchmark.js`) that automates the lifecycle of the test:

```text
+-----------------------+
|  benchmark.js Runner  |
+-----------+-----------+
            |
            | 1. Generates complex dataset in ./temp_benchmark_data/
            v
+-----------+-----------+
| ./temp_benchmark_data | <---+ (50+ files: Markdown, Text, JSON, CSV, mock PNGs/MP4s)
+-----------+-----------+
            |
            | 2. Spawns and communicates via stdio (JSON-RPC)
            v
+-------------------------------+
|  anubis-engine.exe --mcp      |
+---------------+---------------+
                |
                | 3. Resolves queries, checks down-ranks, expands graph
                v
+---------------+---------------+
|  SQLite DB & Tantivy Index    |
+-------------------------------+
```

---

## 3. Detailed Dataset Specification

The script will programmatically generate a directory containing **52 files** of diverse formats, sizes, and relational properties.

### 3.1 File List

1. **Large/Structured Files (2 files)**:
   * `inventory_audit.json` (~600 KB): Contains an array of 60 records mapping components, nodes, and device statuses. Designed to test chunking speed and UI non-blocking lock releases.
   * `activity_log.csv` (~400 KB): Tabular dataset with columns `log_id`, `node`, `severity`, `message`, and `timestamp`.

2. **Core Content Files (35 files)**:
   * `shipping_module.md` to `billing_module.md` (15 files): Markdown guides detailing specific codebase services.
   * `syslog_01.txt` to `syslog_20.txt` (20 files): Plain text server logs, simulating system alerts.

3. **Media Files with Mock Preprocessing Sidecars (13 files)**:
   * 8 files (`img_invoice_01.png` to `img_invoice_08.png`) alongside `.anubis.txt` sidecar transcripts containing OCR text.
     * *Timestamp test*: 4 sidecars will be created with `mtime >= source_file` (cached-fresh, skipped). 4 sidecars will be missing or older (triggers preprocessing).
   * 5 files (`video_record_01.mp4` to `video_record_05.mp4`) with corresponding whisper-transcript sidecars (`.anubis.txt`).

4. **Reference Files (2 files)**:
   * `readme_master.md`: A root readme referencing codebase nodes and anchors.
   * `manifest.json`: A standard file manifest listing all indexed filenames.

### 3.2 Anchor Network Distribution
To test the graph relations engine, we will embed specific uppercase **ANCHOR** values inside the content files:
* **Anchor A (`INC-2026-ATLAS-014`)**: Placed in `syslog_03.txt` (detailing the error) and `shipping_module.md` (detailing the resolution). Creates a `shared_anchor` edge.
* **Anchor B (`VID-APPROVAL-005`)**: Placed in `img_invoice_02.png` and `readme_master.md`. Used to test reference manifest boundary edges.
* **Anchor C (`SHIP-NODE-SURYA`)**: Placed in `shipping_module.md` and `inventory_audit.json`.

---

## 4. Test Queries & Expected Assertions

The benchmark runner will execute **15 queries** split across four evaluation categories:

### 1. Semantic Retrieval Accuracy
* **Query**: `"thermal printer printhead replacement"`
* **Assertion**: `shipping_module.md` must appear in the top 3 results.
* **Metric**: Recall@3 and Precision@3.

### 2. Anchor-Linked Graph Expansion
* **Query**: `"INC-2026-ATLAS-014"`
* **Assertion**: Querying this anchor must return the syslog and resolution files. Their relationship in `anubis_get_graph_neighborhood` must contain an edge of type `shared_anchor` with `reason = "anchor:INC-2026-ATLAS-014"` and valid `src_span` / `dst_span` evidence blocks.

### 3. Reference Down-Ranking
* **Query**: `"active module listing"` (matching terms inside both `readme_master.md` and content modules).
* **Assertion**: Content modules must rank higher than `readme_master.md`. The score components of `readme_master.md` must verify the `0.6` reference down-rank multiplier.

### 4. JSON Chunk Splitting
* **Query**: `"audit log item 42"`
* **Assertion**: Must return the specific paginated chunk from `inventory_audit.json` matching that element, confirming that the 600 KB JSON file was correctly paginated into distinct pages instead of a single giant page.

---

## 5. Metrics Formulation

### 5.1 Performance Metrics
* **Bootstrap Time ($T_{boot}$)**: Ms elapsed from process start to the first successful JSON-RPC handshake response.
* **Preprocessing Time ($T_{prep}$)**: Ms elapsed during Stage B (the pre-pass). Verifies that cached media sidecars skip Whisper/OCR.
* **Indexing Throughput ($Th_{idx}$)**:
  $$Th_{idx} = \frac{\text{Total Files Indexed}}{T_{index\_pass} \text{ (seconds)}} \quad [\text{files/sec}]$$
* **Query Latency**: Calculated across all test runs:
  * **p50 (Median)**: 50% of queries complete under this duration.
  * **p95**: 95% of queries complete under this duration (tail latency).

### 5.2 Retrieval Quality Metrics
* **Recall@K**:
  $$\text{Recall@K} = \frac{|\text{Relevant Chunks Returned in Top K}|}{|\text{Total Relevant Chunks in Dataset}|}$$
* **Precision@K**:
  $$\text{Precision@K} = \frac{|\text{Relevant Chunks Returned in Top K}|}{K}$$
* **Anubis Quality Index (AQI)**: A composite score ($0 - 100$) combining average accuracy and latency:
  $$\text{AQI} = 0.7 \times (\text{Average Recall@5} \times 100) + 0.3 \times \max\left(0, 100 - \frac{\text{p95 Latency (ms)}}{5}\right)$$

---

## 6. Output Report Format (Mockup)

```text
========================================================================
                      ANUBIS ENGINE BENCHMARK REPORT                    
========================================================================
[SYSTEM INFO]
  Bootstrap Time   : 412 ms
  Database Path    : C:\Users\User\AppData\Roaming\com.anubis-os.app\anubis.db
  Database Size    : 12.4 MB

[INDEXING PERFORMANCE]
  Pre-pass Duration: 145 ms (13 files checked, 9 cache hits, 4 OCR runs)
  Index Pass Time  : 4.82 s
  Throughput       : 10.8 files/sec (207.5 KB/s)
  Total Row Counts : Documents: 52 | Chunks: 412 | Edges: 1820

[QUERY LATENCY]
  Average Latency  : 28 ms
  p50 Latency      : 19 ms
  p95 Latency      : 64 ms

[RETRIEVAL ACCURACY]
  ----------------------------------------------------------------------
  Query                           Recall@5  Prec@5  Downrank  Status
  ----------------------------------------------------------------------
  1. printer replacement          1.00      0.40    PASS      OK
  2. INC-2026-ATLAS-014           0.80      0.20    PASS      OK
  3. active module listing        1.00      0.60    PASS      OK
  ...
  ----------------------------------------------------------------------
  AVERAGE                         0.92      0.43    100%      PASS

========================================================================
   ANUBIS QUALITY INDEX (AQI): 89.2 / 100
========================================================================
```

---

## 7. Migration & Rollout

* The harness script `benchmark.js` will be created in `scratch/benchmark.js` or `bin/benchmark.js`.
* It requires no changes to the core Tauri Rust codebase.
* Temporary directories and test databases are cleaned up automatically upon exit, ensuring zero footprint on developer environments.
