# AGENTS.md — NyxEdit (NyxEdit)

## Stack
- **Frontend**: SvelteKit 5 + TypeScript + Vite 6, static adapter (SPA mode, fallback `index.html`)
- **Backend**: Tauri 2 (Rust), crate `nyxedit_lib`
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
  - `ai.rs` — multi-agent AI chat + ReAct loop + fallback system + CLI Gateway integration
  - `fs.rs` — file management + git operasi
  - `pio.rs` — PlatformIO integration
  - `secrets.rs` — credential storage (keyring)
  - `cli.rs` — CLI Discovery Engine + CliAdapter trait + 6 adapter impl (Claude/Gemini/OpenCode/Aider/Codex/Agy) + streaming subprocess
  - `executor.rs` — tool execution engine + permission state + diff computation
  - `handoff.rs` — handoff file management for cross-agent task persistence
  - `provider.rs` — AiProvider trait + LiteLLM provider abstraction
  - `router.rs` — RouterDecision (RouteClass + ExecutionMode) + sensitive content detection
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
  - 14 unit tests (tool execution, CLI adapter discovery/args, router intent/content detection, executor diff, routing engine context strip)
  - 5 API tests (`#[ignore]` — run with `cargo test -- --ignored`, needs env vars)
- `env_logger::init()` di `run()` — log Rust via env var `RUST_LOG`

## Progress
### Done
- **CMMO 14 stage architecture**: Smart Routing, Tool-First Engine, Chaining, SQLite, Knowledge Graph, Project Intel, Review, Multi-Model, Multi-Agent, DAG, Self-Healing, Performance & DX, RAG Context, Cost Routing.
- **Low RAM Optimization**: Replaced default memory allocator with `mimalloc` to mitigate fragmentation. Implemented Lazy Loading for the Stage 5 symbol graph (`SymbolGraph`) which loads on demand only when queried, and added `graph_unload_workspace` to allow unloading the graph and freeing up memory on demand.
- **Multi-Folder Workspace Support**: Implemented workspace folder stacking (multi-root project workspaces) in the Svelte file explorer tree, configuration saving/loading to `.workspace` files (including support for VS Code `.code-workspace` configuration JSON schemas), dynamic path separator resolution, and merging file entries for AI contextual mentions.
- **CLI Agent Gateway**: Full integration of 6 external CLI agents (Claude, Gemini, OpenCode, Aider, Codex, Agy) into the ReAct loop as tools. CLI Discovery Engine auto-detects installed CLIs. External agents can be triggered via auto-routing (Intent::ExternalAgent) or directly called as tools (`claude_run`, `gemini_run`, etc.) from the ReAct loop.

## API Test Results
| Provider | API Key | Model | Status |
|---|---|---|---|
| Cerebras | `csk-REDACTED` | `gpt-oss-120b` | ✅ Works |
| Mistral | `Ieor4Xt-REDACTED` | `mistral-large-latest` | ✅ Works |
| Vercel | `vck-REDACTED` | `openai/gpt-4o-mini` | ✅ Works |
| Gemini | `AIzaSyD-REDACTED` | `gemini-2.0-flash` | ✅ Key valid, free-tier quota exhausted |

## Testing
- `cargo test` — runs 14 unit tests (no API keys needed, ~9s)
  - 36 unit tests (tool execution, model routing/fallback, model price, system prompt resolution — plus DAG, context, cost routing, CLI adapters, executor, router, provider, model registry)
- `CEREBRAS_API_KEY=... cargo test -- --ignored` — API integration tests
- React loop test (`test_react_loop_coder_read_file`) confirms tool-calling ReAct loop works end-to-end

## Provider Model Notes
- **Model Registry** now has ~100 models from 15+ providers (OpenAI, Anthropic, Gemini, Mistral, DeepSeek, Groq, Together, Fireworks, Cohere, Alibaba, Perplexity, AI21, Cerebras, xAI, Nvidia, SambaNova, DeepInfra, OpenRouter, Ollama)
- **External config**: `%APPDATA%/NyxEdit/models.toml` is auto-created on first run. Edit it to customize models without rebuilding.
- **Auto Routing** prefers local Ollama models over cloud providers when available
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
| **3** Chaining | ✅ `chain_engine.rs`, `run_chain()` | ✅ `ai-proc` compact bar (AIChat.svelte) | ✅ Ya — kalau routing bikin chain plan |
| **4** SQLite | ✅ `sessions.rs` SQLite rewrite, 4 commands | ✅ Session list/save/load | ✅ Ya — tiap chat pake database |
| **5** Knowledge Graph | ✅ `symbol_graph.rs`, `parsers.rs`, file watcher | ❌ 12 commands registered | 🔶 Parsial — search/query lazy-loaded (hanya di-load ke RAM saat query pertama), `graph_unload_workspace` unloads RAM |
| **6** Project Intel | ✅ `project_intel.rs`, framework detection | ❌ 2 commands | ✅ Ya — otomatis dijalankan di routing loop jika context belum di-load |
| **7** Review | ✅ `review.rs`, 3 rules | ❌ 2 commands | ✅ Ya — otomatis menjalankan aturan tinjauan statis pada setiap kode yang dihasilkan model |
| **8** Multi-Model | ✅ `provider_stats.rs`, CircuitBreaker | ❌ 2 commands | ✅ Ya — circuit breaker aktif di fallback loop |
| **9** Multi-Agent | ✅ `agent_orch.rs`, 3 sub-agents | ❌ 4 commands | ✅ Ya — otomatis mendelegasikan tugas kompleks (Refactor, Review, Arch) |
| **10** DAG | ✅ `DagPlan`, `run_dag()` parallel tokio | ❌ | 🔶 Routing prioritaskan DAG untuk RefactorFull/CodeReview |
| **11** Self-Healing | ✅ `self_heal.rs`, health tracking | ❌ 2 commands | ✅ Ya — otomatis memulihkan komponen yang rusak saat startup ketika mendeteksi crash marker |
| **12** Performance & DX | ✅ Cache warming, crash marker, startup health check, mimalloc allocator | ❌ `heal_check_startup`, `heal_clear_crash_marker` | ✅ Cache warm di `ensure_loaded`, crash marker di startup, mimalloc global allocator |
| **13** RAG Conversation Memory | ✅ `context.rs` — compression, cross-session retrieval | ❌ | ✅ Compression aktif di `ai_chat_stream` (OnceLock) |
| **14** Smart Cost Routing | ✅ `cost_routing.rs` — cheapest model, budget limit | ❌ 3 commands | ✅ Ya — otomatis memilih model termurah dalam satu reasoning tier (budget constraint routing) |

**Kesimpulan**: Backend 100% aktif, frontend masih ada beberapa kontrol manual yang belum di-Svelte-in. Yang benar-benar aktif end-to-end secara otomatis di alur utama (main flow): Stage 1, 2, 3, 4, 6, 7, 8, 9 (auto-delegate), 11, 12, 13, dan 14. Lazy loading symbol graph (Stage 5) dan mimalloc allocator menjaga penggunaan RAM NyxEdit tetap rendah.

### Rust files added (Stage 1–14)
- `src-tauri/models.toml` — compiled-in + external model definitions (100 models, 15+ providers, auto-copied to `{APPDATA}/NyxEdit/models.toml` on first run)
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
- `src-tauri/src/modules/cli.rs` — CLI Discovery Engine + 6 adapter impl + streaming subprocess
- `src-tauri/src/modules/executor.rs` — tool execution engine + permission state + diff computation
- `src-tauri/src/modules/handoff.rs` — handoff file management for cross-agent task persistence
- `src-tauri/src/modules/provider.rs` — AiProvider trait + LiteLLM provider abstraction
- `src-tauri/src/modules/router.rs` — RouterDecision (RouteClass + ExecutionMode) + sensitive content detection
- `src-tauri/src/modules/mod.rs` — semua module registrations
- `src-tauri/src/modules/model_registry.rs` — +`load()` TOML method
- `src-tauri/src/modules/routing_engine.rs` — SYMBOL_LOOKUP → TreeSitter, +`route_with_context()`, +ExternalAgent intent
- `src-tauri/src/modules/tool_registry.rs` — +TreeSitter in load_default()
- `src-tauri/src/modules/ai.rs` — +tool-only route, +fallback loop with circuit breaker, +`run_chain()`, +`run_dag()`, +health reporting, +cost budget, +context compression, +`execute_cli_tool()`, +CLI tools in `build_tools()`, +ExternalAgent route in `ai_chat_stream`
- `src-tauri/src/modules/sessions.rs` — full rewrite: JSON files → SQLite (async commands) + `recover_last_session`
- `src-tauri/src/lib.rs` — +graph state + all new module states + commands

### Frontend changes
- `src/lib/components/AIChat.svelte` — +chainSteps state, `ai:route_progress` parser, chain/DAG → compact `ai-proc` bar (animated grey line + summary text, auto-hilang)
- `src/lib/components/FileManager.svelte` — implemented multi-root workspace explorer headers, folder-plus/save icons, "Open Workspace" empty states, dynamic path slashes, and mount reloading hooks
- `src/lib/stores.svelte.ts` — updated `loadWorkspace` to read multi-root file entries and populate merged autocomplete indexes
- `src/lib/components/GitStatus.svelte` — implemented folder selector dropdown to switch active Git target on multi-folder workspaces
- `src/lib/components/SearchInFiles.svelte` — implemented parallel multi-folder file/content search and relative path rendering with folder tags
- `src/lib/components/FileManager.svelte` & `GitStatus.svelte` — migrated store subscriptions to `onMount` and wrapped reactive effects in Svelte 5 `untrack()` to eliminate infinite rendering loops and CPU spikes
- `src/lib/components/IntelPanel.svelte` — implemented active workspace target dropdown selector to dynamically load and index symbols for multi-folder targets
- `src/lib/components/Terminal.svelte` — added paste confirmation with `[Paste ~N lines]` floating bar + `attachCustomKeyEventHandler` for Ctrl+V/Shift+Insert + right-click context menu (Copy/Paste/Select All)
- `src/lib/components/AIFloatingBar.svelte` — added paste confirmation (Paste ~N lines bar, Enter confirm, Esc cancel) for text paste, image/file paste unchanged
- `src/lib/components/AIChat.svelte` — replaced chain/DAG stepper with compact `ai-proc` grey shimmer line + single-line status summary, auto-clear on done

### Verification
- `cargo check`: 0 errors
- `cargo test`: 14/14 passed, 5 ignored (API tests need env vars)
- `npm run check`: 0 errors (74 pre-existing warnings)
- Total CMMO: Stage 1–14 100% backend, ~40% frontend integration
