# Quick Reference — Anubis Engine

## Versi dependency kunci
| Crate | Versi |
|---|---|
| tauri | 2.11.2 |
| fastembed | 5.13.4 |
| ocrs | 0.12.2 |
| rusqlite | 0.39.0 (bundled) |
| tantivy | 0.26.1 |
| petgraph | 0.8.3 |
| lopdf | 0.40.0 |

## Chunk config
- window: 512 karakter
- overlap: 64 karakter
- min: 50 karakter

## Embedding
- Model: all-MiniLM-L6-v2
- Dim: 384 float32
- Storage: BLOB 1536 bytes per chunk

## Query score weights
- Vector (cosine): 50%
- BM25 (tantivy): 30%
- Graph boost:     20%

## Format yang didukung
.md .txt .pdf .docx .xlsx .png .jpg .jpeg .webp .tiff .mp4 .mov .avi

## Edge types di graph
- semantic: cosine_sim > 0.75
- same_doc: weight 0.5
- shared_entity: weight 0.6

## AppData paths
- DB: {AppData}/anubis.db
- FTS: {AppData}/fts_index/
- Model: {Resource}/models/all-MiniLM-L6-v2/
