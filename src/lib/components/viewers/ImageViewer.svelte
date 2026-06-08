<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { onDestroy } from "svelte";

  let { filePath = "", fileName = "" } = $props();

  let containerEl: HTMLDivElement;
  let imgEl = $state<HTMLImageElement | null>(null);
  let blobUrl = $state("");
  let loading = $state(true);
  let error = $state("");
  let zoom = $state(1);
  let isDragging = $state(false);
  let dragStart = $state({ x: 0, y: 0 });
  let translate = $state({ x: 0, y: 0 });
  let imgNaturalSize = $state({ w: 0, h: 0 });

  $effect(() => {
    if (filePath) {
      zoom = 1;
      translate = { x: 0, y: 0 };
      loadImage();
    }
  });

  function loadImage() {
    loading = true;
    error = "";
    try {
      blobUrl = convertFileSrc(filePath);
    } catch (e: any) {
      error = String(e);
    }
    loading = false;
  }

  function onImgLoad() {
    imgNaturalSize = { w: imgEl?.naturalWidth ?? 0, h: imgEl?.naturalHeight ?? 0 };
    fitToWindow();
  }

  function fitToWindow() {
    if (!containerEl || !imgEl || !imgNaturalSize.w) return;
    const cw = containerEl.clientWidth - 48;
    const ch = containerEl.clientHeight - 80;
    const scaleW = cw / imgNaturalSize.w;
    const scaleH = ch / imgNaturalSize.h;
    zoom = Math.min(scaleW, scaleH, 1);
    translate = { x: 0, y: 0 };
  }

  function resetZoom() {
    zoom = 1;
    translate = { x: 0, y: 0 };
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    const delta = e.deltaY > 0 ? 0.9 : 1.1;
    zoom = Math.min(Math.max(zoom * delta, 0.05), 20);
  }

  function handleMousedown(e: MouseEvent) {
    if (e.button !== 0) return;
    isDragging = true;
    dragStart = { x: e.clientX - translate.x, y: e.clientY - translate.y };
  }

  function handleMousemove(e: MouseEvent) {
    if (!isDragging) return;
    translate = { x: e.clientX - dragStart.x, y: e.clientY - dragStart.y };
  }

  function handleMouseup() { isDragging = false; }

  onDestroy(() => {
    if (blobUrl && blobUrl.startsWith("blob:")) URL.revokeObjectURL(blobUrl);
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="img-viewer"
  bind:this={containerEl}
  onwheel={handleWheel}
  onmousedown={handleMousedown}
  onmousemove={handleMousemove}
  onmouseup={handleMouseup}
  onmouseleave={handleMouseup}
>
  <!-- Toolbar -->
  <div class="viewer-toolbar">
    <div class="viewer-file-info">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" stroke-width="1.5">
        <rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/>
      </svg>
      <span class="viewer-filename">{fileName || filePath.split(/[\\/]/).pop()}</span>
      {#if imgNaturalSize.w > 0}
        <span class="viewer-dimensions">{imgNaturalSize.w} × {imgNaturalSize.h}px</span>
      {/if}
    </div>
    <div class="viewer-controls">
      <button class="ctrl-btn" onclick={() => { zoom = Math.min(zoom * 1.25, 20); }} title="Zoom In">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/><line x1="11" y1="8" x2="11" y2="14"/><line x1="8" y1="11" x2="14" y2="11"/></svg>
      </button>
      <button class="ctrl-btn" onclick={() => { zoom = Math.max(zoom * 0.8, 0.05); }} title="Zoom Out">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/><line x1="8" y1="11" x2="14" y2="11"/></svg>
      </button>
      <button class="ctrl-btn" onclick={fitToWindow} title="Fit to Window">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3"/></svg>
      </button>
      <button class="ctrl-btn" onclick={resetZoom} title="Reset (100%)">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>
      </button>
      <span class="zoom-pct">{Math.round(zoom * 100)}%</span>
    </div>
  </div>

  <!-- Canvas -->
  <div class="viewer-canvas" class:dragging={isDragging}>
    {#if loading}
      <div class="viewer-loading">
        <div class="spinner"></div>
        <span>Loading image...</span>
      </div>
    {:else if error}
      <div class="viewer-error">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="var(--accent-red)" stroke-width="1.5"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
        <span>Failed to load image</span>
        <code>{error}</code>
      </div>
    {:else}
      <img
        bind:this={imgEl}
        src={blobUrl}
        alt={fileName}
        class="viewer-img"
        style="transform: translate({translate.x}px, {translate.y}px) scale({zoom});"
        onload={onImgLoad}
        draggable="false"
      />
    {/if}
  </div>
</div>

<style>
  .img-viewer {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-primary);
    overflow: hidden;
    user-select: none;
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
    gap: 4px;
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

  .zoom-pct {
    font-size: var(--fs-10);
    color: var(--text-muted);
    font-family: monospace;
    min-width: 38px;
    text-align: right;
  }

  .viewer-canvas {
    flex: 1;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    background:
      radial-gradient(circle at 20% 30%, rgba(129, 140, 248, 0.03) 0%, transparent 50%),
      linear-gradient(135deg, var(--bg-primary) 0%, var(--bg-surface) 100%);
    /* Checkerboard pattern to indicate transparency */
    background-image:
      linear-gradient(45deg, var(--bg-elevated) 25%, transparent 25%),
      linear-gradient(-45deg, var(--bg-elevated) 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, var(--bg-elevated) 75%),
      linear-gradient(-45deg, transparent 75%, var(--bg-elevated) 75%);
    background-size: 16px 16px;
    background-position: 0 0, 0 8px, 8px -8px, -8px 0;
    cursor: grab;
  }

  .viewer-canvas.dragging {
    cursor: grabbing;
  }

  .viewer-img {
    transform-origin: center center;
    transition: transform 0.05s ease-out;
    image-rendering: -webkit-optimize-contrast;
    image-rendering: crisp-edges;
    max-width: none;
    pointer-events: none;
    box-shadow: 0 4px 40px rgba(0, 0, 0, 0.5);
  }

  .viewer-loading,
  .viewer-error {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
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
    border-top-color: var(--accent-blue);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
