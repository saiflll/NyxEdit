<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { addToast } from "../../stores.svelte";
  import ExpandableSection from "./ExpandableSection.svelte";

  interface ComponentInfo {
    name: string;
    status: "Healthy" | { Degraded: string } | { Down: string };
    last_check: string;
    uptime_secs: number;
    error_count: number;
    last_error: string | null;
  }

  interface HealLogEntry {
    timestamp: string;
    component: string;
    event_type: string;
    details: string;
    recovery_time_ms: number | null;
  }

  interface HealthDashboard {
    global_status: string;
    health_score: number;
    components: ComponentInfo[];
    restart_counts: Record<string, number>;
    auto_repair_active: boolean;
    uptime_secs: number;
  }

  let dashboard = $state<HealthDashboard | null>(null);
  let healHistory = $state<HealLogEntry[]>([]);
  let isRestarting = $state<Record<string, boolean>>({});
  let autoRepair = $state(true);
  let historyLimit = $state(100);
  let polling = $state(false);
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  async function loadDashboard() {
    try {
      const d = await invoke<HealthDashboard>("get_health_dashboard");
      dashboard = d;
      autoRepair = d.auto_repair_active;
    } catch (e) {
      addToast("Failed to load health dashboard: " + String(e), "error");
    }
  }

  async function loadHealHistory() {
    try {
      healHistory = await invoke<HealLogEntry[]>("get_heal_history", { limit: historyLimit });
    } catch (e) {
      addToast("Failed to load heal history: " + String(e), "error");
    }
  }

  async function handleRestart(name: string) {
    isRestarting[name] = true;
    try {
      const res = await invoke<string>("trigger_module_restart", { name });
      addToast(res, "success");
      await loadDashboard();
      await loadHealHistory();
    } catch (e) {
      addToast(`Failed to restart ${name}: ` + String(e), "error");
    } finally {
      isRestarting[name] = false;
    }
  }

  async function handleToggleAutoRepair() {
    try {
      await invoke("set_auto_repair", { enabled: !autoRepair });
      autoRepair = !autoRepair;
      addToast(`Auto-repair ${autoRepair ? 'enabled' : 'disabled'}`, "success");
    } catch (e) {
      addToast("Failed to toggle auto-repair: " + String(e), "error");
    }
  }

  function startPolling() {
    if (pollTimer) return;
    polling = true;
    pollTimer = setInterval(() => {
      loadDashboard();
    }, 2000);
  }

  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
    polling = false;
  }

  function getStatusString(s: any): string {
    if (s === "Healthy") return "Healthy";
    if (typeof s === "object") {
      if ("Degraded" in s) return `Degraded`;
      if ("Down" in s) return `Down`;
    }
    return "Unknown";
  }

  function getStatusClass(s: any): string {
    if (s === "Healthy") return "status-green";
    if (typeof s === "object" && "Degraded" in s) return "status-yellow";
    return "status-red";
  }

  function getStatusReason(s: any): string {
    if (typeof s === "object" && "Degraded" in s) return s.Degraded;
    if (typeof s === "object" && "Down" in s) return s.Down;
    return "";
  }

  function formatUptime(secs: number): string {
    if (secs < 60) return `${secs}s`;
    const m = Math.floor(secs / 60);
    if (m < 60) return `${m}m ${secs % 60}s`;
    return `${Math.floor(m / 60)}h ${m % 60}m`;
  }

  function formatTime(iso: string): string {
    try { return new Date(iso).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" }); } catch { return iso; }
  }

  function formatDate(iso: string): string {
    try { return new Date(iso).toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }); } catch { return iso; }
  }

  let initialized = $state(false);
  $effect(() => {
    if (!initialized) {
      initialized = true;
      loadDashboard();
      loadHealHistory();
    }
  });
</script>

<div class="health-card">
  <div class="card-header">
    <div class="card-header-left">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 8v4"/><path d="M12 16h.01"/></svg>
      <span>Health & Self-Healing</span>
    </div>
    <div class="card-header-right">
      <button class="btn-icon" class:btn-active={polling} onclick={polling ? stopPolling : startPolling} title="Toggle live polling">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>
      </button>
      <button class="btn-icon" onclick={() => { loadDashboard(); loadHealHistory(); }} title="Refresh">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M1 4v6h6M23 20v-6h-6"/><path d="M20.49 9A9 9 0 0 0 5.64 5.64L1 10m22 4l-4.64 4.36A9 9 0 0 1 3.51 15"/></svg>
      </button>
    </div>
  </div>

  {#if dashboard}
    <div class="health-score-bar">
      <div class="score-ring">
        <svg width="48" height="48" viewBox="0 0 48 48">
          <circle cx="24" cy="24" r="20" fill="none" stroke="var(--bg-primary)" stroke-width="4"/>
          <circle cx="24" cy="24" r="20" fill="none" stroke="var(--accent-green)" stroke-width="4" stroke-dasharray="125.6" stroke-dashoffset={125.6 - (125.6 * dashboard.health_score)} stroke-linecap="round" transform="rotate(-90 24 24)"/>
        </svg>
        <span class="score-value">{(dashboard.health_score * 100).toFixed(0)}%</span>
      </div>
      <div class="score-meta">
        <span class="score-status" class:status-green={dashboard.global_status === 'healthy'} class:status-yellow={dashboard.global_status === 'degraded'} class:status-red={dashboard.global_status === 'down'}>
          {dashboard.global_status.toUpperCase()}
        </span>
        <span class="score-uptime">Uptime {formatUptime(dashboard.uptime_secs)}</span>
        {#if dashboard.auto_repair_active}
          <span class="badge-auto">Auto-Repair ON</span>
        {/if}
      </div>
    </div>

    <ExpandableSection title="Component Status ({dashboard.components.length})">
      <div class="comp-grid">
        {#each dashboard.components as comp}
          <div class="comp-row" class:comp-unhealthy={getStatusString(comp.status) !== 'Healthy'}>
            <div class="comp-left">
              <span class="comp-dot {getStatusClass(comp.status)}"></span>
              <span class="comp-name">{comp.name}</span>
            </div>
            <div class="comp-center">
              <span class="comp-status {getStatusClass(comp.status)}">{getStatusString(comp.status)}</span>
              {#if getStatusReason(comp.status)}
                <span class="comp-reason">{getStatusReason(comp.status)}</span>
              {/if}
            </div>
            <div class="comp-right">
              <span class="comp-errors" class:has-errors={comp.error_count > 0}>{comp.error_count} err</span>
              <button class="btn-heal" onclick={() => handleRestart(comp.name)} disabled={isRestarting[comp.name]}>
                {isRestarting[comp.name] ? '...' : 'Heal'}
              </button>
            </div>
          </div>
        {/each}
      </div>
    </ExpandableSection>

    <ExpandableSection title="Heal History ({healHistory.length})">
      <div class="history-controls">
        <label class="limit-label">
          Limit
          <select bind:value={historyLimit} onchange={loadHealHistory}>
            <option value={50}>50</option>
            <option value={100}>100</option>
            <option value={200}>200</option>
            <option value={500}>500</option>
          </select>
        </label>
      </div>
      {#if healHistory.length === 0}
        <div class="empty-state">No heal events recorded yet.</div>
      {:else}
        <div class="history-list">
          {#each healHistory as entry, i}
            <div class="history-row">
              <span class="h-time">{formatDate(entry.timestamp)}</span>
              <span class="h-comp">{entry.component}</span>
              <span class="h-type">{entry.event_type.replace(/_/g, ' ')}</span>
              <span class="h-detail">{entry.details}</span>
            </div>
          {/each}
        </div>
      {/if}
    </ExpandableSection>

    <ExpandableSection title="Advanced Controls">
      <div class="adv-controls">
        <div class="adv-row">
          <span>Auto-Repair</span>
          <button class="toggle-btn" class:on={autoRepair} onclick={handleToggleAutoRepair}>
            {autoRepair ? 'ON' : 'OFF'}
          </button>
        </div>
        <div class="adv-restart-counts">
          <span class="adv-subtitle">Restart Counts</span>
          {#each Object.entries(dashboard.restart_counts) as [component, count]}
            <div class="rc-row">
              <span>{component}</span>
              <span class="rc-count">{count}</span>
            </div>
          {:else}
            <span class="adv-empty">No restarts recorded.</span>
          {/each}
        </div>
      </div>
    </ExpandableSection>
  {:else}
    <div class="loading-state">Loading health data...</div>
  {/if}
</div>

<style>
  .health-card {
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

  .health-score-bar {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 14px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
  }
  .score-ring {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .score-value {
    position: absolute;
    font-size: var(--fs-10);
    font-weight: 700;
    color: var(--text-primary);
  }
  .score-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .score-status {
    font-size: var(--fs-11);
    font-weight: 700;
    letter-spacing: 0.05em;
  }
  .score-uptime {
    font-size: var(--fs-9);
    color: var(--text-muted);
  }
  .badge-auto {
    font-size: var(--fs-9);
    padding: 1px 6px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--accent-green) 12%, transparent);
    color: var(--accent-green);
    border: 1px solid color-mix(in srgb, var(--accent-green) 25%, transparent);
    width: fit-content;
  }
  .status-green { color: var(--accent-green); }
  .status-yellow { color: var(--accent-yellow); }
  .status-red { color: var(--accent-red); }

  .comp-grid {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .comp-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 6px;
    background: var(--bg-primary);
    border: 1px solid var(--border-subtle);
    transition: border-color 0.12s;
  }
  .comp-unhealthy { border-color: color-mix(in srgb, var(--accent-red) 20%, var(--border-subtle)); }
  .comp-left {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 100px;
  }
  .comp-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .comp-dot.status-green { background: var(--accent-green); }
  .comp-dot.status-yellow { background: var(--accent-yellow); }
  .comp-dot.status-red { background: var(--accent-red); }
  .comp-name {
    font-size: var(--fs-10);
    font-weight: 600;
    color: var(--text-primary);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .comp-center {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .comp-status {
    font-size: var(--fs-9);
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 6px;
    background: color-mix(in srgb, currentColor 10%, transparent);
  }
  .comp-reason {
    font-size: var(--fs-9);
    color: var(--text-muted);
    font-style: italic;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .comp-right {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .comp-errors {
    font-size: var(--fs-9);
    color: var(--text-muted);
  }
  .comp-errors.has-errors { color: var(--accent-red); font-weight: 600; }
  .btn-heal {
    padding: 2px 8px;
    font-size: var(--fs-9);
    font-weight: 600;
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    background: var(--bg-hover);
    color: var(--accent-blue);
    cursor: pointer;
    transition: all 0.12s;
  }
  .btn-heal:hover:not(:disabled) { border-color: var(--accent-blue); }
  .btn-heal:disabled { opacity: 0.5; cursor: not-allowed; }

  .history-controls {
    display: flex;
    justify-content: flex-end;
    margin-bottom: 8px;
  }
  .limit-label {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-9);
    color: var(--text-muted);
  }
  .limit-label select {
    background: var(--bg-primary);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 2px 4px;
    font-size: var(--fs-9);
  }

  .history-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-height: 240px;
    overflow-y: auto;
  }
  .history-row {
    display: grid;
    grid-template-columns: auto auto 1fr;
    gap: 8px;
    align-items: flex-start;
    padding: 4px 6px;
    border-radius: 4px;
    font-size: var(--fs-9);
    background: var(--bg-primary);
    border: 1px solid var(--border-subtle);
  }
  .h-time { color: var(--text-muted); white-space: nowrap; font-family: monospace; }
  .h-comp { font-weight: 600; color: var(--text-primary); text-transform: uppercase; }
  .h-type { color: var(--accent-blue); text-transform: capitalize; }
  .h-detail { color: var(--text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; grid-column: 1 / -1; }

  .adv-controls {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .adv-row {
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
  .adv-restart-counts {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .adv-subtitle {
    font-size: var(--fs-9);
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .rc-row {
    display: flex;
    justify-content: space-between;
    padding: 3px 6px;
    font-size: var(--fs-9);
    color: var(--text-secondary);
    background: var(--bg-primary);
    border-radius: 4px;
  }
  .rc-count { font-weight: 700; color: var(--text-primary); }
  .adv-empty {
    font-size: var(--fs-9);
    color: var(--text-muted);
    font-style: italic;
  }

  .empty-state, .loading-state {
    padding: 16px;
    text-align: center;
    font-size: var(--fs-10);
    color: var(--text-muted);
  }
</style>
