# AGENTS.md — contlib (NyxEdit)

## Stack
- **Frontend**: SvelteKit 5 + TypeScript + Vite 6, static adapter (SPA mode, fallback `index.html`)
- **Backend**: Tauri 2 (Rust), crate `codlib_lib`
- **Nama app**: NyxEdit (binary `codlib.exe`)
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
  - 19 unit tests (tool execution, model price, system prompt resolution)
  - 5 API tests (`#[ignore]` — run with `cargo test -- --ignored`, needs env vars)
- `env_logger::init()` di `run()` — log Rust via env var `RUST_LOG`

## Progress
### Done
- **Char-by-char streaming fix**: `run_react_loop` emits full content at once.
- **Rust warnings cleanup**: removed dead `walk` variable, wired `timeout` in `bash_run`.
- **Session management** (`sessions.rs`): `ChatSession` persisted as JSON in `{app_data_dir}/sessions/`, 4 Tauri commands (`ai_list_sessions`, `ai_get_session`, `ai_save_session`, `ai_delete_session`), `SessionsState` init via `.setup()` callback.
- **AIChat.svelte sessions UI**: sidebar toggle, session list, New Chat, auto-save after each response.
- **Provider-specific model detection**: `PROVIDER_ENDPOINTS` for 6 providers; `fetch_gemini_models()` (query-param auth); `fetch_openrouter_models()` (with model names); `ai_list_models` routes per provider.
- **Settings.svelte form redesign**: `PROVIDERS` array with 9 providers, provider dropdown, 2-column grid (model, API key, base URL, temperature, persona, system prompt), single-agent-at-a-time.
- **Compilation**: `npm run check` 0 errors, `cargo check` 0 errors.
- **Rust tests**: 19 tool execution unit tests (read_file, write_file, edit, grep, glob, list_directory, bash_run, system prompt, model price, etc.).
- **API verification**: Mistral (`mistral-large-latest`) and Vercel (`openai/gpt-4o-mini`) confirmed working. Cerebras (`gpt-oss-120b`) and Gemini (free-tier quota) keys valid.
- **ReAct loop end-to-end**: Cerebras Coder persona → read_file tool → answer about Cargo.toml package name — passes.
- **Vercel model ID fix**: changed `:` to `/` separator (Vercel AI Gateway uses `openai/gpt-4o-mini`, not `openai:gpt-4o-mini`).

## API Test Results
| Provider | API Key | Model | Status |
|---|---|---|---|
| Cerebras | `csk-vm3kv...` | `gpt-oss-120b` | ✅ Works |
| Mistral | `Ieor4Xt...` | `mistral-large-latest` | ✅ Works |
| Vercel | `vck_2Cty...` | `openai/gpt-4o-mini` | ✅ Works |
| Gemini | `AIzaSyD...` | `gemini-2.0-flash` | ✅ Key valid, free-tier quota exhausted |

## Testing
- `cargo test` — runs 19 unit tests (no API keys needed, ~9s)
- `CEREBRAS_API_KEY=... cargo test -- --ignored` — API integration tests
- React loop test (`test_react_loop_coder_read_file`) confirms tool-calling ReAct loop works end-to-end

## Provider Model Notes
- **Vercel** uses `/` separator: `openai/gpt-4o-mini`, `meta/llama-3.1-8b`, etc.
- **Cerebras direct API** has `gpt-oss-120b`, `zai-glm-4.7` (different from Vercel's `cerebras:llama3.1-8b`)
- **Gemini** uses query-param auth: `?key=...`, models listed as `models/gemini-2.0-flash`
- **Mistral direct API** uses standard OpenAI-compatible format
