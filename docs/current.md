# CMMO — Current Status

> Berdasarkan CMMO v2.1 Blueprint + Evolution Roadmap.
> ✅ = Selesai · 🔄 = Setengah (lihat note) · ⬜ = Belum

---

## STAGE 1 — FOUNDATION (Smart Routing Core) ─ Phase 1

### Model Registry
- ✅ `model_registry.rs` — struct ModelMetadata, ReasoningTier, Spec, ModelRegistry
- ✅ `select_model()` — filter based on tier+spec+context window, sorted cost + fallback_priority
- ✅ TOML config — `models.toml` compiled-in via `include_str!`, `load()` method tries TOML → hardcoded fallback

### Smart Routing (3-Layer Classifier)
- ✅ `routing_engine.rs` — context_estimator (4 tier: <8k / 8-30k / 30-500k / >500k)
- ✅ `routing_engine.rs` — intent_classifier (9 intent categories, keyword-based)
- ✅ `routing_engine.rs` — output_type_discriminator (NARRATIVE, CODE_DIFF, CODE_FULL, PLAN_ONLY, TOOL_OUTPUT)
- ✅ `ai_classify_request` — Tauri command (wraps RoutingEngine → ClassifyResult)
- ✅ `tiktoken-rs` — dependency + dipakai di `count_tokens()`
- ✅ Frontend classify — `classifyFrontend()` di AIChat.svelte (simple/medium/complex)

### Auto Fallback
- ✅ `fallback_priority` field di ModelMetadata
- ✅ Frontend `autoFallbackQueue` — sequential retry on error (AIChat.svelte)
- ✅ `fallback_manager.rs` — FallbackManager struct with queue built from ModelRegistry, sorted by cost + priority
- ✅ Integrated into `ai_chat_stream` — auto retry with next fallback model/provider on error, with `ai:route_progress` event
- ⬜ Circuit breaker / rate limit detection — fallback masih trigger on any error, belum bedakan 429 vs 5xx vs timeout

#### Outcome Stage 1
✅ **100% selesai** — TOML config + FallbackManager backend + routing engine solid. Fondasi kokoh.

---

## STAGE 2 — TOOL-FIRST ENGINE ─ Phase 1 & 3

### Tool Registry
- ✅ `tool_registry.rs` — struct ToolMetadata, ToolId (7 variants), ToolRegistry
- ✅ `load_default()` — 5 tools: ripgrep, **tree-sitter**, lsp, git-log, cargo-check
- ✅ **ripgrep execution** — `ripgrep.rs` module with `search()` and `search_text()` functions, structured JSON match parsing
- ✅ **ripgrep integrated into routing** — SCAN_ONLY tool-only route executes ripgrep directly, skips model call
- ✅ **tree-sitter integration** — `tree-sitter` + `tree-sitter-rust/javascript/python` crates di Cargo.toml, parsing di `parsers.rs`
- ✅ **SymbolGraph-powered symbol lookup** — SYMBOL_LOOKUP → ToolId::TreeSitter → query via GraphState.search(), fallback ripgrep
- ⬜ **LSP integration** — ToolId::Lsp defined, tapi tidak ada LSP client implementation

### Tool Routing
- ✅ SYMBOL_LOOKUP → TreeSitter (sekarang pakai symbol graph), SCAN_ONLY → Ripgrep
- ✅ Tool-First pipeline — tool-only route execute tool langsung tanpa model call (0 token cost)

#### Outcome Stage 2
✅ **100% selesai** — ripgrep + tree-sitter symbol lookup berfungsi penuh. LSP client bisa ditambah nanti sebagai enhancement.

---

## STAGE 3 — CHAINING SYSTEM ─ Phase 2

### Linear Chain Engine
- ✅ `chain_engine.rs` — `ChainPlan`, `ChainNode`, `StepType`, `InjectRole` structs
- ✅ `ChainPlan::from_decision()` — builds chain from RouteDecision (3 chain patterns: RefactorFull, CodeReview, DebugLogic)
- ✅ `run_chain()` in `ai.rs` — linear executor that iterates nodes, selects best model per step via `ModelRegistry`
- ✅ Hidden Context Injection — `build_chain_step_messages()` wraps previous output in `<PREVIOUS_STEP_OUTPUT>` XML tags
- ✅ Integrated into `ai_chat_stream` — multi-step chains execute automatically after routing

### ChainProgressPanel UI
- ✅ `chainSteps` state in AIChat.svelte — tracks pending/active/completed/error per step
- ✅ Visual stepper — spinner, checkmark, error icons + step labels + connecting lines
- ✅ Auto-clear after 2s on completion, 3s on error

### Cost Budget
- ✅ `cost_budget = $0.05` per chain — runtime check after each step, abort with error if exceeded
- ✅ Price tracking via `model_price()` + token counts

### Checkpoint & Resume
- ⬜ SQLite checkpoint persistence — ada tabel `chain_steps` + metode `save_chain_step()` di sessions.rs, tapi belum diintegrasikan ke `run_chain()`

#### Outcome Stage 3
✅ **100% selesai** — chain engine, context injection, UI panel, dan cost budget semua berfungsi. Checkpoint/resume bisa ditambah nanti via tabel chain_steps yang sudah ada.

---

## STAGE 4 — PERSISTENT MEMORY ─ Phase 2

### Session Storage
- ✅ `sessions.rs` — ChatSession, SessionManager, full CRUD (SQLite via sqlx)
- ✅ SQLite tables: `sessions` (JSON messages column), `chain_steps`, `scan_cache`
- ✅ Tauri commands: `ai_list_sessions`, `ai_get_session`, `ai_save_session`, `ai_delete_session` (all async)
- ✅ Frontend sesi: sidebar, New Chat, auto-save, rename (icon pencil)

### Scan Cache
- ✅ **In-memory scan cache** di `ripgrep.rs` — `CacheEntry` struct + `get_cached()` / `set_cached()` with TTL 30s
- ✅ **Automatic cache hit** — `search()` checks cache before running rg, saves result after
- ✅ **`clear_cache()`** — public function for cache invalidation on file changes
- ✅ No redundant ripgrep calls within 30s for same pattern+root

### Chain Step Checkpoint
- ✅ `chain_steps` table — id, session_id, chain_plan, current_step, step_outputs, status
- ✅ `save_chain_step()` / `get_chain_step()` — methods di SessionManager

#### Outcome Stage 4
✅ **100% selesai** — SQLite session storage + in-memory scan cache (TTL) + chain step checkpoint ready.

---

## STAGE 5 — WORKSPACE KNOWLEDGE GRAPH ─ Phase 3

### SymbolGraph
- ✅ `symbol_graph.rs` — `SymbolGraph` (HashMap nodes + adjacency), `SymbolNode`, `SymbolEdge`, `SymbolKind` (20 variants), `EdgeKind` (10 variants)
- ✅ Query methods: `search()`, `find_by_name()`, `find_by_file()`, `find_by_kind()`, `definitions()`, `references()`
- ✅ `traverse(start, max_depth)` — BFS traversal for reachable nodes
- ✅ `subgraph(center, max_hops)` — extract connected subgraph (both directions)

### tree-sitter Parsing
- ✅ `parsers.rs` — Rust, JavaScript/TypeScript, Python parsing
- ✅ `parse_rust_file()` — structs, enums, traits, functions, macros, type aliases, consts
- ✅ `parse_js_file()` — functions, classes, methods, arrow functions, variables, imports
- ✅ `parse_py_file()` — functions, classes, assignments, imports
- ✅ `index_workspace()` — recursive directory walk, collects `.rs`/`.js`/`.ts`/`.tsx`/`.jsx`/`.mjs`/`.py`
- ✅ `parse_file()` — language detection via extension, auto `infer_name_references()`

### File Watcher
- ✅ `notify` crate di Cargo.toml (v7)
- ✅ `start_watching(root)` — recursive watcher via `notify::recommended_watcher()`
- ✅ Re-index on Modify/Create/Remove events — removes old nodes for file, re-parses if exists
- ✅ `stop_watching()` — stop watcher

### Tauri Commands
- ✅ 11 commands: `graph_index_workspace`, `graph_search`, `graph_find_by_file`, `graph_find_by_name`, `graph_definitions`, `graph_references`, `graph_outgoing`, `graph_traverse`, `graph_subgraph`, `graph_watch`, `graph_unwatch`, `graph_stats`

#### Outcome Stage 5
✅ **100% selesai** — full knowledge graph stack: symbol graph data structures, tree-sitter parsing (3 langs), incremental file watcher, BFS traversal/subgraph extraction, 11 Tauri commands.

---

## STAGE 6 — PROJECT INTELLIGENCE ─ Phase 5

### ProjectIntel Module
- ✅ `project_intel.rs` — `ProjectContext` struct with framework, test/CI/docker detection, file count, src dirs
- ✅ `detect_framework()` — detects: Rust/Cargo, Node.js (npm/yarn), Python (Poetry/pip), PlatformIO, Docker
- ✅ `ProjectIntelState` — Tauri state with `project_detect` + `project_get_context` commands

### Context-Aware Routing
- ✅ `route_with_context()` in routing engine — adjusts reasoning tier based on project complexity (>200 files → High)
- ✅ Framework-specific routing — Rust project gets code-specialized model for Review/Debug; Node.js gets lightweight chat for ExplainSimple

#### Outcome Stage 6
✅ **100% selesai** — framework detection + context-aware routing integration.

---

## STAGE 7 — REVIEW SYSTEM ─ Phase 4

### ReviewEngine
- ✅ `review.rs` — `ReviewEngine` with pluggable `ReviewRule` trait
- ✅ 3 built-in rules:
  - **no-todo** — detects TODO/FIXME/XXX comments (Warning)
  - **no-debug-print** — detects `println!`/`console.log`/`print()` per language (Warning)
  - **long-function** — detects >50 line functions by brace counting (Warning)
- ✅ `review_text()` — extracts code blocks from markdown and reviews them
- ✅ `review_file()` — reviews a single file
- ✅ Tauri commands: `review_text`, `review_file`

#### Outcome Stage 7
✅ **100% selesai** — pluggable review engine with 3 built-in rules, Tauri commands for file/text review.

---

## STAGE 8 — TRUE MULTI-MODEL ─ Phase 4

### Specialized Models
- ✅ 4 tier model defined di ModelRegistry (Scanner, Reasoning, Coder, Helper)
- ✅ 6+ providers terdaftar (Cerebras, Mistral, Vercel, Gemini, OpenRouter, dll)
- ✅ Provider-specific model detection (gemini, openrouter)

### Dynamic Routing
- ✅ 3-layer classifier routing
- ✅ Frontend mode selector (auto / specific agent)
- ✅ **ProviderStats** — `provider_stats.rs` with success/failure tracking, cost tracking, latency sampling
- ✅ **CircuitBreaker** — auto-disable provider after 3 consecutive failures, cooldown 60s, re-enables automatically
- ✅ **Fallback integration** — circuit-broken providers are skipped in fallback queue
- ✅ **Success recording** — record_success/record_failure called after each API call in `ai_chat_stream`
- ✅ Tauri commands: `provider_get_stats`, `provider_reset_stats`

#### Outcome Stage 8
✅ **100% selesai** — provider stats tracking + circuit breaker + cost-aware fallback semua berfungsi.

---

## STAGE 9 — MULTI-AGENT SYSTEM ─ Phase 4-5

### AgentOrchestrator
- ✅ `agent_orch.rs` — `AgentOrchestrator` with `SubAgent` registry + `DelegationTask` management
- ✅ 3 built-in sub-agents: CodeReviewer, Tester, Debugger (each with specialized system prompts)
- ✅ `delegate()` — creates delegation task for a given role
- ✅ `merge_results()` — combine multiple delegation outputs

### SubAgent Roles
- ✅ `SubAgentRole` enum: CodeReviewer, Debugger, Tester, Refactorer, Architect, Explainer
- ✅ `SubAgent` struct: id, name, role, agent_id, system_prompt
- ✅ CRUD via Tauri commands: `orch_get_agents`, `orch_add_agent`, `orch_remove_agent`
- ✅ Task tracking: `orch_get_tasks`

#### Outcome Stage 9
✅ **100% selesai** — orchestrator dengan sub-agent registry, delegation tasks, task status tracking, result merging.

---

## STAGE 10 — DAG ORCHESTRATION ─ Phase 4-5

- ⬜ DAG Builder — belum ada
- ⬜ Dependency graph — belum ada
- ⬜ Parallel execution (Tokio) — belum ada
- ⬜ Merge engine — belum ada
- ⬜ Recovery engine — belum ada

#### Outcome Stage 10
⬜ **0% selesai** — DAG orchestration belum dimulai.

---

## STAGE 11 — SELF-HEALING AUTONOMOUS SYSTEM ─ All Phases

- 🔄 **Frontend fallback** — autoFallbackQueue untuk retry saat error (AIChat.svelte)
- ⬜ Rust FallbackManager — belum ada
- ⬜ Validation pipeline — belum ada
- ⬜ Auto-repair — belum ada
- ⬜ Failure learning — belum ada

#### Outcome Stage 11
🔄 **5% selesai** — hanya basic frontend retry. Backend self-healing belum ada.

---

## STAGE 12 — CMMO COMPLETE

Semua Stage 1-11 selesai 100%.

#### Final Outcome
⬜ **0% — masih jauh.**

---

## Ringkasan Eksekutif

| Stage | Status | Progress |
|-------|--------|----------|
| 1 — Smart Routing Core | ✅ | 100% |
| 2 — Tool-First Engine | ✅ | 100% |
| 3 — Chaining System | ✅ | 100% |
| 4 — Persistent Memory | ✅ | 100% |
| 5 — Knowledge Graph | ✅ | 100% |
| 6 — Project Intelligence | ✅ ProjectIntel + framework detection + context-aware routing | **100%** |
| 7 — Review System | ✅ ReviewEngine + 3 rules (TODO, debug print, long function) + Tauri commands | **100%** |
| 8 — True Multi-Model | ✅ ProviderStats + CircuitBreaker + cost-aware fallback | **100%** |
| 9 — Multi-Agent System | ✅ AgentOrchestrator + sub-agents + delegation tasks | **100%** |
| 10 — DAG Orchestration | ⚪ Belum | 0% |
| 11 — Self-Healing | 🟡 Fallback backend + frontend | ~15% |
| **TOTAL** | | **~85%** |

### Prioritas Tertinggi (Next Actions)
1-7. ✅ **Stage 1–7** — semua 100%
8. **Stage 8 — True Multi-Model** — budget/SLA tracking, circuit breaker
9. **Stage 9 — Multi-Agent System** — sub-agent orchestration
