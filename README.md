# NyxEdit

[![Build and Release](https://github.com/saiflll/NyxEdit/actions/workflows/build.yml/badge.svg)](https://github.com/saiflll/NyxEdit/actions/workflows/build.yml)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-blue.svg?logo=tauri)](https://tauri.app/)
[![Svelte](https://img.shields.io/badge/SvelteKit-5.0-orange.svg?logo=svelte)](https://svelte.dev/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

AI-native desktop workspace and editor. Combines code editing, native split-terminal, multi-agent AI orchestration, and local knowledge graph in a unified desktop interface.

Built with **Tauri 2 (Rust)** + **SvelteKit 5 (TypeScript)**.

---

## Key Features

- **Native Split Terminal** — integrated terminal emulator via `portable-pty`, private sessions, splits.
- **Multi-Agent AI Orchestration** — smart routing, chain-of-thought plans, parallel DAG executors, self-healing code review, budget enforcement.
- **Multimodal Clipboard** — paste code, drag-drop files, paste clipboard images into AI chat.
- **Knowledge Graph** — incremental symbol indexing, semantic code lookup via `tree-sitter`.
- **PlatformIO Integration** — IoT compilation, firmware flashing, hardware board management.
- **Secure Credentials** — API keys stored via OS-native keychains (`keyring`).
- **Low RAM** — `mimalloc` allocator, lazy-loaded symbol graphs.

---

## Technology Stack

- **Frontend** — SvelteKit 5 (SPA, `adapter-static`), TypeScript, Vite 6, Vanilla CSS.
- **Backend** — Tauri 2 (Rust), `nyxedit_lib` crate.
- **Database** — SQLite (`sqlx` async).
- **Allocator** — `mimalloc`.

---

## Getting Started

### Prerequisites

- Node.js 22+
- Rust Stable Toolchain (MSVC on Windows)
- WebView2 (pre-bundled on Windows 10/11)

### Development Setup

```bash
# Clone
git clone https://github.com/saiflll/NyxEdit.git
cd NyxEdit

# Install deps
npm ci

# Run dev mode
npm run tauri dev
```

---

## Build

```bash
npm run tauri build
```

Artifacts in `src-tauri/target/release/bundle/`.

---

## Test

```bash
cd src-tauri && cargo test   # Rust tests (14 unit tests)
npm run check                # Svelte/TS type check
```

---

## CI/CD

GitHub Actions on push/PR to `main`. Tag `v*` triggers release build via `tauri-apps/tauri-action`.
