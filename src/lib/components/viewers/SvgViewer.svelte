<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  let { filePath = "", fileName = "" } = $props();

  let svgContent = $state("");
  let loading = $state(true);
  let error = $state("");
  let zoom = $state(1);

  async function loadSvg() {
    loading = true;
    error = "";
    try {
      const content = await invoke<string>("fs_read_file", { path: filePath });
      svgContent = content;
    } catch (e: any) {
      error = String(e);
    }
    loading = false;
  }

  $effect(() => {
    if (filePath) { zoom = 1; loadSvg(); }
  });
</script>

<div class="svg-viewer">
  <div class="viewer-toolbar">
    <div class="viewer-file-info">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#fb923c" stroke-width="1.5">
        <path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/>
      </svg>
      <span class="viewer-filename">{fileName || filePath.split(/[\\/]/).pop()}</span>
    </div>
    <div class="viewer-controls">
      <button class="ctrl-btn" onclick={() => zoom = Math.min(zoom * 1.25, 10)} title="Zoom In">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/><line x1="11" y1="8" x2="11" y2="14"/><line x1="8" y1="11" x2="14" y2="11"/></svg>
      </button>
      <button class="ctrl-btn" onclick={() => zoom = Math.max(zoom * 0.8, 0.1)} title="Zoom Out">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/><line x1="8" y1="11" x2="14" y2="11"/></svg>
      </button>
      <button class="ctrl-btn" onclick={() => zoom = 1} title="Reset Zoom">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>
      </button>
      <span class="zoom-pct">{Math.round(zoom * 100)}%</span>
      <span class="svg-badge">SVG</span>
    </div>
  </div>

  <div class="svg-canvas">
    {#if loading}
      <div class="viewer-loading"><div class="spinner"></div><span>Loading SVG...</span></div>
    {:else if error}
      <div class="viewer-error">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="var(--accent-red)" stroke-width="1.5"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
        <span>Failed to load SVG</span>
        <code>{error}</code>
      </div>
    {:else}
      <div class="svg-wrapper" style="transform: scale({zoom}); transform-origin: center center;">
        {@html svgContent}
      </div>
    {/if}
  </div>
</div>

<style>
  .svg-viewer { display:flex; flex-direction:column; height:100%; background:var(--bg-primary); overflow:hidden; }
  .viewer-toolbar { display:flex; align-items:center; justify-content:space-between; height:42px; padding:0 14px; background:var(--bg-secondary); border-bottom:1px solid var(--border-primary); flex-shrink:0; gap:12px; }
  .viewer-file-info { display:flex; align-items:center; gap:7px; overflow:hidden; }
  .viewer-filename { font-size:var(--font-size); font-weight:600; color:var(--text-primary); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .viewer-controls { display:flex; align-items:center; gap:6px; flex-shrink:0; }
  .ctrl-btn { display:flex; align-items:center; justify-content:center; width:26px; height:26px; background:transparent; border:1px solid var(--border-subtle); border-radius:5px; color:var(--text-secondary); cursor:pointer; transition:all 0.12s ease; }
  .ctrl-btn:hover { background:var(--bg-hover); border-color:var(--accent-blue); color:var(--accent-blue); }
  .zoom-pct { font-size:var(--fs-10); color:var(--text-muted); font-family:monospace; min-width:38px; text-align:right; }
  .svg-badge { font-size:var(--fs-9-5); font-family:monospace; font-weight:700; color:#fb923c; background:rgba(251,146,60,0.1); border:1px solid rgba(251,146,60,0.25); padding:1px 7px; border-radius:4px; }
  .svg-canvas { flex:1; overflow:auto; display:flex; align-items:center; justify-content:center; background-image:linear-gradient(45deg, var(--bg-elevated) 25%, transparent 25%), linear-gradient(-45deg, var(--bg-elevated) 25%, transparent 25%), linear-gradient(45deg, transparent 75%, var(--bg-elevated) 75%), linear-gradient(-45deg, transparent 75%, var(--bg-elevated) 75%); background-size:16px 16px; background-position:0 0,0 8px,8px -8px,-8px 0; }
  .svg-wrapper { max-width:100%; max-height:100%; transition:transform 0.1s ease; }
  .svg-wrapper :global(svg) { max-width:100%; height:auto; display:block; }
  .viewer-loading, .viewer-error { display:flex; flex-direction:column; align-items:center; justify-content:center; gap:10px; height:100%; color:var(--text-muted); font-size:var(--fs-11); }
  .viewer-error code { font-size:var(--fs-9-5); color:var(--accent-red); font-family:monospace; max-width:360px; text-align:center; opacity:0.7; }
  .spinner { width:24px; height:24px; border:2px solid var(--border-subtle); border-top-color:#fb923c; border-radius:50%; animation:spin 0.7s linear infinite; }
  @keyframes spin { to { transform:rotate(360deg); } }
</style>
