<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { agents, aiSendRequest, fileEntries } from "../stores.svelte";

  let input = $state("");
  let attachedFiles = $state<string[]>([]);
  let selectedAgentId = $state("auto");
  let agentList = $state<{ id: string; label: string }[]>([]);
  let isFileDragging = $state(false);

  let allFiles = $state<any[]>([]);
  let showMentionDropdown = $state(false);
  let mentionQuery = $state("");
  let selectedMentionIndex = $state(0);
  let textareaRef = $state<HTMLTextAreaElement | null>(null);
  let isSelectFocused = $state(false);

  let pendingPaste = $state<string | null>(null);
  let pasteConfirmVisible = $state(false);

  let posX = $state(typeof window !== "undefined" ? Math.round(window.innerWidth / 2 - 240) : 400);
  let posY = $state(typeof window !== "undefined" ? window.innerHeight - 130 : 600);
  let isDraggingBar = $state(false);
  let dragStartX = $state(0);
  let dragStartY = $state(0);
  let dragInitX = $state(0);
  let dragInitY = $state(0);

  $effect(() => {
    const unsubscribe = agents.subscribe(list => {
      agentList = [
        { id: "auto", label: "Auto (default)" },
        ...list.map(a => ({ id: a.id, label: a.name }))
      ];
    });
    return unsubscribe;
  });

  $effect(() => {
    const unsubscribe = fileEntries.subscribe(entries => {
      allFiles = entries || [];
    });
    return unsubscribe;
  });

  $effect(() => {
    let active = true;
    let listenPromises: Promise<UnlistenFn>[] = [];

    const pEnter = listen("tauri://drag-enter", () => {
      if (active) isFileDragging = true;
    });
    listenPromises.push(pEnter);

    const pLeave = listen("tauri://drag-leave", () => {
      if (active) isFileDragging = false;
    });
    listenPromises.push(pLeave);

    const pDrop = listen("tauri://drag-drop", (event: any) => {
      if (active) {
        isFileDragging = false;
        if (event.payload && event.payload.paths) {
          for (const path of event.payload.paths) {
            if (path && !attachedFiles.includes(path)) {
              attachedFiles = [...attachedFiles, path];
            }
          }
        }
      }
    });
    listenPromises.push(pDrop);

    return () => {
      active = false;
      for (const p of listenPromises) {
        p.then(fn => fn());
      }
    };
  });

  let filteredFiles = $derived(
    allFiles.filter(f => !f.is_dir && f.name.toLowerCase().includes(mentionQuery.toLowerCase()))
  );

  $effect(() => {
    if (!isDraggingBar) return;
    const handleMouseMove = (e: MouseEvent) => {
      posX = Math.max(0, dragInitX + (e.clientX - dragStartX));
      posY = Math.max(0, dragInitY + (e.clientY - dragStartY));
    };
    const handleMouseUp = () => {
      isDraggingBar = false;
    };
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  });

  function onBarMouseDown(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (target.closest("textarea, select, button, input")) return;
    isDraggingBar = true;
    dragStartX = e.clientX;
    dragStartY = e.clientY;
    dragInitX = posX;
    dragInitY = posY;
  }

  function removeAttachedFile(index: number) {
    attachedFiles = attachedFiles.filter((_, i) => i !== index);
  }

  function handleSend() {
    if (!input.trim()) return;
    aiSendRequest.set({
      content: input,
      files: [...attachedFiles],
      agentId: selectedAgentId
    });
    input = "";
    attachedFiles = [];
    showMentionDropdown = false;
  }

  function confirmPaste() {
    if (pendingPaste != null && textareaRef) {
      const selStart = textareaRef.selectionStart;
      const selEnd = textareaRef.selectionEnd;
      const before = input.slice(0, selStart);
      const after = input.slice(selEnd);
      input = before + pendingPaste + after;
      const newCursor = selStart + pendingPaste.length;
      setTimeout(() => {
        textareaRef!.selectionStart = textareaRef!.selectionEnd = newCursor;
        textareaRef!.focus();
      }, 10);
    }
    pendingPaste = null;
    pasteConfirmVisible = false;
  }

  function cancelPaste() {
    pendingPaste = null;
    pasteConfirmVisible = false;
    textareaRef?.focus();
  }

  function triggerPaste(text: string) {
    if (!text) return;
    pendingPaste = text;
    pasteConfirmVisible = true;
  }

  function checkMention(text: string, selectionEnd: number) {
    const textBeforeCursor = text.slice(0, selectionEnd);
    const atIndex = textBeforeCursor.lastIndexOf("@");
    if (atIndex !== -1 && !/\s/.test(textBeforeCursor.slice(atIndex + 1))) {
      showMentionDropdown = true;
      mentionQuery = textBeforeCursor.slice(atIndex + 1);
      selectedMentionIndex = 0;
    } else {
      showMentionDropdown = false;
      mentionQuery = "";
    }
  }

  function handleInput(e: Event) {
    const target = e.target as HTMLTextAreaElement;
    input = target.value;
    checkMention(input, target.selectionEnd);
  }

  function insertMention(filePath: string) {
    if (!textareaRef) return;
    const text = input;
    const selEnd = textareaRef.selectionEnd;
    const textBeforeCursor = text.slice(0, selEnd);
    const atIndex = textBeforeCursor.lastIndexOf("@");
    if (atIndex !== -1) {
      const before = text.slice(0, atIndex);
      const after = text.slice(selEnd);
      input = before + after;
      if (!attachedFiles.includes(filePath)) {
        attachedFiles = [...attachedFiles, filePath];
      }
      showMentionDropdown = false;
      mentionQuery = "";
      setTimeout(() => {
        textareaRef?.focus();
        textareaRef!.selectionStart = textareaRef!.selectionEnd = atIndex;
      }, 10);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (pasteConfirmVisible) {
      if (e.key === "Enter") {
        e.preventDefault();
        confirmPaste();
      } else if (e.key === "Escape") {
        e.preventDefault();
        cancelPaste();
      }
      return;
    }

    if (showMentionDropdown && filteredFiles.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        selectedMentionIndex = (selectedMentionIndex + 1) % filteredFiles.length;
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        selectedMentionIndex = (selectedMentionIndex - 1 + filteredFiles.length) % filteredFiles.length;
      } else if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        if (filteredFiles[selectedMentionIndex]) {
          insertMention(filteredFiles[selectedMentionIndex].path);
        }
      } else if (e.key === "Escape") {
        e.preventDefault();
        showMentionDropdown = false;
      }
      return;
    }

    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    isFileDragging = true;
  }

  function handleDragLeave(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    isFileDragging = false;
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    isFileDragging = false;
    if (e.dataTransfer && e.dataTransfer.files) {
      const filesList = Array.from(e.dataTransfer.files);
      for (const file of filesList) {
        const path = (file as any).path || file.name;
        if (path && !attachedFiles.includes(path)) {
          attachedFiles = [...attachedFiles, path];
        }
      }
    }
  }

  async function handlePaste(e: ClipboardEvent) {
    if (!e.clipboardData) return;

    const items = Array.from(e.clipboardData.items);
    let hasImage = false;
    for (const item of items) {
      if (item.type.indexOf("image") !== -1) {
        hasImage = true;
        const blob = item.getAsFile();
        if (blob) {
          const reader = new FileReader();
          reader.onload = (event) => {
            const dataUrl = event.target?.result as string;
            if (dataUrl && !attachedFiles.includes(dataUrl)) {
              attachedFiles = [...attachedFiles, dataUrl];
            }
          };
          reader.readAsDataURL(blob);
        }
      }
    }
    if (hasImage) {
      e.preventDefault();
      return;
    }

    const filesList = Array.from(e.clipboardData.files);
    if (filesList.length > 0) {
      e.preventDefault();
      for (const file of filesList) {
        const path = (file as any).path || file.name;
        if (path && !attachedFiles.includes(path)) {
          attachedFiles = [...attachedFiles, path];
        }
      }
      return;
    }

    const text = e.clipboardData.getData("text/plain");
    if (text) {
      e.preventDefault();
      triggerPaste(text);
    }
  }
</script>

<div
  class="ai-floating-bar-outer"
  style="left: {posX}px; top: {posY}px;"
  role="region"
  aria-label="AI floating input"
>
  {#if isFileDragging}
    <div class="drag-overlay">
      <div class="drag-overlay-content">
        <svg class="drag-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
          <polyline points="17 8 12 3 7 8"/>
          <line x1="12" y1="3" x2="12" y2="15"/>
        </svg>
        <span>Drop files to attach</span>
      </div>
    </div>
  {/if}

  <div
    class="ai-floating-bar"
    class:active-focus={isSelectFocused}
    class:is-dragging={isDraggingBar}
    onmousedown={onBarMouseDown}
    ondragover={handleDragOver}
    ondragleave={handleDragLeave}
    ondrop={handleDrop}
  >
    {#if attachedFiles.length > 0}
      <div class="attached-files-container">
        {#each attachedFiles as file, index}
          <div class="file-tag">
            <span class="file-name" title={file}>{file.split(/[\\/]/).pop()}</span>
            <button class="remove-tag" onclick={() => removeAttachedFile(index)}>&times;</button>
          </div>
        {/each}
      </div>
    {/if}

    {#if pasteConfirmVisible}
      <div class="pc-bar" role="alert" onclick={confirmPaste}>
        <span class="pc-text">Paste ~{pendingPaste?.split('\n').length || 0} lines</span>
        <span class="pc-hint">
          <span class="pc-key">Enter</span> paste
          <span class="pc-key">Esc</span> cancel
        </span>
      </div>
    {/if}

    <div class="floating-bar-content">
      <select
        bind:value={selectedAgentId}
        class="agent-select"
        onfocus={() => isSelectFocused = true}
        onblur={() => isSelectFocused = false}
      >
        {#each agentList as agent}
          <option value={agent.id}>{agent.label}</option>
        {/each}
      </select>

      <div class="bar-divider"></div>

      <textarea
        bind:this={textareaRef}
        value={input}
        oninput={handleInput}
        onkeydown={handleKeydown}
        onpaste={handlePaste}
        placeholder="Ask AI... (@ to attach)"
        rows={1}
        class="bar-textarea"
      ></textarea>

      <button class="send-btn" onclick={handleSend} disabled={!input.trim()} title="Send">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
          <line x1="22" y1="2" x2="11" y2="13"/>
          <polygon points="22 2 15 22 11 13 2 9 22 2"/>
        </svg>
      </button>
    </div>
  </div>

  {#if showMentionDropdown && filteredFiles.length > 0}
    <div class="mention-dropdown">
      {#each filteredFiles as file, index}
        <button
          class="mention-item"
          class:active={index === selectedMentionIndex}
          onclick={() => insertMention(file.path)}
        >
          <span class="mention-name">{file.name}</span>
          <span class="mention-path">{file.path}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .ai-floating-bar-outer {
    position: fixed;
    display: flex;
    flex-direction: column;
    align-items: center;
    width: auto;
    z-index: 1000;
    cursor: default;
  }

  .ai-floating-bar {
    display: flex;
    flex-direction: column;
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: 10px;
    padding: 3px 8px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.25);
    min-width: 320px;
    max-width: 500px;
    width: 90vw;
    transition: box-shadow 0.2s ease, border-color 0.2s ease;
    cursor: grab;
    user-select: none;
  }
  .ai-floating-bar:active {
    cursor: grabbing;
  }
  .ai-floating-bar:hover,
  .ai-floating-bar.active-focus {
    border-color: var(--accent-blue);
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.35);
  }

  .attached-files-container {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 3px 4px 6px 4px;
    border-bottom: 1px solid var(--border-subtle);
    margin-bottom: 2px;
  }

  .file-tag {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    background: var(--bg-hover);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    padding: 1px 6px;
    font-size: var(--fs-10);
    color: var(--text-primary);
  }

  .file-name {
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .remove-tag {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: var(--fs-12);
    padding: 0;
    line-height: 1;
  }
  .remove-tag:hover {
    color: var(--accent-red);
  }

  .floating-bar-content {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .bar-divider {
    width: 1px;
    height: 16px;
    background: var(--border-primary);
    flex-shrink: 0;
  }

  .agent-select {
    background: transparent;
    color: var(--text-secondary);
    border: none;
    font-size: var(--fs-10);
    font-weight: 500;
    outline: none;
    cursor: pointer;
    max-width: 90px;
    padding: 2px 2px;
    flex-shrink: 0;
  }
  .agent-select option {
    background: var(--bg-secondary);
    color: var(--text-primary);
  }

  .bar-textarea {
    flex: 1;
    background: transparent;
    color: var(--text-primary);
    border: none;
    outline: none;
    resize: none;
    font-size: var(--font-size);
    line-height: 1.4;
    padding: 5px 2px;
    font-family: inherit;
    min-width: 0;
  }
  .bar-textarea::placeholder {
    color: var(--text-muted);
  }

  .send-btn {
    background: var(--accent-blue);
    color: var(--bg-primary);
    border: none;
    border-radius: 6px;
    width: 26px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.12s ease;
    flex-shrink: 0;
  }
  .send-btn:disabled {
    background: var(--bg-hover);
    color: var(--text-muted);
    cursor: not-allowed;
    opacity: 0.5;
  }
  .send-btn:hover:not(:disabled) {
    filter: brightness(1.15);
  }

  .mention-dropdown {
    position: absolute;
    bottom: calc(100% + 4px);
    left: 50%;
    transform: translateX(-50%);
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: 8px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.4);
    max-height: 180px;
    overflow-y: auto;
    width: 90%;
    max-width: 460px;
    z-index: 1001;
    display: flex;
    flex-direction: column;
    padding: 3px;
  }

  .mention-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    background: transparent;
    border: none;
    border-radius: 6px;
    color: var(--text-primary);
    text-align: left;
    cursor: pointer;
    transition: background 0.1s ease;
    font-size: var(--fs-10);
  }
  .mention-item:hover, .mention-item.active {
    background: var(--bg-hover);
  }

  .mention-name {
    font-weight: 500;
    flex-shrink: 0;
  }
  .mention-path {
    font-size: var(--fs-9);
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ai-floating-bar-outer.is-dragging .ai-floating-bar {
    transform: scale(1.02);
    border-color: var(--accent-blue);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
  }

  .drag-overlay {
    position: absolute;
    inset: -4px;
    background: color-mix(in srgb, var(--accent-blue) 12%, transparent);
    border: 2px dashed var(--accent-blue);
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1010;
    animation: fadeInScale 0.15s ease;
    pointer-events: none;
  }

  .drag-overlay-content {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--accent-blue);
    font-size: var(--fs-11);
    font-weight: 600;
  }
  .drag-icon {
    animation: bounceUpDown 0.8s infinite alternate ease-in-out;
  }

  @keyframes fadeInScale {
    from { opacity: 0; transform: scale(0.96); }
    to { opacity: 1; transform: scale(1); }
  }
  @keyframes bounceUpDown {
    from { transform: translateY(-2px); }
    to { transform: translateY(2px); }
  }

  .pc-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    margin: 0 0 4px;
    background: color-mix(in srgb, var(--accent-blue) 8%, var(--bg-primary));
    border: 1px solid var(--accent-blue);
    border-radius: 6px;
    cursor: pointer;
    user-select: none;
    transition: background 0.15s ease;
    animation: slideDown 0.12s ease;
  }
  .pc-bar:hover {
    background: color-mix(in srgb, var(--accent-blue) 15%, var(--bg-primary));
  }
  .pc-text {
    font-size: var(--fs-11);
    font-weight: 600;
    color: var(--text-primary);
  }
  .pc-hint {
    display: flex;
    align-items: center;
    gap: 3px;
    margin-left: auto;
    font-size: var(--fs-9);
    color: var(--text-muted);
  }
  .pc-key {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 14px;
    height: 13px;
    padding: 0 2px;
    background: var(--bg-surface);
    border: 1px solid var(--border-primary);
    border-radius: 2px;
    font-size: 8px;
    font-weight: 600;
    color: var(--text-secondary);
    line-height: 1;
  }

  @keyframes slideDown {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
