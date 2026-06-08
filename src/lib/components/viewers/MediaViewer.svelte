<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";

  let { filePath = "", fileName = "" } = $props();

  const ext = $derived(filePath.split(".").pop()?.toLowerCase() || "");

  const isVideo = $derived(["mp4", "webm", "ogg", "mkv", "mov", "avi"].includes(ext));
  const isAudio = $derived(["mp3", "wav", "ogg", "flac", "aac", "m4a", "opus"].includes(ext));

  const assetUrl = $derived(filePath ? convertFileSrc(filePath) : "");

  const name = $derived(fileName || filePath.split(/[\\/]/).pop() || "");

  const badge = $derived(isVideo ? "VIDEO" : isAudio ? "AUDIO" : "MEDIA");
  const badgeColor = $derived(isVideo ? "#6366f1" : "#10b981");
</script>

<div class="media-viewer">
  <div class="viewer-toolbar">
    <div class="viewer-file-info">
      {#if isVideo}
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#6366f1" stroke-width="1.5">
          <polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2" ry="2"/>
        </svg>
      {:else}
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#10b981" stroke-width="1.5">
          <path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>
        </svg>
      {/if}
      <span class="viewer-filename">{name}</span>
    </div>
    <span class="media-badge" style="color:{badgeColor}; background:color-mix(in srgb, {badgeColor} 12%, transparent); border-color:color-mix(in srgb, {badgeColor} 28%, transparent);">{badge}</span>
  </div>

  <div class="media-canvas">
    {#if !filePath}
      <div class="media-empty">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="1.5">
          <circle cx="12" cy="12" r="10"/><polygon points="10 8 16 12 10 16 10 8" fill="var(--text-muted)"/>
        </svg>
        <span>No media file selected</span>
      </div>
    {:else if isVideo}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video
        src={assetUrl}
        controls
        class="media-element"
      >
        <track kind="captions" />
      </video>
    {:else if isAudio}
      <div class="audio-wrapper">
        <div class="audio-icon">
          <svg width="56" height="56" viewBox="0 0 24 24" fill="none" stroke="#10b981" stroke-width="1">
            <circle cx="12" cy="12" r="10" stroke-opacity="0.2" fill="#10b981" fill-opacity="0.05"/>
            <path d="M9 18V5l12-2v13"/>
            <circle cx="6" cy="18" r="3"/>
            <circle cx="18" cy="16" r="3"/>
          </svg>
        </div>
        <div class="audio-name">{name}</div>
        <audio src={assetUrl} controls class="audio-element"></audio>
      </div>
    {:else}
      <div class="media-empty">
        <span>Unsupported media format: .{ext}</span>
      </div>
    {/if}
  </div>
</div>

<style>
  .media-viewer { display:flex; flex-direction:column; height:100%; background:var(--bg-primary); overflow:hidden; }
  .viewer-toolbar { display:flex; align-items:center; justify-content:space-between; height:42px; padding:0 14px; background:var(--bg-secondary); border-bottom:1px solid var(--border-primary); flex-shrink:0; gap:12px; }
  .viewer-file-info { display:flex; align-items:center; gap:7px; overflow:hidden; }
  .viewer-filename { font-size:var(--font-size); font-weight:600; color:var(--text-primary); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .media-badge { font-size:var(--fs-9-5); font-family:monospace; font-weight:700; border:1px solid; padding:1px 7px; border-radius:4px; flex-shrink:0; }
  .media-canvas { flex:1; display:flex; align-items:center; justify-content:center; overflow:hidden; background:radial-gradient(ellipse at center, var(--bg-surface) 0%, var(--bg-primary) 100%); }
  .media-element { max-width:100%; max-height:100%; outline:none; border-radius:4px; }
  .audio-wrapper { display:flex; flex-direction:column; align-items:center; gap:20px; padding:40px; }
  .audio-icon { opacity:0.85; }
  .audio-name { font-size:var(--fs-11); color:var(--text-secondary); font-weight:600; }
  .audio-element { width:320px; max-width:90vw; }
  .media-empty { display:flex; flex-direction:column; align-items:center; justify-content:center; gap:10px; color:var(--text-muted); font-size:var(--fs-11); }
</style>
