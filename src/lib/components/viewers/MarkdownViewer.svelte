<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { marked } from "marked";

  let { filePath = "", fileName = "" } = $props();

  let htmlContent = $state("");
  let loading = $state(true);
  let error = $state("");
  let wordCount = $state(0);

  async function loadMarkdown() {
    loading = true;
    error = "";
    try {
      const content = await invoke<string>("fs_read_file", { path: filePath });
      wordCount = content.trim().split(/\s+/).length;
      htmlContent = await marked.parse(content, {
        gfm: true,
        breaks: false,
      }) as string;
    } catch (e: any) {
      error = String(e);
    }
    loading = false;
  }

  $effect(() => {
    if (filePath) loadMarkdown();
  });
</script>

<div class="md-viewer">
  <div class="viewer-toolbar">
    <div class="viewer-file-info">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" stroke-width="1.5">
        <rect x="3" y="5" width="18" height="14" rx="2"/>
        <path d="M7 15V9l3 3 3-3v6"/>
        <path d="M16 11l2-2m0 0l2 2m-2-2v5" stroke-linecap="round"/>
      </svg>
      <span class="viewer-filename">{fileName || filePath.split(/[\\/]/).pop()}</span>
      {#if wordCount > 0}
        <span class="viewer-dimensions">{wordCount} words</span>
      {/if}
    </div>
    <div class="viewer-controls">
      <button class="ctrl-btn" onclick={loadMarkdown} title="Reload">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
        </svg>
      </button>
      <span class="md-badge">MD</span>
    </div>
  </div>

  <div class="md-canvas">
    {#if loading}
      <div class="viewer-loading">
        <div class="spinner"></div>
        <span>Rendering markdown...</span>
      </div>
    {:else if error}
      <div class="viewer-error">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="var(--accent-red)" stroke-width="1.5">
          <circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>
        </svg>
        <span>Failed to render markdown</span>
        <code>{error}</code>
      </div>
    {:else}
      <div class="md-body markdown-content">
        {@html htmlContent}
      </div>
    {/if}
  </div>
</div>

<style>
  .md-viewer {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-primary);
    overflow: hidden;
  }

  .viewer-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 42px;
    padding: 0 14px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-primary);
    flex-shrink: 0;
    gap: 12px;
  }

  .viewer-file-info {
    display: flex;
    align-items: center;
    gap: 7px;
    overflow: hidden;
  }

  .viewer-filename {
    font-size: var(--font-size);
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .viewer-dimensions {
    font-size: var(--fs-9-5);
    color: var(--text-muted);
    font-family: monospace;
    flex-shrink: 0;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    padding: 1px 6px;
    border-radius: 4px;
  }

  .viewer-controls {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .ctrl-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    background: transparent;
    border: 1px solid var(--border-subtle);
    border-radius: 5px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .ctrl-btn:hover {
    background: var(--bg-hover);
    border-color: var(--accent-blue);
    color: var(--accent-blue);
  }

  .md-badge {
    font-size: var(--fs-9-5);
    font-family: monospace;
    font-weight: 700;
    color: #a855f7;
    background: rgba(168, 85, 247, 0.1);
    border: 1px solid rgba(168, 85, 247, 0.25);
    padding: 1px 7px;
    border-radius: 4px;
  }

  .md-canvas {
    flex: 1;
    overflow-y: auto;
    padding: 0;
  }

  .md-canvas::-webkit-scrollbar { width: 6px; }
  .md-canvas::-webkit-scrollbar-track { background: transparent; }
  .md-canvas::-webkit-scrollbar-thumb { background: var(--border-primary); border-radius: 3px; }

  .md-body {
    max-width: 820px;
    margin: 0 auto;
    padding: 32px 40px;
  }

  /* Markdown content styles */
  .md-body :global(h1),
  .md-body :global(h2),
  .md-body :global(h3),
  .md-body :global(h4),
  .md-body :global(h5),
  .md-body :global(h6) {
    color: var(--text-primary);
    font-weight: 700;
    line-height: 1.3;
    margin-top: 1.5em;
    margin-bottom: 0.5em;
    border-bottom: 1px solid var(--border-subtle);
    padding-bottom: 0.25em;
  }

  .md-body :global(h1) { font-size: 1.9em; border-bottom-width: 2px; border-bottom-color: var(--border-primary); }
  .md-body :global(h2) { font-size: 1.5em; }
  .md-body :global(h3) { font-size: 1.2em; border-bottom: none; }
  .md-body :global(h4), .md-body :global(h5), .md-body :global(h6) { font-size: 1em; border-bottom: none; color: var(--text-secondary); }

  .md-body :global(p) {
    color: var(--text-secondary);
    line-height: 1.7;
    margin: 0 0 1em;
    font-size: 0.92em;
  }

  .md-body :global(a) {
    color: var(--accent-blue);
    text-decoration: none;
  }
  .md-body :global(a:hover) { text-decoration: underline; }

  .md-body :global(code) {
    font-family: 'Cascadia Code', 'Fira Code', 'Consolas', monospace;
    font-size: 0.85em;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    padding: 1px 5px;
    border-radius: 4px;
    color: var(--accent-green);
  }

  .md-body :global(pre) {
    background: var(--bg-surface);
    border: 1px solid var(--border-primary);
    border-radius: 8px;
    padding: 14px 16px;
    overflow-x: auto;
    margin: 0 0 1em;
  }

  .md-body :global(pre code) {
    background: none;
    border: none;
    padding: 0;
    font-size: 0.88em;
    color: var(--text-primary);
  }

  .md-body :global(blockquote) {
    border-left: 3px solid var(--accent-blue);
    margin: 0 0 1em;
    padding: 6px 16px;
    background: rgba(129, 140, 248, 0.05);
    border-radius: 0 6px 6px 0;
    color: var(--text-secondary);
    font-style: italic;
  }

  .md-body :global(ul), .md-body :global(ol) {
    padding-left: 1.5em;
    margin: 0 0 1em;
    color: var(--text-secondary);
    font-size: 0.92em;
  }

  .md-body :global(li) { margin-bottom: 0.25em; line-height: 1.6; }

  .md-body :global(table) {
    width: 100%;
    border-collapse: collapse;
    margin: 0 0 1em;
    font-size: 0.88em;
  }

  .md-body :global(th) {
    background: var(--bg-elevated);
    color: var(--text-primary);
    font-weight: 600;
    padding: 8px 12px;
    border: 1px solid var(--border-primary);
    text-align: left;
  }

  .md-body :global(td) {
    padding: 7px 12px;
    border: 1px solid var(--border-subtle);
    color: var(--text-secondary);
    vertical-align: top;
  }

  .md-body :global(tr:nth-child(even) td) {
    background: rgba(255, 255, 255, 0.015);
  }

  .md-body :global(img) {
    max-width: 100%;
    border-radius: 6px;
    margin: 0.5em 0;
  }

  .md-body :global(hr) {
    border: none;
    border-top: 1px solid var(--border-primary);
    margin: 1.5em 0;
  }

  .md-body :global(strong) { color: var(--text-primary); font-weight: 700; }
  .md-body :global(em) { color: var(--text-secondary); }

  .viewer-loading,
  .viewer-error {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    height: 100%;
    color: var(--text-muted);
    font-size: var(--fs-11);
  }

  .viewer-error code {
    font-size: var(--fs-9-5);
    color: var(--accent-red);
    font-family: monospace;
    max-width: 360px;
    text-align: center;
    opacity: 0.7;
  }

  .spinner {
    width: 24px;
    height: 24px;
    border: 2px solid var(--border-subtle);
    border-top-color: #a855f7;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
