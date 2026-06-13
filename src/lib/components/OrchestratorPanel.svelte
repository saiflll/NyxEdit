<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { agents, addToast } from "../stores.svelte";
  import { get } from "svelte/store";

  type SubAgentRole = "CodeReviewer" | "Debugger" | "Tester" | "Refactorer" | "Architect" | "Explainer";

  type SubAgent = {
    id: string;
    name: string;
    role: SubAgentRole;
    agent_id: string;
    system_prompt: string;
  };

  type DelegateStatus = "Pending" | "Running" | "Completed" | { Failed: string };

  type DelegationTask = {
    id: string;
    role: SubAgentRole;
    prompt: string;
    result: string | null;
    status: DelegateStatus;
  };

  // Local state
  let subAgents = $state<SubAgent[]>([]);
  let delegationTasks = $state<DelegationTask[]>([]);
  let activeSubTab = $state<"agents" | "tasks">("agents");
  let isLoading = $state(false);

  // Form state for adding a sub-agent
  let showAddForm = $state(false);
  let formId = $state("");
  let formName = $state("");
  let formRole = $state<SubAgentRole>("CodeReviewer");
  let formAgentId = $state("");
  let formSystemPrompt = $state("");

  // Expansible task details state
  let expandedTaskId = $state<string | null>(null);

  // Subscribe to global agents
  let llmAgents = $state<any[]>([]);
  $effect(() => {
    const unsub = agents.subscribe(val => {
      llmAgents = val;
    });
    return unsub;
  });

  // Recommendation engine integration
  const ROLE_SPECS: Record<SubAgentRole, { tier: string; spec: string; primary: string }> = {
    CodeReviewer: { tier: "UltraHigh", spec: "Review", primary: "deepseek-r1" },
    Debugger: { tier: "UltraHigh", spec: "Chat", primary: "deepseek-r1" },
    Tester: { tier: "High", spec: "Test", primary: "qwen/qwen-2.5-coder-32b-instruct" },
    Refactorer: { tier: "High", spec: "Code", primary: "qwen/qwen-2.5-coder-32b-instruct" },
    Architect: { tier: "UltraHigh", spec: "Chat", primary: "deepseek-r1" },
    Explainer: { tier: "Low", spec: "Chat", primary: "meta-llama/llama-3-8b-instruct" }
  };

  let recommendedModel = $state<{ model_id: string; provider: string; reason: string } | null>(null);

  $effect(() => {
    if (showAddForm && formRole) {
      fetchRecommendation();
    }
  });

  async function fetchRecommendation() {
    const specInfo = ROLE_SPECS[formRole];
    if (!specInfo) return;
    try {
      const res = await invoke<any>("cost_recommend", {
        tier: specInfo.tier,
        spec: specInfo.spec,
        primaryModel: specInfo.primary
      });
      recommendedModel = res;
    } catch (e) {
      console.warn("Failed to get cost recommendation:", e);
      recommendedModel = null;
    }
  }

  function applyRecommendation() {
    if (recommendedModel && recommendedModel.model_id) {
      const modelId = recommendedModel.model_id;
      // Find LLM agent matching this model
      const found = llmAgents.find(a => a.model === modelId || (a.model && a.model.includes(modelId)));
      if (found) {
        formAgentId = found.id;
        addToast(`Linked LLM Agent set to "${found.name}" (${found.model})`, "success");
      } else {
        addToast(`No active LLM Agent found using model "${modelId}". Please configure this model under Settings > Agents first.`, "error");
      }
    }
  }

  const ROLES = [
    { value: "CodeReviewer", label: "Code Reviewer" },
    { value: "Debugger", label: "Debugger" },
    { value: "Tester", label: "Tester" },
    { value: "Refactorer", label: "Refactorer" },
    { value: "Architect", label: "Architect" },
    { value: "Explainer", label: "Explainer" }
  ];

  async function loadSubAgents() {
    try {
      subAgents = await invoke<SubAgent[]>("orch_get_agents");
    } catch (e) {
      console.error("Failed to load sub-agents:", e);
      addToast("Failed to load sub-agents: " + String(e), "error");
    }
  }

  async function loadTasks() {
    try {
      delegationTasks = await invoke<DelegationTask[]>("orch_get_tasks");
    } catch (e) {
      console.error("Failed to load orchestrator tasks:", e);
      addToast("Failed to load task logs: " + String(e), "error");
    }
  }

  async function handleAddAgent() {
    if (!formId.trim() || !formName.trim() || !formSystemPrompt.trim()) {
      addToast("Please fill in all fields", "error");
      return;
    }

    const cleanId = formId.trim().toLowerCase().replace(/[^a-z0-9-]/g, '-');
    
    // Check duplication
    if (subAgents.some(a => a.id === cleanId)) {
      addToast("Agent ID already exists", "error");
      return;
    }

    const agentPayload: SubAgent = {
      id: cleanId,
      name: formName.trim(),
      role: formRole,
      agent_id: formAgentId,
      system_prompt: formSystemPrompt.trim()
    };

    isLoading = true;
    try {
      await invoke("orch_add_agent", { agent: agentPayload });
      addToast(`Sub-Agent "${formName}" added successfully`, "success");
      showAddForm = false;
      // Reset form
      formId = "";
      formName = "";
      formRole = "CodeReviewer";
      formAgentId = "";
      formSystemPrompt = "";
      await loadSubAgents();
    } catch (e) {
      console.error("Failed to add sub-agent:", e);
      addToast("Failed to add sub-agent: " + String(e), "error");
    } finally {
      isLoading = false;
    }
  }

  async function handleDeleteAgent(id: string, name: string) {
    if (["review-specialist", "test-specialist", "debug-specialist"].includes(id)) {
      addToast("Cannot delete system built-in sub-agent", "error");
      return;
    }

    if (!confirm(`Are you sure you want to delete sub-agent "${name}"?`)) {
      return;
    }

    try {
      await invoke("orch_remove_agent", { id });
      addToast(`Sub-agent "${name}" removed`, "success");
      await loadSubAgents();
    } catch (e) {
      console.error("Failed to remove sub-agent:", e);
      addToast("Failed to delete: " + String(e), "error");
    }
  }

  function toggleExpandTask(id: string) {
    if (expandedTaskId === id) {
      expandedTaskId = null;
    } else {
      expandedTaskId = id;
    }
  }

  function getStatusString(status: DelegateStatus): string {
    if (typeof status === "string") return status;
    if (status && "Failed" in status) return `Failed: ${status.Failed}`;
    return "Unknown";
  }

  function getStatusClass(status: DelegateStatus): string {
    if (status === "Completed") return "badge-installed";
    if (status === "Running") return "badge-checking";
    if (status === "Pending") return "badge-unknown";
    return "badge-missing";
  }

  function getRoleLabel(role: string): string {
    return ROLES.find(r => r.value === role)?.label || role;
  }

  $effect(() => {
    loadSubAgents();
    loadTasks();
  });
</script>

<div class="orchestrator-panel" style="padding: 16px; display: flex; flex-direction: column; gap: 16px; flex: 1; overflow-y: auto;">
  <div class="settings-header" style="padding: 0 0 10px 0; border-bottom: 1px solid var(--border-subtle); display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 10px;">
    <span class="settings-title">Multi-Agent Orchestrator</span>
    <div class="mode-toggles" style="display: flex; background: var(--bg-primary); padding: 2px; border-radius: 6px; border: 1px solid var(--border-subtle);">
      <button class="toggle-btn" class:active={activeSubTab === "agents"} onclick={() => activeSubTab = "agents"} style="border: none; background: transparent; padding: 4px 12px; font-size: var(--fs-10); font-weight: 600; cursor: pointer; border-radius: 4px; color: activeSubTab === 'agents' ? 'var(--accent-blue)' : 'var(--text-muted)'; transition: all 0.1s ease;">
        Sub-Agents
      </button>
      <button class="toggle-btn" class:active={activeSubTab === "tasks"} onclick={() => { activeSubTab = "tasks"; loadTasks(); }} style="border: none; background: transparent; padding: 4px 12px; font-size: var(--fs-10); font-weight: 600; cursor: pointer; border-radius: 4px; color: activeSubTab === 'tasks' ? 'var(--accent-blue)' : 'var(--text-muted)'; transition: all 0.1s ease;">
        Delegated Tasks Logs
      </button>
    </div>
  </div>

  {#if activeSubTab === "agents"}
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px;">
      <span style="font-size: var(--fs-11); color: var(--text-secondary); font-weight: 500;">
        Configured sub-agents for specialized micro-tasks.
      </span>
      <button class="settings-btn settings-btn-add" onclick={() => showAddForm = !showAddForm} disabled={showAddForm} style="background: var(--accent-blue); color: var(--bg-primary); padding: 6px 12px; font-weight: 600; border-radius: 6px; font-size: var(--fs-10); cursor: pointer; border: none; display: flex; align-items: center; gap: 4px;">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        Add Sub-Agent
      </button>
    </div>

    {#if showAddForm}
      <div class="settings-form" style="background: var(--bg-surface); border: 1px solid var(--border-primary); border-radius: 8px; padding: 16px; display: flex; flex-direction: column; gap: 12px; animation: slideDown 0.15s ease-out;">
        <div style="font-size: var(--fs-11); font-weight: 700; color: var(--text-primary); border-bottom: 1px solid var(--border-subtle); padding-bottom: 6px;">
          Create New Sub-Agent
        </div>
        
        <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 12px;">
          <label class="form-field" style="display: flex; flex-direction: column; gap: 4px;">
            <span style="font-size: var(--fs-10); font-weight: 600; color: var(--text-secondary);">Sub-Agent ID (Alphanumeric, unique)</span>
            <input bind:value={formId} type="text" placeholder="e.g. security-expert" style="background: var(--bg-primary); color: var(--text-primary); border: 1px solid var(--border-subtle); border-radius: 4px; padding: 6px 10px; font-size: var(--fs-11); outline: none;" />
          </label>
          <label class="form-field" style="display: flex; flex-direction: column; gap: 4px;">
            <span style="font-size: var(--fs-10); font-weight: 600; color: var(--text-secondary);">Sub-Agent Name</span>
            <input bind:value={formName} type="text" placeholder="e.g. Security Expert" style="background: var(--bg-primary); color: var(--text-primary); border: 1px solid var(--border-subtle); border-radius: 4px; padding: 6px 10px; font-size: var(--fs-11); outline: none;" />
          </label>
        </div>

        <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 12px;">
          <label class="form-field" style="display: flex; flex-direction: column; gap: 4px;">
            <span style="font-size: var(--fs-10); font-weight: 600; color: var(--text-secondary);">Specialized Role</span>
            <select bind:value={formRole} style="background: var(--bg-primary); color: var(--text-primary); border: 1px solid var(--border-subtle); border-radius: 4px; padding: 6px 10px; font-size: var(--fs-11); outline: none;">
              {#each ROLES as role}
                <option value={role.value}>{role.label}</option>
              {/each}
            </select>
          </label>
          <label class="form-field" style="display: flex; flex-direction: column; gap: 4px;">
            <span style="font-size: var(--fs-10); font-weight: 600; color: var(--text-secondary);">Linked LLM Agent</span>
            <select bind:value={formAgentId} style="background: var(--bg-primary); color: var(--text-primary); border: 1px solid var(--border-subtle); border-radius: 4px; padding: 6px 10px; font-size: var(--fs-11); outline: none;">
              <option value="">Default Routing (Auto Model)</option>
              {#each llmAgents as a}
                <option value={a.id}>{a.name} ({a.provider} / {a.model})</option>
              {/each}
            </select>
          </label>
        </div>

        {#if recommendedModel}
          <div class="recommendation-badge" style="background: color-mix(in srgb, var(--accent-blue) 8%, transparent); border: 1px dashed var(--accent-blue); padding: 8px 12px; border-radius: 6px; display: flex; align-items: center; justify-content: space-between; gap: 8px; animation: fadeIn 0.15s ease-out;">
            <div style="display: flex; flex-direction: column; gap: 2px;">
              <span style="font-size: var(--fs-9); font-weight: 700; color: var(--accent-blue); text-transform: uppercase; letter-spacing: 0.5px;">💡 Recommended Cost-Aware Model</span>
              <span style="font-size: var(--fs-10-5); font-weight: 600; color: var(--text-primary); font-family: var(--font-mono, monospace);">
                {recommendedModel.model_id} ({recommendedModel.provider || "auto"})
              </span>
              <span style="font-size: var(--fs-9-5); color: var(--text-muted); font-style: italic;">
                {recommendedModel.reason}
              </span>
            </div>
            <button 
              type="button"
              onclick={applyRecommendation}
              style="background: var(--accent-blue); color: var(--bg-primary); border: none; border-radius: 4px; padding: 4px 10px; font-size: var(--fs-9-5); font-weight: 700; cursor: pointer; transition: all 0.12s ease;"
            >
              Link Agent
            </button>
          </div>
        {/if}

        <label class="form-field" style="display: flex; flex-direction: column; gap: 4px;">
          <span style="font-size: var(--fs-10); font-weight: 600; color: var(--text-secondary);">System Prompt Instructions</span>
          <textarea bind:value={formSystemPrompt} rows={4} placeholder="You are a specialist in... Analyze... Solve..." style="background: var(--bg-primary); color: var(--text-primary); border: 1px solid var(--border-subtle); border-radius: 4px; padding: 8px; font-size: var(--fs-11); font-family: monospace; resize: vertical; outline: none;"></textarea>
        </label>

        <div style="display: flex; justify-content: flex-end; gap: 8px; border-top: 1px solid var(--border-subtle); padding-top: 10px;">
          <button class="settings-btn settings-btn-cancel" onclick={() => showAddForm = false} style="padding: 6px 12px; font-size: var(--fs-10);">Cancel</button>
          <button class="settings-btn settings-btn-save" onclick={handleAddAgent} disabled={isLoading} style="padding: 6px 14px; font-size: var(--fs-10); background: var(--accent-blue); color: var(--bg-primary); border: none; border-radius: 4px; font-weight: 600; cursor: pointer;">
            {#if isLoading}Adding...{:else}Add Sub-Agent{/if}
          </button>
        </div>
      </div>
    {/if}

    <div class="sub-agents-grid" style="display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 12px;">
      {#each subAgents as agent}
        <div class="setup-card" style="display: flex; flex-direction: column; justify-content: space-between; gap: 8px; padding: 12px; background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: 8px; transition: border-color 0.15s ease;">
          <div>
            <div style="display: flex; justify-content: space-between; align-items: flex-start; gap: 8px;">
              <span style="font-size: var(--fs-12); font-weight: 700; color: var(--text-primary);">{agent.name}</span>
              <span style="font-size: var(--fs-9); font-weight: 600; padding: 2px 6px; border-radius: 4px; background: color-mix(in srgb, var(--accent-blue) 10%, transparent); color: var(--accent-blue); text-transform: uppercase;">
                {getRoleLabel(agent.role)}
              </span>
            </div>
            
            <div style="font-size: var(--fs-9); color: var(--text-muted); font-family: monospace; margin-top: 2px;">
              ID: {agent.id}
            </div>

            <div style="margin-top: 8px; font-size: var(--fs-10); color: var(--text-secondary); line-height: 1.4; display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; text-overflow: ellipsis; white-space: normal;">
              {agent.system_prompt}
            </div>
          </div>

          <div style="display: flex; justify-content: space-between; align-items: center; border-top: 1px solid var(--border-subtle); padding-top: 8px; margin-top: 4px;">
            <span style="font-size: var(--fs-9); color: var(--text-muted); font-style: italic;">
              Model: {agent.agent_id ? llmAgents.find(a => a.id === agent.agent_id)?.model || agent.agent_id : "Auto routing"}
            </span>
            
            {#if !["review-specialist", "test-specialist", "debug-specialist"].includes(agent.id)}
              <button onclick={() => handleDeleteAgent(agent.id, agent.name)} style="background: transparent; border: none; cursor: pointer; color: var(--accent-red); padding: 4px; display: flex; align-items: center; justify-content: center; border-radius: 4px;" title="Remove Sub-Agent">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
              </button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <div style="display: flex; justify-content: space-between; align-items: center;">
      <span style="font-size: var(--fs-11); color: var(--text-secondary); font-weight: 500;">
        Review history of agent orchestration tasks.
      </span>
      <button class="settings-btn settings-btn-add" onclick={loadTasks} style="background: var(--bg-surface); color: var(--text-primary); border: 1px solid var(--border-subtle); padding: 5px 10px; font-weight: 600; border-radius: 6px; font-size: var(--fs-10); cursor: pointer;">
        Refresh Tasks
      </button>
    </div>

    {#if delegationTasks.length === 0}
      <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 40px 20px; text-align: center; color: var(--text-muted); border: 1px dashed var(--border-subtle); border-radius: 8px;">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="margin-bottom: 8px;"><path d="M9 12h6m-6 4h6m2 5H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5.586a1 1 0 0 1 .707.293l5.414 5.414a1 1 0 0 1 .293.707V19a2 2 0 0 1-2 2z"/></svg>
        <p style="margin: 0; font-size: var(--fs-11); font-weight: 500;">No orchestrated tasks found in this session.</p>
        <p style="margin: 4px 0 0 0; font-size: var(--fs-10); color: var(--text-muted);">Tasks are logged when the main agent delegates micro-jobs.</p>
      </div>
    {:else}
      <div class="tasks-list" style="display: flex; flex-direction: column; gap: 8px;">
        {#each delegationTasks as task}
          <div class="task-card" style="background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: 8px; overflow: hidden; transition: border-color 0.15s ease;">
            <div 
              role="button" 
              tabindex="0" 
              onclick={() => toggleExpandTask(task.id)} 
              onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); toggleExpandTask(task.id); } }}
              style="padding: 12px; display: flex; align-items: center; justify-content: space-between; gap: 12px; cursor: pointer; flex-wrap: wrap;"
            >
              <div style="display: flex; align-items: center; gap: 10px; min-width: 0;">
                <span style="font-size: var(--fs-9); font-weight: 600; font-family: monospace; color: var(--text-muted); background: var(--bg-primary); padding: 2px 6px; border-radius: 4px; text-overflow: ellipsis; overflow: hidden; white-space: nowrap; max-width: 150px;">
                  {task.id}
                </span>
                <span style="font-size: var(--fs-11); font-weight: 600; color: var(--text-primary);">
                  {getRoleLabel(task.role)}
                </span>
              </div>

              <div style="display: flex; align-items: center; gap: 10px;">
                <span class="badge {getStatusClass(task.status)}" style="font-size: var(--fs-9); padding: 2px 6px;">
                  {getStatusString(task.status).split(":")[0]}
                </span>
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="2" style="transform: rotate({expandedTaskId === task.id ? '180deg' : '0deg'}); transition: transform 0.15s ease;"><polyline points="6 9 12 15 18 9"/></svg>
              </div>
            </div>

            {#if expandedTaskId === task.id}
              <div style="padding: 12px; border-top: 1px solid var(--border-subtle); background: var(--bg-primary); display: flex; flex-direction: column; gap: 8px; animation: slideDown 0.12s ease-out;">
                <div>
                  <span style="font-size: var(--fs-9); font-weight: 600; color: var(--text-muted); text-transform: uppercase;">Task Prompt</span>
                  <div style="font-size: var(--fs-10); color: var(--text-primary); background: var(--bg-surface); padding: 8px; border-radius: 4px; border: 1px solid var(--border-subtle); font-family: monospace; white-space: pre-wrap; margin-top: 4px;">
                    {task.prompt}
                  </div>
                </div>

                {#if task.result}
                  <div>
                    <span style="font-size: var(--fs-9); font-weight: 600; color: var(--text-muted); text-transform: uppercase;">Execution Output</span>
                    <div style="font-size: var(--fs-10); color: var(--text-secondary); background: var(--bg-surface); padding: 8px; border-radius: 4px; border: 1px solid var(--border-subtle); font-family: monospace; white-space: pre-wrap; margin-top: 4px; max-height: 250px; overflow-y: auto;">
                      {task.result}
                    </div>
                  </div>
                {/if}

                {#if typeof task.status === "object" && "Failed" in task.status}
                  <div>
                    <span style="font-size: var(--fs-9); font-weight: 600; color: var(--accent-red); text-transform: uppercase;">Error Details</span>
                    <div style="font-size: var(--fs-10); color: var(--accent-red); background: color-mix(in srgb, var(--accent-red) 8%, var(--bg-surface)); padding: 8px; border-radius: 4px; border: 1px solid color-mix(in srgb, var(--accent-red) 20%, var(--bg-surface)); font-family: monospace; margin-top: 4px;">
                      {task.status.Failed}
                    </div>
                  </div>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
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
  .setup-card:hover, .task-card:hover {
    border-color: var(--border-primary) !important;
  }
  .badge {
    border-radius: 4px;
    font-weight: 600;
  }
</style>
