**COGNITIVE MULTI-MODEL**

**ORCHESTRATOR**

CMMO — Blueprint v2.1

*Architecture & Implementation Planning*

Smart Routing · Multi-Agent · Tool-First · Knowledge Graph · Self-Healing

*Revised with CTO Review — Updated Phase Roadmap*

# **0. Respons terhadap CTO Review — Apa yang Berubah di v2.1**

Blueprint v2.0 mendapat skor **8.7/10** dari reviewer eksternal. Tiga kritik utama diintegrasikan penuh ke v2.1 ini:

**Kritik CTO**

**Status di v2.0**

**Tindakan di v2.1**

Belum ada Tool Registry

❌ Tidak ada

✅ Ditambah sebagai Section 4 — tool-first sebelum model

Belum ada Knowledge Graph

❌ Tidak ada

✅ Ditambah sebagai Section 7 — SymbolGraph + dependency index

Belum ada Learning Layer

❌ Tidak ada

✅ Ditambah sebagai Section 8 — Project Intelligence store

Multi-agent terlalu kompleks v1

7 agent sekaligus

✅ Dipecah: 3 agent di Phase 1, bertahap sampai Phase 5

DAG terlalu berat untuk awal

DAG dari Phase 1

✅ Linear Vec<ChainNode> dulu, DAG masuk Phase 4

*Nilai dari CTO: Arsitektur 9/10 · Skalabilitas 9/10 · Realistis Diimplementasikan 7.5/10. v2.1 ini menargetkan skor implementasi naik ke 9/10 dengan roadmap bertahap yang lebih realistis.*

# **1. Executive Summary**

CMMO adalah AI IDE orchestration engine yang bertujuan melampaui Cursor melalui empat keunggulan struktural: **(1) Tool-first routing** — alat dipilih sebelum model; **(2) Capability-based model selection** — routing berdasarkan metadata, bukan nama model; **(3) Workspace Knowledge Graph** — dependency graph kode menggantikan brute-force context dump; **(4) Project Intelligence** — sistem belajar pola proyek dari waktu ke waktu.

**Dimensi**

**Cursor**

**CMMO v2.1**

**Model Strategy**

Single model, manual switch

Auto-route berdasarkan capability metadata

**Tool Integration**

Model menjadi tool executor

Tool Registry mandiri, model adalah last step

**Codebase Understanding**

Context window dump

SymbolGraph + dependency index

**Project Memory**

Session only, tidak persisten

Project Intelligence — belajar dari waktu ke waktu

**Failure Recovery**

Manual retry

Self-healing: checkpoint + fallback queue

**Multi-Agent**

Tidak ada

DAG multi-agent (bertahap per phase)

# **2. Taksonomi Model & Capability Metadata**

Routing engine tidak pernah menggunakan nama model secara hardcoded. Semua keputusan berdasarkan metadata kemampuan. Ketika model baru rilis, hanya perlu update config — routing logic tidak berubah.

## **2.1 Empat Tier Model**

**Tier**

**Context Window**

**Reasoning**

**Contoh Model**

**Primary Use Case**

**Wide-Context Scanner**

100k–2M+ tokens

Medium

Gemini 1.5-Pro/2.0-Flash

Scan & compress large codebases

**Reasoning Engine**

32k–128k tokens

Ultra-High

DeepSeek-R1, o1, o3

Algorithm, debug logic, arch design

**Execution Coder**

32k–64k tokens

Medium

Qwen-2.5-Coder, Claude-3.5

Write/apply code, generate diffs

**Simple Helper**

8k–32k tokens

Low

Llama-3-8B, Gemini Flash

Q&A, explain, format, lookup

## **2.2 ModelMetadata Schema (Rust)**

struct ModelMetadata {

  id: String,

  context_window_limit: u32,      // token count

  reasoning_tier: ReasoningTier,  // UltraHigh | High | Medium | Low

  specialization: Vec<Spec>,      // [Code, Scan, Chat, Test, Review]

  cost_per_1k_tokens: f64,        // USD — untuk cost-aware routing

  avg_latency_p95_ms: u32,        // P95 latency — untuk SLA routing

  max_parallel_calls: u8,

  supports_streaming: bool,

  supports_tool_use: bool,        // baru: apakah model bisa pakai tools

  fallback_priority: u8,

}

# **3. Tool Registry — Tool-First Before Model [BARU]**

*Ini adalah penambahan terbesar dari v2.0. CTO reviewer benar: sistem harus tool-centric, bukan model-centric. Model adalah last resort setelah tools deterministic dijalankan.*

Sebelum model dipanggil, Routing Engine memeriksa Tool Registry: **apakah task ini bisa diselesaikan sepenuhnya atau sebagian oleh tools deterministic?** Tools jauh lebih cepat, lebih murah, dan lebih akurat untuk tugas pencarian/traversal kode.

## **3.1 Tool Registry Schema**

struct ToolMetadata {

  id: ToolId,

  name: String,                   // "ripgrep", "tree-sitter", "lsp"

  capability: Vec<ToolCapability>, // [Search, Parse, Navigate, Execute]

  input_type: Vec<InputType>,     // [FilePath, Symbol, Query, Regex]

  output_type: OutputType,        // StructuredData | Text | FileList

  avg_latency_ms: u32,            // biasanya < 100ms

  cost: ToolCost,                 // Free | ComputeOnly

  requires_index: bool,           // butuh pre-built index?

}

## **3.2 Tool Registry — Daftar Tools**

**Tool**

**Capability**

**Input**

**Menggantikan Apa di Model Call**

**ripgrep**

Search

Regex / keyword

"Cari semua panggilan fungsi X" — tidak perlu model sama sekali

**tree-sitter**

Parse

File path

"Extract semua function signature" — hasil struktural akurat

**LSP (rust-analyzer dll)**

Navigate

Symbol name

"Go to definition", "find references" — deterministic

**git-log/blame**

History

File / symbol

"Siapa yang ubah fungsi ini?" — tidak perlu AI

**cargo/npm check**

Compile

Project path

"Ada error compile?" — ground truth sebelum AI analisis

**AST Differ**

Diff

Two file versions

Structural diff lebih akurat dari model untuk apply patch

## **3.3 Tool-First Routing Logic**

Sebelum masuk ke Model Routing Engine, pipeline selalu cek Tool Registry:

fn route_with_tools(intent: &Intent, workspace: &Workspace) -> RoutePlan {

  // Step 1: cek apakah tools bisa selesaikan task sepenuhnya

  let tool_plan = TOOL_REGISTRY.match_tools(intent);

  if tool_plan.fully_resolves(intent) {

    return RoutePlan::ToolOnly(tool_plan);  // model tidak dipanggil

  }

  // Step 2: tools sebagai pre-processor, hasilnya dimasukkan ke model

  let tool_output = tool_plan.run_partial(workspace);

  let enriched_context = Context::merge(intent, tool_output);

  // Step 3: baru routing ke model tier yang tepat

  MODEL_ROUTER.route(enriched_context)

}

*Contoh nyata: 'Cari semua memory leak di folder /auth' — ripgrep cari pattern unsafe pointer, tree-sitter parse ownership, compiler check error. Hasilnya baru dimasukkan ke Reasoning Engine. Model tidak perlu baca 50k baris dari awal.*

# **4. Routing Engine — 3-Layer Decision**

Setelah Tool Registry dijalankan, sisa context yang tidak bisa diselesaikan tools masuk ke 3-layer model routing.

## **4.1 Layer 1 — Context Size Estimator (tiktoken-rs)**

**Threshold**

**Flag**

**Action**

< 8k tokens

CONTEXT_SMALL

Direct ke target tier, tidak perlu Scanner

8k–30k tokens

CONTEXT_MEDIUM

Eligible Reasoning Engine atau Execution Coder

> 30k tokens

CONTEXT_LARGE

Wajib melalui Wide-Context Scanner (compression stage)

> 500k tokens

CONTEXT_MASSIVE

Streaming chunked + incremental summary merge

## **4.2 Layer 2 — Intent Classifier (9 Kategori)**

**Intent**

**Trigger Signal**

**Model Target**

EXPLAIN_SIMPLE

"apa itu", "jelaskan", "bedanya"

Simple Helper

CODE_WRITE

"tulis fungsi", "buat class"

Execution Coder

CODE_REVIEW

"review", "tinjau kode"

Reasoning Engine (+ Scanner jika LARGE)

DEBUG_LOGIC

"bug", "kenapa salah"

Reasoning Engine

REFACTOR_FULL

"refactor", "restructure"

Full Chain (lihat Section 5)

ARCH_DESIGN

"arsitektur", "desain sistem"

Reasoning Engine (High Priority)

TEST_GENERATE

"unit test", "coverage"

Execution Coder (spec=test)

SCAN_ONLY

"cari file", "list semua"

Tool Registry → Scanner jika perlu

SYMBOL_LOOKUP

"dimana fungsi X", "definisi Y"

Tool-Only (LSP/ripgrep), no model

## **4.3 Layer 3 — Output Type Discriminator**

Mencegah over-routing. 'Review folder ini' dengan output NARRATIVE tidak butuh Coder — Scanner + Reasoning sudah cukup.

**Output Type**

**Karakteristik**

**Implikasi Routing**

NARRATIVE

Penjelasan, summary, analisis teks

Wide-Scanner cukup, stop sebelum Coder

CODE_DIFF

Patch/diff spesifik ke file

Full chain atau Execution Coder

CODE_FULL

File baru atau rewrite lengkap

Execution Coder (+ Reasoning jika complex)

PLAN_ONLY

Pseudocode, arsitektur, langkah

Reasoning Engine, stop (hemat Coder)

TOOL_OUTPUT

Hasil ripgrep/LSP/compiler

Tool-Only, model tidak dipanggil

## **4.4 Final Routing Decision Matrix**

**Context Size**

**Intent**

**Output Type**

**Route**

ANY

SYMBOL_LOOKUP

TOOL_OUTPUT

→ Tool-Only (ripgrep/LSP) — 0 token cost

SMALL

EXPLAIN_SIMPLE

NARRATIVE

→ Simple Helper

MEDIUM

CODE_WRITE

CODE_FULL

→ Execution Coder

MEDIUM

DEBUG_LOGIC

CODE_DIFF

→ Tools pre-process → Reasoning → Coder

LARGE

SCAN_ONLY

NARRATIVE

→ Scanner only (cache check dulu)

LARGE

CODE_REVIEW

NARRATIVE

→ Scanner → Reasoning (stop, no Coder)

LARGE

REFACTOR_FULL

CODE_DIFF

→ Tools → Scanner → Reasoning → Coder (Full)

# **5. Chaining Engine — Linear dulu, DAG kemudian**

*Mengikuti saran CTO: implementasi dimulai dari Vec<ChainNode> linear. DAG parallel masuk di Phase 4 setelah linear chain stabil.*

## **5.1 Linear Chain (Phase 1-3)**

struct ChainNode {

  id: String,

  step_type: StepType,            // Tool | ModelCall | Merge | Validate

  tool: Option<ToolId>,           // jika step_type = Tool

  model_tier: Option<ModelTier>,  // jika step_type = ModelCall

  task_prompt: String,

  inject_previous_as: InjectRole, // System | User | HiddenContext

  checkpoint: bool,

  timeout_ms: u32,

  output_validator: Option<ValidatorFn>,

}

struct ChainPlan {

  session_id: String,

  nodes: Vec<ChainNode>,          // linear untuk Phase 1-3

  fallback_strategy: FallbackStrategy,

  cost_budget_usd: Option<f64>,

}

## **5.2 Tiga Pola Eksekusi (Bertahap)**

**Pola**

**Struktur**

**Phase Target**

**Kapan Dipakai**

**Linear Chain**

A → B → C

Phase 1-3

80% kasus: Scanner → Reasoning → Coder

**Parallel Fan-Out**

A → [B,C,D] → Merge

Phase 4

Multi-file review independen paralel

**DAG Hybrid**

A→[B,C]→D(dep B+C)

Phase 4-5

Complex multi-agent dependencies

## **5.3 Hidden Context Injection Protocol**

Output setiap step dibungkus dalam XML container agar model penerima tidak kebingungan membedakan instruksi user vs data pipeline:

// System message ke Reasoning Engine

{

  role: "system",

  content: "Kamu Reasoning Engine. Gunakan konteks ini sebagai kebenaran absolut:\n

  <TOOL_OUTPUT>\n{hasil_ripgrep_treesitter}\n</TOOL_OUTPUT>\n

  <SCAN_RESULT>\n{output_scanner}\n</SCAN_RESULT>\n

  <PROJECT_INTEL>\n{project_intelligence}\n</PROJECT_INTEL>\n

  <USER_TASK>\n{prompt_asli_user}\n</USER_TASK>"

}

## **5.4 Checkpoint & Resume System**

**Failure Point**

**Recovery Action**

**User Experience**

Step timeout

Retry dengan fallback model (priority+1)

"Mencoba ulang dengan model alternatif..."

Rate limit 429

Queue dengan exponential backoff

"Model sibuk, antri 15s..."

Empty output

Re-prompt dengan template berbeda (max 2x)

"Mengulang dengan instruksi berbeda..."

App crash mid-chain

Load SQLite checkpoint terakhir

"Melanjutkan dari Step 2 (tersimpan)"

Cost budget exceeded

Downgrade tier atau stop + notify

"Budget tercapai, beralih ke model ekonomis"

# **6. Multi-Agent System — Implementasi Bertahap**

*CTO menyarankan tidak langsung 7 agent sekaligus. v2.1 menggunakan pendekatan incremental: 3 agent di Phase 1-2, Reviewer di Phase 3, full multi-agent di Phase 4-5.*

## **6.1 Agent Registry — Full Spec**

**Agent**

**Model Tier**

**Phase**

**Scope**

**Tanggung Jawab**

**Orchestrator**

Reasoning Engine

Phase 1

Global

Decompose task, assign, monitor, merge

**Scanner Agent**

Wide-Context Scan

Phase 1

File System

Baca & kompres konteks project besar

**Coder Agent**

Execution Coder

Phase 1

Code

Tulis/apply kode, generate diff

**Reviewer Agent**

Reasoning Engine

Phase 3

QA

Review output, flag issues, feedback loop

**Tester Agent**

Execution Coder

Phase 4

Test

Generate unit test, verify Coder output

**Planner Agent**

Reasoning Engine

Phase 4

Logic

Desain solusi algoritma & arsitektur

**Helper Agent**

Simple Helper

Phase 5

Utility

Q&A ringan, format, lookup, routing kecil

## **6.2 Agent Message Bus**

enum AgentMessage {

  TaskAssign   { to: AgentId, task: TaskSpec, priority: u8 },

  TaskComplete { from: AgentId, output: AgentOutput, cost_usd: f64 },

  TaskFailed   { from: AgentId, error: AgentError, retry_ok: bool },

  ContextShare { from: AgentId, data: ContextPayload, ttl_s: u32 },

  ReviewReq    { from: AgentId, artifact: CodeArtifact },

  ReviewResult { from: AgentId, approved: bool, comments: Vec<String> },

}

# **7. Workspace Knowledge Graph [BARU]**

*Ini adalah diferensiasi terkuat dari Cursor. Bukan soal context window lebih besar — tapi tentang memahami kode sebagai graph, bukan kumpulan teks.*

Alih-alih dump seluruh kodebase ke context window model (mahal, lambat, noise tinggi), CMMO membangun **SymbolGraph** — dependency graph dari semua simbol dalam workspace. Ketika user bertanya tentang fungsi X, system sudah tahu: siapa yang memanggil X, apa yang X panggil, module mana yang import X.

## **7.1 SymbolGraph Schema**

struct SymbolNode {

  id: SymbolId,                   // hash dari fully-qualified name

  name: String,                   // "auth::validate_token"

  kind: SymbolKind,               // Fn | Struct | Trait | Module | Const

  file_path: PathBuf,

  line_range: (u32, u32),

  language: Language,

}

struct SymbolEdge {

  from: SymbolId,

  to: SymbolId,

  relation: EdgeRelation,         // Calls | Imports | Implements | Uses | Tests

}

struct SymbolGraph {

  nodes: HashMap<SymbolId, SymbolNode>,

  edges: Vec<SymbolEdge>,

  last_indexed: SystemTime,

  dirty_files: HashSet<PathBuf>,  // file berubah sejak index terakhir

}

## **7.2 Cara SymbolGraph Mengoptimalkan Routing**

**Tanpa Knowledge Graph (v2.0)**

**Dengan SymbolGraph (v2.1)**

**Penghematan**

Dump semua file ke Scanner (100k+ token)

Graph traversal: ambil hanya subgraph relevan

80-95% pengurangan token

Scanner tidak tahu mana file yang berkaitan

Edge graph langsung tunjukkan dependency chain

0 noise, 100% relevan

Re-scan setiap prompt

Incremental re-index hanya dirty_files

< 100ms update setelah edit

## **7.3 Graph Query Engine**

Sebelum memanggil Scanner, Routing Engine query SymbolGraph untuk mengambil subgraph yang relevan:

// Contoh: user minta fix bug di fungsi "validate_token"

let subgraph = symbol_graph.query(QuerySpec {

  root_symbol: "auth::validate_token",

  depth: 2,                       // 2 level dependency ke atas & bawah

  include_callers: true,          // siapa yang panggil fungsi ini

  include_callees: true,          // apa yang fungsi ini panggil

  include_tests: true,            // test yang cover fungsi ini

});

// Hasilnya: 8 file relevan dari 2000 file project

// Scanner hanya perlu baca 8 file — bukan seluruh folder

## **7.4 Indexing Strategy**

Initial index saat project pertama kali dibuka: tree-sitter parse semua file

Incremental re-index: file watcher (notify-rs) trigger update hanya untuk file yang berubah

Index disimpan di SQLite — persist antar session, tidak perlu re-build dari nol

Index berjalan di background thread (Tokio), tidak blocking UI

# **8. Project Intelligence — Learning Layer [BARU]**

*Setelah beberapa hari pemakaian, CMMO harus 'tahu' bahwa project ini pakai clean architecture, Go 1.25, testify untuk testing, dan gorm untuk ORM — tanpa user harus jelaskan setiap saat.*

Project Intelligence adalah **accumulated knowledge store** yang dibangun dari interaksi nyata dengan project. Setiap kali model membuat keputusan tentang kode, hasilnya diekstrak dan disimpan sebagai pengetahuan project yang persisten.

## **8.1 ProjectIntelligence Schema**

struct ProjectIntelligence {

  // Detected automatically

  languages: Vec<LanguageInfo>,    // Go 1.25, Rust 1.80

  frameworks: Vec<String>,         // ["gorm", "gin", "testify"]

  architecture_pattern: String,    // "clean architecture", "MVC"

  test_framework: String,

  code_style: CodeStyle,           // indent, naming convention

  // Learned from sessions

  common_patterns: Vec<CodePattern>, // pola yang sering muncul

  anti_patterns: Vec<CodePattern>,   // yang selalu direfactor

  preferred_solutions: Vec<SolutionPreference>, // user selalu prefer X over Y

  error_history: Vec<ErrorPattern>,  // error yang sering muncul + solusinya

  // Stats

  total_sessions: u32,

  last_updated: SystemTime,

}

## **8.2 Cara Project Intelligence Digunakan**

**Konteks**

**Manfaat Project Intelligence**

Coder Agent menulis kode baru

Otomatis pakai style, framework, dan naming convention yang benar

Reviewer Agent review kode

Tahu anti-pattern spesifik project ini, bukan generic best practice

Debug error

Cek error_history dulu — mungkin sudah pernah ketemu dan ada solusinya

System message ke semua model

Otomatis inject ProjectIntelligence sebagai context tanpa user harus jelaskan

## **8.3 Learning Loop**

// Setiap kali session selesai, ekstrak pengetahuan

fn extract_and_update_intelligence(session: &CompletedSession) {

  let new_patterns = pattern_extractor.analyze(session);

  let preferences = preference_tracker.detect(session); // user accept/reject apa

  let errors = error_extractor.pull(session);

  PROJECT_INTEL.merge_update(new_patterns, preferences, errors);

  PROJECT_INTEL.save_to_sqlite();

}

# **9. Unified Session Memory (USM) — Arsitektur Lengkap**

## **9.1 Tiga Lapisan Memory**

**Layer**

**Teknologi**

**Lifetime**

**Isi**

**L1: Hot Memory**

Svelte Store

Per-turn (in-memory)

stream_buffer, agent_statuses, UI state

**L2: Session Store**

SQLite via Tauri

Per-session (persists)

Message history, chain checkpoints, cost logs

**L3: Project Memory**

SQLite (long-term)

Cross-session (permanent)

SymbolGraph, scan cache, Project Intelligence

## **9.2 SQLite Schema Lengkap**

-- L2: Session data

CREATE TABLE sessions (

  id TEXT PRIMARY KEY,

  created_at INTEGER NOT NULL,

  status TEXT DEFAULT 'active',

  total_cost_usd REAL DEFAULT 0.0

);

CREATE TABLE chain_steps (

  id TEXT PRIMARY KEY,

  session_id TEXT REFERENCES sessions(id),

  step_index INTEGER NOT NULL,

  step_type TEXT NOT NULL,         -- Tool | ModelCall | Merge

  model_tier TEXT,

  tool_id TEXT,

  input_hash TEXT NOT NULL,        -- SHA256 untuk cache

  output TEXT,

  status TEXT DEFAULT 'pending',

  cost_usd REAL,

  latency_ms INTEGER,

  created_at INTEGER NOT NULL

);

-- L3: Project-level persistent memory

CREATE TABLE scan_cache (

  path_hash TEXT PRIMARY KEY,

  content_hash TEXT NOT NULL,

  summary TEXT NOT NULL,

  created_at INTEGER NOT NULL,

  expires_at INTEGER NOT NULL      -- TTL default 1 jam

);

CREATE TABLE symbol_graph (

  project_id TEXT,

  symbol_data BLOB NOT NULL,       -- serialized SymbolGraph

  indexed_at INTEGER NOT NULL,

  PRIMARY KEY (project_id)

);

CREATE TABLE project_intelligence (

  project_id TEXT PRIMARY KEY,

  intelligence_data TEXT NOT NULL, -- JSON ProjectIntelligence

  updated_at INTEGER NOT NULL

);

# **10. Streaming UX & Progress Transparency**

Setiap step — termasuk tool calls — mengirim status ke UI secara real-time. User tidak pernah melihat layar kosong.

**State**

**UI Indicator**

**Pesan**

Tool: ripgrep running

⚡ Flash ungu cepat

"Mencari pola di 2,847 file..."

Cache Hit

⚡ Flash hijau

"Peta kode dari cache (0.08s)"

Scanner Running

🔍 Spinner biru

"Memindai 247 file... (est. 20s)"

SymbolGraph Query

🕸️ Graph pulse

"Memetakan 8 file relevan dari dependency graph"

Reasoning Thinking

🧠 Stream thought

Tampilkan <think> block live di panel collapsible

Reviewer Checking

👁️ Review badge

"Reviewer Agent memeriksa keamanan kode..."

Fallback Triggered

⚠️ Kuning warning

"Model utama timeout, beralih ke fallback..."

Project Intel Injected

📚 Info badge

"Menggunakan pola project: clean arch + testify"

# **11. Self-Healing Fallback System**

**Trigger**

**Root Cause**

**Fallback Action**

HTTP 429

Rate limit

Pindah provider alternatif tier sama

HTTP 5xx

Provider down

Retry 2x lalu pindah provider

Timeout

Model lambat

Cancel + fallback_priority + 1

Empty output

Prompt issue

Re-prompt template berbeda (max 2x)

Invalid format

Model non-compliant

Auto-validator + re-prompt contoh eksplisit

Cost > budget

Pipeline mahal

Downgrade ke tier lebih murah atau stop + notify

Tool binary missing

ripgrep/LSP belum install

Fallback ke Scanner model, log missing tool

# **12. Revised Implementation Roadmap**

*Urutan ini mengikuti prioritas CTO reviewer: Smart Routing → Cache+Checkpoint → Knowledge Graph → Reviewer Agent → Full Multi-Agent DAG.*

**Phase**

**Focus**

**Deliverable Utama**

**Timeline**

**Phase 1**

Smart Routing Core

tiktoken-rs, 3-layer classifier, Tool Registry basic (ripgrep), 3 model tier routing

Week 1–2

**Phase 2**

Cache + Checkpoint

Linear chain engine, SQLite schema, scan cache SHA256, checkpoint resume, streaming UI, fallback queue

Week 3–4

**Phase 3**

Knowledge Graph

SymbolGraph tree-sitter indexing, incremental update, graph query engine, Scanner pakai subgraph bukan full dump

Week 5–7

**Phase 4**

Reviewer + DAG

Reviewer Agent + feedback loop, Tool Registry lengkap (LSP, git, compiler), DAG parallel execution

Week 8–10

**Phase 5**

Full System

Project Intelligence learning loop, Tester+Planner Agent, cost dashboard, full 7-agent multi-model

Week 11–14

## **12.1 Phase 1 — Smart Routing Core (Detail)**

Tambah tiktoken-rs ke Cargo.toml, implementasi token_count() di ai.rs

Implementasi ToolRegistry struct, daftarkan ripgrep sebagai tool pertama

Implementasi 3-layer classifier: context_estimator → intent_classifier → output_type_discriminator

ModelRegistry: load ModelMetadata dari config TOML, routing function berdasarkan metadata

Update AIChat.svelte: tampilkan model/tool yang dipilih di UI, mode auto aktif

Test 8 skenario routing dasar — verifikasi keputusan benar

## **12.2 Phase 2 — Cache + Checkpoint (Detail)**

SQLite setup via sqlx: create tables sessions + chain_steps + scan_cache

ChainEngine: struct ChainPlan + Vec<ChainNode>, linear executor

SHA256 content hash cache: check cache sebelum panggil Scanner

Streaming proxy Rust → Tauri event emitter → Svelte store

ChainProgressPanel Svelte component: per-step status real-time

FallbackManager: trigger detection per error type, provider queue

Checkpoint resume: on startup, cek ada chain yang interrupted?

## **12.3 Phase 3 — Knowledge Graph (Detail)**

Integrasikan tree-sitter-rust/go/typescript sebagai Rust dependency

Implementasi SymbolGraph: parse → nodes → edges → serialize ke SQLite

File watcher (notify-rs): trigger incremental re-index pada file save

GraphQueryEngine: depth-bounded traversal, subgraph extraction

Routing update: sebelum Scanner, query graph → inject subgraph ke context

Benchmark: compare token usage dengan/tanpa Knowledge Graph

# **13. Skenario Verifikasi — 12 Test Cases**

**ID**

**Prompt**

**Expected Route**

**Success Criteria**

T01

"Apa perbedaan compiler & interpreter?"

→ Simple Helper

< 3s, model ringan

T02

"Tulis fungsi binary search di Go"

→ Execution Coder

Kode valid, no R1 wasted

T03

"Dimana fungsi validate_token didefinisikan?"

→ Tool-Only (LSP)

0 token cost, instant

T04

"Cari semua pemanggil fungsi parseJWT"

→ Tool-Only (ripgrep)

< 200ms, deterministic

T05

"Cari error di log 5MB ini"

→ Wide-Context Scanner

CONTEXT_LARGE trigger

T06

"Kenapa fungsi sort ini output salah?"

→ Reasoning Engine

R1 digunakan, akurat

T07

"Review security folder /auth" (60k token)

→ Graph → Scanner → Reasoning (stop)

No Coder, narasi saja

T08

"Refactor /auth, fix leak, buat test"

→ Full Chain + 3 agents

4 agent, diff valid

T09

T08 diulang, file tidak berubah

→ Cache hit, skip Scanner

Step 1 < 200ms

T10

T06 saat DeepSeek rate limited

→ Auto fallback ke o1-mini

Transparan ke user

T11

Kill app saat mid-chain step 2

→ Resume dari checkpoint

Step 1 tidak diulang

T12

T08 di session ke-5 (PI sudah belajar)

→ Project Intel injected

Kode pakai framework project

# **14. Tech Stack & Dependencies**

**Layer**

**Teknologi**

**Fungsi Spesifik dalam CMMO**

Runtime

Tauri 2.x (Rust)

Backend engine, SQLite, file system, streaming proxy

Frontend

Svelte 5 + SvelteKit

UI reaktif, streaming render, agent status panel

Token Counting

tiktoken-rs

BPE tokenization akurat untuk routing threshold

Code Parsing

tree-sitter (Rust)

SymbolGraph indexing, AST parsing semua bahasa

Code Search

ripgrep (bin)

Tool Registry: regex/keyword search cepat

File Watch

notify-rs

Incremental SymbolGraph re-index on file change

Database

SQLite via sqlx

Sessions, checkpoints, scan cache, symbol graph, project intel

Async Runtime

Tokio

Parallel agent exec, SSE streaming, timeout

Hashing

sha2 (Rust)

Content hash untuk scan cache key

Config

TOML

Model registry, fallback queues, routing thresholds