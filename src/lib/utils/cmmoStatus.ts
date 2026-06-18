// === CMMO STATUS REAL ===
// Legend: ✅ = Done, ❌ = Not Done, 🔶 = Partial

export type CmmoStage = {
  id: number;
  name: string;
  coreLogic: boolean;
  frontendUi: boolean;
  activeInMainFlow: "yes" | "partial" | "no";
  notes?: string;
};

export const CMMO_STAGES: CmmoStage[] = [
  {
    id: 1,
    name: "Smart Routing",
    coreLogic: true,
    frontendUi: true,
    activeInMainFlow: "yes",
    notes: "ModelRegistry, FallbackManager, models.toml - dipanggil tiap auto mode"
  },
  {
    id: 2,
    name: "Tool-First Engine",
    coreLogic: true,
    frontendUi: false,
    activeInMainFlow: "yes",
    notes: "ripgrep.rs, TreeSitter, scan cache - tool-only skip model call"
  },
  {
    id: 3,
    name: "Chaining",
    coreLogic: true,
    frontendUi: true,
    activeInMainFlow: "yes",
    notes: "chain_engine.rs, run_chain() - kalau routing bikin chain plan"
  },
  {
    id: 4,
    name: "SQLite",
    coreLogic: true,
    frontendUi: true,
    activeInMainFlow: "yes",
    notes: "sessions.rs SQLite rewrite, 4 commands - tiap chat pake database"
  },
  {
    id: 5,
    name: "Knowledge Graph",
    coreLogic: true,
    frontendUi: false,
    activeInMainFlow: "partial",
    notes: "symbol_graph.rs, parsers.rs - lazy-loaded saat query pertama"
  },
  {
    id: 6,
    name: "Project Intel",
    coreLogic: true,
    frontendUi: false,
    activeInMainFlow: "partial",
    notes: "project_intel.rs - perlu panggil project_detect dulu (AUTO-FIXED)"
  },
  {
    id: 7,
    name: "Review",
    coreLogic: true,
    frontendUi: false,
    activeInMainFlow: "partial",
    notes: "review.rs, 3 rules - perlu panggil review_text manual (AUTO-FIXED)"
  },
  {
    id: 8,
    name: "Multi-Model",
    coreLogic: true,
    frontendUi: false,
    activeInMainFlow: "yes",
    notes: "provider_stats.rs, CircuitBreaker - aktif di fallback loop"
  },
  {
    id: 9,
    name: "Multi-Agent",
    coreLogic: true,
    frontendUi: false,
    activeInMainFlow: "partial",
    notes: "agent_orch.rs, 3 sub-agents - perlu panggil orch_delegate"
  },
  {
    id: 10,
    name: "DAG",
    coreLogic: true,
    frontendUi: false,
    activeInMainFlow: "partial",
    notes: "DagPlan, run_dag() parallel tokio - routing prioritaskan DAG untuk RefactorFull/CodeReview"
  },
  {
    id: 11,
    name: "Self-Healing",
    coreLogic: true,
    frontendUi: false,
    activeInMainFlow: "partial",
    notes: "self_heal.rs - report_degraded() dipanggil di error path, frontend perlu get_status"
  },
  {
    id: 12,
    name: "Performance & DX",
    coreLogic: true,
    frontendUi: false,
    activeInMainFlow: "yes",
    notes: "Cache warming, crash marker, startup health check, mimalloc allocator"
  },
  {
    id: 13,
    name: "RAG Conversation Memory",
    coreLogic: true,
    frontendUi: false,
    activeInMainFlow: "yes",
    notes: "context.rs - compression aktif di ai_chat_stream (OnceLock)"
  },
  {
    id: 14,
    name: "Smart Cost Routing",
    coreLogic: true,
    frontendUi: false,
    activeInMainFlow: "partial",
    notes: "cost_routing.rs - routing preferensi, belum auto-dipanggil"
  }
];

export function getCmmoProgress(): { total: number; completed: number; partial: number; percentage: number } {
  const total = CMMO_STAGES.length;
  const completed = CMMO_STAGES.filter(s => s.coreLogic && s.frontendUi && s.activeInMainFlow === "yes").length;
  const partial = CMMO_STAGES.filter(s => s.activeInMainFlow === "partial").length;
  const percentage = Math.round((completed / total) * 100);
  
  return { total, completed, partial, percentage };
}

export function isAutoEnabled(stageId: number): boolean {
  // Stage yang sudah auto-enabled
  const autoEnabled = [1, 2, 3, 4, 8, 12, 13];
  return autoEnabled.includes(stageId);
}

export function needsManualTrigger(stageId: number): boolean {
  // Stage yang masih perlu manual trigger
  const manual = [6, 7, 9, 11, 14];
  return manual.includes(stageId);
}
