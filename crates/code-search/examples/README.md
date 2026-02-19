# LanceDB Embedding Test

Tests LanceDB's built-in sentence-transformers embedding for code search.

## Prerequisites

1. **protoc** (Protocol Buffer compiler) — required by LanceDB's dependencies:
   ```bash
   # Debian/Ubuntu
   sudo apt-get install protobuf-compiler

   # macOS
   brew install protobuf
   ```

2. **Build and run**:
   ```bash
   cargo run -p code-search --example lancedb_embedding_test --features lancedb-embedding-test
   ```

## What it does

- Indexes sample code chunks (login, session, users API) with LanceDB's sentence-transformers
- Runs a semantic search: "user authentication login"
- Prints top results with path and content preview

## Backends

- **LanceDB (default)**: Embedded vector store with sentence-transformers (all-MiniLM-L6-v2). No external services. Used by `appz code index` and `appz code search` in the default build.
- **Qdrant (opt-in)**: Build with `--no-default-features --features qdrant` to use fastembed + Qdrant. Requires Qdrant running (Docker or mise).
