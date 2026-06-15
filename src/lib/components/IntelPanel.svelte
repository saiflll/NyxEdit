<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { currentDir, workspaceFolders, openFile, addToast } from "../stores.svelte";
  import CodeReviewPanel from "./CodeReviewPanel.svelte";
  import DiagnosticsPanel from "./DiagnosticsPanel.svelte";
  import { onMount } from "svelte";

  let foldersList = $state<string[]>([]);
  let activeFolder = $state("");
  let workspaceDir = $derived(activeFolder);

  let graphSectionOpen = $state(true);
  let reviewSectionOpen = $state(false);
  let diagSectionOpen = $state(false);

  let graphNodesCount = $state(0);
  let graphEdgesCount = $state(0);
  let isIndexingGraph = $state(false);
  let indexGraphStatus = $state("");
  let indexTotalFiles = $state(0);
  let indexCurrentFile = $state("");
  let indexCurrentIndex = $state(0);
  let indexProgress = $state(0);

  let searchQuery = $state("");
  let searchResults = $state<any[]>([]);
  let isSearchingGraph = $state(false);
  let expandedSymbolId = $state<string | null>(null);
  let symbolReferences = $state<any[]>([]);
  let symbolOutgoing = $state<any[]>([]);
  let isLoadingDetails = $state(false);

  let projectIntel = $state<{
    framework: string; has_tests: boolean; has_ci: boolean;
    has_docker: boolean; has_kubernetes: boolean; file_count: number; language: string; src_dirs: string[];
  } | null>(null);
  let projectIntelError = $state("");

  onMount(() => {
    const unsubFolders = workspaceFolders.subscribe((val) => {
      foldersList = val || [];
      if (foldersList.length > 0 && (!activeFolder || !foldersList.includes(activeFolder))) {
        activeFolder = foldersList[0];
      }
    });

    const unsubDir = currentDir.subscribe((val) => {
      if (val && foldersList.includes(val)) {
        activeFolder = val;
      }
    });

    return () => {
      unsubFolders();
      unsubDir();
    };
  });

  function kindIcon(kind: string): string {
    const map: Record<string, string> = {
      function: "ƒ", fn: "ƒ", method: "ƒ", closure: "ƒ",
      class: "C", struct: "S", enum: "E", trait: "T",
      interface: "I", type: "T", module: "◉", mod: "◉",
      variable: "x", var: "x", const: "c", constant: "c",
      macro: "µ", field: "•", arg: "@", parameter: "@",
      impl: "⊞", block: "□",
    };
    return map[kind.toLowerCase()] || "?";
  }

  async function searchSymbolGraph() {
    if (!searchQuery.trim()) { searchResults = []; return; }
    isSearchingGraph = true;
    expandedSymbolId = null;
    try {
      const results = await invoke<any[]>("graph_search", { query: searchQuery.trim() });
      searchResults = results;
      if (results.length === 0) addToast("No symbols found matching query", "info");
    } catch (e) {
      addToast("Failed to query symbol graph: " + String(e), "error");
    } finally { isSearchingGraph = false; }
  }

  async function showSymbolDetails(node: any) {
    if (expandedSymbolId === node.id) { expandedSymbolId = null; return; }
    expandedSymbolId = node.id;
    isLoadingDetails = true;
    symbolReferences = [];
    symbolOutgoing = [];
    try {
      symbolReferences = await invoke<any[]>("graph_references", { id: node.id });
      symbolOutgoing = await invoke<any[]>("graph_outgoing", { id: node.id });
    } catch (e) { console.error(e); }
    finally { isLoadingDetails = false; }
  }

  async function loadWorkspaceData() {
    if (!workspaceDir) { projectIntel = null; graphNodesCount = 0; graphEdgesCount = 0; return; }
    // Try loading persisted graph first
    try {
      await invoke<boolean>("graph_load_workspace", { root: workspaceDir });
    } catch (e) { /* no saved graph yet, ignore */ }
    try {
      const stats = await invoke<[number, number]>("graph_stats");
      graphNodesCount = stats[0];
      graphEdgesCount = stats[1];
    } catch (e) { console.error("Failed to load graph stats:", e); }
    try {
      const ctx = await invoke<any>("project_detect", { root: workspaceDir });
      let fwLabel = "Unknown";
      if (ctx.framework === "RustCargo") fwLabel = "Rust/Cargo";
      else if (ctx.framework === "NodeNpm") fwLabel = "Node.js (npm)";
      else if (ctx.framework === "NodeYarn") fwLabel = "Node.js (yarn)";
      else if (ctx.framework === "PythonPoetry") fwLabel = "Python (Poetry)";
      else if (ctx.framework === "PythonPip") fwLabel = "Python (pip)";
      else if (ctx.framework === "GoMod") fwLabel = "Go";
      else if (ctx.framework === "PlatformIO") fwLabel = "PlatformIO";
      else if (ctx.framework === "Docker") fwLabel = "Docker";
      projectIntel = {
        framework: fwLabel, has_tests: ctx.has_tests, has_ci: ctx.has_ci,
        has_docker: ctx.has_docker, has_kubernetes: ctx.has_kubernetes, file_count: ctx.file_count,
        language: ctx.language, src_dirs: ctx.src_dirs || [],
      };
      projectIntelError = "";
    } catch (e) {
      projectIntelError = String(e);
      projectIntel = null;
    }
  }

  async function indexWorkspace() {
    if (!workspaceDir) return;
    isIndexingGraph = true;
    indexGraphStatus = "Indexing workspace...";
    indexProgress = 0; indexCurrentFile = "Starting...";
    indexCurrentIndex = 0; indexTotalFiles = 0;
    try {
      const res = await invoke<string>("graph_index_workspace", { root: workspaceDir });
      indexGraphStatus = res;
      addToast(res, "success");
      await loadWorkspaceData();
    } catch (e) {
      indexGraphStatus = "Failed to index workspace: " + String(e);
      addToast("Workspace indexing failed", "error");
    } finally { isIndexingGraph = false; }
  }

  $effect(() => {
    loadWorkspaceData();
  });

  $effect(() => {
    let unlistenStart: UnlistenFn | null = null;
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenEnd: UnlistenFn | null = null;
    async function setupListeners() {
      unlistenStart = await listen<number>("graph:index_start", (event) => {
        isIndexingGraph = true;
        indexTotalFiles = event.payload;
        indexProgress = 0; indexCurrentFile = "Analyzing workspace files...";
        indexCurrentIndex = 0;
      });
      unlistenProgress = await listen<any>("graph:index_progress", (event) => {
        isIndexingGraph = true;
        const data = event.payload;
        indexProgress = data.progress;
        indexCurrentFile = data.current_file;
        indexTotalFiles = data.total_files;
        indexCurrentIndex = data.current_index;
      });
      unlistenEnd = await listen<number>("graph:index_end", () => {
        isIndexingGraph = false;
        indexProgress = 100;
        indexCurrentFile = "";
        loadWorkspaceData();
      });
    }
    setupListeners();
    return () => {
      if (unlistenStart) unlistenStart();
      if (unlistenProgress) unlistenProgress();
      if (unlistenEnd) unlistenEnd();
    };
  });

  function frameworkIcon(fw: string): string {
    const map: Record<string, string> = {
      "Rust/Cargo": '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/></svg>',
      "Node.js (npm)": '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>',
      "Node.js (yarn)": '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>',
      "Python (Poetry)": '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/></svg>',
      "Python (pip)": '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/></svg>',
      "Go": '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="12" cy="12" r="2"/><path d="M12 2a10 10 0 0 1 10 10"/><path d="M12 22a10 10 0 0 1-10-10"/></svg>',
      "PlatformIO": '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="12" cy="12" r="9"/><path d="M9 12h6M12 9v6"/></svg>',
      "Docker": '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="4" y="2" width="16" height="20" rx="2"/><line x1="9" y1="6" x2="15" y2="6"/><line x1="12" y1="2" x2="12" y2="6"/></svg>',
    };
    return map[fw] || '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>';
  }
</script>

<div class="intel-panel">
  <div class="intel-section">
    <button class="intel-section-header" onclick={() => graphSectionOpen = !graphSectionOpen}>
      <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
      <span>Workspace</span>
      <span class="ic" class:ic-r={graphSectionOpen}><svg width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="9 18 15 12 9 6"/></svg></span>
    </button>

    {#if graphSectionOpen}
      <div class="intel-body">
        {#if !workspaceDir}
          <div class="ws-empty">Open a project folder</div>
        {:else}
          {#if foldersList.length > 1}
            <div class="ws-folder-select-row">
              <span class="ws-select-label">Active Target:</span>
              <select class="ws-folder-select" bind:value={activeFolder} onchange={loadWorkspaceData}>
                {#each foldersList as folder}
                  <option value={folder}>
                    {folder.includes("\\") ? folder.split("\\").pop() : folder.split("/").pop()}
                  </option>
                {/each}
              </select>
            </div>
          {/if}
          <div class="ws-tools">
            <button class="ws-btn ws-btn-idx" onclick={indexWorkspace} disabled={isIndexingGraph}>
              {isIndexingGraph ? '⏳ Indexing...' : 'Index'}
            </button>
            <div class="ws-search">
              <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
              <input bind:value={searchQuery} placeholder="Search symbol..." onkeydown={(e) => { if (e.key === "Enter") searchSymbolGraph(); if (e.key === "Escape") { searchQuery = ""; searchResults = []; }}} />
            </div>
          </div>

          {#if projectIntel}
            <div class="ws-summary">
              <span class="ws-fw">{@html frameworkIcon(projectIntel.framework)} {projectIntel.framework}</span>
            </div>
            <div class="ws-stats">
              <span>{graphNodesCount} sym</span><span class="ws-dot">•</span>
              <span>{graphEdgesCount} dep</span><span class="ws-dot">•</span>
              <span>{projectIntel.file_count} files</span>
              {#if projectIntel.has_tests}<span class="ws-ft">Tests</span>{/if}
              {#if projectIntel.has_ci}<span class="ws-ft ws-ft-ci">CI/CD</span>{/if}
              {#if projectIntel.has_docker}<span class="ws-ft ws-ft-dk">Docker</span>{/if}
              {#if projectIntel.has_kubernetes}<span class="ws-ft ws-ft-k8s">Kubernetes</span>{/if}
            </div>
          {:else if !isIndexingGraph}
            <div class="ws-stats">
              <span>{graphNodesCount} sym</span><span class="ws-dot">•</span>
              <span>{graphEdgesCount} dep</span>
            </div>
          {/if}

          {#if isIndexingGraph}
            <div class="ws-progress">
              <div class="ws-pr-row">
                <span>{indexGraphStatus || 'Indexing'} {Math.round(indexProgress)}%</span>
                <span class="ws-pr-file">{indexCurrentFile ? indexCurrentFile.split('\\').pop()?.split('/').pop() : ''} ({indexCurrentIndex}/{indexTotalFiles})</span>
              </div>
              <div class="ws-pr-bar"><div class="ws-pr-fill" style="width:{indexProgress}%"></div></div>
            </div>
          {:else if indexGraphStatus}
            <div class="ws-ok">{indexGraphStatus}</div>
          {/if}

          {#if searchResults.length > 0}
            <div class="ws-results">
              {#each searchResults as node}
                <div class="ws-sym" class:ws-sym-act={expandedSymbolId === node.id} onclick={() => showSymbolDetails(node)} role="button" tabindex="0" onkeydown={(e) => e.key === "Enter" && showSymbolDetails(node)}>
                  <div class="ws-sym-l1">
                    <span class="ws-sym-ik">{kindIcon(node.kind)}</span>
                    <span class="ws-sym-n">{node.name}</span>
                    <span class="ws-sym-x">{expandedSymbolId === node.id ? '▴' : '▾'}</span>
                  </div>
                  <div class="ws-sym-l2">
                    <button class="ws-sym-f" onclick={(e) => { e.stopPropagation(); openFile(workspaceDir + '/' + node.file_path); }}>{node.file_path}:{node.line}</button>
                    {#if expandedSymbolId === node.id}
                      <span class="ws-sym-info">↳ {symbolReferences.length} refs • {symbolOutgoing.length} deps</span>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        {/if}
      </div>
    {/if}
  </div>

  <div class="intel-section">
    <button class="intel-section-header" onclick={() => reviewSectionOpen = !reviewSectionOpen}>
      <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
      <span>Code Review</span>
      <span class="ic" class:ic-r={reviewSectionOpen}><svg width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="9 18 15 12 9 6"/></svg></span>
    </button>
    {#if reviewSectionOpen}
      <div class="intel-body"><CodeReviewPanel /></div>
    {/if}
  </div>

  <div class="intel-section">
    <button class="intel-section-header" onclick={() => diagSectionOpen = !diagSectionOpen}>
      <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
      <span>Diagnostics</span>
      <span class="ic" class:ic-r={diagSectionOpen}><svg width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="9 18 15 12 9 6"/></svg></span>
    </button>
    {#if diagSectionOpen}
      <div class="intel-body"><DiagnosticsPanel workspaceDir={workspaceDir} /></div>
    {/if}
  </div>
</div>

<style>
  .intel-panel { display:flex;flex-direction:column;gap:3px;padding:4px;overflow-y:auto;height:100%; }
  .intel-section { border:1px solid var(--border-subtle);border-radius:5px;background:var(--bg-surface);overflow:hidden; }
  .intel-section-header { display:flex;align-items:center;gap:5px;width:100%;padding:4px 8px;background:var(--bg-secondary);border:none;color:var(--text-primary);font-size:var(--fs-9);font-weight:700;cursor:pointer;letter-spacing:.3px; }
  .intel-section-header:hover { background:var(--bg-hover); }
  .ic { margin-left:auto;transition:transform .12s;display:flex; }
  .ic-r { transform:rotate(90deg); }
  .intel-body { padding:5px;display:flex;flex-direction:column;gap:5px; }

  .ws-empty { padding:10px;text-align:center;color:var(--text-muted);font-size:var(--fs-9); }
  .ws-folder-select-row { display:flex;align-items:center;gap:6px;padding:2px 4px;border-bottom:1px solid var(--border-subtle);margin-bottom:2px; }
  .ws-select-label { font-size:var(--fs-8);color:var(--text-muted);font-weight:600;text-transform:uppercase; }
  .ws-folder-select { background:var(--bg-primary);color:var(--text-primary);border:1px solid var(--border-subtle);border-radius:3px;padding:1px 4px;font-size:var(--fs-8);font-family:inherit;font-weight:600;outline:none;cursor:pointer;flex:1;min-width:0; }
  .ws-folder-select:focus { border-color:var(--accent-blue); }
  .ws-tools { display:flex;gap:4px;align-items:center; }
  .ws-btn { display:inline-flex;align-items:center;gap:3px;padding:2px 7px;border-radius:3px;border:1px solid var(--border-subtle);background:var(--bg-primary);color:var(--text-primary);font-size:var(--fs-9);cursor:pointer;font-weight:600;white-space:nowrap; }
  .ws-btn:disabled { opacity:.4;cursor:default; }
  .ws-btn-idx { background:var(--accent-blue);color:var(--bg-primary);border-color:var(--accent-blue); }
  .ws-search { display:flex;align-items:center;gap:4px;flex:1;background:var(--bg-primary);border:1px solid var(--border-subtle);border-radius:3px;padding:2px 6px;min-width:0; }
  .ws-search input { flex:1;background:none;color:var(--text-primary);border:none;padding:2px 0;font-size:var(--fs-9);outline:none;min-width:0; }
  .ws-search svg { flex-shrink:0;color:var(--text-muted); }

  .ws-summary { font-size:var(--fs-9);font-weight:600;color:var(--text-primary);display:flex;align-items:center;gap:4px; }
  .ws-fw { display:inline-flex;align-items:center;gap:4px; }
  .ws-stats { display:flex;align-items:center;gap:4px;flex-wrap:wrap;font-size:var(--fs-8);color:var(--text-muted);font-family:monospace; }
  .ws-dot { color:var(--border-subtle); }
  .ws-ft { display:inline-flex;align-items:center;padding:0 5px;border-radius:2px;font-size:var(--fs-7);font-weight:700;text-transform:uppercase;letter-spacing:.3px;background:var(--bg-primary);border:1px solid var(--border-subtle);color:var(--text-muted);line-height:14px; }
  .ws-ft-ci { background:color-mix(in srgb,var(--accent-blue)10%,transparent);border-color:color-mix(in srgb,var(--accent-blue)25%,transparent);color:var(--accent-blue); }
  .ws-ft-dk { background:color-mix(in srgb,var(--accent-blue)10%,transparent);border-color:color-mix(in srgb,var(--accent-blue)25%,transparent);color:var(--accent-blue); }
  .ws-ft-k8s { background:color-mix(in srgb,var(--accent-blue)10%,transparent);border-color:color-mix(in srgb,var(--accent-blue)25%,transparent);color:var(--accent-blue); }

  .ws-progress { padding:4px 6px;border-radius:3px;background:var(--bg-primary);border:1px solid var(--border-subtle); }
  .ws-pr-row { display:flex;justify-content:space-between;font-size:var(--fs-8);margin-bottom:2px; }
  .ws-pr-file { color:var(--text-muted);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:55%;text-align:right; }
  .ws-pr-bar { height:3px;background:var(--bg-surface);border-radius:2px;overflow:hidden; }
  .ws-pr-fill { height:100%;background:linear-gradient(90deg,var(--accent-blue),var(--accent-green));border-radius:2px;transition:width .2s; }
  .ws-ok { padding:3px 6px;border-radius:3px;background:color-mix(in srgb,var(--accent-green)8%,transparent);border:1px solid var(--accent-green);font-size:var(--fs-8); }

  .ws-results { display:flex;flex-direction:column;gap:2px;max-height:200px;overflow-y:auto; }
  .ws-sym { padding:3px 5px;border-radius:3px;cursor:pointer;border:1px solid transparent;transition:border-color .1s,background .1s; }
  .ws-sym:hover { background:var(--bg-hover);border-color:var(--border-subtle); }
  .ws-sym-act { border-color:var(--accent-blue) !important;background:color-mix(in srgb,var(--accent-blue)6%,transparent); }
  .ws-sym-l1 { display:flex;align-items:center;gap:4px; }
  .ws-sym-ik { font-size:var(--fs-9);font-weight:700;width:12px;text-align:center;color:var(--accent-blue);font-family:monospace;flex-shrink:0; }
  .ws-sym-n { font-size:var(--fs-9);font-weight:600;color:var(--text-primary);flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap; }
  .ws-sym-x { font-size:var(--fs-7);color:var(--text-muted);flex-shrink:0; }
  .ws-sym-l2 { display:flex;align-items:center;gap:4px;font-size:var(--fs-8);color:var(--text-muted);overflow:hidden; }
  .ws-sym-f { background:none;border:none;color:var(--accent-green);cursor:pointer;padding:0;font-family:monospace;font-size:var(--fs-8);text-decoration:underline;overflow:hidden;text-overflow:ellipsis;white-space:nowrap; }
  .ws-sym-info { flex-shrink:0;color:var(--text-muted);font-family:monospace;white-space:nowrap; }
</style>
