> [!WARNING]
> This repository is managed by AI.
>
> Please review carefully before using, modifying, or merging any code from this repository.  
> Some parts may have been generated or modified automatically, so manual review is still required to ensure:
>
> - The code meets the project requirements
> - There are no bugs or regressions
> - No sensitive information is included
> - Security and quality standards are maintained

# Anubis Engine

Anubis Engine is a local-first Tauri app for indexing a workspace, building a
searchable knowledge graph, and serving hybrid retrieval results with evidence.
It combines dense embeddings, Tantivy full-text search, SQLite storage, entity
extraction, and graph expansion so files can be queried through the UI or MCP
tools.

## Requirements

- Node.js LTS
- Rust stable
- npm

## Setup

Install dependencies:

```bash
npm ci
```

Run the web UI during development:

```bash
npm run dev
```

Run the Tauri app during development:

```bash
npm run tauri:dev
```

Build the web frontend:

```bash
npm run build
```

Build the packaged Tauri app:

```bash
npm run tauri:build
```

## Tests and Benchmarks

Run the JavaScript test suite:

```bash
npm test
```

Check the Rust workspace:

```bash
cargo check
```

Run the benchmark harness:

```bash
npm run benchmark
```

Emit a machine-readable benchmark summary:

```bash
node bin/benchmark.js --json
```

## Versioning

Keep these version fields aligned:

- `package.json`
- `package-lock.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `Cargo.lock`

The current release workflow derives the release tag from
`src-tauri/tauri.conf.json`. Pushing `main` creates `app-v<version>` when that
tag does not already exist. Pushing that tag triggers the Tauri build matrix and
publishes the GitHub release.

## Release Flow

1. Bump all app version files.
2. Run `npm test`, `npm run build`, and `cargo check`.
3. Push `main`.
4. Let GitHub Actions create and push `app-v<version>`.
5. The tag workflow builds macOS Apple Silicon and Windows bundles and publishes
   the release.

For an immediate local release trigger, push the matching tag directly:

```bash
git tag app-v0.2.1
git push origin app-v0.2.1
```
