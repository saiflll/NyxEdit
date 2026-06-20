<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { addToast } from "../stores.svelte";

  type RecoveryEvent = {
    component: string;
    timestamp: string;
    event_type: "crash_detected" | "heal_started" | "restart_success" | "restart_failed";
    details: string | null;
    recovery_time_ms: number | null;
  };

  type ComponentInfo = {
    name: string;
    status: "Healthy" | { Degraded: string } | { Down: string };
    last_check: string;
    uptime_secs: number;
    error_count: number;
    last_error: string | null;
  };

  type BreakdownEntry = {
    model: string;
    calls: number;
    cost: number;
    percent: number;
  };

  type CostBreakdown = {
    by_model: BreakdownEntry[];
    total_cost: number;
    total_calls: number;
    budget_limit: number;
    budget_used_percent: number;
    recommendations_ignored: number;
    budget_alerts_triggered: number;
  };

  type RecommendationLog = {
    session_id: string;
    model: string;
    provider: string;
    input_tokens: number;
    output_tokens: number;
    cost: number;
  };

  type AutoHealMap = Record<string, boolean>;

  // Tab state
  let activeTab = $state<"health" | "history" | "cost" | "breakdown" | "memory">("health");

  // Health data
  let components = $state<ComponentInfo[]>([]);
  let isRestarting = $state<Record<string, boolean>>({});
  let autoHealMap = $state<AutoHealMap>({});

  // Recovery History
  let recoveryHistory = $state<RecoveryEvent[]>([]);

  // Cost data
  let costBreakdown = $state<CostBreakdown | null>(null);
  let recommendationLog = $state<RecommendationLog[]>([]);

  // Budget form
  let maxPerSession = $state(0.10);
  let maxPerDay = $state(1.00);
  let isSavingBudget = $state(false);

  // Memory stats (RAG/context)
  let memoryStats = $state<any>(null);

  // ─── Loaders ───────────────────────────────────────────────────────

  async function loadHealthStatus() {
    try {
      components = await invoke<ComponentInfo[]>("heal_get_status");
    } catch (e) {
      addToast("Failed to load health: " + String(e), "error");
    }
  }

  async function loadAutoHealMap() {
    try {
      autoHealMap = await invoke<AutoHealMap>("heal_get_auto_heal_map");
    } catch { /* ignore */ }
  }

  async function loadRecoveryHistory() {
    try {
      recoveryHistory = await invoke<RecoveryEvent[]>("heal_get_recovery_history");
    } catch (e) {
      addToast("Failed to load recovery history: " + String(e), "error");
    }
  }

  async function loadCostBreakdown() {
    try {
      const raw = await invoke<any>("cost_get_summary");
      maxPerSession = raw.budget.max_per_session;
      maxPerDay = raw.budget.max_per_day;
      costBreakdown = await invoke<CostBreakdown>("cost_get_breakdown");
      recommendationLog = await invoke<RecommendationLog[]>("cost_get_recommendation_log");
    } catch (e) {
      addToast("Failed to load cost data: " + String(e), "error");
    }
  }

  async function loadMemoryStats() {
    try {
      memoryStats = await invoke<any>("session_get_memory_stats");
    } catch { /* ignore */ }
  }

  async function handleRestart(name: string) {
    isRestarting[name] = true;
    try {
      const res = await invoke<string>("heal_restart_component", { name });
      addToast(res, "success");
      await loadHealthStatus();
      await loadRecoveryHistory();
    } catch (e) {
      addToast(`Failed to heal ${name}: ` + String(e), "error");
    } finally {
      isRestarting[name] = false;
    }
  }

  async function handleSimulateCrash(name: string) {
    try {
      const res = await invoke<string>("heal_simulate_crash", { component: name });
      addToast(res, "warning");
      await loadHealthStatus();
      await loadRecoveryHistory();
    } catch (e) {
      addToast("Simulate crash failed: " + String(e), "error");
    }
  }

  async function handleToggleAutoHeal(name: string) {
    const current = autoHealMap[name] ?? true;
    try {
      autoHealMap = await invoke<AutoHealMap>("heal_toggle_auto_heal", { component: name, enabled: !current });
    } catch (e) {
      addToast("Failed to toggle auto-heal: " + String(e), "error");
    }
  }

  async function handleSaveBudget() {
    isSavingBudget = true;
    try {
      await invoke("cost_set_budget", { maxPerSession, maxPerDay });
      addToast("Budget limits updated", "success");
      await loadCostBreakdown();
    } catch (e) {
      addToast("Failed to save budget: " + String(e), "error");
    } finally {
      isSavingBudget = false;
    }
  }

  // ─── Helpers ───────────────────────────────────────────────────────

  function getHealthString(status: any): string {
    if (status === "Healthy") return "Healthy";
    if (typeof status === "object" && "Degraded" in status) return `Degraded`;
    if (typeof status === "object" && "Down" in status) return `Down`;
    return "Unknown";
  }

  function getHealthClass(status: any): string {
    if (status === "Healthy") return "pill-green";
    if (typeof status === "object" && "Degraded" in status) return "pill-yellow";
    return "pill-red";
  }

  function getHealthReason(status: any): string {
    if (typeof status === "object" && "Degraded" in status) return status.Degraded;
    if (typeof status === "object" && "Down" in status) return status.Down;
    return "";
  }

  function formatUptime(secs: number): string {
    if (secs < 60) return `${secs}s`;
    const mins = Math.floor(secs / 60);
    if (mins < 60) return `${mins}m`;
    return `${Math.floor(mins / 60)}h ${mins % 60}m`;
  }

  function formatTime(iso: string): string {
    try { return new Date(iso).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" }); } catch { return iso; }
  }

  function eventIcon(type: string): string {
    switch (type) {
      case "crash_detected": return "💥";
      case "heal_started": return "🔧";
      case "restart_success": return "✅";
      case "restart_failed": return "❌";
      default: return "•";
    }
  }

  function eventColor(type: string): string {
    switch (type) {
      case "crash_detected": return "var(--accent-red)";
      case "heal_started": return "var(--accent-yellow)";
      case "restart_success": return "var(--accent-green)";
      case "restart_failed": return "var(--accent-red)";
      default: return "var(--text-muted)";
    }
  }

  $effect(() => {
    loadHealthStatus();
    loadAutoHealMap();
    loadRecoveryHistory();
    loadCostBreakdown();
    loadMemoryStats();
  });
</script>

<div class="shcp-wrap">
  <!-- Header -->
  <div class="shcp-header">
    <span class="shcp-title">System Health & Cost</span>
    <div class="shcp-tabs">
      {#each [["health", "Health"], ["history", "Recovery Log"], ["cost", "Budget"], ["breakdown", "Breakdown"], ["memory", "RAG Memory"]] as [id, label]}
        <button
          class="shcp-tab"
          class:active={activeTab === id}
          onclick={() => { activeTab = id as any; }}
        >{label}</button>
      {/each}
    </div>
  </div>

  <!-- ── HEALTH TAB ── -->
  {#if activeTab === "health"}
    <div class="shcp-toolbar">
      <span class="shcp-hint">Live daemon health monitor</span>
      <button class="btn-ghost" onclick={loadHealthStatus}>↺ Refresh</button>
    </div>
    <div class="health-grid">
      {#each components as comp}
        <div class="comp-card" class:card-red={getHealthString(comp.status) !== "Healthy"}>
          <div class="comp-card-top">
            <span class="comp-name">{comp.name}</span>
            <span class="pill {getHealthClass(comp.status)}">{getHealthString(comp.status)}</span>
          </div>

          {#if getHealthReason(comp.status)}
            <div class="comp-reason">{getHealthReason(comp.status)}</div>
          {/if}

          <div class="comp-stats">
            <div class="stat-kv">
              <span class="stat-k">Uptime</span>
              <span class="stat-v">{formatUptime(comp.uptime_secs)}</span>
            </div>
            <div class="stat-kv">
              <span class="stat-k">Errors</span>
              <span class="stat-v" style="color: {comp.error_count > 0 ? 'var(--accent-red)' : 'inherit'}">{comp.error_count}</span>
            </div>
            <div class="stat-kv">
              <span class="stat-k">Auto-Heal</span>
              <button
                class="toggle-mini"
                class:on={autoHealMap[comp.name] !== false}
                onclick={() => handleToggleAutoHeal(comp.name)}
                title="Toggle auto-heal for {comp.name}"
              >{autoHealMap[comp.name] !== false ? "ON" : "OFF"}</button>
            </div>
          </div>

          {#if comp.last_error}
            <div class="comp-error">{comp.last_error}</div>
          {/if}

          <div class="comp-actions">
            <span class="comp-ts">Checked {formatTime(comp.last_check)}</span>
            <div style="display: flex; gap: 6px;">
              <button class="btn-ghost btn-sm" onclick={() => handleSimulateCrash(comp.name)} title="Simulate crash for testing">💥 Test</button>
              <button class="btn-primary btn-sm" onclick={() => handleRestart(comp.name)} disabled={isRestarting[comp.name]}>
                {isRestarting[comp.name] ? "Healing…" : "Heal"}
              </button>
            </div>
          </div>
        </div>
      {/each}
    </div>

  <!-- ── RECOVERY HISTORY TAB ── -->
  {:else if activeTab === "history"}
    <div class="shcp-toolbar">
      <span class="shcp-hint">Last 50 recovery events (newest first)</span>
      <button class="btn-ghost" onclick={loadRecoveryHistory}>↺ Refresh</button>
    </div>
    {#if recoveryHistory.length === 0}
      <div class="empty-state">
        <span style="font-size: 2rem;">🛡️</span>
        <span>No recovery events yet — system is running clean.</span>
      </div>
    {:else}
      <div class="timeline">
        {#each recoveryHistory as ev}
          <div class="timeline-item">
            <div class="tl-dot" style="background: {eventColor(ev.event_type)}">
              {eventIcon(ev.event_type)}
            </div>
            <div class="tl-body">
              <div class="tl-top">
                <span class="tl-comp">{ev.component}</span>
                <span class="tl-type" style="color: {eventColor(ev.event_type)}">{ev.event_type.replace(/_/g, ' ')}</span>
                <span class="tl-time">{formatTime(ev.timestamp)}</span>
              </div>
              {#if ev.details}
                <div class="tl-detail">{ev.details}</div>
              {/if}
              {#if ev.recovery_time_ms !== null}
                <div class="tl-badge">⚡ {ev.recovery_time_ms}ms</div>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}

  <!-- ── BUDGET TAB ── -->
  {:else if activeTab === "cost"}
    {#if costBreakdown}
      <div class="budget-section">
        <!-- Budget progress meters -->
        <div class="meter-grid">
          <div class="meter-card">
            <div class="meter-top">
              <span class="meter-label">Daily Spend</span>
              <span class="meter-val" style="color: var(--accent-blue)">
                {(costBreakdown.budget_used_percent).toFixed(1)}% of ${maxPerDay.toFixed(2)}
              </span>
            </div>
            <div class="meter-bar-bg">
              <div class="meter-bar-fg" style="width: {Math.min(costBreakdown.budget_used_percent, 100)}%; background: {costBreakdown.budget_used_percent > 80 ? 'var(--accent-red)' : 'var(--accent-blue)'};"></div>
            </div>
          </div>

          <div class="meter-card">
            <div class="meter-top">
              <span class="meter-label">Total Calls</span>
              <span class="meter-val" style="color: var(--accent-green)">{costBreakdown.total_calls}</span>
            </div>
            <div class="meter-top" style="margin-top: 4px;">
              <span class="meter-label">Total Cost</span>
              <span class="meter-val">${costBreakdown.total_cost.toFixed(5)}</span>
            </div>
          </div>
        </div>

        <!-- Budget config -->
        <div class="budget-form">
          <span class="form-section-title">Spending Limits (USD)</span>
          <div class="form-row">
            <label class="form-field">
              <span class="field-label">Per Session ($)</span>
              <input bind:value={maxPerSession} type="number" step="0.01" min="0.01" max="10" class="field-input" />
            </label>
            <label class="form-field">
              <span class="field-label">Per Day ($)</span>
              <input bind:value={maxPerDay} type="number" step="0.05" min="0.05" max="50" class="field-input" />
            </label>
          </div>
          <div style="display: flex; justify-content: flex-end; margin-top: 8px;">
            <button class="btn-primary" onclick={handleSaveBudget} disabled={isSavingBudget}>
              {isSavingBudget ? "Saving…" : "Update Limits"}
            </button>
          </div>
        </div>
      </div>
    {:else}
      <div class="empty-state"><span>Loading budget data…</span></div>
    {/if}

  <!-- ── COST BREAKDOWN TAB ── -->
  {:else if activeTab === "breakdown"}
    <div class="shcp-toolbar">
      <span class="shcp-hint">Spending breakdown by model</span>
      <button class="btn-ghost" onclick={loadCostBreakdown}>↺ Refresh</button>
    </div>
    {#if costBreakdown && costBreakdown.by_model && costBreakdown.by_model.length > 0}
      <div class="breakdown-list">
        {#each [...costBreakdown.by_model].sort((a, b) => b.cost - a.cost) as entry}
          <div class="breakdown-item">
            <div class="bd-top">
              <span class="bd-model">{entry.model}</span>
              <span class="bd-cost">${entry.cost.toFixed(5)}</span>
            </div>
            <div class="bd-bar-bg">
              <div class="bd-bar-fg" style="width: {entry.percent}%;"></div>
            </div>
            <div class="bd-meta">
              <span>{entry.calls} calls</span>
              <span>{entry.percent.toFixed(1)}% of spend</span>
            </div>
          </div>
        {/each}
      </div>

      {#if recommendationLog.length > 0}
        <div class="rec-log">
          <span class="form-section-title">Recent API Calls</span>
          <div class="rec-table">
            {#each recommendationLog as log}
              <div class="rec-row">
                <span class="rec-model">{log.model}</span>
                <span class="rec-tokens">{(log.input_tokens + log.output_tokens).toLocaleString()} tok</span>
                <span class="rec-cost" style="color: var(--accent-green)">${log.cost.toFixed(5)}</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    {:else}
      <div class="empty-state">
        <span style="font-size: 2rem;">📊</span>
        <span>No spending data yet. Send an AI message to start tracking.</span>
      </div>
    {/if}

  <!-- ── RAG MEMORY TAB ── -->
  {:else if activeTab === "memory"}
    <div class="shcp-toolbar">
      <span class="shcp-hint">RAG context compression & cross-session memory</span>
      <button class="btn-ghost" onclick={loadMemoryStats}>↺ Refresh</button>
    </div>
    {#if memoryStats}
      <div class="memory-grid">
        <div class="mem-card">
          <span class="mem-k">Raw Context Tokens</span>
          <span class="mem-v">{memoryStats.raw_tokens.toLocaleString()}</span>
        </div>
        <div class="mem-card">
          <span class="mem-k">Compressed Tokens</span>
          <span class="mem-v" style="color: var(--accent-green)">{memoryStats.compressed_tokens.toLocaleString()}</span>
        </div>
        <div class="mem-card">
          <span class="mem-k">Compression Ratio</span>
          <span class="mem-v">{(memoryStats.compression_ratio * 100).toFixed(1)}%</span>
        </div>
        <div class="mem-card">
          <span class="mem-k">Cross-Session Retrievals</span>
          <span class="mem-v">{memoryStats.cross_session_retrievals}</span>
        </div>
      </div>
      {#if memoryStats.note}
        <div style="margin-top: 12px; padding: 10px 12px; background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: 6px; font-size: var(--fs-10); color: var(--text-muted); font-style: italic;">
          ℹ️ {memoryStats.note}
        </div>
      {/if}
    {:else}
      <div class="empty-state"><span>Loading memory stats…</span></div>
    {/if}
  {/if}
</div>

<style>
  .shcp-wrap {
    display: flex;
    flex-direction: column;
    gap: 0;
    flex: 1;
    overflow: hidden;
    background: var(--bg-primary);
  }

  /* Header */
  .shcp-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 12px 16px 10px;
    border-bottom: 1px solid var(--border-subtle);
    flex-wrap: wrap;
  }
  .shcp-title {
    font-size: var(--fs-12);
    font-weight: 700;
    color: var(--text-primary);
    white-space: nowrap;
  }

  /* Tabs */
  .shcp-tabs {
    display: flex;
    gap: 2px;
    background: var(--bg-surface);
    padding: 2px;
    border-radius: 7px;
    border: 1px solid var(--border-subtle);
    flex-wrap: wrap;
  }
  .shcp-tab {
    background: transparent;
    border: none;
    padding: 4px 10px;
    font-size: var(--fs-9-5);
    font-weight: 600;
    color: var(--text-muted);
    cursor: pointer;
    border-radius: 5px;
    transition: all 0.12s ease;
    white-space: nowrap;
  }
  .shcp-tab.active {
    background: var(--bg-primary);
    color: var(--accent-blue);
    box-shadow: 0 1px 3px rgba(0,0,0,0.15);
  }
  .shcp-tab:hover:not(.active) { color: var(--text-primary); }

  /* Toolbar */
  .shcp-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .shcp-hint {
    font-size: var(--fs-10);
    color: var(--text-muted);
  }

  /* Buttons */
  .btn-ghost {
    background: transparent;
    border: 1px solid var(--border-subtle);
    color: var(--text-secondary);
    padding: 4px 10px;
    font-size: var(--fs-10);
    font-weight: 600;
    border-radius: 5px;
    cursor: pointer;
    transition: all 0.12s;
  }
  .btn-ghost:hover { background: var(--bg-surface); color: var(--text-primary); }
  .btn-primary {
    background: var(--accent-blue);
    border: none;
    color: var(--bg-primary);
    padding: 4px 12px;
    font-size: var(--fs-10);
    font-weight: 700;
    border-radius: 5px;
    cursor: pointer;
    transition: opacity 0.12s;
  }
  .btn-primary:hover:not(:disabled) { opacity: 0.85; }
  .btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
  .btn-sm { padding: 3px 8px; font-size: var(--fs-9-5); }

  /* Pills */
  .pill {
    font-size: var(--fs-9);
    font-weight: 700;
    padding: 2px 7px;
    border-radius: 10px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .pill-green { background: color-mix(in srgb, var(--accent-green) 14%, transparent); color: var(--accent-green); }
  .pill-yellow { background: color-mix(in srgb, var(--accent-yellow) 14%, transparent); color: var(--accent-yellow); }
  .pill-red { background: color-mix(in srgb, var(--accent-red) 14%, transparent); color: var(--accent-red); }

  /* Health Grid */
  .health-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 10px;
    padding: 14px 16px;
    overflow-y: auto;
    flex: 1;
  }
  .comp-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    transition: border-color 0.15s;
  }
  .comp-card:hover { border-color: var(--border-primary); }
  .comp-card.card-red { border-color: color-mix(in srgb, var(--accent-red) 25%, var(--border-subtle)); }
  .comp-card-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .comp-name {
    font-size: var(--fs-11);
    font-weight: 700;
    color: var(--text-primary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .comp-reason {
    font-size: var(--fs-9-5);
    color: var(--accent-yellow);
    font-style: italic;
  }
  .comp-stats {
    display: flex;
    gap: 12px;
    padding: 6px 0;
    border-top: 1px solid var(--border-subtle);
    border-bottom: 1px solid var(--border-subtle);
  }
  .stat-kv { display: flex; flex-direction: column; gap: 2px; align-items: center; }
  .stat-k { font-size: var(--fs-9); color: var(--text-muted); font-weight: 600; text-transform: uppercase; }
  .stat-v { font-size: var(--fs-11); font-weight: 700; color: var(--text-primary); }
  .comp-error {
    font-size: var(--fs-9-5);
    color: var(--accent-red);
    background: color-mix(in srgb, var(--accent-red) 5%, var(--bg-primary));
    border: 1px solid color-mix(in srgb, var(--accent-red) 12%, var(--border-subtle));
    border-radius: 4px;
    padding: 5px 8px;
    font-family: monospace;
    white-space: pre-wrap;
    max-height: 60px;
    overflow-y: auto;
  }
  .comp-actions {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    margin-top: 2px;
  }
  .comp-ts { font-size: var(--fs-9); color: var(--text-muted); }

  /* Toggle mini */
  .toggle-mini {
    font-size: var(--fs-9);
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 4px;
    cursor: pointer;
    border: 1px solid var(--border-subtle);
    background: var(--bg-primary);
    color: var(--text-muted);
    transition: all 0.12s;
  }
  .toggle-mini.on {
    background: color-mix(in srgb, var(--accent-green) 12%, var(--bg-primary));
    color: var(--accent-green);
    border-color: color-mix(in srgb, var(--accent-green) 25%, var(--border-subtle));
  }

  /* Timeline */
  .timeline {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 16px;
    overflow-y: auto;
    flex: 1;
  }
  .timeline-item {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 8px 10px;
    border-radius: 6px;
    transition: background 0.12s;
  }
  .timeline-item:hover { background: var(--bg-surface); }
  .tl-dot {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 12px;
    flex-shrink: 0;
    opacity: 0.9;
  }
  .tl-body { display: flex; flex-direction: column; gap: 3px; flex: 1; min-width: 0; }
  .tl-top {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .tl-comp { font-size: var(--fs-10); font-weight: 700; color: var(--text-primary); text-transform: uppercase; }
  .tl-type { font-size: var(--fs-10); font-weight: 600; text-transform: capitalize; }
  .tl-time { font-size: var(--fs-9); color: var(--text-muted); margin-left: auto; }
  .tl-detail { font-size: var(--fs-9-5); color: var(--text-secondary); font-family: monospace; }
  .tl-badge {
    display: inline-block;
    font-size: var(--fs-9);
    background: color-mix(in srgb, var(--accent-green) 10%, var(--bg-primary));
    color: var(--accent-green);
    border: 1px solid color-mix(in srgb, var(--accent-green) 20%, var(--border-subtle));
    padding: 1px 6px;
    border-radius: 8px;
    width: fit-content;
  }

  /* Budget meters */
  .budget-section {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 14px 16px;
    overflow-y: auto;
    flex: 1;
  }
  .meter-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 10px;
  }
  .meter-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: 14px;
  }
  .meter-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .meter-label { font-size: var(--fs-10); font-weight: 700; color: var(--text-secondary); text-transform: uppercase; }
  .meter-val { font-size: var(--fs-11); font-weight: 700; color: var(--text-primary); }
  .meter-bar-bg {
    width: 100%;
    height: 8px;
    background: var(--bg-primary);
    border-radius: 4px;
    overflow: hidden;
    margin-top: 10px;
    border: 1px solid var(--border-subtle);
  }
  .meter-bar-fg {
    height: 100%;
    border-radius: 4px;
    transition: width 0.3s ease;
    background: var(--accent-blue);
  }

  /* Budget form */
  .budget-form {
    background: var(--bg-surface);
    border: 1px solid var(--border-primary);
    border-radius: 8px;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .form-section-title {
    font-size: var(--fs-10);
    font-weight: 700;
    color: var(--text-primary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    border-bottom: 1px solid var(--border-subtle);
    padding-bottom: 6px;
    display: block;
  }
  .form-row {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 12px;
  }
  .form-field { display: flex; flex-direction: column; gap: 4px; }
  .field-label { font-size: var(--fs-10); font-weight: 600; color: var(--text-secondary); }
  .field-input {
    background: var(--bg-primary);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 6px 10px;
    font-size: var(--fs-11);
    outline: none;
    transition: border-color 0.12s;
  }
  .field-input:focus { border-color: var(--accent-blue); }

  /* Breakdown */
  .breakdown-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px 0;
    overflow-y: auto;
  }
  .breakdown-item {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 7px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .bd-top { display: flex; justify-content: space-between; align-items: center; }
  .bd-model { font-size: var(--fs-10); font-weight: 700; color: var(--text-primary); font-family: monospace; }
  .bd-cost { font-size: var(--fs-10); font-weight: 700; color: var(--accent-green); }
  .bd-bar-bg { width: 100%; height: 6px; background: var(--bg-primary); border-radius: 3px; overflow: hidden; }
  .bd-bar-fg { height: 100%; background: linear-gradient(90deg, var(--accent-blue), var(--accent-purple, var(--accent-blue))); border-radius: 3px; transition: width 0.3s ease; }
  .bd-meta { display: flex; justify-content: space-between; font-size: var(--fs-9); color: var(--text-muted); }

  /* Rec log */
  .rec-log {
    padding: 10px 16px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .rec-table { display: flex; flex-direction: column; gap: 3px; }
  .rec-row {
    display: grid;
    grid-template-columns: 1fr auto auto;
    gap: 10px;
    align-items: center;
    padding: 5px 8px;
    border-radius: 4px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
  }
  .rec-model { font-size: var(--fs-9-5); color: var(--text-primary); font-family: monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rec-tokens { font-size: var(--fs-9); color: var(--text-muted); white-space: nowrap; }
  .rec-cost { font-size: var(--fs-9-5); font-weight: 600; white-space: nowrap; }

  /* Memory grid */
  .memory-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 10px;
    padding: 14px 16px;
  }
  .mem-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 7px;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .mem-k { font-size: var(--fs-9); color: var(--text-muted); font-weight: 600; text-transform: uppercase; }
  .mem-v { font-size: var(--fs-16, 1.2rem); font-weight: 700; color: var(--text-primary); }

  /* Empty state */
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 40px 20px;
    color: var(--text-muted);
    font-size: var(--fs-11);
    text-align: center;
    flex: 1;
  }
</style>
