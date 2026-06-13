<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { addToast } from "../stores.svelte";

  type HealthStatus = "Healthy" | { Degraded: string } | { Down: string };

  type ComponentInfo = {
    name: string;
    status: HealthStatus;
    last_check: string;
    uptime_secs: number;
    error_count: number;
    last_error: string | null;
  };

  type CostBudget = {
    max_per_session: number;
    max_per_day: number;
    current_session: number;
    current_day: number;
  };

  type CostSummary = {
    budget: CostBudget;
    total_calls: number;
    total_cost: number;
    avg_costs: Record<string, number>;
  };

  // State
  let activeTab = $state<"health" | "cost">("health");
  let components = $state<ComponentInfo[]>([]);
  let costSummary = $state<CostSummary | null>(null);
  let isRestarting = $state<Record<string, boolean>>({});
  
  // Budget Form State
  let maxPerSession = $state(0.10);
  let maxPerDay = $state(1.00);
  let isSavingBudget = $state(false);

  async function loadHealthStatus() {
    try {
      components = await invoke<ComponentInfo[]>("heal_get_status");
    } catch (e) {
      console.error("Failed to load component health:", e);
      addToast("Failed to load health status: " + String(e), "error");
    }
  }

  async function handleRestart(name: string) {
    isRestarting[name] = true;
    try {
      const res = await invoke<string>("heal_restart_component", { name });
      addToast(res, "success");
      await loadHealthStatus();
    } catch (e) {
      console.error("Failed to restart component:", e);
      addToast(`Failed to heal ${name}: ` + String(e), "error");
    } finally {
      isRestarting[name] = false;
    }
  }

  async function loadCostSummary() {
    try {
      const res = await invoke<CostSummary>("cost_get_summary");
      costSummary = res;
      maxPerSession = res.budget.max_per_session;
      maxPerDay = res.budget.max_per_day;
    } catch (e) {
      console.error("Failed to load cost summary:", e);
      addToast("Failed to load budget data: " + String(e), "error");
    }
  }

  async function handleSaveBudget() {
    isSavingBudget = true;
    try {
      await invoke("cost_set_budget", {
        maxPerSession,
        maxPerDay
      });
      addToast("Budget limits updated successfully", "success");
      await loadCostSummary();
    } catch (e) {
      console.error("Failed to update budget:", e);
      addToast("Failed to save budget: " + String(e), "error");
    } finally {
      isSavingBudget = false;
    }
  }

  function getHealthString(status: HealthStatus): string {
    if (status === "Healthy") return "Healthy";
    if (typeof status === "object" && "Degraded" in status) return `Degraded: ${status.Degraded}`;
    if (typeof status === "object" && "Down" in status) return `Down: ${status.Down}`;
    return "Unknown";
  }

  function getHealthClass(status: HealthStatus): string {
    if (status === "Healthy") return "badge-installed";
    if (typeof status === "object" && "Degraded" in status) return "badge-checking";
    return "badge-missing";
  }

  function formatUptime(secs: number): string {
    if (secs < 60) return `${secs}s`;
    const mins = Math.floor(secs / 60);
    if (mins < 60) return `${mins}m ${secs % 60}s`;
    const hrs = Math.floor(mins / 60);
    return `${hrs}h ${mins % 60}m`;
  }

  $effect(() => {
    loadHealthStatus();
    loadCostSummary();
  });
</script>

<div class="health-cost-panel" style="padding: 16px; display: flex; flex-direction: column; gap: 16px; flex: 1; overflow-y: auto;">
  <div class="settings-header" style="padding: 0 0 10px 0; border-bottom: 1px solid var(--border-subtle); display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 10px;">
    <span class="settings-title">Health & Spending Dashboard</span>
    <div class="mode-toggles" style="display: flex; background: var(--bg-primary); padding: 2px; border-radius: 6px; border: 1px solid var(--border-subtle);">
      <button class="toggle-btn" class:active={activeTab === "health"} onclick={() => activeTab = "health"} style="border: none; background: transparent; padding: 4px 12px; font-size: var(--fs-10); font-weight: 600; cursor: pointer; border-radius: 4px; color: activeTab === 'health' ? 'var(--accent-blue)' : 'var(--text-muted)'; transition: all 0.1s ease;">
        System Health
      </button>
      <button class="toggle-btn" class:active={activeTab === "cost"} onclick={() => { activeTab = "cost"; loadCostSummary(); }} style="border: none; background: transparent; padding: 4px 12px; font-size: var(--fs-10); font-weight: 600; cursor: pointer; border-radius: 4px; color: activeTab === 'cost' ? 'var(--accent-blue)' : 'var(--text-muted)'; transition: all 0.1s ease;">
        Cost & Budget Limits
      </button>
    </div>
  </div>

  {#if activeTab === "health"}
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px;">
      <span style="font-size: var(--fs-11); color: var(--text-secondary); font-weight: 500;">
        Monitor the real-time health of backend daemon micro-services.
      </span>
      <button class="settings-btn settings-btn-add" onclick={loadHealthStatus} style="background: var(--bg-surface); color: var(--text-primary); border: 1px solid var(--border-subtle); padding: 5px 10px; font-weight: 600; border-radius: 6px; font-size: var(--fs-10); cursor: pointer;">
        Refresh Health
      </button>
    </div>

    <div class="health-grid" style="display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 12px;">
      {#each components as comp}
        <div class="setup-card" style="display: flex; flex-direction: column; justify-content: space-between; gap: 8px; padding: 12px; background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: 8px; transition: border-color 0.15s ease;">
          <div>
            <div style="display: flex; justify-content: space-between; align-items: center; gap: 8px;">
              <span style="font-size: var(--fs-12); font-weight: 700; color: var(--text-primary); text-transform: uppercase;">{comp.name}</span>
              <span class="badge {getHealthClass(comp.status)}" style="font-size: var(--fs-9); padding: 2px 6px;">
                {getHealthString(comp.status).split(":")[0]}
              </span>
            </div>

            <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 10px; border-bottom: 1px solid var(--border-subtle); padding-bottom: 8px;">
              <div>
                <span style="font-size: var(--fs-9); color: var(--text-muted); font-weight: 600; text-transform: uppercase; display: block;">Uptime</span>
                <span style="font-size: var(--fs-11); font-weight: 600; color: var(--text-primary);">{formatUptime(comp.uptime_secs)}</span>
              </div>
              <div>
                <span style="font-size: var(--fs-9); color: var(--text-muted); font-weight: 600; text-transform: uppercase; display: block;">Errors Encountered</span>
                <span style="font-size: var(--fs-11); font-weight: 600; color: comp.error_count > 0 ? 'var(--accent-red)' : 'var(--text-primary)';">{comp.error_count}</span>
              </div>
            </div>

            {#if comp.last_error}
              <div style="margin-top: 8px;">
                <span style="font-size: var(--fs-9); color: var(--accent-red); font-weight: 600; text-transform: uppercase; display: block;">Last Error</span>
                <div style="font-size: var(--fs-9-5); color: var(--accent-red); background: color-mix(in srgb, var(--accent-red) 5%, var(--bg-primary)); border: 1px solid color-mix(in srgb, var(--accent-red) 12%, var(--bg-surface)); border-radius: 4px; padding: 6px; font-family: monospace; white-space: pre-wrap; margin-top: 4px; max-height: 80px; overflow-y: auto;">
                  {comp.last_error}
                </div>
              </div>
            {/if}
          </div>

          <div style="display: flex; justify-content: space-between; align-items: center; border-top: 1px solid var(--border-subtle); padding-top: 8px; margin-top: 4px;">
            <span style="font-size: var(--fs-9); color: var(--text-muted);">
              Checked: {new Date(comp.last_check).toLocaleTimeString()}
            </span>
            
            <button class="settings-btn" onclick={() => handleRestart(comp.name)} disabled={isRestarting[comp.name]} style="background: var(--bg-primary); border: 1px solid var(--border-subtle); padding: 4px 10px; font-size: var(--fs-9-5); font-weight: 600; border-radius: 4px; cursor: pointer; color: var(--text-primary); transition: all 0.1s ease;">
              {#if isRestarting[comp.name]}Healing...{:else}Heal / Restart{/if}
            </button>
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <div style="display: flex; flex-direction: column; gap: 16px;">
      <!-- Spending Meters -->
      {#if costSummary}
        <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 16px;">
          <!-- Session Budget Meter -->
          <div class="setup-card" style="padding: 16px; background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: 8px;">
            <div style="display: flex; justify-content: space-between; align-items: center;">
              <span style="font-size: var(--fs-11); font-weight: 700; color: var(--text-primary); text-transform: uppercase;">Session Spend</span>
              <span style="font-size: var(--fs-11); font-weight: 700; color: var(--accent-blue);">${costSummary.budget.current_session.toFixed(4)} / ${costSummary.budget.max_per_session.toFixed(2)}</span>
            </div>
            
            <div class="progress-bar-bg" style="width: 100%; height: 10px; background: var(--bg-primary); border-radius: 5px; overflow: hidden; border: 1px solid var(--border-subtle); margin-top: 12px;">
              <div class="progress-bar-fg" style="height: 100%; background: var(--accent-blue); width: {Math.min((costSummary.budget.current_session / costSummary.budget.max_per_session) * 100, 100)}%; transition: width 0.3s ease;"></div>
            </div>
            <span style="font-size: var(--fs-9); color: var(--text-muted); display: block; margin-top: 6px;">
              Max spending limit enforced per single agent conversation.
            </span>
          </div>

          <!-- Daily Budget Meter -->
          <div class="setup-card" style="padding: 16px; background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: 8px;">
            <div style="display: flex; justify-content: space-between; align-items: center;">
              <span style="font-size: var(--fs-11); font-weight: 700; color: var(--text-primary); text-transform: uppercase;">Daily Spend</span>
              <span style="font-size: var(--fs-11); font-weight: 700; color: var(--accent-green);">${costSummary.budget.current_day.toFixed(4)} / ${costSummary.budget.max_per_day.toFixed(2)}</span>
            </div>
            
            <div class="progress-bar-bg" style="width: 100%; height: 10px; background: var(--bg-primary); border-radius: 5px; overflow: hidden; border: 1px solid var(--border-subtle); margin-top: 12px;">
              <div class="progress-bar-fg" style="height: 100%; background: var(--accent-green); width: {Math.min((costSummary.budget.current_day / costSummary.budget.max_per_day) * 100, 100)}%; transition: width 0.3s ease;"></div>
            </div>
            <span style="font-size: var(--fs-9); color: var(--text-muted); display: block; margin-top: 6px;">
              Rolling daily budget protection against infinite loop generation.
            </span>
          </div>
        </div>

        <!-- Budget Configuration Form -->
        <div class="settings-form" style="background: var(--bg-surface); border: 1px solid var(--border-primary); border-radius: 8px; padding: 16px; display: flex; flex-direction: column; gap: 14px;">
          <div style="font-size: var(--fs-11); font-weight: 700; color: var(--text-primary); border-bottom: 1px solid var(--border-subtle); padding-bottom: 6px;">
            Enforce Spending Budgets (USD)
          </div>
          
          <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 14px;">
            <label class="form-field" style="display: flex; flex-direction: column; gap: 4px;">
              <span style="font-size: var(--fs-10); font-weight: 600; color: var(--text-secondary);">Max Budget per Session ($)</span>
              <input bind:value={maxPerSession} type="number" step="0.01" min="0.01" max="10" style="background: var(--bg-primary); color: var(--text-primary); border: 1px solid var(--border-subtle); border-radius: 4px; padding: 6px 10px; font-size: var(--fs-11); outline: none;" />
            </label>
            <label class="form-field" style="display: flex; flex-direction: column; gap: 4px;">
              <span style="font-size: var(--fs-10); font-weight: 600; color: var(--text-secondary);">Max Budget per Day ($)</span>
              <input bind:value={maxPerDay} type="number" step="0.05" min="0.05" max="50" style="background: var(--bg-primary); color: var(--text-primary); border: 1px solid var(--border-subtle); border-radius: 4px; padding: 6px 10px; font-size: var(--fs-11); outline: none;" />
            </label>
          </div>

          <div style="display: flex; justify-content: space-between; align-items: center; border-top: 1px solid var(--border-subtle); padding-top: 10px; margin-top: 4px;">
            <span style="font-size: var(--fs-9); color: var(--text-muted); font-style: italic;">
              Total API calls logged: {costSummary.total_calls} calls (Total session costs: ${costSummary.total_cost.toFixed(4)})
            </span>
            <button class="settings-btn settings-btn-save" onclick={handleSaveBudget} disabled={isSavingBudget} style="padding: 6px 14px; font-size: var(--fs-10); background: var(--accent-blue); color: var(--bg-primary); border: none; border-radius: 4px; font-weight: 600; cursor: pointer;">
              {#if isSavingBudget}Saving...{:else}Update Budget Limits{/if}
            </button>
          </div>
        </div>

        <!-- Average Costs cache -->
        {#if Object.keys(costSummary.avg_costs).length > 0}
          <div style="background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: 8px; padding: 12px; display: flex; flex-direction: column; gap: 8px;">
            <span style="font-size: var(--fs-9-5); font-weight: 700; color: var(--text-muted); text-transform: uppercase;">Average Model Costs (Per 1K tokens)</span>
            <div style="display: flex; flex-wrap: wrap; gap: 6px;">
              {#each Object.entries(costSummary.avg_costs) as [model, cost]}
                <span style="display: inline-block; background: var(--bg-primary); border: 1px solid var(--border-subtle); border-radius: 12px; padding: 3px 8px; font-size: var(--fs-9-5); color: var(--text-secondary); font-family: monospace;">
                  {model}: <span style="color: var(--accent-green);">${cost.toFixed(5)}</span>
                </span>
              {/each}
            </div>
          </div>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .toggle-btn.active {
    background: var(--bg-surface) !important;
    color: var(--accent-blue) !important;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12);
  }
  .toggle-btn:hover:not(.active) {
    color: var(--text-primary) !important;
  }
  .setup-card:hover {
    border-color: var(--border-primary) !important;
  }
  .badge {
    border-radius: 4px;
    font-weight: 600;
  }
</style>
