<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { addToast } from "../stores.svelte";

  let {
    searchPath = "",
    onFileOpen = (_path: string) => {},
    onDirChange = (_path: string) => {},
  } = $props();

  let query = $state("");
  let mode: "filename" | "content" = $state("filename");
  let results = $state<{ path: string; line?: number; content?: string }[]>([]);
  let busy = $state(false);
  let status = $state("");

  let searchTimer: ReturnType<typeof setTimeout> | null = null;

  async function doSearch() {
    const q = query.trim();
    if (!q || !searchPath) { results = []; status = ""; return; }

    busy = true;
    status = "Searching...";
    results = [];

    try {
      if (mode === "filename") {
        const files = await invoke<{ name: string; path: string; is_dir: boolean; size: number; modified: string }[]>("fs_search_files", { path: searchPath, query: q });
        results = files.map((f) => ({ path: f.path, content: f.name }));
        status = files.length > 0 ? `${files.length} file(s) found` : "No matches";
      } else {
        const matches = await invoke<{ path: string; line: number; content: string }[]>("fs_search_contents", { path: searchPath, query: q, maxResults: 200 });
        results = matches.map((m) => ({ path: m.path, line: m.line, content: m.content }));
        status = matches.length > 0 ? `${matches.length} match(es) found` : "No matches";
      }
    } catch (e: any) {
      status = `Error: ${e}`;
      addToast(`Search failed: ${e}`, "error");
    }
    busy = false;
  }

  function onInput() {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(doSearch, 300);
  }

  function openResult(item: { path: string; line?: number }) {
    onFileOpen(item.path);
  }

  function resultPath(item: { path: string; line?: number }): string {
    if (!searchPath) return item.path;
    if (item.path.startsWith(searchPath)) {
      const rel = item.path.slice(searchPath.length).replace(/^[\\/]+/, "");
      return rel;
    }
    return item.path;
  }
</script>

<div class="sf">
  <div class="sf-header">
    <span class="sf-title">SEARCH</span>
  </div>
  <div class="sf-input-row">
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
    <input class="sf-input" bind:value={query} oninput={onInput} placeholder="Search..." autofocus
      onkeydown={(e) => { if (e.key === "Enter") doSearch(); if (e.key === "Escape") { query = ""; results = []; status = ""; }}}
    />
  </div>
  <div class="sf-mode-row">
    <button class="sf-mode-btn" class:active={mode === "filename"} onclick={() => { mode = "filename"; if (query.trim()) doSearch(); }}>Files</button>
    <button class="sf-mode-btn" class:active={mode === "content"} onclick={() => { mode = "content"; if (query.trim()) doSearch(); }}>Content</button>
  </div>
  {#if status}
    <div class="sf-status">{status}</div>
  {/if}
  <div class="sf-results">
    {#each results as item, i (item.path + (item.line ?? ""))}
      <button class="sf-result" onclick={() => openResult(item)} tabindex="0">
        <span class="sf-result-path">{resultPath(item)}</span>
        {#if item.line}
          <span class="sf-result-line">:{item.line}</span>
        {/if}
        {#if item.content}
          <span class="sf-result-preview">{item.content}</span>
        {/if}
      </button>
    {:else}
      {#if busy}
        <div class="sf-empty">Searching...</div>
      {:else if query.trim()}
        <div class="sf-empty">No results</div>
      {:else}
        <div class="sf-empty">Type to search</div>
      {/if}
    {/each}
  </div>
</div>

<style>
  .sf { display:flex; flex-direction:column; height:100%; font-size:var(--font-size); overflow:hidden; }
  .sf-header { display:flex; align-items:center; justify-content:space-between; padding:6px 10px; border-bottom:1px solid var(--border-subtle); flex-shrink:0; }
  .sf-title { font-size:var(--fs-10); font-weight:600; color:var(--text-muted); letter-spacing:0.8px; text-transform:uppercase; }
  .sf-input-row { display:flex; align-items:center; gap:6px; padding:6px 10px; border-bottom:1px solid var(--border-subtle); color:var(--text-muted); }
  .sf-input { flex:1; background:var(--bg-surface); border:1px solid var(--border-subtle); border-radius:4px; padding:4px 8px; font-size:var(--fs-11); color:var(--text-primary); font-family:monospace; min-width:0; }
  .sf-input:focus { outline:none; border-color:var(--accent-blue); }
  .sf-mode-row { display:flex; gap:4px; padding:4px 10px; border-bottom:1px solid var(--border-subtle); }
  .sf-mode-btn { padding:2px 10px; border:1px solid var(--border-subtle); border-radius:4px; background:transparent; color:var(--text-muted); font-size:var(--fs-10); cursor:pointer; transition:all 0.12s ease; }
  .sf-mode-btn:hover { color:var(--text-primary); border-color:var(--border-primary); }
  .sf-mode-btn.active { color:var(--accent-blue); border-color:var(--accent-blue); background:color-mix(in srgb, var(--accent-blue) 8%, transparent); }
  .sf-status { padding:4px 10px; font-size:var(--fs-10); color:var(--text-muted); border-bottom:1px solid var(--border-subtle); }
  .sf-results { flex:1; overflow-y:auto; padding:2px 0; }
  .sf-result { display:flex; align-items:baseline; gap:4px; width:100%; padding:4px 10px; border:none; background:none; color:var(--text-primary); font-size:var(--font-size); cursor:pointer; text-align:left; transition:all 0.1s ease; }
  .sf-result:hover { background:var(--bg-hover); }
  .sf-result-path { font-size:var(--fs-11); color:var(--accent-blue); }
  .sf-result-line { font-size:var(--fs-10); color:var(--accent-green); }
  .sf-result-preview { font-size:var(--fs-10); color:var(--text-muted); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; flex:1; min-width:0; }
  .sf-empty { padding:20px; text-align:center; color:var(--text-muted); font-size:var(--fs-11); }
</style>
