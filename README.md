# NyxEdit

[![Build and Release](https://github.com/saiflll/Codlib/actions/workflows/build.yml/badge.svg)](https://github.com/saiflll/Codlib/actions/workflows/build.yml)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-blue.svg?logo=tauri)](https://tauri.app/)
[![Svelte](https://img.shields.io/badge/SvelteKit-5.0-orange.svg?logo=svelte)](https://svelte.dev/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

**NyxEdit** is a premium, AI-native desktop workspace and editor designed for modern developers. It combines powerful code editing, a native split-terminal emulator, automated multi-agent AI chat orchestration, and a local knowledge graph, all integrated into a unified desktop interface.

Built using **Tauri 2 (Rust)** and **SvelteKit 5 (TypeScript)**, NyxEdit is optimized to maintain a lightweight system footprint with high performance.

---

## 🚀 Key Features

*   🖥️ **Native Split Terminal**: Fully integrated terminal emulator using `portable-pty` supporting private sessions and splits.
*   🤖 **Multi-Agent AI Orchestration**: Smart routing, chain-of-thought plans, parallel DAG executors, self-healing code checks, and budget enforcement.
*   📎 **Multimodal Clipboard Attachments**: Copy code text, drag & drop files, or paste clipboard images (screenshots) directly (`Ctrl + V`) into the AI chat.
*   🧠 **Knowledge Graph**: Incremental symbol indexing and semantic code lookup powered by `tree-sitter`.
*   🔌 **PlatformIO Integration**: Built-in IoT compilation, firmware flashing, and hardware board management.
*   🔒 **Secure Credentials**: Credentials and API keys are stored securely using OS-native keychains via `keyring`.
*   ⚡ **Low RAM Footprint**: Optimized using the `mimalloc` allocator and lazy-loaded symbol graphs.

---

## 🛠️ Technology Stack

*   **Frontend**: SvelteKit 5 (SPA mode, `adapter-static`), TypeScript, Vite 6, Vanilla CSS.
*   **Backend**: Tauri 2 (Rust), `codlib_lib` crate.
*   **Database**: SQLite (`sqlx` async).
*   **Memory Allocator**: `mimalloc` (mitigates heap fragmentation).

---

## 🏃 Getting Started

### Prerequisites

Ensure you have the following installed on your development machine:
*   [Node.js 22+](https://nodejs.org/)
*   [Rust Stable Toolchain](https://www.rust-lang.org/) (MSVC toolchain on Windows)
*   [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (pre-bundled in Windows 10/11)

### Development Setup

1.  **Clone the Repository** (note: the repository name is `Codlib`, but the compiled desktop app product name is **NyxEdit** and the application ID is `com.lenovo.nyxedit`) and navigate to the project directory:
    ```bash
    git clone https://github.com/saiflll/Codlib.git
    cd Codlib
    ```

2.  **Install dependencies**:
    ```bash
    npm ci
    ```

3.  **Run in Development Mode**:
    ```bash
    npm run tauri dev
    ```
    This launches the Vite dev server alongside the native Tauri desktop application.

---

## 📦 Building and Packaging

### Production Build (Standalone Installers)

To compile the SvelteKit frontend and the Rust backend into platform-specific installers (e.g., `.msi` or `.exe` on Windows):
```bash
npm run tauri build
```
The resulting installers will be saved in `src-tauri/target/release/bundle/`.

---

## 🧪 Verification and Testing

### Backend Unit Tests
Run the Rust test suite containing unit tests for tool executors, model routing, DAG workflows, and cost budgets:
```bash
cd src-tauri
cargo test
```

### Type Checking
Validate the TypeScript and Svelte frontend components for compiler warnings and errors:
```bash
npm run check
```

---

## ✈️ CI/CD Deployment

We use GitHub Actions to automate our release workflow:
*   **Commit Validation**: Every push to the `main` branch or pull request is checked and compiled to ensure stability across Linux, Windows, and macOS.
*   **Automated Release**: Creating a tag matching `v*` (e.g. `v1.0.0`) automatically compiles the final production packages and attaches them to a new GitHub Release page using the official `tauri-apps/tauri-action`.
