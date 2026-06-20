# API Reference — NyxEdit

## Tauri Commands

### AI Chat

| Command | Description |
|---------|-------------|
| `ai_chat_stream` | Streaming chat dengan routing otomatis |
| `ai_chat` | Non-streaming chat (legacy) |
| `probe_single_model` | Test koneksi ke model tertentu |

### Sessions

| Command | Description |
|---------|-------------|
| `list_sessions` | Daftar session chat |
| `load_session` | Load session by ID |
| `save_session` | Simpan session |
| `delete_session` | Hapus session |
| `recover_last_session` | Recovery crash terakhir |

### File System

| Command | Description |
|---------|-------------|
| `search_files` | Ripgrep search dengan scan cache |
| `git_status` | Git status current workspace |
| `git_diff` | Git diff file |
| `git_log` | Git log history |
| `git_commit` | Commit staging |

### System

| Command | Description |
|---------|-------------|
| `get_health_dashboard` | Health status semua komponen |
| `get_heal_history` | Riwayat heal/error |
| `set_auto_repair` | Toggle auto-repair |
| `get_cost_analytics` | Cost dan usage analytics |
| `get_routing_config` | Konfigurasi routing saat ini |
| `update_routing_config` | Update konfigurasi routing |
| `graph_index_workspace` | Index workspace ke knowledge graph |
| `graph_search` | Search symbol graph |

## Events (Frontend ↔ Backend)

| Event | Direction | Description |
|-------|-----------|-------------|
| `ai:chunk` | Backend → Frontend | Stream chunk teks |
| `ai:done` | Backend → Frontend | Stream selesai |
| `ai:error` | Backend → Frontend | Error dari AI |
| `ai:route_progress` | Backend → Frontend | Status routing/fallback |
| `ai:budget_warning` | Backend → Frontend | Budget mendekati limit |
