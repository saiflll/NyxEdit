# AGENTS.md — contlib (NyxEdit)

## Stack
- **Frontend**: SvelteKit 5 + TypeScript + Vite 6, static adapter (SPA mode, fallback `index.html`)
- **Backend**: Tauri 2 (Rust), crate `codlib_lib`
- **Nama app**: NyxEdit (binary `NyxEdit.exe`)
- **ID**: `com.lenovo.nyxedit`

## Perintah Penting
| Perintah | Fungsi |
|---|---|
| `npm run dev` | Dev server Vite (port 1420, strict port) |
| `npm run build` | Build frontend saja → `build/` |
| `npm run tauri dev` | Dev mode Tauri (buka desktop app) |
| `npm run tauri build` | Build produksi lengkap → installer di `src-tauri/target/release/bundle/{msi,nsis}/` |
| `npm run check` | `svelte-kit sync && svelte-check` (typecheck Svelte) |
| `npm run preview` | Preview production build |

## Arsitektur
- `src/` — frontend SvelteKit (`routes/`, `lib/components/`)
- `src-tauri/` — backend Rust dengan modul:
  - `pty.rs` — terminal emulation (portable-pty)
  - `ai.rs` — multi-agent AI chat
  - `fs.rs` — file management + git operasi
  - `pio.rs` — PlatformIO integration
  - `secrets.rs` — credential storage (keyring)
- `build/` — output frontend (gitignored, tapio dibaca Tauri)
- `lib/` dan `include/` — sisa PlatformIO project (tidak dipakai Tauri)

## Build & Install
1. Prasyarat: Node.js 22, Rust stable (MSVC toolchain di Windows), WebView2 (bundled di Windows 10+)
2. `npm ci` — install JS deps
3. `npm run tauri build` — build semua (frontend + Rust)
4. Installer Windows: `src-tauri/target/release/bundle/nsis/NyxEdit_*_setup.exe`
5. CI: GitHub Actions di `.github/workflows/build.yml` — build untuk linux/windows/macos, release otomatis saat tag `v*`

## Catatan
- `tauri.conf.json` atur `beforeBuildCommand: "npm run build"` — Tauri build otomatis panggil frontend build dulu
- `vite.config.js` ignore `**/src-tauri/**` — Vite tidak perlu watch Rust files
- Jendela tanpa dekorasi (`decorations: false`), drag via `data-tauri-drag-region`
- Gunakan `adapter-static` -> jangan pakai server-side rendering
- `npm run check` **sebelum** commit untuk cek error Svelte/TypeScript
- Rust tests: `cd src-tauri && cargo test`
  - 23 unit tests (tool execution, model routing/fallback, model price, system prompt resolution — plus DAG, context, cost routing)
  - 5 API tests (`#[ignore]` — run with `cargo test -- --ignored`, needs env vars)
- `env_logger::init()` di `run()` — log Rust via env var `RUST_LOG`

## Progress
### Done
- **CMMO 14 stage architecture**: Smart Routing, Tool-First Engine, Chaining, SQLite, Knowledge Graph, Project Intel, Review, Multi-Model, Multi-Agent, DAG, Self-Healing, Performance & DX, RAG Context, Cost Routing.
- **Low RAM Optimization**: Replaced default memory allocator with `mimalloc` to mitigate fragmentation. Implemented Lazy Loading for the Stage 5 symbol graph (`SymbolGraph`) which loads on demand only when queried, and added `graph_unload_workspace` to allow unloading the graph and freeing up memory on demand.
- **Multi-Folder Workspace Support**: Implemented workspace folder stacking (multi-root project workspaces) in the Svelte file explorer tree, configuration saving/loading to `.workspace` files (including support for VS Code `.code-workspace` configuration JSON schemas), dynamic path separator resolution, and merging file entries for AI contextual mentions.

## API Test Results
| Provider | API Key | Model | Status |
|---|---|---|---|
| Cerebras | `csk-REDACTED` | `gpt-oss-120b` | ✅ Works |
| Mistral | `Ieor4Xt-REDACTED` | `mistral-large-latest` | ✅ Works |
| Vercel | `vck-REDACTED` | `openai/gpt-4o-mini` | ✅ Works |
| Gemini | `AIzaSyD-REDACTED` | `gemini-2.0-flash` | ✅ Key valid, free-tier quota exhausted |

## Testing
- `cargo test` — runs 23 unit tests (no API keys needed, ~9s)
  - 23 unit tests (tool execution, model routing/fallback, model price, system prompt resolution — plus DAG, context, cost routing) 
- `CEREBRAS_API_KEY=... cargo test -- --ignored` — API integration tests
- React loop test (`test_react_loop_coder_read_file`) confirms tool-calling ReAct loop works end-to-end

## Provider Model Notes
- **Vercel** uses `/` separator: `openai/gpt-4o-mini`, `meta/llama-3.1-8b`, etc.
- **Cerebras direct API** has `gpt-oss-120b`, `zai-glm-4.7` (different from Vercel's `cerebras:llama3.1-8b`)
- **Gemini** uses query-param auth: `?key=...`, models listed as `models/gemini-2.0-flash`
- **Mistral direct API** uses standard OpenAI-compatible format

## CMMO — Status Real

### Legend
| Stage | Core Logic | Frontend UI | Active in main flow |
|---|---|---|---|
| **1** Smart Routing | ✅ `ModelRegistry`, `FallbackManager`, `models.toml` | ✅ Auto Mode di `ai_chat_stream` | ✅ Ya — dipanggil tiap auto mode |
| **2** Tool-First Engine | ✅ `ripgrep.rs`, TreeSitter, scan cache | ❌ Tool-only route lewat stream | ✅ Ya — tool-only skip model call |
| **3** Chaining | ✅ `chain_engine.rs`, `run_chain()` | ✅ `ChainProgressPanel.svelte` | ✅ Ya — kalau routing bikin chain plan |
| **4** SQLite | ✅ `sessions.rs` SQLite rewrite, 4 commands | ✅ Session list/save/load | ✅ Ya — tiap chat pake database |
| **5** Knowledge Graph | ✅ `symbol_graph.rs`, `parsers.rs`, file watcher | ❌ 12 commands registered | 🔶 Parsial — search/query lazy-loaded (hanya di-load ke RAM saat query pertama), `graph_unload_workspace` unloads RAM |
| **6** Project Intel | ✅ `project_intel.rs`, framework detection | ❌ 2 commands | 🔶 Tidak otomatis — perlu panggil `project_detect` dulu |
| **7** Review | ✅ `review.rs`, 3 rules | ❌ 2 commands | 🔶 Tidak otomatis — perlu panggil `review_text` manual |
| **8** Multi-Model | ✅ `provider_stats.rs`, CircuitBreaker | ❌ 2 commands | ✅ Ya — circuit breaker aktif di fallback loop |
| **9** Multi-Agent | ✅ `agent_orch.rs`, 3 sub-agents | ❌ 4 commands | 🔶 Tidak otomatis — perlu panggil `orch_delegate` |
| **10** DAG | ✅ `DagPlan`, `run_dag()` parallel tokio | ❌ | 🔶 Routing prioritaskan DAG untuk RefactorFull/CodeReview |
| **11** Self-Healing | ✅ `self_heal.rs`, health tracking | ❌ 2 commands | 🔶 `report_degraded()` dipanggil di error path, frontend perlu `get_status` |
| **12** Performance & DX | ✅ Cache warming, crash marker, startup health check, mimalloc allocator | ❌ `heal_check_startup`, `heal_clear_crash_marker` | ✅ Cache warm di `ensure_loaded`, crash marker di startup, mimalloc global allocator |
| **13** RAG Conversation Memory | ✅ `context.rs` — compression, cross-session retrieval | ❌ | ✅ Compression aktif di `ai_chat_stream` (OnceLock) |
| **14** Smart Cost Routing | ✅ `cost_routing.rs` — cheapest model, budget limit | ❌ 3 commands | 🔶 Routing preferensi, belum auto-dipanggil |

**Kesimpulan**: Backend 100%, frontend masih banyak yang belum di-Svelte-in. Yang benar-benar aktif end-to-end: Stage 1, 2, 3, 4, 8, 12, 13 (partial). Sisanya (5, 6, 7, 9, 10, 11, 14) jalan di Rust tapi belum punya UI / trigger otomatis penuh. Lazy loading symbol graph (Stage 5) and mimalloc allocator keep NyxEdit's RAM footprint low.

### Rust files added (Stage 1–14)
- `src-tauri/models.toml` — compiled-in model definitions
- `src-tauri/src/modules/fallback_manager.rs` — fallback queue builder
- `src-tauri/src/modules/ripgrep.rs` — ripgrep search + in-memory scan cache
- `src-tauri/src/modules/chain_engine.rs` — chain + DAG plan/execution structs (`DagNode`, `DagPlan`, `DagEdge`, `run_dag()`)
- `src-tauri/src/modules/symbol_graph.rs` — knowledge graph data structures
- `src-tauri/src/modules/parsers.rs` — tree-sitter source code parser (Rust/JS/Python)
- `src-tauri/src/modules/graph.rs` — GraphState + Tauri commands + file watcher
- `src-tauri/src/modules/project_intel.rs` — project framework detection + context
- `src-tauri/src/modules/review.rs` — review engine + 3 rules
- `src-tauri/src/modules/provider_stats.rs` — provider metrics + circuit breaker
- `src-tauri/src/modules/agent_orch.rs` — multi-agent orchestration
- `src-tauri/src/modules/self_heal.rs` — SelfHealEngine + component health tracking + crash marker
- `src-tauri/src/modules/context.rs` — conversation context compression + cross-session retrieval
- `src-tauri/src/modules/cost_routing.rs` — cost-aware model selection + budget enforcement
- `src-tauri/src/modules/mod.rs` — semua module registrations
- `src-tauri/src/modules/model_registry.rs` — +`load()` TOML method
- `src-tauri/src/modules/routing_engine.rs` — SYMBOL_LOOKUP → TreeSitter, +`route_with_context()`
- `src-tauri/src/modules/tool_registry.rs` — +TreeSitter in load_default()
- `src-tauri/src/modules/ai.rs` — +tool-only route, +fallback loop with circuit breaker, +`run_chain()`, +`run_dag()`, +health reporting, +cost budget, +context compression
- `src-tauri/src/modules/sessions.rs` — full rewrite: JSON files → SQLite (async commands) + `recover_last_session`
- `src-tauri/src/lib.rs` — +graph state + all new module states + commands

### Frontend changes
- `src/lib/components/AIChat.svelte` — +chainSteps state, `ai:route_progress` parser, ChainProgressPanel stepper
- `src/lib/components/FileManager.svelte` — implemented multi-root workspace explorer headers, folder-plus/save icons, "Open Workspace" empty states, dynamic path slashes, and mount reloading hooks
- `src/lib/stores.svelte.ts` — updated `loadWorkspace` to read multi-root file entries and populate merged autocomplete indexes
- `src/lib/components/GitStatus.svelte` — implemented folder selector dropdown to switch active Git target on multi-folder workspaces
- `src/lib/components/SearchInFiles.svelte` — implemented parallel multi-folder file/content search and relative path rendering with folder tags
- `src/lib/components/FileManager.svelte` & `GitStatus.svelte` — migrated store subscriptions to `onMount` and wrapped reactive effects in Svelte 5 `untrack()` to eliminate infinite rendering loops and CPU spikes
- `src/lib/components/IntelPanel.svelte` — implemented active workspace target dropdown selector to dynamically load and index symbols for multi-folder targets

### Verification
- `cargo check`: 0 errors
- `cargo test`: 19/19 passed, 5 ignored (API tests need env vars)
- `npm run check`: 0 errors (71 pre-existing warnings)
- Total CMMO: Stage 1–14 100% backend, ~40% frontend integration
