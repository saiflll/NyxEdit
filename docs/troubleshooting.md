# Troubleshooting — NyxEdit

## AI Chat — "All providers exhausted"

**Penyebab**: Semua provider gagal — Ollama tidak berjalan, API key salah, atau kuota habis.

**Solusi**:
1. Pastikan Ollama berjalan: `ollama ps`
2. Cek API key di Settings → Agents
3. Buka `%APPDATA%/nyxedit/models.toml`, pastikan provider tercantum

## Build Error — Rust Compilation

**Error**: `linker 'link.exe' not found`
**Solusi**: Install MSVC Build Tools:
```bash
winget install Microsoft.VisualStudio.2022.BuildTools
```

**Error**: `package `xxx` not found`
**Solusi**: Update Rust toolchain:
```bash
rustup update stable
```

## Tauri — "WebView2 not found"

**Penyebab**: Windows 10/11 seharusnya sudah include WebView2. Jika tidak:

**Solusi**: Download [Evergreen WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

## Port 1420 already in use

**Solusi**: Matikan proses lain yang memakai port 1420, atau ubah port di `vite.config.ts`.

## Model Registry — Provider not recognized

**Penyebab**: Provider ada di Settings tapi tidak ada di `models.toml`.

**Solusi**:
1. Buka `%APPDATA%/nyxedit/models.toml`
2. Tambahkan model baru dengan format TOML:
```toml
[[models]]
id = "model-id"
name = "Model Name"
provider = "provider-name"
context_window_limit = 32768
reasoning_tier = "Medium"
specialization = ["Chat"]
cost_per_1k_tokens = 0.0
avg_latency_p95_ms = 2000
max_parallel_calls = 5
supports_streaming = true
supports_tool_use = true
fallback_priority = 1
```

## Circuit Breaker — Provider skipped for 60s

**Penyebab**: Provider gagal 3x berturut-turut, circuit breaker terbuka.

**Solusi**: Tunggu 60 detik atau restart app. Circuit breaker akan reset otomatis.

## Logging

Aktifkan logging Rust:
```bash
$env:RUST_LOG="info"
npm run tauri dev
```

Log akan muncul di terminal. Untuk debug lebih dalam:
```bash
$env:RUST_LOG="debug"
```
