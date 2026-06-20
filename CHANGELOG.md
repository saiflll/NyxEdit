# Changelog NyxEdit

Semua perubahan signifikan didokumentasikan di sini dengan format [SemVer](https://semver.org/).

## [Unreleased]
### Ditambahkan
- `docs/setup.md`, `docs/api.md`, `docs/troubleshooting.md` — dokumentasi development.
- `docs/architecture/cmmo.md` — arsitektur CMMO lengkap.
- `PULL_REQUEST_TEMPLATE.md` & `ISSUE_TEMPLATE/` (bug_report + feature_request).
- `connect_timeout(5s)` pada reqwest client — connection refused gagal cepat tanpa nunggu 120s.
- Deteksi connection error di fallback loop — langsung skip intra-provider fallback ketika service down.

### Diperbaiki
- Fallback loop tidak lagi stuck mencoba 22 model Ollama berturut-turut saat Ollama mati — langsung loncat ke provider lain.
- `.gitignore` diperbarui untuk mengabaikan cache (`.nyx/`, `.notepad_temp/`).

### Ditingkatkan
- Model registry diperluas dari 4 model → ~100 model dari 15+ provider.
- Model registry sekarang load dari `%APPDATA%/contlib/models.toml` (bisa diedit tanpa rebuild).

---

## [1.2.0] - 2024-05-20
### Ditambahkan
- Fitur multi-folder workspace (kerja dengan beberapa folder sekaligus).
- Integrasi PlatformIO untuk IoT development.
- Clipboard multimodal (teks, gambar, file).
- Stage 14 (Smart Cost Routing) aktif otomatis.

### Diperbaiki
- Kinerja terminal split (masalah lag pada Windows).
- Bug autentikasi API (Vercel/Gemini).
- Mode debug terpusat via `.env` (kontrol `DEBUG=1/0`).

### Ditingkatkan
- Dokumentasi CMMO Stage 5 (Knowledge Graph) di `docs/architecture/cmmo.md`.
- CI/CD lebih cepat dengan caching `node_modules`.

---

## [1.1.0] - 2024-04-15
### Ditambahkan
- Stage 13 (Multi-Tool Workspace) aktif otomatis.
- Integrasi GitHub Actions untuk deployment otomatis.
- Mode debug terpusat via environment variable.

### Diperbaiki
- Bug rendering markdown di terminal split.
- Masalah koneksi ke server backend.

### Ditingkatkan
- Dokumentasi API untuk Stage 12 (Smart Search).
- Struktur proyek dimodularisasi lebih baik.

---

## [1.0.0] - 2024-03-01
### Ditambahkan
- Aplikasi desktop pertama dengan fitur dasar:
  - Stage 1-10 (Basic Features) aktif.
  - UI/UX dengan SvelteKit.
  - Backend Rust (Tauri) untuk performa tinggi.
- Dokumentasi awal di `README.md` dan `docs/architecture/cmmo.md`.
- CI/CD untuk Linux, Windows, dan macOS.

---

### Format
Setiap rilis memiliki format:
```markdown
## [versi] - tanggal
### Ditambahkan
- Fitur atau perubahan yang ditambahkan.

### Diperbaiki
- Bug yang diperbaiki.

### Ditingkatkan
- Performa, dokumentasi, atau pengalaman pengguna yang ditingkatkan.
```

---

### Panduan Kontribusi
Jika Anda ingin berkontribusi, lihat [CONTRIBUTING.md](.github/CONTRIBUTING.md) untuk detailnya.
