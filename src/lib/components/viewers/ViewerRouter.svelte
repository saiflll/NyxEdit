<script lang="ts">
  import CodeEditor from "../CodeEditor.svelte";
  import ImageViewer from "./ImageViewer.svelte";
  import MarkdownViewer from "./MarkdownViewer.svelte";
  import SvgViewer from "./SvgViewer.svelte";
  import MediaViewer from "./MediaViewer.svelte";
  import { open } from "@tauri-apps/plugin-shell";

  let {
    filePath = $bindable(""),
    fileContent = "",
    onSave = (_path: string, _content: string) => {},
    onDirtyChange = (_dirty: boolean) => {},
  } = $props();

  const IMAGE_EXTS  = new Set(["png","jpg","jpeg","gif","webp","bmp","ico","tiff","avif"]);
  const MEDIA_EXTS  = new Set(["mp4","webm","ogg","mkv","mov","avi","mp3","wav","flac","aac","m4a","opus"]);

  const ext = $derived(filePath.split(".").pop()?.toLowerCase() || "");
  const fileName = $derived(filePath.split(/[\\/]/).pop() || "");
  const baseName = $derived(fileName.toLowerCase());

  const isHtml = $derived(ext === "html" || ext === "htm");
  const isMd = $derived(ext === "md" || ext === "markdown" || ext === "mdx");
  const hasToggle = $derived(isHtml || isMd);

  let showPreview = $state(false);
  let htmlBlobUrl = $state("");

  function togglePreview() {
    showPreview = !showPreview;
  }

  $effect(() => {
    if (isHtml && showPreview && fileContent) {
      const blob = new Blob([fileContent], { type: "text/html" });
      htmlBlobUrl = URL.createObjectURL(blob);
    } else if (htmlBlobUrl) {
      URL.revokeObjectURL(htmlBlobUrl);
      htmlBlobUrl = "";
    }
  });

  function openInBrowser() {
    const url = "file:///" + filePath.replace(/\\/g, "/");
    open(url);
  }

  type ViewerType = "image" | "svg" | "media" | "code";

  function getViewerType(e: string): ViewerType {
    if (IMAGE_EXTS.has(e)) return "image";
    if (e === "svg") return "svg";
    if (MEDIA_EXTS.has(e)) return "media";
    return "code";
  }

  const viewerType: ViewerType = $derived(getViewerType(ext));
</script>

{#if viewerType === "image"}
  <ImageViewer {filePath} {fileName} />
{:else if viewerType === "svg"}
  <SvgViewer {filePath} {fileName} />
{:else if viewerType === "media"}
  <MediaViewer {filePath} {fileName} />
{:else if isHtml && showPreview}
  <div class="toggleable-wrap">
    <div class="toggleable-bar">
      <span class="toggleable-name">{fileName}</span>
      <div class="toggleable-actions">
        <button class="toggleable-btn edit-btn" onclick={togglePreview} title="Edit Code">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
        </button>
        <button class="toggleable-btn" onclick={openInBrowser} title="Open in Browser">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>
        </button>
      </div>
    </div>
    <iframe class="html-frame" src={htmlBlobUrl} title="HTML Preview"></iframe>
  </div>
{:else if isMd && showPreview}
  <div class="toggleable-wrap">
    <div class="toggleable-bar">
      <span class="toggleable-name">{fileName}</span>
      <div class="toggleable-actions">
        <button class="toggleable-btn edit-btn" onclick={togglePreview} title="Edit Code">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
        </button>
      </div>
    </div>
    <MarkdownViewer {filePath} {fileName} />
  </div>
{:else}
  <div class="code-view-wrap">
    {#if hasToggle && !showPreview}
      <div class="toggleable-bar">
        <span class="toggleable-name">{fileName}</span>
        <button class="toggleable-btn preview-btn" onclick={togglePreview} title="Preview">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>
        </button>
      </div>
    {/if}
    <CodeEditor bind:filePath initialContent={fileContent} {onSave} {onDirtyChange} />
  </div>
{/if}

<style>
  .toggleable-wrap { display:flex; flex-direction:column; height:100%; }
  .toggleable-bar { display:flex; align-items:center; justify-content:space-between; padding:3px 10px; border-bottom:1px solid var(--border-subtle); background:var(--bg-surface); flex-shrink:0; min-height:30px; }
  .toggleable-name { font-size:var(--fs-11); color:var(--text-muted); font-weight:500; }
  .toggleable-actions { display:flex; gap:4px; }
  .toggleable-btn { display:flex; align-items:center; justify-content:center; width:26px; height:24px; border:1px solid var(--border-subtle); border-radius:4px; background:transparent; color:var(--text-muted); cursor:pointer; transition:all 0.12s ease; }
  .toggleable-btn:hover { color:var(--accent-blue); border-color:var(--accent-blue); }
  .edit-btn:hover { color:var(--accent-orange); border-color:var(--accent-orange); }
  .preview-btn:hover { color:var(--accent-green); border-color:var(--accent-green); }
  .html-frame { flex:1; border:none; background:#fff; }

  .code-view-wrap { display:flex; flex-direction:column; height:100%; }
</style>