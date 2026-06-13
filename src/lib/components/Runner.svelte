<script module lang="ts">
  export interface RunnerDef {
    id: string;
    name: string;
    command: string;
    shell: "system" | "wsl";
    wslDistro?: string;
    cwd: string;
    description?: string;
  }

  interface RunnersFile {
    version: number;
    runners: RunnerDef[];
  }
</script>

<script lang="ts">
  import { Play, Plus, ChevronDown, ChevronRight } from "lucide-svelte";
  import { currentDir, activeFile, activeTerminalSessionId } from "../stores.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { loadGlobalFile, saveGlobalFile } from "$lib/nyxConfig";

  let expandedWorkspace = $state(true);
  let expandedUniversal = $state(true);
  let workspaceRunners = $state<RunnerDef[]>([]);
  let universalRunners = $state<RunnerDef[]>([]);

  let editing = $state<RunnerDef | null>(null);
  let showEditor = $state(false);
  let editName = $state("");
  let editCommand = $state("");
  let editShell = $state<"system" | "wsl">("system");
  let editWslDistro = $state("");
  let editCwd = $state("${workspace}");
  let editDesc = $state("");
  let editScope = $state<"workspace" | "universal">("universal");

  let currentWorkspace = $state("");
  let currentActiveFile = $state("");

  $effect(() => {
    const unsubDir = currentDir.subscribe(val => {
      currentWorkspace = val;
    });
    const unsubFile = activeFile.subscribe(val => {
      currentActiveFile = val || "";
    });
    return () => {
      unsubDir();
      unsubFile();
    };
  });

  async function runnersPath(): Promise<string | null> {
    if (!currentWorkspace) return null;
    return currentWorkspace + "/.nyx/runners.json";
  }

  async function loadRunners() {
    // 1. Load workspace runners
    const p = await runnersPath();
    if (!p) {
      workspaceRunners = [];
    } else {
      try {
        const raw = await invoke<string>("fs_read_file", { path: p });
        const data = JSON.parse(raw) as RunnersFile;
        workspaceRunners = data.runners ?? [];
      } catch {
        workspaceRunners = [];
      }
    }

    // 2. Load universal runners from disk (with localStorage fallback)
    let loaded = await loadGlobalFile<RunnerDef[]>("runners.json", []);
    if (loaded.length > 0) {
      universalRunners = loaded;
    } else {
      try {
        const raw = localStorage.getItem("nyxedit-universal-runners");
        if (raw) {
          universalRunners = JSON.parse(raw) as RunnerDef[];
          // Migrate to disk
          saveGlobalFile("runners.json", universalRunners);
        } else {
          universalRunners = [];
        }
      } catch {
        universalRunners = [];
      }
    }
  }

  async function saveWorkspaceRunners() {
    const p = await runnersPath();
    if (!p) return;
    try {
      const parent = p.split(/[\\/]/).slice(0, -1).join("/");
      await invoke("fs_create_dir", { path: parent }).catch(() => {});
      const data: RunnersFile = { version: 1, runners: workspaceRunners };
      await invoke("fs_write_file", { path: p, content: JSON.stringify(data, null, 2) });
    } catch (e) {
      console.error("Failed to save workspace runners:", e);
    }
  }

  async function saveUniversalRunners() {
    await saveGlobalFile("runners.json", universalRunners);
  }

  $effect(() => {
    if (currentWorkspace) {
      loadRunners();
    } else {
      workspaceRunners = [];
      // Still load universal runners when no workspace is open
      try {
        const raw = localStorage.getItem("nyxedit-universal-runners");
        universalRunners = raw ? JSON.parse(raw) : [];
      } catch {
        universalRunners = [];
      }
    }
  });

  function interpolateVars(cmd: string): string {
    return cmd
      .replace(/\$\{workspace\}/g, currentWorkspace || ".")
      .replace(/\$\{file\}/g, currentActiveFile || "")
      .replace(/\$\{wsl:workspace\}/g, currentWorkspace ? currentWorkspace.replace(/^([A-Z]):[\\/]/, "/mnt/$1/").replace(/\\/g, "/") : ".");
  }

  async function runScript(runner: RunnerDef) {
    let session: string | null = null;
    activeTerminalSessionId.subscribe(val => { session = val; })();
    
    if (!session) {
      alert("No active terminal session to run command in. Click inside a terminal first.");
      return;
    }

    let cwd = interpolateVars(runner.cwd);
    let command = interpolateVars(runner.command);

    if (runner.shell === "wsl") {
      const distroArg = runner.wslDistro ? `-d ${runner.wslDistro}` : "";
      const wslCwd = cwd.replace(/^([A-Z]):[\\/]/, "/mnt/$1/").replace(/\\/g, "/");
      command = `wsl ${distroArg} --cd "${wslCwd}" ${command}`;
    } else if (cwd && cwd !== ".") {
      await invoke("pty_write", { sessionId: session, data: `cd "${cwd}"\r` });
      await new Promise(r => setTimeout(r, 150));
    }

    await invoke("pty_write", { sessionId: session, data: command + "\r" });
  }

  function openNew(scope: "workspace" | "universal") {
    editName = "";
    editCommand = "";
    editShell = "system";
    editWslDistro = "";
    editCwd = "${workspace}";
    editDesc = "";
    editScope = scope;
    editing = null;
    showEditor = true;
  }

  function openEdit(r: RunnerDef, scope: "workspace" | "universal") {
    editName = r.name;
    editCommand = r.command;
    editShell = r.shell;
    editWslDistro = r.wslDistro ?? "";
    editCwd = r.cwd;
    editDesc = r.description ?? "";
    editScope = scope;
    editing = r;
    showEditor = true;
  }

  async function saveEdit() {
    if (!editName.trim() || !editCommand.trim()) return;
    const r: RunnerDef = {
      id: editing?.id ?? `runner-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
      name: editName.trim(),
      command: editCommand.trim(),
      shell: editShell,
      wslDistro: editWslDistro || undefined,
      cwd: editCwd || "${workspace}",
      description: editDesc.trim() || undefined,
    };

    // Remove from both lists first to handle potential scope change
    if (editing) {
      workspaceRunners = workspaceRunners.filter(x => x.id !== editing!.id);
      universalRunners = universalRunners.filter(x => x.id !== editing!.id);
    }

    if (editScope === "workspace") {
      workspaceRunners.push(r);
      workspaceRunners = workspaceRunners;
      await saveWorkspaceRunners();
      // If we switched from universal, save universal list cleanup too
      if (editing) saveUniversalRunners();
    } else {
      universalRunners.push(r);
      universalRunners = universalRunners;
      saveUniversalRunners();
      // If we switched from workspace, save workspace list cleanup too
      if (editing) await saveWorkspaceRunners();
    }

    showEditor = false;
  }

  async function deleteRunner(id: string) {
    workspaceRunners = workspaceRunners.filter(r => r.id !== id);
    universalRunners = universalRunners.filter(r => r.id !== id);
    await saveWorkspaceRunners();
    saveUniversalRunners();
  }
</script>

<div class="runner-panel">
  <!-- ═══ Workspace Runners ═══ -->
  <div class="section-header" onclick={() => { expandedWorkspace = !expandedWorkspace; }} role="button" tabindex="0" onkeydown={(e) => { if (e.key === "Enter") expandedWorkspace = !expandedWorkspace; }}>
    {#if expandedWorkspace}
      <ChevronDown size={12} />
    {:else}
      <ChevronRight size={12} />
    {/if}
    <span class="section-title">WORKSPACE RUNNERS</span>
    <button
      class="icon-btn"
      title="Add workspace runner"
      disabled={!currentWorkspace}
      onclick={(e) => { e.stopPropagation(); openNew("workspace"); }}
    >
      <Plus size={12} />
    </button>
  </div>

  {#if expandedWorkspace}
    <div class="runner-list">
      {#if !currentWorkspace}
        <div class="empty-runners">Open a workspace folder to configure workspace runners</div>
      {:else}
        {#each workspaceRunners as r (r.id)}
          <div class="runner-row">
            <div class="runner-info" onclick={() => openEdit(r, "workspace")} role="button" tabindex="0" onkeydown={(e) => { if (e.key === "Enter") openEdit(r, "workspace"); }}>
              <span class="runner-name">{r.name}</span>
              <span class="runner-cmd">{r.command}</span>
            </div>
            <button class="run-btn" title="Run {r.name}" onclick={() => runScript(r)}>
              <Play size={12} fill="currentColor" />
            </button>
          </div>
        {:else}
          <div class="empty-runners">No workspace runners configured</div>
        {/each}
      {/if}
    </div>
  {/if}

  <!-- ═══ Universal Runners ═══ -->
  <div class="section-header" onclick={() => { expandedUniversal = !expandedUniversal; }} role="button" tabindex="0" onkeydown={(e) => { if (e.key === "Enter") expandedUniversal = !expandedUniversal; }} style="border-top: 1px solid var(--border-subtle)">
    {#if expandedUniversal}
      <ChevronDown size={12} />
    {:else}
      <ChevronRight size={12} />
    {/if}
    <span class="section-title">UNIVERSAL RUNNERS</span>
    <button
      class="icon-btn"
      title="Add universal runner"
      onclick={(e) => { e.stopPropagation(); openNew("universal"); }}
    >
      <Plus size={12} />
    </button>
  </div>

  {#if expandedUniversal}
    <div class="runner-list">
      {#each universalRunners as r (r.id)}
        <div class="runner-row">
          <div class="runner-info" onclick={() => openEdit(r, "universal")} role="button" tabindex="0" onkeydown={(e) => { if (e.key === "Enter") openEdit(r, "universal"); }}>
            <span class="runner-name">{r.name}</span>
            <span class="runner-cmd">{r.command}</span>
          </div>
          <button class="run-btn" title="Run {r.name}" onclick={() => runScript(r)}>
            <Play size={12} fill="currentColor" />
          </button>
        </div>
      {:else}
        <div class="empty-runners">No universal runners configured</div>
      {/each}
    </div>
  {/if}
</div>

{#if showEditor}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal-backdrop" onclick={() => showEditor = false} role="dialog">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation">
      <div class="modal-header">
        <span class="modal-title">{editing ? "Edit Runner" : "New Runner"}</span>
      </div>
      <div class="modal-body">
        <label class="field">
          <span>Scope</span>
          <select bind:value={editScope}>
            <option value="workspace" disabled={!currentWorkspace}>Workspace (.nyx/runners.json)</option>
            <option value="universal">Universal (Global)</option>
          </select>
        </label>
        <label class="field">
          <span>Name</span>
          <input bind:value={editName} placeholder="Flash ESP32" />
        </label>
        <label class="field">
          <span>Command</span>
          <input bind:value={editCommand} placeholder="pio run -t upload" />
        </label>
        <label class="field">
          <span>Shell</span>
          <select bind:value={editShell}>
            <option value="system">System</option>
            <option value="wsl">WSL</option>
          </select>
        </label>
        {#if editShell === "wsl"}
          <label class="field">
            <span>WSL Distro</span>
            <input bind:value={editWslDistro} placeholder="Ubuntu" />
          </label>
        {/if}
        <label class="field">
          <span>Working directory</span>
          <input bind:value={editCwd} placeholder={"${workspace}"} />
        </label>
        <label class="field">
          <span>Description (optional)</span>
          <input bind:value={editDesc} placeholder="Build and flash firmware" />
        </label>
        {#if editing}
          <button class="delete-btn" onclick={async () => { await deleteRunner(editing!.id); showEditor = false; }}>Delete runner</button>
        {/if}
      </div>
      <div class="modal-footer">
        <button class="btn btn-secondary" onclick={() => showEditor = false}>Cancel</button>
        <button class="btn btn-primary" onclick={saveEdit}>Save</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .runner-panel { display: flex; flex-direction: column; overflow: hidden; height: 100%; width: 100%; background: transparent; }

  .section-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border-subtle);
    color: var(--text-muted);
    cursor: pointer;
    user-select: none;
    flex-shrink: 0;
  }
  .section-header:hover { color: var(--text-primary); }

  .section-title {
    flex: 1;
    font-size: var(--fs-10);
    font-weight: 700;
    letter-spacing: 1px;
    text-transform: uppercase;
  }

  .icon-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    padding: 2px;
    border-radius: 4px;
    outline: none;
  }
  .icon-btn:hover:not(:disabled) { color: var(--text-primary); background: var(--bg-hover); }
  .icon-btn:disabled { opacity: 0.35; cursor: not-allowed; }

  .runner-list { overflow-y: auto; flex: 1; padding: 6px 0; min-height: 80px; }

  .runner-row {
    display: flex;
    align-items: center;
    padding: 6px 14px;
    gap: 8px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .runner-row:hover { background: var(--bg-hover); }

  .runner-info { flex: 1; overflow: hidden; cursor: pointer; text-align: left; background: none; border: none; padding: 0; }

  .runner-name {
    display: block;
    font-size: var(--fs-11);
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .runner-cmd {
    display: block;
    font-size: var(--fs-10);
    color: var(--text-muted);
    font-family: monospace;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: 2px;
  }

  .run-btn {
    background: transparent;
    border: none;
    color: var(--accent-green);
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    padding: 6px;
    border-radius: 6px;
    flex-shrink: 0;
    outline: none;
    transition: background 0.15s;
  }
  .run-btn:hover { background: var(--bg-hover); }

  .empty-runners {
    padding: 20px;
    font-size: var(--fs-11);
    color: var(--text-muted);
    text-align: center;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 999;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0,0,0,0.55);
  }

  .modal {
    background: var(--bg-surface);
    border: 1px solid var(--border-primary);
    border-radius: 10px;
    width: 420px;
    max-width: 90vw;
    box-shadow: 0 12px 40px rgba(0,0,0,0.5);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .modal-header {
    padding: 14px 18px;
    border-bottom: 1px solid var(--border-subtle);
    background: var(--bg-secondary);
  }

  .modal-title {
    font-size: var(--fs-13);
    font-weight: 600;
    color: var(--text-primary);
  }

  .modal-body {
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-height: 70vh;
    overflow-y: auto;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .field span {
    font-size: var(--fs-10);
    color: var(--text-muted);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .field input, .field select {
    background: var(--bg-primary);
    border: 1px solid var(--border-subtle);
    color: var(--text-primary);
    padding: 8px 12px;
    font-size: var(--fs-11);
    border-radius: 6px;
    font-family: monospace;
    outline: none;
    transition: border-color 0.15s;
  }
  .field input:focus, .field select:focus {
    border-color: var(--accent-blue);
  }

  .delete-btn {
    background: transparent;
    border: 1px solid var(--accent-red);
    color: var(--accent-red);
    font-size: var(--fs-11);
    font-weight: 600;
    cursor: pointer;
    text-align: center;
    padding: 8px;
    border-radius: 6px;
    outline: none;
    transition: all 0.15s;
    margin-top: 4px;
  }
  .delete-btn:hover { background: var(--accent-red); color: var(--bg-primary); }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 18px;
    border-top: 1px solid var(--border-subtle);
    background: var(--bg-secondary);
  }

  .btn {
    padding: 8px 16px;
    font-size: var(--fs-11);
    font-weight: 600;
    border-radius: 6px;
    border: none;
    cursor: pointer;
    transition: filter 0.15s, opacity 0.15s;
  }
  .btn-primary { background: var(--accent-blue); color: var(--bg-primary); }
  .btn-primary:hover { filter: brightness(1.1); }
  .btn-secondary { background: var(--bg-hover); color: var(--text-primary); }
  .btn-secondary:hover { opacity: 0.9; }
</style>
