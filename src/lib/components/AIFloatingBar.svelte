<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { agents, aiSendRequest, fileEntries } from "../stores.svelte";

  let input = $state("");
  let attachedFiles = $state<string[]>([]);
  let selectedAgentId = $state("auto");
  let agentList = $state<{ id: string; label: string }[]>([]);
  let isDragging = $state(false);

  let allFiles = $state<any[]>([]);
  let showMentionDropdown = $state(false);
  let mentionQuery = $state("");
  let selectedMentionIndex = $state(0);
  let textareaRef = $state<HTMLTextAreaElement | null>(null);
  let isSelectFocused = $state(false);

  let pendingPaste = $state<string | null>(null);
  let pasteConfirmVisible = $state(false);

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
    let unlistenFns: UnlistenFn[] = [];

    async function setup() {
      const enter = await listen("tauri://drag-enter", () => {
        if (active) isDragging = true;
      });
      unlistenFns.push(enter);

      const leave = await listen("tauri://drag-leave", () => {
        if (active) isDragging = false;
      });
      unlistenFns.push(leave);

      const drop = await listen("tauri://drag-drop", (event: any) => {
        if (active) {
          isDragging = false;
          if (event.payload && event.payload.paths) {
            for (const path of event.payload.paths) {
              if (path && !attachedFiles.includes(path)) {
                attachedFiles = [...attachedFiles, path];
              }
            }
          }
        }
      });
      unlistenFns.push(drop);
    }

    setup();

    return () => {
      active = false;
      for (const fn of unlistenFns) {
        fn();
      }
    };
  });

  let filteredFiles = $derived(
    allFiles.filter(f => !f.is_dir && f.name.toLowerCase().includes(mentionQuery.toLowerCase()))
  );

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
    isDragging = true;
  }

  function handleDragLeave(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    isDragging = false;
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    isDragging = false;
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
  class="ai-floating-bar-wrapper"
  class:is-dragging={isDragging}
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
  role="region"
  aria-label="File drop zone"
>
  {#if isDragging}
    <div class="drag-overlay">
      <div class="drag-overlay-content">
        <svg class="drag-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
          <polyline points="17 8 12 3 7 8"/>
          <line x1="12" y1="3" x2="12" y2="15"/>
        </svg>
        <span>Drop files to attach to chat context</span>
      </div>
    </div>
  {/if}
  {#if showMentionDropdown && filteredFiles.length > 0}
    <div class="mention-dropdown">
      {#each filteredFiles as file, index}
        <button
          class="mention-item"
          class:active={index === selectedMentionIndex}
          onclick={() => insertMention(file.path)}
        >
          <span class="mention-icon">📄</span>
          <span class="mention-name">{file.name}</span>
          <span class="mention-path">{file.path}</span>
        </button>
      {/each}
    </div>
  {/if}

  <div class="ai-floating-bar" class:active-focus={isSelectFocused}>
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
      <div class="pc-bar" role="alert">
        <svg class="pc-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="8" y="2" width="8" height="4" rx="1" ry="1"/>
          <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>
        </svg>
        <span class="pc-text">Paste ~{pendingPaste?.split('\n').length || 0} lines</span>
        <span class="pc-hint">
          <span class="pc-key">Enter</span> paste
          <span class="pc-key">Esc</span> cancel
        </span>
      </div>
    {/if}

    <div class="floating-bar-content">
      <div class="agent-select-wrapper">
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
      </div>

      <div class="bar-divider"></div>

      <textarea
        bind:this={textareaRef}
        value={input}
        oninput={handleInput}
        onkeydown={handleKeydown}
        onpaste={handlePaste}
        placeholder="Ask AI anything... (Type @ to attach files)"
        rows={1}
        class="bar-textarea"
      ></textarea>

      <button class="send-btn" onclick={handleSend} disabled={!input.trim()} title="Send prompt to AI">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
          <line x1="22" y1="2" x2="11" y2="13"/>
          <polygon points="22 2 15 22 11 13 2 9 22 2"/>
        </svg>
      </button>
    </div>
  </div>
</div>

<style>
  .ai-floating-bar-wrapper {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
    pointer-events: auto;
  }

  .ai-floating-bar {
    display: flex;
    flex-direction: column;
    background: color-mix(in srgb, var(--bg-secondary) 85%, transparent);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid var(--border-primary);
    border-radius: 20px;
    padding: 4px 10px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
    z-index: 1000;
    max-width: 480px;
    width: 90%;
    opacity: 0.15;
    transform: translateY(0);
    transition: opacity 0.3s cubic-bezier(0.16, 1, 0.3, 1), transform 0.3s cubic-bezier(0.16, 1, 0.3, 1), box-shadow 0.3s cubic-bezier(0.16, 1, 0.3, 1), border-color 0.3s ease;
  }

  .ai-floating-bar:hover,
  .ai-floating-bar:focus-within,
  .ai-floating-bar.active-focus {
    opacity: 1;
    transform: translateY(-2px);
    box-shadow: 0 12px 36px rgba(0, 0, 0, 0.5);
    border-color: var(--accent-blue);
  }

  .attached-files-container {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 4px 6px 8px 6px;
    border-bottom: 1px solid var(--border-subtle);
    margin-bottom: 4px;
  }

  .file-tag {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: var(--bg-hover);
    border: 1px solid var(--border-subtle);
    border-radius: 12px;
    padding: 2px 8px;
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
    gap: 8px;
  }

  .bar-divider {
    width: 1px;
    height: 18px;
    background: var(--border-primary);
  }

  .action-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 6px;
    border-radius: 50%;
    transition: all 0.15s ease;
  }

  .action-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .agent-select-wrapper {
    display: flex;
    align-items: center;
  }

  .agent-select {
    background: transparent;
    color: var(--text-secondary);
    border: none;
    font-size: var(--fs-11);
    font-weight: 500;
    outline: none;
    cursor: pointer;
    max-width: 120px;
    padding: 2px 4px;
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
    padding: 6px 4px;
    font-family: inherit;
  }

  .bar-textarea::placeholder {
    color: var(--text-muted);
  }

  .send-btn {
    background: var(--accent-blue);
    color: var(--bg-primary);
    border: none;
    border-radius: 50%;
    width: 28px;
    height: 28px;
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
    opacity: 0.6;
  }

  .send-btn:hover:not(:disabled) {
    transform: scale(1.05);
    filter: brightness(1.1);
  }

  .mention-dropdown {
    position: absolute;
    bottom: calc(100% + 8px);
    background: color-mix(in srgb, var(--bg-secondary) 95%, transparent);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid var(--border-primary);
    border-radius: 12px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    max-height: 200px;
    overflow-y: auto;
    width: 100%;
    max-width: 480px;
    z-index: 1001;
    display: flex;
    flex-direction: column;
    padding: 4px;
  }

  .mention-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    background: transparent;
    border: none;
    border-radius: 8px;
    color: var(--text-primary);
    text-align: left;
    cursor: pointer;
    transition: background 0.12s ease;
    font-size: var(--fs-11);
  }

  .mention-item:hover, .mention-item.active {
    background: var(--bg-hover);
  }

  .mention-icon {
    font-size: var(--fs-12);
  }

  .mention-name {
    font-weight: 500;
  }

  .mention-path {
    font-size: var(--fs-9);
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    margin-left: 8px;
  }

  .ai-floating-bar-wrapper.is-dragging .ai-floating-bar {
    opacity: 1;
    transform: translateY(-2px) scale(1.02);
    border-color: var(--accent-blue);
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.6);
  }

  .drag-overlay {
    position: absolute;
    inset: -6px;
    background: color-mix(in srgb, var(--accent-blue) 15%, transparent);
    border: 2px dashed var(--accent-blue);
    border-radius: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1010;
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    animation: fadeInScale 0.2s cubic-bezier(0.16, 1, 0.3, 1);
    pointer-events: none;
  }

  .drag-overlay-content {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--accent-blue);
    font-size: var(--fs-11);
    font-weight: 600;
    text-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  }

  .drag-icon {
    animation: bounceUpDown 1s infinite alternate ease-in-out;
  }

  @keyframes fadeInScale {
    from { opacity: 0; transform: scale(0.96); }
    to { opacity: 1; transform: scale(1); }
  }

  @keyframes bounceUpDown {
    from { transform: translateY(-3px); }
    to { transform: translateY(3px); }
  }

  .pc-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    margin: 0 2px 4px;
    background: color-mix(in srgb, var(--accent-blue) 10%, var(--bg-primary));
    border: 1px solid var(--accent-blue);
    border-radius: 10px;
    cursor: pointer;
    user-select: none;
    transition: all 0.15s ease;
    animation: slideDown 0.15s ease;
  }
  .pc-bar:hover {
    background: color-mix(in srgb, var(--accent-blue) 18%, var(--bg-primary));
  }
  .pc-icon {
    color: var(--accent-blue);
    flex-shrink: 0;
  }
  .pc-text {
    font-size: var(--fs-11);
    font-weight: 600;
    color: var(--text-primary);
  }
  .pc-hint {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
    font-size: 9px;
    color: var(--text-muted);
  }
  .pc-key {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 16px;
    height: 14px;
    padding: 0 3px;
    background: var(--bg-surface);
    border: 1px solid var(--border-primary);
    border-radius: 3px;
    font-size: 8px;
    font-weight: 600;
    color: var(--text-secondary);
    line-height: 1;
  }
  @keyframes slideDown {
    from { opacity: 0; transform: translateY(-6px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
