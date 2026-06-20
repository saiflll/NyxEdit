# Changelog

Format: [SemVer](https://semver.org/).

## [Unreleased]

### Added
- docs/setup.md, docs/api.md, docs/troubleshooting.md — development docs.
- docs/architecture/cmmo.md — full CMMO architecture.
- PULL_REQUEST_TEMPLATE.md & ISSUE_TEMPLATE/ (bug_report + feature_request).
- connect_timeout(5s) on reqwest client — connection refused fails fast.
- Connection error detection in fallback loop — skip intra-provider on service down.

### Fixed
- Fallback loop no longer stuck trying 22 Ollama models when Ollama is down.
- .gitignore updated (cache dirs: .nyx/, .notepad_temp/).

### Improved
- Model registry expanded: 4 -> ~100 models, 15+ providers.
- Model registry loads from %APPDATA%/contlib/models.toml (editable without rebuild).

---

## [1.2.0] - 2024-05-20

### Added
- Multi-folder workspace support.
- PlatformIO integration for IoT development.
- Multimodal clipboard (text, images, files).
- Stage 14 (Smart Cost Routing) auto-enabled.

### Fixed
- Terminal split performance (Windows lag).
- API auth bugs (Vercel/Gemini).
- Centralized debug mode via .env (`DEBUG=1/0`).

### Improved
- CMMO Stage 5 (Knowledge Graph) docs.
- Faster CI/CD with node_modules caching.

---

## [1.1.0] - 2024-04-15

### Added
- Stage 13 (Multi-Tool Workspace) auto-enabled.
- GitHub Actions for automated deployment.
- Centralized debug mode via env var.

### Fixed
- Markdown rendering in terminal split.
- Backend connection issues.

### Improved
- API docs for Stage 12 (Smart Search).
- Modularized project structure.

---

## [1.0.0] - 2024-03-01

### Added
- Desktop app with core features:
  - Stage 1-10 (Basic Features) active.
  - SvelteKit UI.
  - Rust/Tauri backend.
- Initial docs in README.md and docs/architecture/cmmo.md.
- CI/CD for Linux, Windows, macOS.

---

### Format Reference

```
## [version] - date
### Added
- Features or additions.

### Fixed
- Bug fixes.

### Improved
- Performance, docs, or UX improvements.
```

---

### Contributing

See [CONTRIBUTING.md](.github/CONTRIBUTING.md).
