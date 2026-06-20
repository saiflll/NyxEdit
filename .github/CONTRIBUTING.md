# Kontribusi ke NyxEdit

Terima kasih telah berkontribusi! 🎉 NyxEdit adalah proyek open-source yang dikelola oleh CMMO dan komunitas.

## 📌 Cara Kontribusi

### 1. Fork Repository
Klik tombol **Fork** di pojok kanan atas halaman GitHub untuk membuat salinan repository di akun Anda.

### 2. Clone Repository
```bash
# Ganti `username` dengan username GitHub Anda
git clone https://github.com/username/NyxEdit.git
cd NyxEdit
```

### 3. Buat Branch Baru
Buat branch untuk perubahan Anda:
```bash
git checkout -b fitur-anda-atau-perbaikan-bug
```

### 4. Lakukan Perubahan
- Ikuti pedoman kode di bawah.
- Tambahkan tes jika memungkinkan.
- Dokumentasikan perubahan Anda.

### 5. Commit Perubahan
```bash
git add .
git commit -m "Deskripsi jelas tentang perubahan yang dilakukan"
```

### 6. Push ke Repository Anda
```bash
git push origin fitur-anda-atau-perbaikan-bug
```

### 7. Buka Pull Request
Buka **Pull Request** dari branch Anda ke branch `main` di repository utama.

---

## 🛠 Pedoman Kode

### Struktur Proyek
- **Frontend**: `src/` (SvelteKit + TypeScript)
- **Backend**: `src-tauri/` (Rust)
- **Dokumentasi**: `docs/`
- **Konfigurasi**: `config/`, `.env`

### Bahasa & Framework
| Bagian | Bahasa/Framework |
|--------|------------------|
| Frontend | SvelteKit + TypeScript |
| Backend | Rust (Tauri) |
| Dokumentasi | Markdown |
| Konfigurasi | YAML/JSON/TOML |

### Penamaan
- **File**: Gunakan `kebab-case` untuk file konfigurasi, `snake_case` untuk dokumentasi.
  - Contoh: `setup-guide.md`, `api_reference.yaml`
- **Variabel**: Gunakan `camelCase` untuk TypeScript/Rust, `snake_case` untuk dokumentasi.
- **Konstanta**: Gunakan `UPPER_SNAKE_CASE` (contoh: `MAX_RETRY`).

### Gaya Kode
- **Indentation**:
  - TypeScript/Svelte: 2 spasi
  - Rust: Tab
  - Markdown: Standar GitHub
- **Komponen Svelte**: Gunakan format `.svelte` dengan skop gaya lokal.
- **Error Handling**: Gunakan pola early return dan hindari `try/catch` yang tersebar.

### Tes & Validasi
Sebelum mengirim PR, pastikan:
```bash
# Frontend (SvelteKit)
npm run check      # TypeScript + Lint
npm run test       # Tes unit

# Backend (Rust)
cd src-tauri
cargo check        # Cek kompile
cargo test         # Tes unit
cargo clippy       # Lint statis
```

### Dokumentasi
- **Fitur Baru**: Dokumentasikan di `docs/` atau perbarui `README.md`.
- **API**: Jika menambahkan API, dokumentasikan di `docs/api.md` dengan contoh penggunaan.
- **Perubahan**: Catat perubahan di `CHANGELOG.md`.

---

## ✅ Tipe Kontribusi

| Tipe | Deskripsi |
|------|-----------|
| 🐛 **Perbaikan Bug** | Laporkan atau perbaiki bug yang ditemukan. |
| ✨ **Fitur Baru** | Ajukan ide fitur atau implementasikan fitur yang diminta. |
| 📚 **Dokumentasi** | Perbaiki atau tambahkan dokumentasi yang kurang. |
| 🧪 **Tes** | Tambahkan tes untuk meningkatkan cakupan tes. |
| 🎨 **UI/UX** | Perbaiki tampilan atau pengalaman pengguna. |
| 🔧 **Tooling** | Perbaiki atau tambahkan alat pembangunan (CI/CD, skrip, dll.). |

---

## 📂 Direktori Penting

```
NyxEdit/
├── .github/            # Template GitHub (issues, PR, workflows)
├── docs/               # Dokumentasi proyek
│   ├── setup.md        # Panduan setup development
│   ├── api/            # Dokumentasi API
│   └── troubleshooting.md # Pemecahan masalah umum
├── src/                # Frontend (SvelteKit)
├── src-tauri/          # Backend (Rust)
├── static/             # Aset statis (gambar, font, dll.)
├── config/             # Konfigurasi proyek (.env, YAML, dll.)
└── scripts/            # Skrip pembangunan/pengujian
```

---

## 🚫 Apa yang Tidak Diterima

- **Perubahan besar tanpa diskusi**: Jangan lakukan perubahan besar (misal: penggantian framework) tanpa membuka issue terlebih dahulu.
- **Kode tanpa tes**: Kontribusi tanpa tes akan ditolak kecuali untuk perbaikan dokumentasi.
- **Spam**: Kontribusi yang tidak relevan atau berisi spam.

---

## 🤝 Aturan Komunitas

- Hormati semua kontributor.
- Gunakan bahasa yang sopan dan profesional.
- Fokus pada solusi, bukan kritik pribadi.

---

## 📞 Bantuan

Jika Anda memiliki pertanyaan:
1. Baca [README.md](../README.md).
2. Cek [Issues](https://github.com/saiflll/NyxEdit/issues) yang sudah ada.
3. Buka issue baru jika masalah Anda belum tercatat.

---

## 🌟 Ucapan Terima Kasih

Terima kasih kepada semua kontributor yang telah membantu membuat NyxEdit lebih baik!

[![Contributors](https://contrib.rocks/image?repo=saiflll/NyxEdit)](https://github.com/saiflll/NyxEdit/graphs/contributors)

---

> "Bersama, kita bisa membuat alat yang lebih cerdas dan efisien." — CMMO
