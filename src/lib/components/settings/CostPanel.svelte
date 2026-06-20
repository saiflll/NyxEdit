<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { addToast } from "../../stores.svelte";
  import ExpandableSection from "./ExpandableSection.svelte";

  interface SessionCostEvent {
    session_id: string;
    model: string;
    provider: string;
    cost: number;
    input_tokens: number;
    output_tokens: number;
  }

  interface RoutingConfig {
    enabled: boolean;
    preferred_provider: string;
    max_fallback_attempts: number;
    auto_fallback: boolean;
    cost_optimization_level: string;
  }

  interface CostBudget {
    max_per_session: number;
    max_per_day: number;
    current_session_cost: number;
    current_day_cost: number;
  }

  interface DailyCostPoint {
    date: string;
    cost: number;
    calls: number;
  }

  interface CostAnalytics {
    total_cost: number;
    total_calls: number;
    total_input_tokens: number;
    total_output_tokens: number;
    daily_cost: DailyCostPoint[];
    by_model: { model: string; calls: number; cost: number; percent: number }[];
    budget: CostBudget;
    routing_config: RoutingConfig;
  }

  let analytics = $state<CostAnalytics | null>(null);
  let liveEvents = $state<SessionCostEvent[]>([]);
  let routingConfig = $state<RoutingConfig | null>(null);

  let maxPerSession = $state(0.10);
  let maxPerDay = $state(1.00);
  let isSavingBudget = $state(false);

  let editPreferred = $state("");
  let editMaxFallback = $state(3);
  let editAutoFallback = $state(true);
  let editEnabled = $state(true);
  let editCostLevel = $state("balanced");
  let isSavingRouting = $state(false);

  let polling = $state(false);
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let initialized = $state(false);

  async function loadAnalytics() {
    try {
      const a = await invoke<CostAnalytics>("get_cost_analytics");
      analytics = a;
      maxPerSession = a.budget.max_per_session;
      maxPerDay = a.budget.max_per_day;
      if (a.by_model) {
        a.by_model.sort((x, y) => y.cost - x.cost);
      }
    } catch (e) {
      addToast("Failed to load cost analytics: " + String(e), "error");
    }
  }

  async function loadLiveEvents() {
    try {
      liveEvents = await invoke<SessionCostEvent[]>("get_live_cost_stream");
    } catch (e) {
      addToast("Failed to load live cost stream: " + String(e), "error");
    }
  }

  async function loadRoutingConfig() {
    try {
      const r = await invoke<RoutingConfig>("get_routing_config");
      routingConfig = r;
      editPreferred = r.preferred_provider;
      editMaxFallback = r.max_fallback_attempts;
      editAutoFallback = r.auto_fallback;
      editEnabled = r.enabled;
      editCostLevel = r.cost_optimization_level;
    } catch (e) {
      addToast("Failed to load routing config: " + String(e), "error");
    }
  }

  async function handleSaveBudget() {
    isSavingBudget = true;
    try {
      await invoke("cost_set_budget", { maxPerSession, maxPerDay });
      addToast("Budget limits updated", "success");
      await loadAnalytics();
    } catch (e) {
      addToast("Failed to save budget: " + String(e), "error");
    } finally {
      isSavingBudget = false;
    }
  }

  async function handleSaveRouting() {
    isSavingRouting = true;
    try {
      const config: RoutingConfig = {
        enabled: editEnabled,
        preferred_provider: editPreferred,
        max_fallback_attempts: editMaxFallback,
        auto_fallback: editAutoFallback,
        cost_optimization_level: editCostLevel,
      };
      await invoke("update_routing_config", { config });
      addToast("Routing configuration saved", "success");
      await loadRoutingConfig();
    } catch (e) {
      addToast("Failed to update routing config: " + String(e), "error");
    } finally {
      isSavingRouting = false;
    }
  }

  function startPolling() {
    if (pollTimer) return;
    polling = true;
    pollTimer = setInterval(() => {
      loadLiveEvents();
      loadAnalytics();
    }, 2000);
  }

  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
    polling = false;
  }

  function formatCurrency(v: number): string {
    if (v < 0.0001) return `$${v.toExponential(2)}`;
    return `$${v.toFixed(5)}`;
  }

  $effect(() => {
    if (!initialized) {
      initialized = true;
      loadAnalytics();
      loadLiveEvents();
      loadRoutingConfig();
    }
  });
</script>

<div class="cost-card">
  <div class="card-header">
    <div class="card-header-left">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M16 16v-3a4 4 0 0 0-8 0v3"/><path d="M12 8v4"/><path d="M12 16h.01"/></svg>
      <span>Cost Intelligence & Routing</span>
    </div>
    <div class="card-header-right">
      <button class="btn-icon" class:btn-active={polling} onclick={polling ? stopPolling : startPolling} title="Toggle live polling">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>
      </button>
      <button class="btn-icon" onclick={() => { loadAnalytics(); loadLiveEvents(); }} title="Refresh">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M1 4v6h6M23 20v-6h-6"/><path d="M20.49 9A9 9 0 0 0 5.64 5.64L1 10m22 4l-4.64 4.36A9 9 0 0 1 3.51 15"/></svg>
      </button>
    </div>
  </div>

  {#if analytics}
    <div class="summary-bar">
      <div class="summary-item">
        <span class="summary-label">Total Cost</span>
        <span class="summary-value">${analytics.total_cost.toFixed(5)}</span>
      </div>
      <div class="summary-item">
        <span class="summary-label">Calls</span>
        <span class="summary-value">{analytics.total_calls}</span>
      </div>
      <div class="summary-item">
        <span class="summary-label">Tokens</span>
        <span class="summary-value">{(analytics.total_input_tokens + analytics.total_output_tokens).toLocaleString()}</span>
      </div>
      <div class="summary-item">
        <span class="summary-label">Budget Used</span>
        <span class="summary-value" style="color: {analytics.budget.current_day_cost > analytics.budget.max_per_day * 0.8 ? 'var(--accent-red)' : 'inherit'}">
          {(analytics.budget.current_day_cost / Math.max(analytics.budget.max_per_day, 0.01) * 100).toFixed(1)}%
        </span>
      </div>
    </div>

    <ExpandableSection title="Live Cost Stream ({liveEvents.length})" defaultOpen={false}>
      {#if liveEvents.length === 0}
        <div class="empty-state">No recent cost events.</div>
      {:else}
        <div class="live-stream">
          {#each liveEvents as ev}
            <div class="stream-row">
              <span class="stream-model">{ev.model}</span>
              <span class="stream-tokens">{(ev.input_tokens + ev.output_tokens).toLocaleString()} tok</span>
              <span class="stream-cost">{formatCurrency(ev.cost)}</span>
            </div>
          {/each}
        </div>
      {/if}
    </ExpandableSection>

    <ExpandableSection title="Routing Configuration">
      <div class="routing-form">
        <label class="routing-field">
          <span>Enabled</span>
          <button class="toggle-btn" class:on={editEnabled} onclick={() => editEnabled = !editEnabled}>
            {editEnabled ? 'ON' : 'OFF'}
          </button>
        </label>
        <label class="routing-field">
          <span>Preferred Provider</span>
          <input bind:value={editPreferred} placeholder="auto, openai, cerebras, ..." />
        </label>
        <label class="routing-field">
          <span>Max Fallback Attempts</span>
          <input type="number" bind:value={editMaxFallback} min="0" max="10" />
        </label>
        <label class="routing-field">
          <span>Auto Fallback</span>
          <button class="toggle-btn" class:on={editAutoFallback} onclick={() => editAutoFallback = !editAutoFallback}>
            {editAutoFallback ? 'ON' : 'OFF'}
          </button>
        </label>
        <label class="routing-field">
          <span>Cost Optimization Level</span>
          <select bind:value={editCostLevel}>
            <option value="aggressive">Aggressive (cheapest)</option>
            <option value="balanced">Balanced</option>
            <option value="quality">Quality first</option>
          </select>
        </label>
        <button class="btn-save" onclick={handleSaveRouting} disabled={isSavingRouting}>
          {isSavingRouting ? 'Saving...' : 'Save Routing Config'}
        </button>
      </div>
    </ExpandableSection>

    <ExpandableSection title="Budget Limits">
      <div class="budget-form">
        <div class="budget-meters">
          <div class="meter-row">
            <span class="meter-label">Session</span>
            <div class="meter-bar-bg">
              <div class="meter-bar-fg" style="width: {Math.min(analytics.budget.current_session_cost / Math.max(analytics.budget.max_per_session, 0.001), 1) * 100}%;"></div>
            </div>
            <span class="meter-value">{formatCurrency(analytics.budget.current_session_cost)} / {formatCurrency(analytics.budget.max_per_session)}</span>
          </div>
          <div class="meter-row">
            <span class="meter-label">Daily</span>
            <div class="meter-bar-bg">
              <div class="meter-bar-fg" style="width: {Math.min(analytics.budget.current_day_cost / Math.max(analytics.budget.max_per_day, 0.001), 1) * 100}%; background: {analytics.budget.current_day_cost > analytics.budget.max_per_day * 0.8 ? 'var(--accent-red)' : 'var(--accent-blue)'};"></div>
            </div>
            <span class="meter-value">{formatCurrency(analytics.budget.current_day_cost)} / {formatCurrency(analytics.budget.max_per_day)}</span>
          </div>
        </div>
        <div class="budget-inputs">
          <label class="budget-field">
            <span>Max Per Session ($)</span>
            <input type="number" bind:value={maxPerSession} step="0.01" min="0.01" max="10" />
          </label>
          <label class="budget-field">
            <span>Max Per Day ($)</span>
            <input type="number" bind:value={maxPerDay} step="0.05" min="0.05" max="50" />
          </label>
        </div>
        <button class="btn-save" onclick={handleSaveBudget} disabled={isSavingBudget}>
          {isSavingBudget ? 'Saving...' : 'Update Budget'}
        </button>
      </div>
    </ExpandableSection>

    <ExpandableSection title="Spending Breakdown by Model" defaultOpen={false}>
      {#if analytics.by_model && analytics.by_model.length > 0}
        <div class="breakdown-list">
          {#each analytics.by_model as entry}
            <div class="breakdown-item">
              <div class="bd-top">
                <span class="bd-model">{entry.model}</span>
                <span class="bd-cost">{formatCurrency(entry.cost)}</span>
              </div>
              <div class="bd-bar-bg">
                <div class="bd-bar-fg" style="width: {Math.min(entry.percent, 100)}%;"></div>
              </div>
              <div class="bd-meta">
                <span>{entry.calls} calls</span>
                <span>{entry.percent.toFixed(1)}% of spend</span>
              </div>
            </div>
          {/each}
        </div>
      {:else}
        <div class="empty-state">No spending data yet.</div>
      {/if}
    </ExpandableSection>

    <ExpandableSection title="Daily Cost Analytics" defaultOpen={false}>
      {#if analytics.daily_cost && analytics.daily_cost.length > 0}
        <div class="daily-list">
          {#each analytics.daily_cost as day}
            <div class="daily-row">
              <span class="daily-date">{day.date}</span>
              <div class="daily-bar-bg">
                <div class="daily-bar-fg" style="width: {Math.min(day.cost / Math.max(analytics.total_cost, 0.001), 1) * 100}%;"></div>
              </div>
              <span class="daily-cost">{formatCurrency(day.cost)}</span>
              <span class="daily-calls">{day.calls} calls</span>
            </div>
          {/each}
        </div>
      {:else}
        <div class="empty-state">No daily cost data available.</div>
      {/if}
    </ExpandableSection>
  {:else}
    <div class="loading-state">Loading cost data...</div>
  {/if}
</div>

<style>
  .cost-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 10px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
  }
  .card-header-left {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-11);
    font-weight: 700;
    color: var(--text-primary);
  }
  .card-header-right {
    display: flex;
    gap: 4px;
  }
  .btn-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    background: transparent;
    border: 1px solid var(--border-subtle);
    border-radius: 5px;
    color: var(--text-muted);
    cursor: pointer;
    transition: all 0.12s;
  }
  .btn-icon:hover { color: var(--accent-blue); border-color: var(--accent-blue); }
  .btn-active { color: var(--accent-blue); background: color-mix(in srgb, var(--accent-blue) 10%, transparent); }

  .summary-bar {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
    padding: 10px 14px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
  }
  .summary-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }
  .summary-label {
    font-size: var(--fs-9);
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .summary-value {
    font-size: var(--fs-12);
    font-weight: 700;
    color: var(--text-primary);
    font-variant-numeric: tabular-nums;
  }

  .live-stream {
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-height: 200px;
    overflow-y: auto;
  }
  .stream-row {
    display: grid;
    grid-template-columns: 1fr auto auto;
    gap: 8px;
    align-items: center;
    padding: 4px 6px;
    border-radius: 4px;
    background: var(--bg-primary);
    border: 1px solid var(--border-subtle);
    font-size: var(--fs-9);
  }
  .stream-model { color: var(--text-primary); font-family: monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .stream-tokens { color: var(--text-muted); white-space: nowrap; }
  .stream-cost { color: var(--accent-green); font-weight: 600; white-space: nowrap; }

  .routing-form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .routing-field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 8px;
    background: var(--bg-primary);
    border-radius: 6px;
    border: 1px solid var(--border-subtle);
    font-size: var(--fs-10);
    font-weight: 600;
    color: var(--text-primary);
  }
  .routing-field input, .routing-field select {
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 3px 6px;
    font-size: var(--fs-9);
    max-width: 180px;
    text-align: right;
  }
  .toggle-btn {
    padding: 2px 10px;
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    font-size: var(--fs-9);
    font-weight: 700;
    cursor: pointer;
    background: var(--bg-primary);
    color: var(--text-muted);
    transition: all 0.12s;
  }
  .toggle-btn.on {
    background: color-mix(in srgb, var(--accent-green) 12%, transparent);
    color: var(--accent-green);
    border-color: color-mix(in srgb, var(--accent-green) 25%, var(--border-subtle));
  }
  .btn-save {
    align-self: flex-end;
    padding: 5px 12px;
    background: var(--accent-blue);
    color: var(--bg-primary);
    border: none;
    border-radius: 5px;
    font-size: var(--fs-9);
    font-weight: 600;
    cursor: pointer;
    transition: opacity 0.12s;
  }
  .btn-save:hover:not(:disabled) { opacity: 0.85; }
  .btn-save:disabled { opacity: 0.5; cursor: not-allowed; }

  .budget-form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .budget-meters {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .meter-row {
    display: grid;
    grid-template-columns: 70px 1fr auto;
    gap: 8px;
    align-items: center;
  }
  .meter-label {
    font-size: var(--fs-9);
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
  }
  .meter-bar-bg {
    height: 6px;
    background: var(--bg-primary);
    border-radius: 3px;
    overflow: hidden;
    border: 1px solid var(--border-subtle);
  }
  .meter-bar-fg {
    height: 100%;
    background: var(--accent-blue);
    border-radius: 3px;
    transition: width 0.3s ease;
  }
  .meter-value {
    font-size: var(--fs-9);
    color: var(--text-secondary);
    font-family: monospace;
    white-space: nowrap;
  }
  .budget-inputs {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .budget-field {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .budget-field span {
    font-size: var(--fs-9);
    font-weight: 600;
    color: var(--text-muted);
  }
  .budget-field input {
    background: var(--bg-primary);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 4px 6px;
    font-size: var(--fs-10);
  }

  .breakdown-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 300px;
    overflow-y: auto;
  }
  .breakdown-item {
    background: var(--bg-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    padding: 6px 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .bd-top { display: flex; justify-content: space-between; align-items: center; }
  .bd-model { font-size: var(--fs-9); font-weight: 700; color: var(--text-primary); font-family: monospace; }
  .bd-cost { font-size: var(--fs-9); font-weight: 700; color: var(--accent-green); }
  .bd-bar-bg { width: 100%; height: 4px; background: var(--bg-surface); border-radius: 2px; overflow: hidden; }
  .bd-bar-fg { height: 100%; background: linear-gradient(90deg, var(--accent-blue), var(--accent-purple, var(--accent-blue))); border-radius: 2px; transition: width 0.3s ease; }
  .bd-meta { display: flex; justify-content: space-between; font-size: var(--fs-8); color: var(--text-muted); }

  .daily-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .daily-row {
    display: grid;
    grid-template-columns: auto 1fr auto auto;
    gap: 8px;
    align-items: center;
    padding: 4px 6px;
    background: var(--bg-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    font-size: var(--fs-9);
  }
  .daily-date { font-weight: 600; color: var(--text-primary); white-space: nowrap; }
  .daily-bar-bg { height: 4px; background: var(--bg-surface); border-radius: 2px; overflow: hidden; }
  .daily-bar-fg { height: 100%; background: var(--accent-green); border-radius: 2px; transition: width 0.3s ease; }
  .daily-cost { color: var(--accent-green); font-weight: 600; font-family: monospace; white-space: nowrap; }
  .daily-calls { color: var(--text-muted); white-space: nowrap; }

  .empty-state, .loading-state {
    padding: 16px;
    text-align: center;
    font-size: var(--fs-10);
    color: var(--text-muted);
  }
</style>
