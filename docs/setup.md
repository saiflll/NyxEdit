# Setup Development — NyxEdit

## Prasyarat

| Tool | Version | Catatan |
|------|---------|---------|
| Node.js | 22+ | [nodejs.org](https://nodejs.org/) |
| Rust | Stable (MSVC) | [rustup.rs](https://rustup.rs/) — Windows: MSVC toolchain |
| WebView2 | Bundled | Sudah include di Windows 10/11 |

## Setup Awal

```bash
# 1. Clone
git clone https://github.com/saiflll/NyxEdit.git
cd NyxEdit

# 2. Install JS dependencies
npm ci

# 3. Build frontend + Tauri
npm run tauri build
```

## Development Mode

```bash
# Dev server (Vite + Tauri desktop)
npm run tauri dev
```

Vite berjalan di port 1420 (strict port). Tauri akan otomatis membuka jendela desktop.

## Build Produksi

```bash
npm run tauri build
```

Installer: `src-tauri/target/release/bundle/nsis/NyxEdit_*_setup.exe`

## Testing

```bash
# Frontend type check
npm run check

# Rust unit tests (14 tests, no API key needed)
cd src-tauri && cargo test

# Rust API integration tests (butuh env vars)
CEREBRAS_API_KEY=sk-... cargo test -- --ignored
```
