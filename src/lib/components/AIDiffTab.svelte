<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import AIDiffViewer from "./AIDiffViewer.svelte";
  import { addToast } from "$lib/stores.svelte";

  interface Props {
    diffFiles: { path: string; oldContent: string; newContent: string }[];
    onCloseTab: () => void;
  }

  let { diffFiles, onCloseTab }: Props = $props();

  type DiffResult = {
    path: string;
    isEdit: boolean;
    diff: any[];
    additions: number;
    deletions: number;
  };

  let diffResults = $state<DiffResult[]>([]);
  let loading = $state(true);
  let selectedIdx = $state(0);

  $effect(() => {
    if (diffFiles && diffFiles.length > 0) {
      loading = true;
      Promise.all(
        diffFiles.map(async (file) => {
          try {
            const diff = await invoke<any[]>("ai_compute_diff", {
              oldContent: file.oldContent,
              newContent: file.newContent,
            });
            const additions = diff.filter((line) => line.type === "added").length;
            const deletions = diff.filter((line) => line.type === "deleted").length;
            return {
              path: file.path,
              isEdit: !!(file.oldContent && file.oldContent.length > 0),
              diff,
              additions,
              deletions,
            };
          } catch (e) {
            console.error("Failed to compute diff for", file.path, e);
            return {
              path: file.path,
              isEdit: !!(file.oldContent && file.oldContent.length > 0),
              diff: [],
              additions: 0,
              deletions: 0,
            };
          }
        })
      ).then((results) => {
        diffResults = results;
        loading = false;
      });
    } else {
      diffResults = [];
      loading = false;
    }
  });

  let selectedFile = $derived(diffResults[selectedIdx]);

  async function handleRevert() {
    if (!confirm("Are you sure you want to revert all changes made in this message?")) return;
    try {
      for (const file of diffFiles) {
        await invoke("fs_write_file", { path: file.path, content: file.oldContent });
      }
      addToast("Changes reverted successfully", "success");
      onCloseTab();
    } catch (e) {
      console.error("Revert failed", e);
      addToast("Failed to revert changes", "error");
    }
  }

  function handleAccept() {
    addToast("Changes accepted", "success");
    onCloseTab();
  }

  function getFilename(path: string) {
    return path.split(/[\\/]/).pop() || path;
  }
</script>

<div class="diff-tab-container">
  <!-- Diff Header -->
  <div class="diff-tab-header">
    <div class="diff-tab-title-group">
      <svg class="diff-tab-branch-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="6" y1="3" x2="6" y2="15"/>
        <circle cx="18" cy="6" r="3"/>
        <circle cx="6" cy="18" r="3"/>
        <path d="M18 9a9 9 0 0 1-9 9"/>
      </svg>
      <span class="diff-tab-title">AI Code Changes</span>
      <span class="diff-tab-summary">
        ({diffFiles.length} {diffFiles.length === 1 ? 'file' : 'files'} modified)
      </span>
    </div>
    <div class="diff-tab-actions">
      <button class="diff-tab-btn btn-revert" onclick={handleRevert} title="Discard and revert all changes">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7v6h6"/><path d="M21 17a9 9 0 0 0-9-9 9 9 0 0 0-6 2.3L3 13"/></svg>
        Revert Changes
      </button>
      <button class="diff-tab-btn btn-accept" onclick={handleAccept} title="Keep changes and close tab">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
        Accept
      </button>
    </div>
  </div>

  {#if loading}
    <div class="diff-tab-loading">
      <div class="spinner"></div>
      <p>Computing file changes...</p>
    </div>
  {:else if diffResults.length === 0}
    <div class="diff-tab-empty">
      <p>No file changes recorded for this message.</p>
    </div>
  {:else}
    <div class="diff-tab-content">
      <!-- File Selector Sidebar (Only shown if more than 1 file is modified) -->
      {#if diffResults.length > 1}
        <div class="diff-tab-sidebar">
          <div class="sidebar-title">Changed Files</div>
          <div class="sidebar-list">
            {#each diffResults as result, idx}
              <button class="sidebar-item" class:active={selectedIdx === idx} onclick={() => (selectedIdx = idx)}>
                <div class="sidebar-item-meta">
                  <span class="item-name">{getFilename(result.path)}</span>
                  <span class="item-path" title={result.path}>{result.path}</span>
                </div>
                <div class="item-stats">
                  {#if result.additions > 0}
                    <span class="stat-added">+{result.additions}</span>
                  {/if}
                  {#if result.deletions > 0}
                    <span class="stat-deleted">-{result.deletions}</span>
                  {/if}
                </div>
              </button>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Main Diff Viewer -->
      <div class="diff-tab-viewer-area">
        {#if selectedFile}
          <AIDiffViewer path={selectedFile.path} isEdit={selectedFile.isEdit} diff={selectedFile.diff} fullHeight={true} />
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .diff-tab-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-primary);
    color: var(--text-primary);
  }

  .diff-tab-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 16px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }

  .diff-tab-title-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .diff-tab-branch-icon {
    color: var(--accent-blue);
  }

  .diff-tab-title {
    font-size: var(--fs-13);
    font-weight: 600;
  }

  .diff-tab-summary {
    font-size: var(--fs-11);
    color: var(--text-muted);
  }

  .diff-tab-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .diff-tab-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 6px 12px;
    font-size: var(--fs-11);
    font-weight: 600;
    cursor: pointer;
    background: var(--bg-surface);
    color: var(--text-secondary);
    transition: all 0.12s ease;
  }

  .diff-tab-btn:hover {
    color: var(--text-primary);
    border-color: var(--text-muted);
    background: var(--bg-hover);
  }

  .btn-revert {
    border-color: color-mix(in srgb, var(--accent-red) 30%, var(--border-subtle));
    color: var(--accent-red);
  }

  .btn-revert:hover {
    background: color-mix(in srgb, var(--accent-red) 12%, transparent);
    border-color: var(--accent-red);
    color: var(--accent-red);
  }

  .btn-accept {
    background: var(--accent-blue);
    color: var(--bg-primary);
    border-color: var(--accent-blue);
  }

  .btn-accept:hover {
    background: var(--accent-blue);
    filter: brightness(1.15);
    color: var(--bg-primary);
  }

  .diff-tab-loading,
  .diff-tab-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    flex: 1;
    color: var(--text-muted);
    font-size: var(--font-size);
    gap: 12px;
  }

  .spinner {
    width: 24px;
    height: 24px;
    border: 2px solid var(--border-subtle);
    border-top-color: var(--accent-blue);
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .diff-tab-content {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  .diff-tab-sidebar {
    width: 240px;
    background: var(--bg-secondary);
    border-right: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
  }

  .sidebar-title {
    font-size: var(--fs-10);
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .sidebar-list {
    flex: 1;
    overflow-y: auto;
    padding: 6px 0;
  }

  .sidebar-item {
    width: 100%;
    border: none;
    background: none;
    text-align: left;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    cursor: pointer;
    transition: all 0.12s ease;
    border-left: 2px solid transparent;
  }

  .sidebar-item:hover {
    background: var(--bg-hover);
  }

  .sidebar-item.active {
    background: color-mix(in srgb, var(--accent-blue) 10%, transparent);
    border-left-color: var(--accent-blue);
  }

  .sidebar-item-meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .item-name {
    font-size: var(--fs-11);
    font-weight: 600;
    color: var(--text-primary);
  }

  .item-path {
    font-size: var(--fs-9);
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: 2px;
  }

  .item-stats {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 8px;
    font-weight: 700;
    margin-left: 8px;
  }

  .stat-added {
    color: var(--accent-green);
    background: color-mix(in srgb, var(--accent-green) 12%, transparent);
    padding: 1px 4px;
    border-radius: 3px;
  }

  .stat-deleted {
    color: var(--accent-red);
    background: color-mix(in srgb, var(--accent-red) 12%, transparent);
    padding: 1px 4px;
    border-radius: 3px;
  }

  .diff-tab-viewer-area {
    flex: 1;
    overflow: hidden;
    padding: 16px;
    background: var(--bg-primary);
  }
</style>
