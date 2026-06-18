<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { WorkspaceMode } from "$lib/stores/pageState";
  
  let { 
    mode = "explorer", 
    currentDir = "", 
    onOpenFile = () => {}, 
    onOpenFolder = () => {}, 
    onContextMenu = () => {} 
  }: {
    mode?: WorkspaceMode;
    currentDir?: string;
    onOpenFile?: (path: string) => void;
    onOpenFolder?: (path: string) => void;
    onContextMenu?: (e: MouseEvent, path: string, type: string) => void;
  } = $props();

  let files = $state<any[]>([]);
  let expanded = $state<Set<string>>(new Set());
  let busy = $state(false);

  async function loadFiles(dir?: string) {
    const target = dir || currentDir;
    if (!target) return;
    
    busy = true;
    try {
      const entries = await invoke<any[]>("fs_list_dir", { path: target });
      files = entries.sort((a, b) => {
        if (a.is_dir === b.is_dir) return a.name.localeCompare(b.name);
        return a.is_dir ? -1 : 1;
      });
    } catch (err) {
      console.error("Failed to load files:", err);
    } finally {
      busy = false;
    }
  }

  function toggleExpand(path: string) {
    const newExpanded = new Set(expanded);
    if (newExpanded.has(path)) {
      newExpanded.delete(path);
    } else {
      newExpanded.add(path);
    }
    expanded = newExpanded;
  }

  function handleFileClick(entry: any) {
    const fullPath = entry.path || `${currentDir}/${entry.name}`;
    if (entry.is_dir) {
      toggleExpand(fullPath);
      onOpenFolder(fullPath);
    } else {
      onOpenFile(fullPath);
    }
  }

  function handleContextMenu(e: MouseEvent, entry: any) {
    e.preventDefault();
    const fullPath = entry.path || `${currentDir}/${entry.name}`;
    const type = entry.is_dir ? "folder" : "file";
    onContextMenu(e, fullPath, type);
  }

  onMount(() => {
    if (currentDir) loadFiles(currentDir);
  });

  $effect(() => {
    if (currentDir && mode === "explorer") {
      loadFiles(currentDir);
    }
  });
</script>

<div class="file-explorer">
  <div class="explorer-header">
    <span class="header-title">{mode === "explorer" ? "Explorer" : mode === "git" ? "Git" : "SSH"}</span>
  </div>

  {#if busy}
    <div class="explorer-busy">Loading...</div>
  {:else if files.length === 0}
    <div class="explorer-empty">No files</div>
  {:else}
    <div class="file-list">
      {#each files as entry (entry.path || entry.name)}
        {@const isExpanded = expanded.has(entry.path || `${currentDir}/${entry.name}`)}
        
        <div
          class="file-item {entry.is_dir ? 'is-dir' : ''}"
          onclick={() => handleFileClick(entry)}
          oncontextmenu={(e) => handleContextMenu(e, entry)}
        >
          <span class="file-icon">
            {#if entry.is_dir}
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
              </svg>
            {:else}
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"></path>
              </svg>
            {/if}
          </span>
          
          <span class="file-name">{entry.name}</span>
          
          {#if entry.is_dir}
            <span class="expand-icon">
              <svg 
                width="10" height="10" 
                viewBox="0 0 24 24" 
                fill="none" 
                stroke="currentColor" 
                stroke-width="2"
                style="transform: {isExpanded ? 'rotate(90deg)' : 'rotate(0deg)'}"
              >
                <polyline points="9 18 15 12 9 6"></polyline>
              </svg>
            </span>
          {/if}
        </div>

        {#if entry.is_dir && isExpanded}
          <div class="file-sublist">
            <!-- Sub-items would be loaded here -->
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .file-explorer {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .explorer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    border-bottom: 1px solid var(--border-color);
  }

  .explorer-busy, .explorer-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .file-list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
  }

  .file-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 12px;
    cursor: pointer;
    font-size: 13px;
    transition: background 0.1s;
  }

  .file-item:hover {
    background: var(--bg-hover);
  }

  .file-item.is-dir {
    font-weight: 500;
  }

  .file-icon {
    display: flex;
    align-items: center;
    opacity: 0.8;
  }

  .file-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .expand-icon {
    display: flex;
    align-items: center;
    opacity: 0.5;
    transition: transform 0.15s;
  }

  .file-sublist {
    padding-left: 16px;
  }
</style>
