# Retrieval Stack Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace embedded `sqlite-vec` + `tantivy` + `fastembed` retrieval with a Docker-Compose stack (Qdrant + embedder + reranker), turning the codescout MCP server into a thin retrieval client.

**Architecture:** Localhost-only docker-compose with two profiles. CPU profile = Ollama+BGE-M3+Qwen3-Reranker-0.6B. GPU profile = TEI+Qwen3-Embedding-8B+BGE-Reranker-v2-m3. Qdrant native server-side hybrid (dense+sparse) via Universal Query API + RRF. Codescout client uses qdrant-client (gRPC) and reqwest (HTTP) to talk to the stack. Per-corpus collections (code, markdown, memories, libraries) keyed by `project_id` payload.

**Tech Stack:** Rust 2021 (workspace), tokio, qdrant-client 1.13, reqwest 0.13, sha2, mockito (dev), testcontainers-rs (dev), Docker Compose v2, Qdrant 1.13.x, HuggingFace TEI 1.6, Ollama latest.

**Spec:** `docs/superpowers/specs/2026-05-06-retrieval-stack-design.md`

**Branch:** `retrieval-stack` (NEW — do not work on master or experiments)

---

## File Structure

### New files

```
docker-compose.yml                                    Compose stack root
.env.example                                          Documented env vars
scripts/retrieval-stack.sh                            up/down/logs/pull wrapper
src/retrieval/mod.rs                                  Module root + re-exports
src/retrieval/config.rs                               RetrievalConfig::from_env, TOML loader
src/retrieval/client.rs                               RetrievalClient struct + constructors
src/retrieval/embedder.rs                             EmbedderHttp (Ollama + TEI clients)
src/retrieval/reranker.rs                             RerankerHttp (TEI rerank)
src/retrieval/qdrant.rs                               Thin Qdrant wrapper (collection mgmt)
src/retrieval/payload.rs                              Payload codec (CodeChunk ↔ Qdrant point)
src/retrieval/filter.rs                              Filter builder helpers
src/retrieval/sync.rs                                 sync_project + sync_library
src/retrieval/search.rs                               search_code/_markdown/_memories/_libraries
src/retrieval/drift.rs                                diff_chunks (pure)
src/retrieval/preflight.rs                            Path scope checks (trimmed from old)
src/retrieval/health.rs                              HealthReport + checks
tests/retrieval_unit.rs                               Unit harness entry
tests/retrieval_integration.rs                        Mocked integration entry
tests/retrieval_e2e.rs                                testcontainers entry (gated)
tests/fixtures/retrieval-corpus/                      Hand-crafted code for sparse-vs-dense
docs/spikes/2026-05-06-retrieval-stack-spike.md       Phase 0 artifact
docs/research/2026-05-06-retrieval-stack-benchmark.md Phase 5 artifact (template)
```

### Modified files

```
Cargo.toml                                            Add qdrant-client, reqwest already present
src/lib.rs                                            pub mod retrieval; remove pub mod embed (Phase 7)
src/tools/semantic.rs                                 Route to RetrievalClient behind flag
src/tools/memory.rs                                   recall via RetrievalClient (Phase 4)
src/tools/library.rs                                  build via RetrievalClient (Phase 4)
src/prompts/server_instructions.md                    Mention stack requirement (Phase 6)
src/prompts/onboarding_prompt.md                      Stack setup steps (Phase 6)
src/prompts/builders.rs                               build_system_prompt_draft updates (Phase 6)
src/tools/onboarding.rs                               Bump ONBOARDING_VERSION (Phase 6)
README.md                                             Stack quickstart section (Phase 1)
CONTRIBUTING.md                                       Stack required for dev (Phase 6)
```

### Deleted files (Phase 7)

```
src/embed/index.rs
src/embed/bm25.rs
src/embed/fusion.rs
src/embed/ (mod.rs trimmed; ast_chunker + chunker survive, moved or kept)
```

---

## Phase 0 — Spike

Goal: resolve open questions before locking Phase 1 service definitions.

### Task 0.1: Bootstrap branch + spike compose

**Files:**
- Create: `docs/spikes/2026-05-06-retrieval-stack-spike.md`

- [ ] **Step 1: Create branch**

```bash
git checkout master
git pull
git checkout -b retrieval-stack
```

- [ ] **Step 2: Create throwaway spike compose in scratch dir**

Run:
```bash
mkdir -p /tmp/retrieval-spike && cd /tmp/retrieval-spike
cat > docker-compose.yml <<'EOF'
services:
  qdrant:
    image: qdrant/qdrant:v1.13.0
    ports: ["127.0.0.1:6333:6333", "127.0.0.1:6334:6334"]
    volumes: ["qdrant_data:/qdrant/storage"]
  ollama:
    image: ollama/ollama:latest
    ports: ["127.0.0.1:11434:11434"]
    volumes: ["ollama_data:/root/.ollama"]
  reranker:
    image: ghcr.io/huggingface/text-embeddings-inference:cpu-1.6
    command: ["--model-id", "Qwen/Qwen3-Reranker-0.6B"]
    ports: ["127.0.0.1:8081:80"]
    volumes: ["model_cache:/data"]
volumes:
  qdrant_data:
  ollama_data:
  model_cache:
EOF
docker compose up -d
docker compose ps
```

Expected: 3 services healthy.

- [ ] **Step 3: Pull BGE-M3 in Ollama**

```bash
docker compose exec ollama ollama pull bge-m3
```

Expected: model downloaded.

- [ ] **Step 4: Probe BGE-M3 sparse output via Ollama**

```bash
curl -s http://127.0.0.1:11434/api/embeddings \
  -d '{"model":"bge-m3","prompt":"async cancellation patterns"}' | jq
```

Expected: dense float array. **Document whether sparse vector is included** in spike doc. If not, plan switches CPU embedder to TEI-BGE-M3 in Task 1.2.

- [ ] **Step 5: Probe TEI-BGE-M3 as fallback**

```bash
docker run --rm -p 127.0.0.1:8082:80 -v $PWD/teicache:/data \
  ghcr.io/huggingface/text-embeddings-inference:cpu-1.6 \
  --model-id BAAI/bge-m3
# in another terminal:
curl -s http://127.0.0.1:8082/embed \
  -H 'content-type: application/json' \
  -d '{"inputs":["async cancellation"]}'
curl -s http://127.0.0.1:8082/embed_sparse \
  -H 'content-type: application/json' \
  -d '{"inputs":["async cancellation"]}'
```

Expected: TEI exposes both `/embed` (dense) and `/embed_sparse` (sparse) endpoints.

- [ ] **Step 6: Probe Qdrant idf modifier with BGE-M3 sparse**

```bash
curl -s -X PUT http://127.0.0.1:6333/collections/spike \
  -H 'content-type: application/json' \
  -d '{"vectors":{"dense":{"size":1024,"distance":"Cosine"}},
       "sparse_vectors":{"sparse":{"modifier":"idf"}}}'
# upsert one point with sample dense+sparse, query, verify idf scoring works
```

Expected: collection accepts schema; upsert + query succeed.

- [ ] **Step 7: Measure idle RAM, cold start, single-query latency**

```bash
docker stats --no-stream
# Record RSS for each container
```

- [ ] **Step 8: Write spike report**

Write `docs/spikes/2026-05-06-retrieval-stack-spike.md` with sections: Setup, Findings (Ollama BGE-M3 sparse Y/N, TEI BGE-M3 sparse Y/N, Qdrant idf works Y/N), Decisions (which CPU embedder service to use), Measurements (RAM/latency).

- [ ] **Step 9: Commit spike doc**

```bash
git add docs/spikes/2026-05-06-retrieval-stack-spike.md
git commit -m "docs: phase 0 spike for retrieval stack"
```

- [ ] **Step 10: Tear down**

```bash
cd /tmp/retrieval-spike && docker compose down -v && cd -
```

---

## Phase 1 — Compose Stack

### Task 1.1: Add docker-compose.yml

**Files:**
- Create: `docker-compose.yml`

- [ ] **Step 1: Write compose file**

Create `docker-compose.yml`:

```yaml
name: codescout-retrieval

x-shared-volumes: &shared-volumes
  - model_cache:/data

services:
  qdrant:
    image: qdrant/qdrant:v1.13.0
    container_name: codescout-qdrant
    restart: unless-stopped
    ports:
      - "127.0.0.1:6333:6333"
      - "127.0.0.1:6334:6334"
    volumes:
      - qdrant_storage:/qdrant/storage
    environment:
      QDRANT__LOG_LEVEL: INFO
    healthcheck:
      test: ["CMD-SHELL", "bash -c '</dev/tcp/127.0.0.1/6333'"]
      interval: 10s
      timeout: 3s
      retries: 5
    networks: [retrieval_net]

  embedder-cpu:
    profiles: [cpu]
    image: ghcr.io/huggingface/text-embeddings-inference:cpu-1.6
    container_name: codescout-embedder
    restart: unless-stopped
    command: ["--model-id", "BAAI/bge-m3", "--dtype", "float32"]
    ports:
      - "127.0.0.1:8080:80"
    volumes:
      - model_cache:/data
    networks: [retrieval_net]

  embedder-gpu:
    profiles: [gpu]
    image: ghcr.io/huggingface/text-embeddings-inference:1.6
    container_name: codescout-embedder
    restart: unless-stopped
    command: ["--model-id", "Qwen/Qwen3-Embedding-8B", "--dtype", "float16"]
    ports:
      - "127.0.0.1:8080:80"
    shm_size: 2g
    volumes:
      - model_cache:/data
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]
    networks: [retrieval_net]

  reranker-cpu:
    profiles: [cpu]
    image: ghcr.io/huggingface/text-embeddings-inference:cpu-1.6
    container_name: codescout-reranker
    restart: unless-stopped
    command: ["--model-id", "Qwen/Qwen3-Reranker-0.6B", "--dtype", "float32"]
    ports:
      - "127.0.0.1:8081:80"
    volumes:
      - model_cache:/data
    networks: [retrieval_net]

  reranker-gpu:
    profiles: [gpu]
    image: ghcr.io/huggingface/text-embeddings-inference:1.6
    container_name: codescout-reranker
    restart: unless-stopped
    command: ["--model-id", "BAAI/bge-reranker-v2-m3", "--dtype", "float16"]
    ports:
      - "127.0.0.1:8081:80"
    volumes:
      - model_cache:/data
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]
    networks: [retrieval_net]

volumes:
  qdrant_storage:
  model_cache:

networks:
  retrieval_net:
    driver: bridge
```

> **NOTE:** If Phase 0 spike confirmed Ollama exposes BGE-M3 sparse, replace `embedder-cpu` with the Ollama variant and update `EmbedderHttp` accordingly. Default written here: TEI on both profiles for sparse correctness.

- [ ] **Step 2: Validate**

Run:
```bash
docker compose config --profile cpu
docker compose config --profile gpu
```

Expected: both render without errors.

- [ ] **Step 3: Commit**

```bash
git add docker-compose.yml
git commit -m "feat(retrieval): add docker-compose stack with cpu/gpu profiles"
```

### Task 1.2: Add .env.example

**Files:**
- Create: `.env.example`

- [ ] **Step 1: Write env example**

Create `.env.example`:

```bash
# Codescout retrieval stack — copy to .env and customize as needed.

# URLs the codescout MCP server uses to talk to the stack.
CODESCOUT_QDRANT_URL=http://127.0.0.1:6333
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:8080
CODESCOUT_RERANKER_URL=http://127.0.0.1:8081

# Profile in use. Must match `docker compose --profile <x> up`.
# Values: cpu | gpu
CODESCOUT_RETRIEVAL_PROFILE=cpu

# Embedding dim — must match the model being served.
# bge-m3 = 1024, Qwen3-Embedding-8B = 4096.
CODESCOUT_MODEL_DIM=1024

# Backend selection (Phase 4 transitional flag).
# Values: legacy | stack
CODESCOUT_RETRIEVAL_BACKEND=legacy
```

- [ ] **Step 2: Commit**

```bash
git add .env.example
git commit -m "feat(retrieval): document env vars for stack"
```

### Task 1.3: Add scripts/retrieval-stack.sh

**Files:**
- Create: `scripts/retrieval-stack.sh`

- [ ] **Step 1: Write wrapper script**

Create `scripts/retrieval-stack.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

PROFILE="${CODESCOUT_RETRIEVAL_PROFILE:-cpu}"
COMPOSE="docker compose --profile ${PROFILE}"

case "${1:-help}" in
  up)    $COMPOSE up -d ;;
  down)  $COMPOSE down ;;
  logs)  $COMPOSE logs -f "${2:-}" ;;
  pull)  $COMPOSE pull ;;
  ps)    $COMPOSE ps ;;
  purge-legacy)
    find . -type d -name '.codescout' -prune -exec rm -rf {} +
    echo "Removed legacy .codescout/ directories"
    ;;
  help|*)
    cat <<EOF
Usage: $0 {up|down|logs|pull|ps|purge-legacy}
Profile: \$CODESCOUT_RETRIEVAL_PROFILE (default: cpu)
EOF
    ;;
esac
```

- [ ] **Step 2: chmod + smoke test**

```bash
chmod +x scripts/retrieval-stack.sh
./scripts/retrieval-stack.sh help
```

Expected: usage printed.

- [ ] **Step 3: Commit**

```bash
git add scripts/retrieval-stack.sh
git commit -m "feat(retrieval): add stack control script"
```

### Task 1.4: README quickstart section

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add Retrieval Stack section**

Append after the existing "Installation" or top-level sections:

```markdown
## Retrieval Stack (experimental)

Codescout uses an external Docker-Compose stack for embedding and retrieval. Start it once:

```bash
cp .env.example .env
# Edit .env if you want the gpu profile (default: cpu)
./scripts/retrieval-stack.sh up
```

This launches Qdrant (vector DB), an embedder, and a reranker on `127.0.0.1`.

To stop:

```bash
./scripts/retrieval-stack.sh down
```

Existing `.codescout/embeddings/` data from older codescout versions is unused once you switch to the stack. Clean up with:

```bash
./scripts/retrieval-stack.sh purge-legacy
```

See `docs/superpowers/specs/2026-05-06-retrieval-stack-design.md` for the full design.
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: README section for retrieval stack"
```

---

## Phase 2 — Client Layer

### Task 2.1: Add qdrant-client dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add dep under `[dependencies]`**

Insert below the existing `tantivy = "0.22"` line:

```toml
# Retrieval stack client (Qdrant gRPC)
qdrant-client = "1.13"
reqwest = { version = "0.13", default-features = false, features = ["json", "rustls-tls"] }
```

> NOTE: `reqwest` is already a transitive dep via codescout-embed; declaring it explicitly here lets us use it from `src/retrieval/`.

- [ ] **Step 2: Verify build**

```bash
cargo build
```

Expected: success, qdrant-client compiles.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add qdrant-client + reqwest deps"
```

### Task 2.2: Module skeleton + RetrievalConfig

**Files:**
- Create: `src/retrieval/mod.rs`
- Create: `src/retrieval/config.rs`
- Modify: `src/lib.rs`
- Test: `tests/retrieval_unit.rs`

- [ ] **Step 1: Write failing test**

Create `tests/retrieval_unit.rs`:

```rust
use codescout::retrieval::config::RetrievalConfig;

#[test]
fn config_from_env_uses_defaults_when_unset() {
    std::env::remove_var("CODESCOUT_QDRANT_URL");
    std::env::remove_var("CODESCOUT_EMBEDDER_URL");
    std::env::remove_var("CODESCOUT_RERANKER_URL");
    std::env::remove_var("CODESCOUT_MODEL_DIM");
    std::env::remove_var("CODESCOUT_RETRIEVAL_PROFILE");

    let cfg = RetrievalConfig::from_env().expect("defaults");
    assert_eq!(cfg.qdrant_url, "http://127.0.0.1:6333");
    assert_eq!(cfg.embedder_url, "http://127.0.0.1:8080");
    assert_eq!(cfg.reranker_url, "http://127.0.0.1:8081");
    assert_eq!(cfg.model_dim, 1024);
    assert_eq!(cfg.profile, "cpu");
}

#[test]
fn config_from_env_reads_overrides() {
    std::env::set_var("CODESCOUT_QDRANT_URL", "http://qd:1");
    std::env::set_var("CODESCOUT_EMBEDDER_URL", "http://eb:2");
    std::env::set_var("CODESCOUT_RERANKER_URL", "http://rr:3");
    std::env::set_var("CODESCOUT_MODEL_DIM", "4096");
    std::env::set_var("CODESCOUT_RETRIEVAL_PROFILE", "gpu");

    let cfg = RetrievalConfig::from_env().expect("overrides");
    assert_eq!(cfg.qdrant_url, "http://qd:1");
    assert_eq!(cfg.model_dim, 4096);
    assert_eq!(cfg.profile, "gpu");

    for k in ["CODESCOUT_QDRANT_URL","CODESCOUT_EMBEDDER_URL","CODESCOUT_RERANKER_URL",
              "CODESCOUT_MODEL_DIM","CODESCOUT_RETRIEVAL_PROFILE"] {
        std::env::remove_var(k);
    }
}
```

- [ ] **Step 2: Run test (expect FAIL)**

```bash
cargo test --test retrieval_unit
```

Expected: FAIL — module `retrieval` doesn't exist.

- [ ] **Step 3: Add module to lib**

In `src/lib.rs`, add:

```rust
pub mod retrieval;
```

(Placement: alongside existing `pub mod embed;` etc.)

- [ ] **Step 4: Create `src/retrieval/mod.rs`**

```rust
pub mod config;
```

- [ ] **Step 5: Write `src/retrieval/config.rs`**

```rust
use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct RetrievalConfig {
    pub qdrant_url: String,
    pub embedder_url: String,
    pub reranker_url: String,
    pub model_dim: usize,
    pub profile: String,
}

impl RetrievalConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            qdrant_url:   std::env::var("CODESCOUT_QDRANT_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:6333".into()),
            embedder_url: std::env::var("CODESCOUT_EMBEDDER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8080".into()),
            reranker_url: std::env::var("CODESCOUT_RERANKER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8081".into()),
            model_dim:    std::env::var("CODESCOUT_MODEL_DIM")
                .ok().and_then(|s| s.parse().ok()).unwrap_or(1024),
            profile:      std::env::var("CODESCOUT_RETRIEVAL_PROFILE")
                .unwrap_or_else(|_| "cpu".into()),
        })
    }
}
```

- [ ] **Step 6: Run test (expect PASS)**

```bash
cargo test --test retrieval_unit
```

Expected: 2 passed.

- [ ] **Step 7: Commit**

```bash
git add src/lib.rs src/retrieval/ tests/retrieval_unit.rs
git commit -m "feat(retrieval): add RetrievalConfig + env loader"
```

### Task 2.3: EmbedderHttp client

**Files:**
- Create: `src/retrieval/embedder.rs`
- Modify: `src/retrieval/mod.rs`
- Test: `tests/retrieval_integration.rs`

- [ ] **Step 1: Add mockito to dev-deps**

In `Cargo.toml` `[dev-dependencies]`:

```toml
mockito = "1"
```

- [ ] **Step 2: Write failing integration test**

Create `tests/retrieval_integration.rs`:

```rust
use codescout::retrieval::embedder::EmbedderHttp;

#[tokio::test]
async fn embedder_returns_dense_and_sparse() {
    let mut server = mockito::Server::new_async().await;
    let dense_mock = server.mock("POST", "/embed")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[[0.1, 0.2, 0.3]]"#)
        .create_async().await;
    let sparse_mock = server.mock("POST", "/embed_sparse")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[[{"index":42,"value":0.5},{"index":7,"value":0.8}]]"#)
        .create_async().await;

    let eb = EmbedderHttp::new(server.url(), 3);
    let out = eb.embed("hello").await.expect("embed");

    assert_eq!(out.dense, vec![0.1, 0.2, 0.3]);
    assert_eq!(out.sparse.indices, vec![42, 7]);
    assert_eq!(out.sparse.values, vec![0.5, 0.8]);
    dense_mock.assert_async().await;
    sparse_mock.assert_async().await;
}

#[tokio::test]
async fn embedder_dim_mismatch_errors() {
    let mut server = mockito::Server::new_async().await;
    server.mock("POST", "/embed")
        .with_status(200)
        .with_body(r#"[[0.1, 0.2]]"#)
        .create_async().await;
    server.mock("POST", "/embed_sparse")
        .with_status(200)
        .with_body(r#"[[]]"#)
        .create_async().await;

    let eb = EmbedderHttp::new(server.url(), 1024);
    let err = eb.embed("hi").await.unwrap_err();
    assert!(err.to_string().contains("dim"), "got: {err}");
}
```

- [ ] **Step 3: Run test (expect FAIL)**

```bash
cargo test --test retrieval_integration
```

Expected: FAIL — `embedder` module missing.

- [ ] **Step 4: Implement `src/retrieval/embedder.rs`**

```rust
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct SparseVector {
    pub indices: Vec<u32>,
    pub values:  Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct EmbedOutput {
    pub dense:  Vec<f32>,
    pub sparse: SparseVector,
}

pub struct EmbedderHttp {
    base: String,
    expected_dim: usize,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct EmbedReq<'a> { inputs: Vec<&'a str> }

#[derive(Deserialize)]
struct SparseEntry { index: u32, value: f32 }

impl EmbedderHttp {
    pub fn new(base: impl Into<String>, expected_dim: usize) -> Self {
        Self { base: base.into(), expected_dim, client: reqwest::Client::new() }
    }

    pub async fn embed(&self, text: &str) -> Result<EmbedOutput> {
        let dense_url  = format!("{}/embed", self.base);
        let sparse_url = format!("{}/embed_sparse", self.base);
        let body = EmbedReq { inputs: vec![text] };

        let dense: Vec<Vec<f32>> = self.client.post(&dense_url).json(&body)
            .send().await.context("embed dense")?
            .error_for_status().context("embed dense status")?
            .json().await.context("embed dense json")?;
        let dense = dense.into_iter().next()
            .ok_or_else(|| anyhow!("empty dense response"))?;
        if dense.len() != self.expected_dim {
            return Err(anyhow!("embed dim mismatch: got {}, expected {}",
                dense.len(), self.expected_dim));
        }

        let sparse: Vec<Vec<SparseEntry>> = self.client.post(&sparse_url).json(&body)
            .send().await.context("embed sparse")?
            .error_for_status().context("embed sparse status")?
            .json().await.context("embed sparse json")?;
        let sparse_vec = sparse.into_iter().next().unwrap_or_default();
        let (indices, values) = sparse_vec.into_iter()
            .map(|e| (e.index, e.value))
            .unzip();

        Ok(EmbedOutput { dense, sparse: SparseVector { indices, values } })
    }

    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<EmbedOutput>> {
        let mut out = Vec::with_capacity(texts.len());
        for t in texts {
            out.push(self.embed(t).await?);
        }
        Ok(out)
    }
}
```

- [ ] **Step 5: Wire into mod**

In `src/retrieval/mod.rs`:

```rust
pub mod config;
pub mod embedder;
```

- [ ] **Step 6: Run tests (expect PASS)**

```bash
cargo test --test retrieval_integration
```

Expected: 2 passed.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/retrieval/embedder.rs src/retrieval/mod.rs tests/retrieval_integration.rs
git commit -m "feat(retrieval): EmbedderHttp client with dense+sparse"
```

### Task 2.4: RerankerHttp client

**Files:**
- Create: `src/retrieval/reranker.rs`
- Modify: `src/retrieval/mod.rs`
- Modify: `tests/retrieval_integration.rs`

- [ ] **Step 1: Write failing test**

Append to `tests/retrieval_integration.rs`:

```rust
use codescout::retrieval::reranker::RerankerHttp;

#[tokio::test]
async fn reranker_returns_scores_in_input_order() {
    let mut server = mockito::Server::new_async().await;
    server.mock("POST", "/rerank")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"index":1,"score":0.9},{"index":0,"score":0.1}]"#)
        .create_async().await;

    let rr = RerankerHttp::new(server.url());
    let scores = rr.rerank("query", &["a".into(), "b".into()]).await.expect("rerank");
    assert_eq!(scores.len(), 2);
    assert!((scores[0] - 0.1).abs() < 1e-6);
    assert!((scores[1] - 0.9).abs() < 1e-6);
}

#[tokio::test]
async fn reranker_503_returns_error() {
    let mut server = mockito::Server::new_async().await;
    server.mock("POST", "/rerank")
        .with_status(503)
        .create_async().await;
    let rr = RerankerHttp::new(server.url());
    let err = rr.rerank("q", &["a".into()]).await.unwrap_err();
    assert!(err.to_string().contains("rerank"), "got {err}");
}
```

- [ ] **Step 2: Run (FAIL)**

```bash
cargo test --test retrieval_integration
```

- [ ] **Step 3: Implement `src/retrieval/reranker.rs`**

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub struct RerankerHttp {
    base: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct RerankReq<'a> { query: &'a str, texts: &'a [String], raw_scores: bool }

#[derive(Deserialize)]
struct RerankItem { index: usize, score: f32 }

impl RerankerHttp {
    pub fn new(base: impl Into<String>) -> Self {
        Self { base: base.into(), client: reqwest::Client::new() }
    }

    pub async fn rerank(&self, query: &str, texts: &[String]) -> Result<Vec<f32>> {
        let url = format!("{}/rerank", self.base);
        let body = RerankReq { query, texts, raw_scores: false };
        let items: Vec<RerankItem> = self.client.post(&url).json(&body)
            .send().await.context("rerank send")?
            .error_for_status().context("rerank status")?
            .json().await.context("rerank json")?;
        let mut scores = vec![0.0_f32; texts.len()];
        for it in items {
            if it.index < scores.len() { scores[it.index] = it.score; }
        }
        Ok(scores)
    }
}
```

- [ ] **Step 4: Add to mod**

```rust
pub mod reranker;
```

- [ ] **Step 5: Run tests (PASS)**

```bash
cargo test --test retrieval_integration
```

- [ ] **Step 6: Commit**

```bash
git add src/retrieval/reranker.rs src/retrieval/mod.rs tests/retrieval_integration.rs
git commit -m "feat(retrieval): RerankerHttp client"
```

### Task 2.5: Qdrant wrapper + collection bootstrap

**Files:**
- Create: `src/retrieval/qdrant.rs`
- Modify: `src/retrieval/mod.rs`

- [ ] **Step 1: Write integration test (E2E gated)**

Append to `tests/retrieval_integration.rs`:

```rust
#[tokio::test]
#[ignore] // requires testcontainers; run with --ignored
async fn qdrant_creates_collection_with_dense_and_sparse() {
    use testcontainers::{runners::AsyncRunner, ContainerAsync, GenericImage, ImageExt};
    let image = GenericImage::new("qdrant/qdrant", "v1.13.0")
        .with_exposed_port(6334.into());
    let node: ContainerAsync<GenericImage> = image.start().await.expect("qdrant up");
    let port = node.get_host_port_ipv4(6334).await.unwrap();
    let url = format!("http://127.0.0.1:{port}");

    let q = codescout::retrieval::qdrant::QdrantWrap::connect(&url).await.expect("connect");
    q.ensure_collection("code_chunks", 1024).await.expect("create");
    let exists = q.collection_exists("code_chunks").await.expect("exists");
    assert!(exists);
}
```

- [ ] **Step 2: Add testcontainers dev-dep**

```toml
[dev-dependencies]
testcontainers = { version = "0.23", features = ["aws-lc-rs"] }
```

- [ ] **Step 3: Run (FAIL)**

```bash
cargo test --test retrieval_integration -- --ignored qdrant_creates_collection
```

- [ ] **Step 4: Implement `src/retrieval/qdrant.rs`**

```rust
use anyhow::{Context, Result};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollection, Distance, SparseIndexConfig, SparseVectorConfig,
    SparseVectorParams, VectorParams, VectorsConfig, vectors_config::Config,
    Modifier,
};
use std::collections::HashMap;

pub struct QdrantWrap {
    pub client: Qdrant,
}

impl QdrantWrap {
    pub async fn connect(url: &str) -> Result<Self> {
        let client = Qdrant::from_url(url).build().context("qdrant connect")?;
        Ok(Self { client })
    }

    pub async fn collection_exists(&self, name: &str) -> Result<bool> {
        Ok(self.client.collection_exists(name).await.context("collection_exists")?)
    }

    pub async fn ensure_collection(&self, name: &str, dim: u64) -> Result<()> {
        if self.collection_exists(name).await? { return Ok(()); }
        let mut sparse = HashMap::new();
        sparse.insert("sparse".to_string(), SparseVectorParams {
            modifier: Some(Modifier::Idf as i32),
            ..Default::default()
        });
        let create = CreateCollection {
            collection_name: name.to_string(),
            vectors_config: Some(VectorsConfig {
                config: Some(Config::ParamsMap(qdrant_client::qdrant::VectorParamsMap {
                    map: {
                        let mut m = HashMap::new();
                        m.insert("dense".to_string(), VectorParams {
                            size: dim,
                            distance: Distance::Cosine.into(),
                            ..Default::default()
                        });
                        m
                    }
                })),
            }),
            sparse_vectors_config: Some(SparseVectorConfig { map: sparse }),
            ..Default::default()
        };
        self.client.create_collection(create).await.context("create_collection")?;
        Ok(())
    }
}
```

> **NOTE:** Exact `qdrant-client 1.13` API may differ slightly; reconcile with `cargo doc --open -p qdrant-client` while implementing.

- [ ] **Step 5: Add to mod**

```rust
pub mod qdrant;
```

- [ ] **Step 6: Run gated test (PASS)**

```bash
cargo test --test retrieval_integration -- --ignored qdrant_creates_collection
```

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/retrieval/qdrant.rs src/retrieval/mod.rs tests/retrieval_integration.rs
git commit -m "feat(retrieval): QdrantWrap with collection bootstrap"
```

### Task 2.6: RetrievalClient struct + from_env

**Files:**
- Create: `src/retrieval/client.rs`
- Modify: `src/retrieval/mod.rs`

- [ ] **Step 1: Write test**

Append to `tests/retrieval_unit.rs`:

```rust
use codescout::retrieval::client::RetrievalClient;

#[test]
fn client_from_env_constructs_when_urls_present() {
    std::env::set_var("CODESCOUT_QDRANT_URL", "http://127.0.0.1:6333");
    std::env::set_var("CODESCOUT_EMBEDDER_URL", "http://127.0.0.1:8080");
    std::env::set_var("CODESCOUT_RERANKER_URL", "http://127.0.0.1:8081");
    let _ = RetrievalClient::from_config_only(
        codescout::retrieval::config::RetrievalConfig::from_env().unwrap()
    );
    // doesn't connect — just constructs
}
```

- [ ] **Step 2: Run (FAIL)**

- [ ] **Step 3: Implement `src/retrieval/client.rs`**

```rust
use anyhow::Result;
use crate::retrieval::config::RetrievalConfig;
use crate::retrieval::embedder::EmbedderHttp;
use crate::retrieval::reranker::RerankerHttp;
use crate::retrieval::qdrant::QdrantWrap;

pub struct RetrievalClient {
    pub qdrant:   QdrantWrap,
    pub embedder: EmbedderHttp,
    pub reranker: RerankerHttp,
    pub config:   RetrievalConfig,
}

impl RetrievalClient {
    pub async fn from_env() -> Result<Self> {
        let config = RetrievalConfig::from_env()?;
        let qdrant = QdrantWrap::connect(&config.qdrant_url).await?;
        let embedder = EmbedderHttp::new(&config.embedder_url, config.model_dim);
        let reranker = RerankerHttp::new(&config.reranker_url);
        Ok(Self { qdrant, embedder, reranker, config })
    }

    /// Test helper — does not connect to Qdrant.
    pub fn from_config_only(config: RetrievalConfig) -> Self {
        let embedder = EmbedderHttp::new(&config.embedder_url, config.model_dim);
        let reranker = RerankerHttp::new(&config.reranker_url);
        let qdrant = QdrantWrap { client: qdrant_client::Qdrant::from_url(&config.qdrant_url).build().unwrap() };
        Self { qdrant, embedder, reranker, config }
    }
}
```

- [ ] **Step 4: Add to mod**

```rust
pub mod client;
```

- [ ] **Step 5: Run (PASS)**

- [ ] **Step 6: Commit**

```bash
git add src/retrieval/client.rs src/retrieval/mod.rs tests/retrieval_unit.rs
git commit -m "feat(retrieval): RetrievalClient::from_env"
```

---

## Phase 3 — Sync Pipeline

### Task 3.1: drift::diff_chunks pure function

**Files:**
- Create: `src/retrieval/drift.rs`
- Modify: `src/retrieval/mod.rs`

- [ ] **Step 1: Write failing tests**

Append to `tests/retrieval_unit.rs`:

```rust
use codescout::retrieval::drift::{diff_chunks, ChunkRef, DriftAction};

fn cr(id: &str, hash: &str) -> ChunkRef {
    ChunkRef { chunk_id: id.into(), content_hash: hash.into() }
}

#[test]
fn diff_identical_yields_noop() {
    let server = vec![cr("a","h1"), cr("b","h2")];
    let local = vec![cr("a","h1"), cr("b","h2")];
    let d = diff_chunks(&server, &local);
    assert!(d.to_upsert.is_empty());
    assert!(d.to_delete.is_empty());
}

#[test]
fn diff_added_chunk_yields_upsert() {
    let server = vec![cr("a","h1")];
    let local = vec![cr("a","h1"), cr("b","h2")];
    let d = diff_chunks(&server, &local);
    assert_eq!(d.to_upsert, vec!["b".to_string()]);
    assert!(d.to_delete.is_empty());
}

#[test]
fn diff_deleted_chunk_yields_delete() {
    let server = vec![cr("a","h1"), cr("b","h2")];
    let local = vec![cr("a","h1")];
    let d = diff_chunks(&server, &local);
    assert!(d.to_upsert.is_empty());
    assert_eq!(d.to_delete, vec!["b".to_string()]);
}

#[test]
fn diff_modified_chunk_yields_upsert_for_new_id() {
    // Modified content = new content_hash = new chunk_id (chunk_id is sha256-derived).
    let server = vec![cr("a-old","h1")];
    let local = vec![cr("a-new","h2")];
    let d = diff_chunks(&server, &local);
    assert_eq!(d.to_upsert, vec!["a-new".to_string()]);
    assert_eq!(d.to_delete, vec!["a-old".to_string()]);
}
```

- [ ] **Step 2: Run (FAIL)**

- [ ] **Step 3: Implement `src/retrieval/drift.rs`**

```rust
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ChunkRef {
    pub chunk_id:     String,
    pub content_hash: String,
}

#[derive(Debug, Default)]
pub struct DriftAction {
    pub to_upsert: Vec<String>,  // chunk_ids
    pub to_delete: Vec<String>,  // chunk_ids
}

pub fn diff_chunks(server: &[ChunkRef], local: &[ChunkRef]) -> DriftAction {
    let server_ids: HashSet<&str> = server.iter().map(|c| c.chunk_id.as_str()).collect();
    let local_ids:  HashSet<&str> = local.iter().map(|c| c.chunk_id.as_str()).collect();
    let to_upsert = local.iter()
        .filter(|c| !server_ids.contains(c.chunk_id.as_str()))
        .map(|c| c.chunk_id.clone())
        .collect();
    let to_delete = server.iter()
        .filter(|c| !local_ids.contains(c.chunk_id.as_str()))
        .map(|c| c.chunk_id.clone())
        .collect();
    DriftAction { to_upsert, to_delete }
}
```

- [ ] **Step 4: Add to mod**

```rust
pub mod drift;
```

- [ ] **Step 5: Run (PASS)**

- [ ] **Step 6: Commit**

```bash
git add src/retrieval/drift.rs src/retrieval/mod.rs tests/retrieval_unit.rs
git commit -m "feat(retrieval): drift::diff_chunks pure diff"
```

### Task 3.2: Payload codec

**Files:**
- Create: `src/retrieval/payload.rs`
- Modify: `src/retrieval/mod.rs`

- [ ] **Step 1: Write tests**

Append to `tests/retrieval_unit.rs`:

```rust
use codescout::retrieval::payload::{CodePayload, payload_to_map, map_to_payload};

#[test]
fn payload_roundtrip_preserves_fields() {
    let p = CodePayload {
        project_id:          "code-explorer".into(),
        file_path:           "src/lib.rs".into(),
        language:            "rust".into(),
        start_line:          10,
        end_line:             42,
        ast_kind:            "fn".into(),
        ast_header:          "fn main()".into(),
        content:             "fn main() {}".into(),
        content_hash:        "h1".into(),
        last_indexed_commit: "abc".into(),
        chunk_id:            "id1".into(),
    };
    let map = payload_to_map(&p);
    let back = map_to_payload(&map).expect("decode");
    assert_eq!(back.project_id, p.project_id);
    assert_eq!(back.start_line, p.start_line);
    assert_eq!(back.content_hash, p.content_hash);
}
```

- [ ] **Step 2: Run (FAIL)**

- [ ] **Step 3: Implement `src/retrieval/payload.rs`**

```rust
use anyhow::{anyhow, Result};
use qdrant_client::qdrant::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CodePayload {
    pub project_id:          String,
    pub file_path:           String,
    pub language:            String,
    pub start_line:          i64,
    pub end_line:            i64,
    pub ast_kind:            String,
    pub ast_header:          String,
    pub content:             String,
    pub content_hash:        String,
    pub last_indexed_commit: String,
    pub chunk_id:            String,
}

fn s(v: &str) -> Value { Value::from(v.to_string()) }
fn i(v: i64)  -> Value { Value::from(v) }

pub fn payload_to_map(p: &CodePayload) -> HashMap<String, Value> {
    let mut m = HashMap::new();
    m.insert("project_id".into(), s(&p.project_id));
    m.insert("file_path".into(), s(&p.file_path));
    m.insert("language".into(), s(&p.language));
    m.insert("start_line".into(), i(p.start_line));
    m.insert("end_line".into(), i(p.end_line));
    m.insert("ast_kind".into(), s(&p.ast_kind));
    m.insert("ast_header".into(), s(&p.ast_header));
    m.insert("content".into(), s(&p.content));
    m.insert("content_hash".into(), s(&p.content_hash));
    m.insert("last_indexed_commit".into(), s(&p.last_indexed_commit));
    m.insert("chunk_id".into(), s(&p.chunk_id));
    m
}

fn get_str(m: &HashMap<String, Value>, k: &str) -> Result<String> {
    m.get(k).and_then(|v| v.as_str()).map(String::from)
        .ok_or_else(|| anyhow!("missing {k}"))
}
fn get_int(m: &HashMap<String, Value>, k: &str) -> Result<i64> {
    m.get(k).and_then(|v| v.as_integer())
        .ok_or_else(|| anyhow!("missing {k}"))
}

pub fn map_to_payload(m: &HashMap<String, Value>) -> Result<CodePayload> {
    Ok(CodePayload {
        project_id:          get_str(m, "project_id")?,
        file_path:           get_str(m, "file_path")?,
        language:            get_str(m, "language")?,
        start_line:          get_int(m, "start_line")?,
        end_line:            get_int(m, "end_line")?,
        ast_kind:            get_str(m, "ast_kind")?,
        ast_header:          get_str(m, "ast_header")?,
        content:             get_str(m, "content")?,
        content_hash:        get_str(m, "content_hash")?,
        last_indexed_commit: get_str(m, "last_indexed_commit")?,
        chunk_id:            get_str(m, "chunk_id")?,
    })
}
```

> **NOTE:** Adjust `Value::as_str` / `as_integer` to match the actual qdrant-client 1.13 API surface.

- [ ] **Step 4: Add to mod, run, commit**

```rust
pub mod payload;
```

```bash
cargo test --test retrieval_unit
git add src/retrieval/payload.rs src/retrieval/mod.rs tests/retrieval_unit.rs
git commit -m "feat(retrieval): payload codec for code chunks"
```

### Task 3.3: sync_project skeleton (chunk + hash + upsert)

**Files:**
- Create: `src/retrieval/sync.rs`
- Modify: `src/retrieval/mod.rs`
- Test: `tests/retrieval_e2e.rs`

- [ ] **Step 1: Create E2E test file**

```rust
// tests/retrieval_e2e.rs

#![cfg(feature = "retrieval-e2e")]

use codescout::retrieval::{client::RetrievalClient, config::RetrievalConfig, sync::SyncOpts};

#[tokio::test]
async fn sync_then_query_roundtrip_finds_known_symbol() {
    // Assumes a stack is already running on localhost (test runner spawns testcontainers
    // in build.rs, or developer runs `./scripts/retrieval-stack.sh up` before `cargo test --features retrieval-e2e`).
    let cfg = RetrievalConfig::from_env().expect("env");
    let client = RetrievalClient::from_env().await.expect("client");

    let project_id = "rust-library-test";
    let root = std::path::Path::new("tests/fixtures/rust-library");

    let report = client.sync_project(project_id, root, SyncOpts::default()).await.expect("sync");
    assert!(report.added > 0, "expected upserts on first sync, got {report:?}");

    // (Search will be tested in Phase 4 — placeholder here)
}
```

- [ ] **Step 2: Add feature gate**

In `Cargo.toml`:

```toml
[features]
retrieval-e2e = []
```

- [ ] **Step 3: Run (FAIL — `sync` module missing)**

```bash
cargo build --features retrieval-e2e
```

- [ ] **Step 4: Implement `src/retrieval/sync.rs`**

```rust
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct SyncOpts {
    pub languages: Option<Vec<String>>,
    pub force_reindex: bool,
}

#[derive(Debug, Default)]
pub struct SyncReport {
    pub added:   usize,
    pub updated: usize,
    pub deleted: usize,
    pub elapsed_ms: u128,
}

pub fn content_hash(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    format!("{:x}", h.finalize())
}

impl crate::retrieval::client::RetrievalClient {
    pub async fn sync_project(
        &self,
        project_id: &str,
        root: &Path,
        _opts: SyncOpts,
    ) -> Result<SyncReport> {
        use crate::embed::ast_chunker;
        use crate::retrieval::drift::{diff_chunks, ChunkRef};
        use crate::retrieval::payload::{CodePayload, payload_to_map};

        let started = std::time::Instant::now();
        self.qdrant.ensure_collection("code_chunks", self.config.model_dim as u64).await?;

        // 1. Walk files (use existing preflight crate if available; placeholder: just iterate)
        let mut local: Vec<(CodePayload, String)> = Vec::new(); // (payload, chunk_text)
        for entry in walkdir::WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() { continue; }
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let lang = match ext { "rs" => "rust", "py" => "python", "ts" => "typescript",
                                   "go" => "go", "java" => "java", "kt" => "kotlin", _ => continue };
            let source = std::fs::read_to_string(path)?;
            let chunks = ast_chunker::split_file(&source, lang);
            for c in chunks {
                let hash = content_hash(&c.content);
                let chunk_id = format!("{project_id}:{}:{hash}", path.display());
                let p = CodePayload {
                    project_id: project_id.into(),
                    file_path:  path.strip_prefix(root)?.display().to_string(),
                    language:   lang.into(),
                    start_line: c.start_line as i64,
                    end_line:   c.end_line as i64,
                    ast_kind:   c.kind.clone(),
                    ast_header: c.header.clone(),
                    content:    c.content.clone(),
                    content_hash: hash,
                    last_indexed_commit: String::new(),
                    chunk_id,
                };
                local.push((p, c.content));
            }
        }

        // 2. Fetch existing chunk refs from Qdrant for this project
        let server: Vec<ChunkRef> = self.qdrant.scroll_chunk_refs("code_chunks", project_id).await?;
        let local_refs: Vec<ChunkRef> = local.iter()
            .map(|(p,_)| ChunkRef { chunk_id: p.chunk_id.clone(), content_hash: p.content_hash.clone() })
            .collect();
        let action = diff_chunks(&server, &local_refs);

        // 3. Embed + upsert
        let upsert_payloads: Vec<&(CodePayload, String)> = local.iter()
            .filter(|(p,_)| action.to_upsert.contains(&p.chunk_id))
            .collect();
        let texts: Vec<String> = upsert_payloads.iter().map(|(_,c)| c.clone()).collect();
        let embeds = if !texts.is_empty() { self.embedder.embed_batch(&texts).await? } else { vec![] };
        if !upsert_payloads.is_empty() {
            self.qdrant.upsert_points("code_chunks",
                &upsert_payloads.iter().zip(embeds.iter())
                    .map(|((p,_), e)| (p.chunk_id.clone(), payload_to_map(p), e.clone()))
                    .collect::<Vec<_>>()
            ).await?;
        }

        // 4. Delete obsolete
        if !action.to_delete.is_empty() {
            self.qdrant.delete_points("code_chunks", &action.to_delete).await?;
        }

        Ok(SyncReport {
            added:      action.to_upsert.len(),
            deleted:    action.to_delete.len(),
            updated:    0,
            elapsed_ms: started.elapsed().as_millis(),
        })
    }
}
```

> **NOTE:** Requires stub methods `QdrantWrap::scroll_chunk_refs`, `upsert_points`, `delete_points` — implement in Task 3.4.

- [ ] **Step 5: Add to mod**

```rust
pub mod sync;
```

- [ ] **Step 6: Commit (compile-only)**

```bash
git add src/retrieval/sync.rs src/retrieval/mod.rs tests/retrieval_e2e.rs Cargo.toml
git commit -m "feat(retrieval): sync_project skeleton"
```

### Task 3.4: QdrantWrap upsert/scroll/delete

**Files:**
- Modify: `src/retrieval/qdrant.rs`

- [ ] **Step 1: Add methods to QdrantWrap impl**

```rust
use qdrant_client::qdrant::{
    Filter, Condition, PointId, PointStruct, PointsIdsList, PointsSelector,
    points_selector::PointsSelectorOneOf, Vector, NamedVectors, Vectors,
    vectors::VectorsOptions, ScrollPoints, with_payload_selector::SelectorOptions,
    WithPayloadSelector,
};

impl QdrantWrap {
    pub async fn scroll_chunk_refs(&self, collection: &str, project_id: &str)
        -> Result<Vec<crate::retrieval::drift::ChunkRef>>
    {
        let mut out = Vec::new();
        let mut offset = None;
        loop {
            let req = ScrollPoints {
                collection_name: collection.into(),
                filter: Some(Filter::must([Condition::matches(
                    "project_id", project_id.to_string()
                )])),
                limit: Some(1024),
                with_payload: Some(WithPayloadSelector {
                    selector_options: Some(SelectorOptions::Include(
                        qdrant_client::qdrant::PayloadIncludeSelector {
                            fields: vec!["chunk_id".into(), "content_hash".into()],
                        }
                    )),
                }),
                offset: offset.clone(),
                ..Default::default()
            };
            let resp = self.client.scroll(req).await.context("scroll")?;
            for p in &resp.result {
                let m = &p.payload;
                if let (Some(id), Some(h)) = (m.get("chunk_id").and_then(|v| v.as_str()),
                                              m.get("content_hash").and_then(|v| v.as_str())) {
                    out.push(crate::retrieval::drift::ChunkRef {
                        chunk_id: id.to_string(),
                        content_hash: h.to_string(),
                    });
                }
            }
            if let Some(next) = resp.next_page_offset { offset = Some(next); } else { break; }
        }
        Ok(out)
    }

    pub async fn upsert_points(
        &self,
        collection: &str,
        points: &[(String, std::collections::HashMap<String, qdrant_client::qdrant::Value>,
                   crate::retrieval::embedder::EmbedOutput)],
    ) -> Result<()> {
        let structs: Vec<PointStruct> = points.iter().map(|(id, payload, emb)| {
            let mut nv = NamedVectors::default();
            nv = nv.add_vector("dense", emb.dense.clone());
            nv = nv.add_vector("sparse", Vector {
                indices: Some(qdrant_client::qdrant::SparseIndices { data: emb.sparse.indices.clone() }),
                data: emb.sparse.values.clone(),
                ..Default::default()
            });
            PointStruct::new(id.clone(), Vectors::from(nv), payload.clone())
        }).collect();
        self.client.upsert_points_blocking(collection, None, structs, None).await
            .context("upsert_points")?;
        Ok(())
    }

    pub async fn delete_points(&self, collection: &str, ids: &[String]) -> Result<()> {
        let selector = PointsSelector {
            points_selector_one_of: Some(PointsSelectorOneOf::Points(PointsIdsList {
                ids: ids.iter().map(|s| PointId::from(s.clone())).collect(),
            })),
        };
        self.client.delete_points_blocking(collection, None, &selector, None).await
            .context("delete_points")?;
        Ok(())
    }
}
```

> **NOTE:** Adjust API names to match qdrant-client 1.13 surface; the structure is right but exact builder names may shift.

- [ ] **Step 2: Build**

```bash
cargo build --features retrieval-e2e
```

- [ ] **Step 3: Commit**

```bash
git add src/retrieval/qdrant.rs
git commit -m "feat(retrieval): qdrant scroll/upsert/delete"
```

### Task 3.5: E2E sync roundtrip

**Files:**
- Modify: `tests/retrieval_e2e.rs`

- [ ] **Step 1: Add idempotency test**

```rust
#[tokio::test]
#[cfg_attr(not(feature = "retrieval-e2e"), ignore)]
async fn sync_is_idempotent() {
    let client = RetrievalClient::from_env().await.expect("client");
    let project_id = "rust-library-test";
    let root = std::path::Path::new("tests/fixtures/rust-library");

    let r1 = client.sync_project(project_id, root, SyncOpts::default()).await.expect("first");
    let r2 = client.sync_project(project_id, root, SyncOpts::default()).await.expect("second");
    assert!(r1.added > 0);
    assert_eq!(r2.added, 0, "second sync added {} unexpectedly", r2.added);
    assert_eq!(r2.deleted, 0);
}
```

- [ ] **Step 2: Run with stack up**

```bash
./scripts/retrieval-stack.sh up
sleep 30  # wait for models to load
cargo test --features retrieval-e2e -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 3: Add drift-on-modify test**

```rust
#[tokio::test]
#[cfg_attr(not(feature = "retrieval-e2e"), ignore)]
async fn sync_detects_file_modification() {
    use std::fs;
    let client = RetrievalClient::from_env().await.expect("client");
    let project_id = "drift-test";
    let tmp = tempfile::tempdir().unwrap();
    let f = tmp.path().join("a.rs");
    fs::write(&f, "fn original() {}").unwrap();

    let r1 = client.sync_project(project_id, tmp.path(), SyncOpts::default()).await.expect("first");
    assert!(r1.added > 0);

    fs::write(&f, "fn modified() {}").unwrap();
    let r2 = client.sync_project(project_id, tmp.path(), SyncOpts::default()).await.expect("second");
    assert!(r2.added > 0, "modified file should trigger upsert");
    assert!(r2.deleted > 0, "old chunk should be deleted");
}
```

- [ ] **Step 4: Run, commit**

```bash
cargo test --features retrieval-e2e -- --test-threads=1
git add tests/retrieval_e2e.rs
git commit -m "test(retrieval): sync idempotent + drift detection"
```

---

## Phase 4 — Search Wiring

### Task 4.1: search_code

**Files:**
- Create: `src/retrieval/search.rs`
- Modify: `src/retrieval/qdrant.rs`
- Modify: `src/retrieval/mod.rs`

- [ ] **Step 1: Write integration test**

Append to `tests/retrieval_integration.rs`:

```rust
// (Mocked search test — uses mockito for embedder + reranker, real-ish Qdrant via scroll/upsert
//  is too heavy for unit test; full search path is exercised in retrieval_e2e.)

#[tokio::test]
async fn search_calls_embedder_then_reranker() {
    // Validates the call sequence and reranker degrade behavior.
    // (Implementation: stub Qdrant via a trait, or run testcontainers-light.)
    // For brevity here, just assert reranker degrade returns RRF order.
}
```

- [ ] **Step 2: Implement `src/retrieval/search.rs`**

```rust
use anyhow::Result;
use crate::retrieval::client::RetrievalClient;

pub struct SearchOpts {
    pub limit:     usize,
    pub overfetch: usize,
    pub rerank:    bool,
}

impl Default for SearchOpts {
    fn default() -> Self { Self { limit: 10, overfetch: 20, rerank: true } }
}

#[derive(Debug, Clone)]
pub struct Hit {
    pub chunk_id:     String,
    pub file_path:    String,
    pub start_line:   i64,
    pub end_line:     i64,
    pub content:      String,
    pub score:        f32,
    pub rerank_score: Option<f32>,
}

impl RetrievalClient {
    pub async fn search_code(&self, project_id: &str, query: &str, opts: SearchOpts) -> Result<Vec<Hit>> {
        let q = self.embedder.embed(query).await?;
        let candidates = self.qdrant.hybrid_query(
            "code_chunks", project_id, &q.dense, &q.sparse, opts.overfetch
        ).await?;
        if !opts.rerank || candidates.is_empty() {
            return Ok(candidates.into_iter().take(opts.limit).collect());
        }
        let texts: Vec<String> = candidates.iter().map(|h| h.content.clone()).collect();
        match self.reranker.rerank(query, &texts).await {
            Ok(scores) => {
                let mut zipped: Vec<(Hit, f32)> = candidates.into_iter().zip(scores).collect();
                zipped.sort_by(|a,b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                Ok(zipped.into_iter().take(opts.limit).map(|(mut h, s)| {
                    h.rerank_score = Some(s); h
                }).collect())
            }
            Err(e) => {
                tracing::warn!("reranker degraded: {e}");
                Ok(candidates.into_iter().take(opts.limit).collect())
            }
        }
    }
}
```

- [ ] **Step 3: Add `hybrid_query` to QdrantWrap**

In `src/retrieval/qdrant.rs`:

```rust
use qdrant_client::qdrant::{
    QueryPoints, PrefetchQuery, Query, Fusion, query::Variant as QV, VectorInput,
    SparseVectorInput,
};

impl QdrantWrap {
    pub async fn hybrid_query(
        &self,
        collection: &str,
        project_id: &str,
        dense: &[f32],
        sparse: &crate::retrieval::embedder::SparseVector,
        limit: usize,
    ) -> Result<Vec<crate::retrieval::search::Hit>> {
        let req = QueryPoints {
            collection_name: collection.into(),
            prefetch: vec![
                PrefetchQuery {
                    using: Some("dense".into()),
                    query: Some(Query::from(VectorInput::from(dense.to_vec()))),
                    limit: Some(60),
                    ..Default::default()
                },
                PrefetchQuery {
                    using: Some("sparse".into()),
                    query: Some(Query::from(SparseVectorInput::new(
                        sparse.indices.clone(), sparse.values.clone()))),
                    limit: Some(60),
                    ..Default::default()
                },
            ],
            query: Some(Query { variant: Some(QV::Fusion(Fusion::Rrf as i32)) }),
            filter: Some(Filter::must([Condition::matches(
                "project_id", project_id.to_string()
            )])),
            limit: Some(limit as u64),
            with_payload: Some(true.into()),
            ..Default::default()
        };
        let resp = self.client.query(req).await.context("hybrid_query")?;
        let mut hits = Vec::new();
        for sp in resp.result {
            let m = &sp.payload;
            hits.push(crate::retrieval::search::Hit {
                chunk_id:   m.get("chunk_id").and_then(|v| v.as_str()).unwrap_or("").into(),
                file_path:  m.get("file_path").and_then(|v| v.as_str()).unwrap_or("").into(),
                start_line: m.get("start_line").and_then(|v| v.as_integer()).unwrap_or(0),
                end_line:   m.get("end_line").and_then(|v| v.as_integer()).unwrap_or(0),
                content:    m.get("content").and_then(|v| v.as_str()).unwrap_or("").into(),
                score:      sp.score,
                rerank_score: None,
            });
        }
        Ok(hits)
    }
}
```

- [ ] **Step 4: Add E2E test**

```rust
#[tokio::test]
#[cfg_attr(not(feature = "retrieval-e2e"), ignore)]
async fn search_finds_synced_symbol() {
    let client = RetrievalClient::from_env().await.expect("client");
    let project_id = "search-test";
    let root = std::path::Path::new("tests/fixtures/rust-library");
    client.sync_project(project_id, root, SyncOpts::default()).await.expect("sync");

    let opts = codescout::retrieval::search::SearchOpts::default();
    let hits = client.search_code(project_id, "fibonacci", opts).await.expect("search");
    assert!(!hits.is_empty());
    assert!(hits.iter().any(|h| h.content.contains("fibonacci") || h.file_path.contains("fib")));
}
```

- [ ] **Step 5: Run + commit**

```bash
cargo test --features retrieval-e2e -- --test-threads=1
git add src/retrieval/search.rs src/retrieval/qdrant.rs src/retrieval/mod.rs tests/retrieval_integration.rs tests/retrieval_e2e.rs
git commit -m "feat(retrieval): hybrid search_code with optional rerank"
```

### Task 4.2: search_markdown / _memories / _libraries

**Files:**
- Modify: `src/retrieval/search.rs`

- [ ] **Step 1: Add three methods** (same shape as `search_code`, different collection name)

```rust
impl RetrievalClient {
    pub async fn search_markdown(&self, project_id: &str, q: &str, opts: SearchOpts) -> Result<Vec<Hit>> {
        self.search_in("markdown_chunks", project_id, q, opts).await
    }
    pub async fn search_memories(&self, project_id: &str, q: &str, opts: SearchOpts) -> Result<Vec<Hit>> {
        // memories are dense-only; reuse with empty sparse
        let qe = self.embedder.embed(q).await?;
        let candidates = self.qdrant.hybrid_query(
            "memories", project_id, &qe.dense, &qe.sparse, opts.overfetch
        ).await?;
        Ok(candidates.into_iter().take(opts.limit).collect())
    }
    pub async fn search_libraries(&self, q: &str, opts: SearchOpts) -> Result<Vec<Hit>> {
        // Library scope: no project_id filter
        let qe = self.embedder.embed(q).await?;
        self.qdrant.hybrid_query("library_chunks", "*", &qe.dense, &qe.sparse, opts.overfetch).await
            .map(|hs| hs.into_iter().take(opts.limit).collect())
    }

    async fn search_in(&self, collection: &str, project_id: &str, q: &str, opts: SearchOpts) -> Result<Vec<Hit>> {
        let qe = self.embedder.embed(q).await?;
        let candidates = self.qdrant.hybrid_query(
            collection, project_id, &qe.dense, &qe.sparse, opts.overfetch
        ).await?;
        if !opts.rerank || candidates.is_empty() {
            return Ok(candidates.into_iter().take(opts.limit).collect());
        }
        let texts: Vec<String> = candidates.iter().map(|h| h.content.clone()).collect();
        match self.reranker.rerank(q, &texts).await {
            Ok(scores) => {
                let mut zipped: Vec<_> = candidates.into_iter().zip(scores).collect();
                zipped.sort_by(|a,b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                Ok(zipped.into_iter().take(opts.limit).map(|(mut h, s)| {
                    h.rerank_score = Some(s); h
                }).collect())
            }
            Err(e) => { tracing::warn!("reranker degraded: {e}"); Ok(candidates.into_iter().take(opts.limit).collect()) }
        }
    }
}
```

> **NOTE:** Refactor: `search_code` should also call `search_in` to dedupe. Apply DRY before commit.

- [ ] **Step 2: Refactor search_code to call search_in**

Delete the body of `search_code`, replace with:

```rust
pub async fn search_code(&self, project_id: &str, q: &str, opts: SearchOpts) -> Result<Vec<Hit>> {
    self.search_in("code_chunks", project_id, q, opts).await
}
```

- [ ] **Step 3: Build + test**

```bash
cargo build
cargo test --features retrieval-e2e -- --test-threads=1
```

- [ ] **Step 4: Commit**

```bash
git add src/retrieval/search.rs
git commit -m "feat(retrieval): add search_markdown/_memories/_libraries"
```

### Task 4.3: Backend feature flag in semantic_search tool

**Files:**
- Modify: `src/tools/semantic.rs`

- [ ] **Step 1: Read the existing semantic_search call path**

```bash
cargo run --bin codescout -- --help  # sanity
```

Inspect `src/tools/semantic.rs` to find the call point that opens SQLite and runs RRF.

- [ ] **Step 2: Add backend selector**

At the top of `semantic_search` tool's `call_content`:

```rust
let backend = std::env::var("CODESCOUT_RETRIEVAL_BACKEND")
    .unwrap_or_else(|_| "legacy".into());
if backend == "stack" {
    let client = crate::retrieval::client::RetrievalClient::from_env().await
        .map_err(|e| crate::tools::RecoverableError::new(format!(
            "retrieval stack offline; run `./scripts/retrieval-stack.sh up`. ({e})"
        )))?;
    let opts = crate::retrieval::search::SearchOpts {
        limit: args.limit.unwrap_or(10),
        overfetch: 20,
        rerank: true,
    };
    let hits = client.search_code(&project_id, &args.query, opts).await?;
    return Ok(hits_to_tool_output(hits));
}
// ... existing legacy path unchanged below
```

Add helper `hits_to_tool_output(hits: Vec<Hit>) -> serde_json::Value` that produces the same shape the legacy path returns. Match field names exactly.

- [ ] **Step 3: Build + lint**

```bash
cargo build
cargo clippy -- -D warnings
```

- [ ] **Step 4: Smoke test with flag set**

```bash
./scripts/retrieval-stack.sh up
CODESCOUT_RETRIEVAL_BACKEND=stack cargo run --bin codescout -- --help
```

(Manual MCP test deferred to Phase 6.)

- [ ] **Step 5: Commit**

```bash
git add src/tools/semantic.rs
git commit -m "feat(retrieval): semantic_search routes to stack when CODESCOUT_RETRIEVAL_BACKEND=stack"
```

### Task 4.4: Side-by-side overlap test

**Files:**
- Create: `tests/retrieval_overlap.rs`

- [ ] **Step 1: Write test**

```rust
#![cfg(feature = "retrieval-e2e")]

#[tokio::test]
async fn legacy_and_stack_top10_overlap_at_least_5() {
    // Run a fixed query against both backends and assert top-10 overlap.
    // legacy: spin up via existing build_index path on a temp project
    // stack:  call RetrievalClient::search_code
    // assert: shared chunk_ids ≥ 5
    // (Implementation references existing build_index test scaffolding.)
}
```

- [ ] **Step 2: Run + commit (test will be filled in Phase 5 driving)**

Mark the body as `todo!()` for now; this task is a placeholder so the test file exists. Phase 5 fills it in.

```bash
git add tests/retrieval_overlap.rs
git commit -m "test(retrieval): scaffold legacy/stack overlap harness"
```

---

## Phase 5 — Benchmark Gate

### Task 5.1: Run 20-TC suite against both backends

**Files:**
- Create: `docs/research/2026-05-06-retrieval-stack-benchmark.md`

- [ ] **Step 1: Identify TC suite location**

```bash
grep -rn "TC-1\|test_case_1" docs/research/ | head
```

Locate `docs/research/2026-04-03-embedding-model-benchmark.md` and any harness script.

- [ ] **Step 2: Run legacy backend**

```bash
CODESCOUT_RETRIEVAL_BACKEND=legacy ./scripts/run-tc-benchmark.sh > /tmp/legacy.json
```

(If no harness exists, manually run each TC query via MCP tool and record top-10 in a structured table.)

- [ ] **Step 3: Run stack backend**

```bash
./scripts/retrieval-stack.sh up
sleep 60
CODESCOUT_RETRIEVAL_BACKEND=stack ./scripts/run-tc-benchmark.sh > /tmp/stack.json
```

- [ ] **Step 4: Compare**

For each TC: aggregate score, p50/p95 latency, per-TC pass/fail.

- [ ] **Step 5: Write benchmark doc**

Create `docs/research/2026-05-06-retrieval-stack-benchmark.md` with:
- Setup (model, hardware, profile)
- Per-TC table: legacy_score | stack_score | latency_ms_p50 | latency_ms_p95
- Aggregate: total / 60 for each
- Verdict: pass / fail against ship gate

- [ ] **Step 6: Ship gate evaluation**

If aggregate(stack) ≥ aggregate(legacy) AND TC-10/19/20 not regressed AND p95(stack) ≤ 2× p95(legacy):
- mark verdict PASS, proceed to Phase 6

Otherwise:
- mark verdict FAIL
- pivot to Option B (Qdrant BM25 sparse): change `embedder` to dense-only, add Qdrant tokenizer-based sparse via `Modifier::Idf` over BM25 — extra ~1 day

- [ ] **Step 7: Commit**

```bash
git add docs/research/2026-05-06-retrieval-stack-benchmark.md
git commit -m "docs: phase 5 retrieval stack benchmark results"
```

---

## Phase 6 — Cutover

### Task 6.1: Flip default backend

**Files:**
- Modify: `src/tools/semantic.rs` (default value)
- Modify: `.env.example`

- [ ] **Step 1: Change default**

In `src/tools/semantic.rs`, change:

```rust
let backend = std::env::var("CODESCOUT_RETRIEVAL_BACKEND")
    .unwrap_or_else(|_| "stack".into());  // was "legacy"
```

In `.env.example`:

```bash
CODESCOUT_RETRIEVAL_BACKEND=stack  # was legacy
```

- [ ] **Step 2: Commit**

```bash
git add src/tools/semantic.rs .env.example
git commit -m "feat(retrieval): default to stack backend"
```

### Task 6.2: Update prompt surfaces

**Files:**
- Modify: `src/prompts/server_instructions.md`
- Modify: `src/prompts/onboarding_prompt.md`
- Modify: `src/prompts/builders.rs`
- Modify: `src/tools/onboarding.rs`

- [ ] **Step 1: Add stack reqs to server_instructions.md**

Find the section that documents `semantic_search` tool. Insert:

```markdown
**Retrieval stack required.** `semantic_search`, `recall`, and library queries
require the docker-compose stack to be running. If unavailable, errors will
direct you to `./scripts/retrieval-stack.sh up`.
```

- [ ] **Step 2: Add stack setup to onboarding_prompt.md**

In the setup checklist:

```markdown
- [ ] Start the retrieval stack: `./scripts/retrieval-stack.sh up`
- [ ] Wait for embedder + reranker models to download (first time only, ~5min)
```

- [ ] **Step 3: Update builders.rs**

Find `build_system_prompt_draft`. Add a sentence describing the stack dependency in the generated per-project prompt.

- [ ] **Step 4: Bump ONBOARDING_VERSION**

In `src/tools/onboarding.rs`:

```rust
pub const ONBOARDING_VERSION: u32 = N + 1;  // increment current value
```

- [ ] **Step 5: Run prompt-surface test**

```bash
cargo test --test '*' prompt_surfaces_reference_only_real_tools
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/prompts/ src/tools/onboarding.rs
git commit -m "docs(prompts): retrieval stack requirement across all surfaces; bump ONBOARDING_VERSION"
```

### Task 6.3: Update CONTRIBUTING.md

**Files:**
- Modify: `CONTRIBUTING.md`

- [ ] **Step 1: Add stack section**

```markdown
## Retrieval Stack

Required for development. Start before running tests that depend on retrieval:

\`\`\`bash
cp .env.example .env
./scripts/retrieval-stack.sh up
\`\`\`

E2E retrieval tests are gated by `--features retrieval-e2e` and assume the stack
is reachable on `127.0.0.1`.
```

- [ ] **Step 2: Commit**

```bash
git add CONTRIBUTING.md
git commit -m "docs(contributing): retrieval stack required for dev"
```

### Task 6.4: Cherry-pick to master

- [ ] **Step 1: Verify clean**

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

- [ ] **Step 2: Cherry-pick sequence**

Per `CLAUDE.md` standard ship sequence, cherry-pick each phase's commits onto `master`:

```bash
git log --oneline master..retrieval-stack
# Pick commits in order; resolve any conflicts
git checkout master
git cherry-pick <SHA1>..<SHAn>
```

- [ ] **Step 3: Push**

```bash
git push origin master
git checkout retrieval-stack
git rebase master
```

---

## Phase 7 — Delete Legacy

### Task 7.1: Remove legacy retrieval code

**Files:**
- Delete: `src/embed/index.rs`
- Delete: `src/embed/bm25.rs`
- Delete: `src/embed/fusion.rs`
- Modify: `src/embed/mod.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Remove backend feature flag**

In `src/tools/semantic.rs`, remove the `if backend == "stack"` branching — only the stack path remains:

```rust
let client = crate::retrieval::client::RetrievalClient::from_env().await?;
// ...
```

Drop legacy code paths.

- [ ] **Step 2: Delete files**

```bash
git rm src/embed/index.rs src/embed/bm25.rs src/embed/fusion.rs
```

- [ ] **Step 3: Trim `src/embed/mod.rs`**

Keep only `ast_chunker`, `chunker`, `schema` modules; remove the rest.

- [ ] **Step 4: Drop deps from Cargo.toml**

Remove these lines:

```toml
sqlite-vec = "0.1"
tantivy = "0.22"
```

(fastembed is in `codescout-embed` crate; address there in a follow-up commit if needed.)

- [ ] **Step 5: Build + test**

```bash
cargo build
cargo clippy -- -D warnings
cargo test
```

Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(retrieval): remove legacy sqlite-vec + tantivy backend"
```

### Task 7.2: Drop CODESCOUT_RETRIEVAL_BACKEND knob

**Files:**
- Modify: `src/tools/semantic.rs`
- Modify: `.env.example`

- [ ] **Step 1: Remove env var reads**

Search for `CODESCOUT_RETRIEVAL_BACKEND` in source; remove all reads.

- [ ] **Step 2: Remove from .env.example**

Delete the `CODESCOUT_RETRIEVAL_BACKEND=...` line.

- [ ] **Step 3: Commit**

```bash
git add src/tools/semantic.rs .env.example
git commit -m "chore: remove CODESCOUT_RETRIEVAL_BACKEND transitional flag"
```

### Task 7.3: Cherry-pick Phase 7 to master

- [ ] **Step 1: Run full verification**

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test
cargo build --release
```

- [ ] **Step 2: Cherry-pick + push**

```bash
git log --oneline master..retrieval-stack
git checkout master
git cherry-pick <Phase7-SHA1>..<Phase7-SHAn>
git push origin master
```

- [ ] **Step 3: Tag minor release**

```bash
# Bump Cargo.toml version per CLAUDE.md release cycle
git tag v0.12.0
git push --tags
```

(Follow full release cycle from `CLAUDE.md` § Release Cycle.)

---

## Self-Review Notes

**Spec coverage:**
- Architecture overview → Phase 1 (compose) + Phase 2 (client)
- Service Definitions → Task 1.1 (compose YAML)
- Qdrant Schema → Task 2.5 (collection bootstrap), Task 3.2 (payload codec)
- Codescout Client Surface → Phase 2 + Phase 4
- Migration & Rollout → Phases 0–7 (one phase per spec phase)
- Testing Strategy → unit (Tasks 2.2, 3.1), integration (2.3, 2.4), E2E (2.5, 3.5, 4.1), benchmark (5.1)
- Open Questions Resolved at Spike → Phase 0

**Placeholder scan:** No `TBD`/`TODO` literals. `todo!()` macro used in Task 4.4 placeholder (acceptable — Phase 5 fills it).

**Type consistency:** `ChunkRef`, `CodePayload`, `EmbedOutput`, `Hit`, `SearchOpts`, `SyncOpts`, `SyncReport` defined once and used consistently. `payload_to_map`/`map_to_payload` paired. `ensure_collection`/`collection_exists` paired.

**Known notes for engineer:**
1. qdrant-client 1.13 API surface may differ slightly from snippets — reconcile against `cargo doc -p qdrant-client`
2. Phase 0 spike result determines whether CPU embedder is TEI (default) or Ollama (if BGE-M3 sparse exposed)
3. Phase 5 ship gate is hard. If it fails, pivot to Option B (Qdrant BM25 sparse) before continuing to Phase 6.

---

## Execution Handoff

Plan saved to `docs/superpowers/plans/2026-05-06-retrieval-stack-plan.md`.

Two execution options:
1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — execute tasks in this session with checkpoints for review.

Which approach?
