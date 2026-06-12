<script lang="ts">
  type DiffLine = {
    type: "added" | "deleted" | "unchanged";
    text: string;
    old_index?: number;
    new_index?: number;
  };

  interface Props {
    path: string;
    isEdit: boolean;
    diff: DiffLine[];
    fullHeight?: boolean;
  }

  let { path, isEdit, diff, fullHeight = false }: Props = $props();

  // Extract filename
  let fileName = $derived(path.split(/[\\/]/).pop() || path);
</script>

<div class="diff-viewer" class:full-height={fullHeight}>
  <div class="diff-header">
    <span class="diff-icon">{isEdit ? "✏️" : "🆕"}</span>
    <div class="diff-meta">
      <span class="diff-filename">{fileName}</span>
      <span class="diff-path" title={path}>{path}</span>
    </div>
  </div>

  <div class="diff-content">
    {#each diff as line}
      <div class="diff-line {line.type}">
        <div class="line-nums">
          <span class="num old-num">{line.old_index !== null && line.old_index !== undefined ? line.old_index + 1 : ""}</span>
          <span class="num new-num">{line.new_index !== null && line.new_index !== undefined ? line.new_index + 1 : ""}</span>
        </div>
        <div class="line-marker">
          {#if line.type === "added"}+{/if}
          {#if line.type === "deleted"}-{/if}
          {#if line.type === "unchanged"}&nbsp;{/if}
        </div>
        <pre class="line-text">{line.text.replace(/\r?\n$/, "")}</pre>
      </div>
    {/each}
  </div>
</div>

<style>
  .diff-viewer {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    background: color-mix(in srgb, var(--bg-primary) 85%, transparent);
    overflow: hidden;
    margin: 4px 0;
    max-height: 300px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  }

  .diff-viewer.full-height {
    max-height: none;
    flex: 1;
    margin: 0;
    box-shadow: none;
    height: 100%;
  }

  .diff-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    background: var(--bg-hover);
    border-bottom: 1px solid var(--border-subtle);
    font-size: var(--fs-10);
  }

  .diff-icon {
    font-size: var(--fs-12);
  }

  .diff-meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .diff-filename {
    font-weight: 600;
    color: var(--text-primary);
  }

  .diff-path {
    font-size: var(--fs-9);
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .diff-content {
    flex: 1;
    overflow-y: auto;
    font-family: var(--font-mono, monospace);
    font-size: var(--fs-10);
    line-height: 1.5;
  }

  .diff-line {
    display: flex;
    align-items: stretch;
    white-space: pre;
    min-height: 20px;
  }

  .diff-line.added {
    background: color-mix(in srgb, var(--accent-green) 12%, transparent);
    color: color-mix(in srgb, var(--accent-green) 90%, var(--text-primary));
  }

  .diff-line.deleted {
    background: color-mix(in srgb, var(--accent-red) 12%, transparent);
    color: color-mix(in srgb, var(--accent-red) 90%, var(--text-primary));
  }

  .diff-line.unchanged {
    color: var(--text-secondary);
  }

  .line-nums {
    display: flex;
    width: 60px;
    flex-shrink: 0;
    background: var(--bg-hover);
    opacity: 0.5;
    user-select: none;
    border-right: 1px solid var(--border-subtle);
    font-size: 8px;
    color: var(--text-muted);
    text-align: right;
  }

  .num {
    width: 30px;
    padding-right: 4px;
    box-sizing: border-box;
  }

  .old-num {
    border-right: 1px dashed var(--border-subtle);
  }

  .line-marker {
    width: 15px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    user-select: none;
    font-weight: bold;
    font-size: var(--fs-11);
    opacity: 0.7;
  }

  .line-text {
    margin: 0;
    padding: 0 4px;
    flex: 1;
    overflow-x: auto;
    font-family: inherit;
    font-size: inherit;
    white-space: pre-wrap;
    word-break: break-all;
    display: flex;
    align-items: center;
  }

  /* Custom Scrollbar for Diff Content */
  .diff-content::-webkit-scrollbar {
    width: 6px;
    height: 6px;
  }
  .diff-content::-webkit-scrollbar-thumb {
    background: var(--border-subtle);
    border-radius: 3px;
  }
  .diff-content::-webkit-scrollbar-thumb:hover {
    background: var(--text-muted);
  }
</style>
