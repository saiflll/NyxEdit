**COGNITIVE MULTI-MODEL**

**ORCHESTRATOR**

CMMO v2.0

Architecture & Implementation Blueprint

Multi-Agent · Multi-Model · Context-Aware Routing

─── Competing with & Surpassing Cursor ───

# **1. Executive Summary**

CMMO adalah sistem orkestrasi AI generasi berikutnya yang dirancang untuk melampaui Cursor dengan mengimplementasikan **routing cerdas berbasis kemampuan model**, multi-agent collaboration, dan **self-healing pipeline**. Tidak seperti Cursor yang bergantung pada satu model dengan mode berganti, CMMO menjalankan model yang paling tepat untuk setiap sub-tugas secara otomatis dan paralel.

**Dimensi**

**Cursor (Kompetitor)**

**CMMO (Target Kita)**

**Model Strategy**

Single model per sesi, manual switch

Auto-route ke model terbaik per sub-task

**Context Handling**

Batas context window satu model

Multi-stage compression via Wide-Scanner

**Agent Mode**

Single agent linear

DAG-based multi-agent parallel/sequential

**Failure Recovery**

Manual retry oleh user

Auto-fallback queue + checkpoint resume

**Memory**

Session-scoped, hilang setelah tutup

Unified Session Memory + SQLite persistent

**Cost Efficiency**

Selalu pakai model premium

Tier-based routing, hemat 60-80% token cost

# **2. Taksonomi Model & Metadata Kemampuan**

Setiap model dalam sistem didefinisikan dengan metadata kemampuan yang presisi. Routing engine menggunakan metadata ini sebagai input keputusan—bukan nama model statis.

## **2.1 Empat Tier Model**

**Tier**

**Context Window**

**Reasoning**

**Model Contoh**

**Primary Use Case**

**Wide-Context Scanner**

100k – 2M+ tokens

Medium

Gemini 1.5-Pro / 2.0-Flash

Scan proyek, compress konteks, map kode

**Reasoning Engine**

32k – 128k tokens

Ultra-High

DeepSeek-R1 / o1 / o3

Algorithm design, bug logic, arch decision

**Execution Coder**

32k – 64k tokens

Medium

Qwen-2.5-Coder / Claude-3.5-Sonnet

Write code, apply diff, create files

**Simple Helper**

8k – 32k tokens

Low

Llama-3-8B / Gemini Flash

Q&A, explain, format, simple lookup

## **2.2 Metadata Schema (Rust / JSON)**

Setiap Agent/Model dikonfigurasi dengan skema berikut di backend:

struct ModelMetadata {

  context_window_limit: u32,     // dalam token (e.g. 1_000_000)

  reasoning_tier: ReasoningTier, // UltraHigh | High | Medium | Low

  specialization: Vec<String>,   // ["code","scan","chat","test"]

  cost_per_1k_tokens: f64,       // USD, untuk cost-aware routing

  avg_latency_ms: u32,           // untuk SLA-aware routing

  max_parallel_calls: u8,        // batas concurrency per model

  supports_streaming: bool,

  fallback_priority: u8,         // urutan jika model utama gagal

}

# **3. Routing Engine — Decision Matrix**

Routing engine beroperasi sebagai 3-layer classifier. Setiap layer mempersempit kandidat model sampai keputusan final dibuat.

## **3.1 Layer 1 — Context Size Estimator**

Menggunakan **tiktoken-rs** (Rust native) untuk menghitung token secara akurat dari file/workspace aktif. Bukan estimasi karakter — ini adalah BPE tokenization yang sama dengan yang dipakai API model.

**Threshold**

**Flag**

**Action**

< 8k tokens

CONTEXT_SMALL

Rute langsung ke Simple Helper atau target model

8k – 30k tokens

CONTEXT_MEDIUM

Eligible untuk Reasoning Engine atau Execution Coder

> 30k tokens

CONTEXT_LARGE

Wajib melalui Wide-Context Scanner dulu (compression stage)

> 500k tokens

CONTEXT_MASSIVE

Streaming chunked processing + incremental summary merge

## **3.2 Layer 2 — Intent Classifier**

Mendeteksi intent dari prompt user menggunakan keyword pattern + semantic scoring. Berjalan di Rust thread terpisah (non-blocking).

**Intent Category**

**Trigger Signal**

**Routing Target**

EXPLAIN_SIMPLE

"apa itu", "jelaskan", "bedanya"

Simple Helper

CODE_WRITE

"tulis fungsi", "buat class", "implementasi"

Execution Coder

CODE_REVIEW

"review", "tinjau", "periksa kode"

Reasoning Engine (+ Scanner jika LARGE)

DEBUG_LOGIC

"bug", "error", "kenapa tidak jalan"

Reasoning Engine

REFACTOR_FULL

"refactor", "optimasi", "restructure folder"

Full Chain: Scanner → Reasoning → Coder

ARCH_DESIGN

"arsitektur", "desain sistem", "rancang"

Reasoning Engine (High Priority)

TEST_GENERATE

"unit test", "buat test", "coverage"

Execution Coder (specialization=test)

SCAN_ONLY

"cari file", "dimana", "list semua"

Wide-Context Scanner only

## **3.3 Layer 3 — Output Type Discriminator**

Dimensi ketiga yang sering diabaikan: apa bentuk output yang dibutuhkan? Ini mencegah over-routing ke model berat untuk task yang outputnya hanya narasi.

**Output Type**

**Karakteristik**

**Implikasi Routing**

NARRATIVE

Penjelasan, summary, analisis teks

Wide-Scanner cukup, Reasoning tidak perlu

CODE_DIFF

Patch/diff spesifik ke file

Full chain atau Execution Coder saja

CODE_FULL

File baru atau rewrite lengkap

Execution Coder (+ Reasoning jika kompleks)

PLAN_ONLY

Pseudocode, arsitektur, langkah-langkah

Reasoning Engine, stop di sini (hemat Coder)

MIXED

Narasi + potongan kode

Reasoning Engine dengan coding capability

## **3.4 Routing Decision Matrix (Final)**

Kombinasi tiga layer menghasilkan keputusan rute final:

**Context Size**

**Intent**

**Output Type**

**Route**

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

PLAN_ONLY

→ Reasoning Engine

MEDIUM

DEBUG_LOGIC

CODE_DIFF

→ Reasoning Engine → Execution Coder

LARGE

SCAN_ONLY

NARRATIVE

→ Wide-Context Scanner only

LARGE

REFACTOR_FULL

CODE_DIFF

→ Scanner → Reasoning → Coder (Full Chain)

LARGE

CODE_REVIEW

NARRATIVE

→ Scanner → Reasoning (stop, no Coder needed)

MASSIVE

ANY

ANY

→ Chunked Scanner (parallel) → merge → route

# **4. Multi-Step Chaining Engine**

Chaining engine mengeksekusi pipeline multi-model sebagai DAG (Directed Acyclic Graph), bukan linear queue. Ini memungkinkan parallelisme dan dependency tracking antar step.

## **4.1 DAG-Based Pipeline Architecture**

Setiap **ChainPlan** adalah DAG di mana node adalah model calls dan edge adalah data dependencies. Engine dapat menjalankan node tanpa dependency secara paralel.

struct ChainNode {

  id: String,                         // unik per step

  model_tier: ModelTier,              // tier yang akan dipanggil

  task_prompt: String,                // instruksi spesifik

  depends_on: Vec<String>,            // node IDs yang harus selesai dulu

  output_type: OutputType,            // NARRATIVE | CODE_DIFF | etc.

  inject_as: InjectRole,              // SYSTEM | USER | HIDDEN_CONTEXT

  checkpoint: bool,                   // simpan ke SQLite setelah selesai

  timeout_ms: u32,                    // trigger fallback jika melewati

}

struct ChainPlan {

  session_id: String,

  nodes: Vec<ChainNode>,

  execution_mode: ExecutionMode,      // Sequential | Parallel | DAG

  fallback_strategy: FallbackStrategy,

  cost_budget_usd: Option<f64>,       // optional hard limit

}

## **4.2 Tiga Pola Chaining**

**Pola**

**Flow**

**Digunakan Untuk**

**Contoh Skenario**

**Linear Chain**

A → B → C

Dep. ketat antar step

Scanner → Reasoning → Coder

**Parallel Fan-Out**

A → [B,C,D] → Merge

Review multi-file independen

3 file review paralel → merge report

**DAG Hybrid**

A → [B,C] → D (dep B+C)

Mixed dep komplex

Scan + Test analysis → Reasoning

## **4.3 Hidden Context Injection (Anti-Confusion Protocol)**

Output dari setiap step diinjeksikan ke step berikutnya menggunakan **structured XML container** di dalam system message. Model tidak akan bingung antara instruksi user dan data dari step sebelumnya.

// System message yang dikirim ke Reasoning Engine (Step 2)

{

  "role": "system",

  "content": "Kamu adalah Reasoning Engine. Gunakan konteks berikut

  sebagai kebenaran absolut:\n\n<SCAN_RESULT>\n{output_step_1}\n</SCAN_RESULT>\n\n

  <USER_TASK>\n{original_user_prompt}\n</USER_TASK>\n\n

  Tugasmu: analisis SCAN_RESULT dan selesaikan USER_TASK."

}

## **4.4 Checkpoint & Resume System**

Setiap node dengan **checkpoint: true** menulis hasilnya ke SQLite sebelum step berikutnya dimulai. Jika pipeline gagal di step N, user bisa resume dari step N tanpa mengulang dari awal.

**Failure Point**

**Recovery Action**

**User Experience**

Step 1 timeout

Retry step 1 dengan fallback model

Status: "Mencoba ulang scanner..."

Step 2 rate limit

Queue dengan exponential backoff

Status: "Reasoning engine sibuk, antri 15s..."

Step 3 empty output

Re-prompt dengan instruksi berbeda

Status: "Mengulang codegen..."

Step 3 wrong format

Validator → auto-fix atau fallback coder

Status: "Memperbaiki format output..."

App crash mid-chain

Load dari SQLite checkpoint terakhir

"Melanjutkan dari step 2 (tersimpan)"

# **5. Multi-Agent System (Melampaui Single-Agent Cursor)**

Ini adalah diferensiasi terbesar CMMO vs Cursor. Alih-alih satu agent yang mengerjakan semua secara linear, CMMO mendeploy **beberapa agent spesialis secara simultan**, dikoordinasikan oleh Orchestrator Agent.

## **5.1 Agent Registry**

**Agent Name**

**Model Tier**

**Scope**

**Tanggung Jawab**

**Orchestrator**

Reasoning Engine

Global

Decompose task, assign agents, monitor, merge output

**Scanner Agent**

Wide-Context Scanner

File System

Baca dan kompres konteks project besar

**Planner Agent**

Reasoning Engine

Logic

Desain solusi algoritma dan arsitektur kode

**Coder Agent**

Execution Coder

Code

Tulis/apply implementasi kode final

**Tester Agent**

Execution Coder

Test

Generate unit test, verify output coder

**Reviewer Agent**

Reasoning Engine

QA

Review output coder, flag issues, request revision

**Helper Agent**

Simple Helper

Utility

Formatting, lookup, simple Q&A, task routing ringan

## **5.2 Agent Communication Protocol**

Antar-agent berkomunikasi melalui **typed message bus** yang dikelola Orchestrator. Setiap pesan memiliki type, sender, receiver, dan payload yang terstruktur.

enum AgentMessage {

  TaskAssign { to: AgentId, task: TaskSpec, priority: u8 },

  TaskComplete { from: AgentId, output: AgentOutput, cost_usd: f64 },

  TaskFailed { from: AgentId, error: AgentError, retry_eligible: bool },

  ContextShare { from: AgentId, data: ContextPayload, ttl_seconds: u32 },

  ReviewRequest { from: AgentId, target: AgentId, artifact: CodeArtifact },

  ReviewResult { from: AgentId, approved: bool, comments: Vec<String> },

}

## **5.3 Skenario Multi-Agent: Full Refactoring**

Contoh eksekusi nyata untuk prompt: "Refactor seluruh modul auth, tambah unit test, dan review security."

Orchestrator decompose task → 4 sub-task: [scan, plan, code+test, review]

Scanner Agent scan folder /auth (paralel dengan Orchestrator planning)

Planner Agent menerima scan output → desain refactoring plan

Coder Agent + Tester Agent berjalan PARALEL: Coder tulis kode, Tester tulis test template

Reviewer Agent menerima output Coder + Tester → review security & logic

Jika Reviewer reject → Coder Agent mendapat feedback loop, revisi tanpa user interaction

Orchestrator merge semua output → tampilkan ke user sebagai unified diff

# **6. Unified Session Memory (USM)**

USM adalah layer memori yang menjembatani Svelte reactive state (in-memory) dan SQLite persistent storage. Ini memastikan tidak ada data yang hilang saat chaining multi-agent.

## **6.1 Arsitektur Dua Lapisan**

**Layer**

**Teknologi**

**Isi & Fungsi**

**L1: Hot Memory**

Svelte Store (in-memory)

session_id, current_step, stream_buffer, agent_statuses, UI state

**L2: Cold Storage**

SQLite via Tauri invoke()

Full message history, agent outputs, checkpoints, cost logs, cache

## **6.2 SQLite Schema**

-- Session table

CREATE TABLE sessions (

  id TEXT PRIMARY KEY,                  -- UUID

  created_at INTEGER NOT NULL,

  status TEXT DEFAULT 'active',          -- active | completed | failed

  total_cost_usd REAL DEFAULT 0.0

);

-- Chain step checkpoints

CREATE TABLE chain_steps (

  id TEXT PRIMARY KEY,

  session_id TEXT REFERENCES sessions(id),

  step_index INTEGER NOT NULL,

  model_tier TEXT NOT NULL,

  input_hash TEXT NOT NULL,              -- SHA256 untuk cache detection

  output TEXT,                           -- JSON output payload

  status TEXT DEFAULT 'pending',         -- pending | running | done | failed

  cost_usd REAL,

  latency_ms INTEGER,

  created_at INTEGER NOT NULL

);

-- Content hash cache (skip re-scan jika file tidak berubah)

CREATE TABLE scan_cache (

  path_hash TEXT PRIMARY KEY,            -- SHA256 dari file tree paths

  content_hash TEXT NOT NULL,            -- SHA256 dari file contents

  summary TEXT NOT NULL,                 -- compressed summary dari Scanner

  created_at INTEGER NOT NULL,

  expires_at INTEGER NOT NULL            -- TTL: default 1 jam

);

## **6.3 Content Hash Cache — Memotong 90% Latency di Sesi Berulang**

Sebelum memanggil Wide-Context Scanner, sistem menghitung **SHA256 dari file tree**. Jika hash cocok dengan cache yang belum expired, summary lama digunakan langsung. Untuk proyek yang sama, ini memotong step 1 dari 30-60 detik menjadi <100ms.

# **7. Streaming UX & Progress Transparency**

UX adalah pembeda kritis. User tidak boleh merasakan "sistem hang" saat chaining berjalan 30-90 detik. Solusinya: setiap byte yang dihasilkan langsung ditampilkan.

## **7.1 Streaming Pipeline Architecture**

Setiap model call menggunakan **Server-Sent Events (SSE)** streaming. Rust backend meneruskan token dari API model langsung ke Svelte frontend via Tauri event emitter — tanpa buffering penuh.

## **7.2 Progress States yang Ditampilkan ke User**

**Chain State**

**UI Indicator**

**Pesan yang Ditampilkan**

Cache Hit

⚡ Flash hijau

"Peta kode ditemukan di cache (0.1s)"

Scanner Running

🔍 Spinner biru

"Memindai 247 file... (estimasi 20s)"

Scanner Done

✓ Checkmark hijau

"[✓] Peta kode: 12 file relevan ditemukan"

Reasoning Thinking

🧠 Streaming thought

Tampilkan <think> block secara real-time

Coder Streaming

💻 Live code output

Token kode muncul karakter per karakter

Fallback Triggered

⚠️ Kuning warning

"Model utama timeout, beralih ke fallback..."

Agent Reviewing

👁️ Review indicator

"Reviewer Agent memeriksa keamanan kode..."

## **7.3 Thought Block Display (DeepSeek-R1 / o1)**

Model reasoning sering menghasilkan **<think> blocks** sebelum jawaban final. CMMO menampilkan ini secara live di panel terpisah (collapsible), memberi user transparensi proses berpikir AI — fitur yang tidak dimiliki Cursor.

# **8. Fallback & Self-Healing System**

Sistem tidak boleh mati karena satu model gagal. Fallback queue memastikan continuity dengan degradasi yang transparan ke user.

## **8.1 Trigger Conditions & Response**

**Trigger**

**Root Cause**

**Fallback Action**

HTTP 429 Rate Limit

API quota habis

Pindah ke provider alternatif tier sama

HTTP 5xx Server Error

Provider down

Retry 2x lalu pindah provider

Timeout > threshold

Model lambat/hang

Cancel + coba model tier sama (fallback_priority+1)

Empty/null output

Prompt issue atau model failure

Re-prompt dengan template berbeda (max 2x)

Invalid output format

Model tidak ikut format

Auto-validator + re-prompt dengan contoh explicit

Cost budget exceeded

Pipeline terlalu mahal

Downgrade ke tier lebih murah atau stop + notify

## **8.2 Fallback Priority Queue**

Setiap tier memiliki queue fallback yang dikonfigurasi:

// Contoh konfigurasi fallback

fallback_queues: {

  reasoning_engine: [

    { provider: "deepseek", model: "deepseek-r1", priority: 1 },

    { provider: "openai", model: "o1-mini", priority: 2 },

    { provider: "anthropic", model: "claude-3-5-sonnet", priority: 3 },

  ],

  wide_context_scanner: [

    { provider: "google", model: "gemini-1.5-pro", priority: 1 },

    { provider: "google", model: "gemini-2.0-flash", priority: 2 },

  ],

  execution_coder: [

    { provider: "qwen", model: "qwen2.5-coder-32b", priority: 1 },

    { provider: "anthropic", model: "claude-3-5-sonnet", priority: 2 },

  ],

}

# **9. Implementation Roadmap**

Implementasi dibagi dalam 4 fase iteratif. Setiap fase menghasilkan versi yang bisa digunakan (shipable increment).

**Fase**

**Target**

**Deliverable**

**Timeline**

**Phase 1**

Smart Routing Core

tiktoken-rs integration, 3-layer classifier, basic routing ke 4 tier

Week 1-2

**Phase 2**

Linear Chaining

Chain engine (linear), checkpoint SQLite, streaming UI, fallback queue

Week 3-4

**Phase 3**

Multi-Agent

Agent registry, message bus, parallel DAG execution, Reviewer Agent

Week 5-7

**Phase 4**

Full Polish

Cache system, cost tracking, thought block UI, performance tuning

Week 8-10

## **9.1 Phase 1 — Smart Routing Core (Detail)**

Tambah dependency tiktoken-rs ke Cargo.toml

Implementasi ai_classify_request di ai.rs: context estimator + intent classifier

Buat model_registry.rs: struct ModelMetadata + loading dari config file

Ubah auto mode di AIChat.svelte: baca routing decision dari backend, tampilkan model yang dipilih

Integration test: 10 prompt test cases, verifikasi routing decision benar

## **9.2 Phase 2 — Linear Chaining (Detail)**

Buat chain_engine.rs: struct ChainPlan, ChainNode, executor function

Implementasi SQLite schema via Tauri sqlx: sessions + chain_steps tables

Svelte: ChainProgressPanel component (real-time step status display)

Implementasi streaming proxy di Rust: teruskan token SSE ke Tauri event emitter

Fallback queue: FallbackManager struct dengan trigger detection

End-to-end test Skenario 3 (full refactoring chain)

## **9.3 Phase 3 — Multi-Agent (Detail)**

Buat agent_registry.rs: AgentSpec, AgentId, lifecycle management

Implementasi message_bus.rs: typed AgentMessage enum, async channel per agent

Orchestrator Agent: task decomposition logic, DAG builder, output merger

Reviewer Agent dengan feedback loop ke Coder Agent

Parallel execution: Tokio async tasks untuk independent agent calls

Integration test: Full refactoring skenario dengan 4 agent simultan

# **10. Skenario Verifikasi & Acceptance Criteria**

**Skenario**

**Prompt**

**Expected Route**

**Success Criteria**

S1: Simple Q&A

"Apa perbedaan compiler dan interpreter?"

→ Simple Helper

Respons < 3s, model ringan

S2: Code Write

"Tulis fungsi binary search di Go"

→ Execution Coder

Kode valid, no Reasoning wasted

S3: Large Log

"Cari error di log 5MB ini"

→ Wide-Context Scanner

Context size trigger benar

S4: Debug Logic

"Kenapa fungsi sort ini output salah?" (kode 200 baris)

→ Reasoning Engine

R1 used, solusi akurat

S5: Full Refactor

"Refactor folder /auth, fix memory leak, buat test"

→ Full Multi-Agent Chain

4 agent terlibat, diff valid

S6: Review Only

"Review security di seluruh kode server ini" (60k token)

→ Scanner → Reasoning (no Coder)

Chain stop setelah narasi

S7: Cache Hit

S5 diulang di proyek sama, file tidak berubah

→ Cache hit, skip Scanner

Step 1 < 200ms dari SQLite

S8: Fallback

S4 saat DeepSeek rate limited

→ Auto-switch ke o1-mini

Fallback transparan ke user

S9: App Crash

Kill app saat mid-chain step 2

→ Resume dari checkpoint

Step 1 tidak diulang

S10: Cost Guard

Task dengan budget 0.05 USD, chain cost > budget

→ Downgrade/stop pipeline

Budget tidak terlampaui

# **11. Tech Stack & Dependencies**

**Layer**

**Teknologi**

**Fungsi Spesifik dalam CMMO**

Runtime

Tauri 2.x (Rust)

Backend engine, SQLite access, file system, streaming proxy

Frontend

Svelte 5 + SvelteKit

UI, reactive state, streaming render, agent status panel

Token Counting

tiktoken-rs (Rust)

Akurat BPE token estimation untuk routing threshold

Database

SQLite via sqlx

Sessions, checkpoints, scan cache, cost logs

Async Runtime

Tokio (Rust)

Parallel agent execution, SSE streaming, timeout management

State Management

Svelte Store

Hot memory: stream buffer, UI state, per-turn agent statuses

Hashing

sha2 (Rust crate)

Content hash cache key generation

Config

TOML / JSON

Model registry, fallback queues, routing thresholds