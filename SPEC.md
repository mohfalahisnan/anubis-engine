# Anubis OS — Internal Knowledge Engine
## Spec untuk Claude Code

> Baca seluruh file ini sebelum menulis satu baris kode.
> Ini adalah single source of truth untuk implementasi.

---

## 1. Ringkasan

Build internal indexing + graph engine sebagai bagian dari Anubis OS desktop app
(Tauri v2). Engine ini menggantikan Graphify dan berjalan sepenuhnya offline —
tidak ada Python, tidak ada external service, tidak ada instalasi tambahan untuk
user. IT hanya distribute satu installer .exe / .dmg.

**Tujuan engine:**
- Parse dokumen (MD, PDF, DOCX, XLSX, gambar, video)
- OCR teks dari gambar dan keyframe video
- Chunk dan embed teks menggunakan model ONNX lokal
- Simpan chunks + vectors + graph ke SQLite satu file
- Expose query API (hybrid: semantic + BM25 + graph traversal)
- Expose Tauri commands ke frontend

---

## 2. Stack keputusan (sudah final, jangan diganti)

| Layer | Pilihan | Versi | Alasan |
|---|---|---|---|
| App framework | tauri | 2.11.2 | single binary, Windows+Mac |
| Async runtime | tokio | 1.48.0 (LTS) | Tauri sudah pakai ini |
| PDF parser | lopdf | 0.40.0 | pure Rust, aktif |
| MD parser | pulldown-cmark | 0.13.4 | paling banyak dipakai |
| DOCX parser | docx-rs | 0.4.20 | aktif 2026 |
| XLSX parser | calamine | 0.35.0 | aktif 2026 |
| Image decode | image | 0.25.10 | standar ekosistem |
| OCR | ocrs | 0.12.2 | pure Rust, zero C dep, ganti leptess |
| Video frames | ffmpeg-next | 8.1.0 | sidecar binary |
| Embedding | fastembed | 5.13.4 | wrap ort+ONNX, bundle model |
| Full-text search | tantivy | 0.26.1 | BM25 pure Rust |
| Graph | petgraph | 0.8.3 | Dijkstra + traversal |
| Storage | rusqlite | 0.39.0 | SQLite bundled |
| Parallelism | rayon | 1.12.0 | parallel indexing |
| Serialization | serde + serde_json | 1.0.228 / 1.0.149 | standar |
| FS access | tauri-plugin-fs | 2.5.1 | Tauri v2 plugin |

**JANGAN tambah dependency baru tanpa update spec ini.**

---

## 3. Struktur direktori project

```
anubis-engine/                    ← root Tauri project
├── Cargo.toml                    ← workspace root
├── Cargo.lock
├── tauri.conf.json
├── build.rs
│
├── src-tauri/                    ← Rust backend
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs               ← Tauri entry point, register commands
│   │   ├── lib.rs                ← pub mod declarations
│   │   │
│   │   ├── engine/               ← core engine module
│   │   │   ├── mod.rs
│   │   │   ├── indexer.rs        ← orchestrate full index pipeline
│   │   │   ├── watcher.rs        ← file system watcher (auto re-index)
│   │   │   └── state.rs          ← IndexerState shared across threads
│   │   │
│   │   ├── parser/               ← format-specific parsers
│   │   │   ├── mod.rs            ← pub fn parse(path) -> ParsedDoc
│   │   │   ├── markdown.rs
│   │   │   ├── pdf.rs
│   │   │   ├── docx.rs
│   │   │   ├── xlsx.rs
│   │   │   ├── image.rs          ← decode + send to OCR
│   │   │   └── video.rs          ← extract frames via ffmpeg sidecar
│   │   │
│   │   ├── ocr/
│   │   │   ├── mod.rs
│   │   │   └── engine.rs         ← ocrs wrapper, returns String
│   │   │
│   │   ├── chunker/
│   │   │   ├── mod.rs
│   │   │   └── sliding.rs        ← sliding window chunker
│   │   │
│   │   ├── embedder/
│   │   │   ├── mod.rs
│   │   │   └── local.rs          ← fastembed wrapper
│   │   │
│   │   ├── graph/
│   │   │   ├── mod.rs
│   │   │   ├── builder.rs        ← build petgraph from chunks
│   │   │   ├── scorer.rs         ← edge weight computation
│   │   │   └── community.rs      ← simple community detection
│   │   │
│   │   ├── store/
│   │   │   ├── mod.rs
│   │   │   ├── db.rs             ← SQLite schema + migrations
│   │   │   ├── chunks.rs         ← CRUD for chunks table
│   │   │   ├── vectors.rs        ← vector storage + cosine search
│   │   │   ├── graph_store.rs    ← graph edges persistence
│   │   │   └── fts.rs            ← tantivy full-text index
│   │   │
│   │   ├── query/
│   │   │   ├── mod.rs
│   │   │   └── hybrid.rs         ← merge semantic + BM25 + graph results
│   │   │
│   │   └── commands/             ← Tauri command handlers (frontend API)
│   │       ├── mod.rs
│   │       ├── index_commands.rs
│   │       ├── query_commands.rs
│   │       └── status_commands.rs
│   │
│   └── binaries/                 ← ffmpeg sidecar (platform-specific)
│       ├── ffmpeg-x86_64-pc-windows-msvc.exe
│       ├── ffmpeg-x86_64-apple-darwin
│       └── ffmpeg-aarch64-apple-darwin
│
├── src/                          ← Frontend (React + TypeScript)
│   ├── main.tsx
│   ├── App.tsx
│   └── components/
│       ├── KnowledgeBrowser.tsx
│       ├── GraphVisualizer.tsx
│       ├── SearchBar.tsx
│       └── IndexStatus.tsx
│
└── models/                       ← ONNX model (di-bundle saat build)
    └── all-MiniLM-L6-v2/
        ├── model.onnx            ← ~23MB
        └── tokenizer.json
```

---

## 4. Cargo.toml (src-tauri/Cargo.toml)

```toml
[package]
name    = "anubis-engine"
version = "0.1.0"
edition = "2021"
rust-version = "1.71"

[lib]
name = "anubis_engine_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[[bin]]
name = "anubis-engine"
path = "src/main.rs"

[dependencies]
# Tauri
tauri            = { version = "2.11", features = ["protocol-asset"] }
tauri-plugin-fs  = "2.5"

# Async
tokio = { version = "1.48", features = ["full"] }

# Parsers
pulldown-cmark = "0.13"
lopdf          = "0.40"
docx-rs        = "0.4"
calamine       = { version = "0.35", features = ["dates"] }
image          = { version = "0.25", default-features = false, features = ["png","jpeg","webp","tiff"] }

# OCR (pure Rust, zero C dependency)
ocrs = "0.12"

# Embedding (bundles ONNX Runtime + model)
fastembed = "5.13"

# Search
tantivy = "0.26"

# Graph
petgraph = { version = "0.8", features = ["serde-1"] }

# Storage
rusqlite = { version = "0.39", features = ["bundled"] }

# Parallelism
rayon = "1.12"

# Serde
serde      = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Utilities
thiserror = "2.0"
tracing   = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid      = { version = "1.11", features = ["v4"] }
chrono    = { version = "0.4", features = ["serde"] }
walkdir   = "2.5"
notify    = "8.0"
anyhow    = "1.0"

[build-dependencies]
tauri-build = { version = "2.6", features = [] }

[profile.release]
opt-level     = 3
lto           = true
codegen-units = 1
strip         = true

[features]
default         = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

---

## 5. Schema SQLite (store/db.rs)

Jalankan semua CREATE TABLE di bawah sebagai migration saat app pertama kali buka.
Gunakan `PRAGMA user_version` untuk track versi migration.

```sql
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

-- Dokumen yang diindex
CREATE TABLE IF NOT EXISTS documents (
    id          TEXT PRIMARY KEY,          -- UUID v4
    path        TEXT NOT NULL UNIQUE,      -- absolute path
    filename    TEXT NOT NULL,
    format      TEXT NOT NULL,             -- md|pdf|docx|xlsx|image|video
    size_bytes  INTEGER NOT NULL,
    hash        TEXT NOT NULL,             -- blake3 hex, untuk detect perubahan
    indexed_at  TEXT NOT NULL,             -- ISO 8601
    status      TEXT NOT NULL DEFAULT 'pending',  -- pending|indexed|error
    error_msg   TEXT
);

-- Chunks hasil chunking
CREATE TABLE IF NOT EXISTS chunks (
    id          TEXT PRIMARY KEY,
    doc_id      TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    content     TEXT NOT NULL,
    char_start  INTEGER NOT NULL,
    char_end    INTEGER NOT NULL,
    page        INTEGER,                   -- untuk PDF
    created_at  TEXT NOT NULL
);

-- Vectors (embedding sebagai BLOB float32 little-endian)
CREATE TABLE IF NOT EXISTS vectors (
    chunk_id    TEXT PRIMARY KEY REFERENCES chunks(id) ON DELETE CASCADE,
    embedding   BLOB NOT NULL,             -- 384 float32 = 1536 bytes
    dim         INTEGER NOT NULL DEFAULT 384
);

-- Graph edges antar chunk
CREATE TABLE IF NOT EXISTS graph_edges (
    src_chunk   TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
    dst_chunk   TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
    weight      REAL NOT NULL,             -- 0.0 – 1.0
    edge_type   TEXT NOT NULL,             -- semantic|shared_entity|same_doc
    PRIMARY KEY (src_chunk, dst_chunk)
);

-- Entities yang diekstrak (nama, tanggal, keyword)
CREATE TABLE IF NOT EXISTS entities (
    id          TEXT PRIMARY KEY,
    chunk_id    TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL,             -- PERSON|ORG|DATE|PRODUCT|KEYWORD
    value       TEXT NOT NULL,
    confidence  REAL NOT NULL DEFAULT 1.0
);

-- Komunitas hasil community detection
CREATE TABLE IF NOT EXISTS communities (
    id          TEXT PRIMARY KEY,
    label       TEXT NOT NULL,             -- auto-generated label
    chunk_ids   TEXT NOT NULL,             -- JSON array of chunk IDs
    created_at  TEXT NOT NULL
);

-- Index status untuk UI
CREATE TABLE IF NOT EXISTS index_stats (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL              -- JSON value
);

CREATE INDEX IF NOT EXISTS idx_chunks_doc    ON chunks(doc_id);
CREATE INDEX IF NOT EXISTS idx_entities_chunk ON entities(chunk_id);
CREATE INDEX IF NOT EXISTS idx_edges_src     ON graph_edges(src_chunk);
CREATE INDEX IF NOT EXISTS idx_docs_status   ON documents(status);
```

---

## 6. Kontrak tipe data (types.rs — buat di src-tauri/src/)

```rust
use serde::{Deserialize, Serialize};

/// Output dari setiap parser
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDoc {
    pub doc_id:   String,           // UUID v4
    pub path:     String,
    pub format:   DocFormat,
    pub pages:    Vec<ParsedPage>,
    pub metadata: DocMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPage {
    pub page_num: Option<u32>,
    pub text:     String,
    pub images:   Vec<Vec<u8>>,     // raw bytes untuk di-OCR
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocFormat {
    Markdown, Pdf, Docx, Xlsx, Image, Video, Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocMetadata {
    pub filename:   String,
    pub size_bytes: u64,
    pub hash:       String,         // blake3
}

/// Satu chunk siap disimpan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id:          String,        // UUID v4
    pub doc_id:      String,
    pub chunk_index: usize,
    pub content:     String,
    pub char_start:  usize,
    pub char_end:    usize,
    pub page:        Option<u32>,
}

/// Hasil query ke engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub chunk_id:   String,
    pub doc_id:     String,
    pub content:    String,
    pub filename:   String,
    pub page:       Option<u32>,
    pub score:      f32,            // final hybrid score 0.0–1.0
    pub score_bm25: f32,
    pub score_vec:  f32,
    pub score_graph: f32,
}

/// Progress event dikirim ke frontend via Tauri emit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexProgress {
    pub total:     usize,
    pub done:      usize,
    pub current:   String,          // filename sedang diproses
    pub status:    IndexStatus,
    pub errors:    Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IndexStatus {
    Idle, Running, Done, Error,
}
```

---

## 7. Kontrak Tauri commands (commands/)

Semua command ini yang akan dipanggil frontend via `invoke()`.
Implementasikan di `commands/` dan register di `main.rs`.

```rust
// commands/index_commands.rs

/// Trigger full index dari folder yang dipilih user
/// Frontend: invoke("index_folder", { path: "/Users/.../knowledge_base" })
#[tauri::command]
pub async fn index_folder(
    path: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String>

/// Re-index satu file spesifik
#[tauri::command]
pub async fn index_file(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String>

/// Hapus satu dokumen dari index (tidak hapus file asli)
#[tauri::command]
pub async fn remove_document(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String>

/// Reset seluruh index (hapus anubis.db, rebuild dari nol)
#[tauri::command]
pub async fn reset_index(
    state: tauri::State<'_, AppState>,
) -> Result<(), String>

// commands/query_commands.rs

/// Hybrid search: semantic + BM25 + graph
/// Frontend: invoke("query", { q: "promo printer thermal", limit: 10 })
#[tauri::command]
pub async fn query(
    q: String,
    limit: Option<usize>,          // default 10
    state: tauri::State<'_, AppState>,
) -> Result<Vec<QueryResult>, String>

/// Ambil semua neighbors dari satu chunk di graph
#[tauri::command]
pub async fn get_chunk_neighbors(
    chunk_id: String,
    depth: Option<usize>,          // default 1
    state: tauri::State<'_, AppState>,
) -> Result<Vec<QueryResult>, String>

/// Ambil semua chunks dari satu dokumen
#[tauri::command]
pub async fn get_doc_chunks(
    doc_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Chunk>, String>

// commands/status_commands.rs

/// Status index: jumlah docs, chunks, graph edges, last indexed
#[tauri::command]
pub async fn get_index_stats(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String>

/// List semua dokumen yang diindex
#[tauri::command]
pub async fn list_documents(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String>
```

---

## 8. Pipeline indexing (engine/indexer.rs)

Implementasikan sebagai fungsi `index_folder(path, app_handle)` dengan urutan:

```
1. walkdir scan → collect semua file yang didukung
   Ekstensi: .md .txt .pdf .docx .xlsx .png .jpg .jpeg .webp .tiff .mp4 .mov .avi

2. Untuk setiap file (parallel dengan rayon):
   a. Cek hash (blake3) → skip jika sama dengan yang sudah di DB
   b. Insert/update row di documents table (status = "pending")
   c. Emit IndexProgress ke frontend

3. Parse (match format):
   - .md / .txt     → parser::markdown::parse()
   - .pdf           → parser::pdf::parse()
   - .docx          → parser::docx::parse()
   - .xlsx          → parser::xlsx::parse()
   - .png/.jpg/etc  → parser::image::parse() → ocr::engine::run()
   - .mp4/.mov/.avi → parser::video::parse() → extract frames → ocr

4. Chunk: chunker::sliding::chunk(text, window=512, overlap=64)
   Ukuran dalam karakter, bukan tokens. Potong di batas kalimat jika bisa.

5. Embed: embedder::local::embed_batch(chunks)
   Model: all-MiniLM-L6-v2 via fastembed
   Output: Vec<Vec<f32>> panjang 384

6. Extract entities dari setiap chunk (regex + simple rules):
   - TANGGAL: regex \d{1,2}[/-]\d{1,2}[/-]\d{2,4}
   - PRODUK: kata yang diawali huruf kapital dan bukan awal kalimat
   - KEYWORD: TF-IDF sederhana, ambil top 5 per chunk

7. Build graph:
   - Node = setiap chunk
   - Edge semantic: cosine_sim(vec_a, vec_b) > 0.75 → tambah edge
   - Edge same_doc: semua chunks dari dokumen sama → edge weight 0.5
   - Edge shared_entity: dua chunks share entity yang sama → edge weight 0.6

8. Simpan ke SQLite:
   - INSERT INTO chunks
   - INSERT INTO vectors (embedding sebagai BLOB)
   - INSERT INTO graph_edges
   - INSERT INTO entities
   - UPDATE documents SET status = "indexed"

9. Update tantivy full-text index

10. Emit IndexProgress final (status = done)
```

---

## 9. Query pipeline (query/hybrid.rs)

```
Input: query string q, limit N

1. Embed query: embedder::local::embed(q) → Vec<f32> len 384

2. Vector search (cosine similarity):
   - Load semua vectors dari DB (atau paged jika > 100K chunks)
   - Hitung cosine_sim(query_vec, chunk_vec) untuk semua chunks
   - Ambil top N*3 hasil
   - Score dinormalisasi ke 0.0–1.0

3. BM25 search via tantivy:
   - Query q ke tantivy index
   - Ambil top N*3 hasil
   - Score dinormalisasi ke 0.0–1.0

4. Graph boost:
   - Untuk setiap chunk yang muncul di hasil (2) atau (3):
     hitung rata-rata edge weight ke neighbors (depth=1)
   - graph_score = avg neighbor weight (0.0–1.0)

5. Merge dan score final:
   final_score = (0.5 * vec_score) + (0.3 * bm25_score) + (0.2 * graph_score)

6. Deduplicate by chunk_id, sort descending by final_score, return top N

7. Join dengan documents table untuk tambah filename, page, path
```

---

## 10. Chunker spec (chunker/sliding.rs)

```
window_size  = 512 karakter (bukan tokens)
overlap      = 64 karakter
min_chunk    = 50 karakter (buang chunk lebih kecil dari ini)

Algoritma:
1. Split teks menjadi kalimat (split pada ". " / ".\n" / "!\n" / "?\n")
2. Akumulasi kalimat sampai window_size tercapai
3. Simpan sebagai chunk, catat char_start dan char_end
4. Mundur 64 karakter (overlap) untuk chunk berikutnya
5. Ulangi sampai akhir teks

Setiap chunk menyimpan char_start dan char_end relatif terhadap
teks asli dokumen (bukan per halaman).
```

---

## 11. AppState (engine/state.rs)

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AppState {
    pub db:       Arc<Mutex<rusqlite::Connection>>,
    pub embedder: Arc<fastembed::TextEmbedding>,
    pub fts:      Arc<Mutex<tantivy::Index>>,
    pub graph:    Arc<Mutex<petgraph::Graph<String, f32>>>,
    pub indexing: Arc<Mutex<bool>>,        // lock: cegah concurrent indexing
}
```

Init di `main.rs` sebelum `tauri::Builder::default()`.
`AppState` di-manage sebagai Tauri managed state.

---

## 12. Event yang dikirim ke frontend

Gunakan `app_handle.emit("index-progress", payload)` untuk streaming progress.

```typescript
// Frontend listen:
// import { listen } from '@tauri-apps/api/event'
// await listen('index-progress', (event) => { ... })

interface IndexProgressEvent {
  total:   number
  done:    number
  current: string    // nama file sedang diproses
  status:  'idle' | 'running' | 'done' | 'error'
  errors:  string[]
}
```

---

## 13. Penanganan error

- Gunakan `thiserror` untuk define `EngineError` enum di `lib.rs`
- Semua Tauri commands return `Result<T, String>` — convert EngineError ke String dengan `.map_err(|e| e.to_string())`
- Parse error pada satu file TIDAK boleh stop indexing — log error, update `documents.status = 'error'`, lanjut ke file berikutnya
- Embed error: jika satu batch gagal, coba satu per satu. Jika masih gagal, skip file tersebut

```rust
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Parse error for {path}: {msg}")]
    Parse { path: String, msg: String },
    #[error("Embed error: {0}")]
    Embed(String),
    #[error("OCR error: {0}")]
    Ocr(String),
    #[error("Index already running")]
    AlreadyIndexing,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

---

## 14. Lokasi file runtime

Gunakan Tauri path API untuk resolve lokasi yang cross-platform:

```rust
// Database
app.path().app_data_dir()?.join("anubis.db")

// Tantivy index
app.path().app_data_dir()?.join("fts_index/")

// Model ONNX (di-bundle dalam binary, akses via resource)
app.path().resource_dir()?.join("models/all-MiniLM-L6-v2/")

// ffmpeg sidecar
app.path().resource_dir()?.join("binaries/ffmpeg")
```

Semua path ini otomatis handle perbedaan Windows vs Mac.

---

## 15. tauri.conf.json (bagian penting)

```json
{
  "productName": "Anubis OS",
  "version": "0.1.0",
  "identifier": "com.anubis-os.app",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:5173"
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "resources": {
      "models/**": "models/",
      "binaries/*": "binaries/"
    },
    "externalBin": [
      "binaries/ffmpeg"
    ],
    "windows": {
      "certificateThumbprint": null,
      "digestAlgorithm": "sha256",
      "timestampUrl": ""
    }
  },
  "plugins": {
    "fs": {
      "scope": {
        "allow": ["$APPDATA/**", "$DOCUMENT/**", "$DESKTOP/**", "$HOME/**"],
        "deny":  ["$APPDATA/**/.*"]
      }
    }
  }
}
```

---

## 16. Urutan implementasi yang disarankan

Kerjakan dalam urutan ini — setiap step bisa di-test independen:

```
Step 1: Setup project Tauri + Cargo.toml + tauri.conf.json
Step 2: store/db.rs — schema + migration + basic CRUD
Step 3: types.rs — semua struct dan enum
Step 4: parser/markdown.rs + parser/pdf.rs (dua format paling common)
Step 5: chunker/sliding.rs + unit test
Step 6: embedder/local.rs — fastembed wrapper + smoke test
Step 7: store/vectors.rs — simpan dan cosine search
Step 8: store/fts.rs — tantivy index + search
Step 9: engine/indexer.rs — pipeline untuk MD dan PDF dulu
Step 10: query/hybrid.rs — merge 3 score
Step 11: commands/ — expose ke Tauri
Step 12: parser/docx.rs + parser/xlsx.rs
Step 13: ocr/engine.rs + parser/image.rs
Step 14: parser/video.rs (ffmpeg sidecar)
Step 15: graph/builder.rs + graph/community.rs
Step 16: engine/watcher.rs — auto re-index on file change
Step 17: Frontend components (IndexStatus, SearchBar, GraphVisualizer)
```

---

## 17. Test yang wajib ada

```
src-tauri/src/chunker/sliding.rs   → #[cfg(test)] test overlap dan min_chunk
src-tauri/src/store/vectors.rs     → test cosine_sim() dengan known values
src-tauri/src/query/hybrid.rs      → test score merge dengan mock data
src-tauri/src/parser/markdown.rs   → test parse string MD sederhana
src-tauri/src/parser/pdf.rs        → test dengan fixture PDF di tests/fixtures/
```

Sediakan folder `src-tauri/tests/fixtures/` dengan:
- `sample.md` — minimal 3 paragraf
- `sample.pdf` — PDF dengan teks (bukan scan)
- `sample.docx`
- `sample.xlsx`

---

## 18. Yang TIDAK perlu diimplementasikan sekarang

- Authentication / multi-user
- Sync ke cloud
- Custom embedding model (fastembed sudah cukup)
- Graph visualization di frontend (bisa pakai data dari `get_chunk_neighbors`)
- Export/import index
- Video audio transcription (hanya OCR keyframes)

Tandai dengan `// TODO(v2):` jika ada kode placeholder.

---

## 19. Checklist sebelum selesai

- [ ] `cargo build --release` sukses tanpa warning
- [ ] `cargo test` semua pass
- [ ] App buka di Windows (via `cargo tauri dev`)
- [ ] App buka di Mac (via `cargo tauri dev`)
- [ ] Index folder dengan 10+ file berbeda format berhasil
- [ ] Query mengembalikan hasil relevan
- [ ] Tidak ada `unwrap()` di production code — semua pakai `?` atau explicit error handling
- [ ] Tidak ada `println!()` di production — semua pakai `tracing::info!()` / `tracing::error!()`
- [ ] File `anubis.db` terbuat di AppData, bukan di project folder

