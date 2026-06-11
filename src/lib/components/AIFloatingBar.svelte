<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { agents, aiSendRequest, fileEntries } from "../stores.svelte";
  import { get } from "svelte/store";

  let input = $state("");
  let attachedFiles = $state<string[]>([]);
  let selectedAgentId = $state("auto");
  let agentList = $state<{ id: string; label: string }[]>([]);

  // Workspace files for '@' mentions autocomplete
  let allFiles = $state<any[]>([]);
  let showMentionDropdown = $state(false);
  let mentionQuery = $state("");
  let selectedMentionIndex = $state(0);
  let textareaRef = $state<HTMLTextAreaElement | null>(null);
  let isSelectFocused = $state(false);

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

  let filteredFiles = $derived(
    allFiles.filter(f => !f.is_dir && f.name.toLowerCase().includes(mentionQuery.toLowerCase()))
  );

  async function handleAttach() {
    try {
      const selected = await openDialog({
        multiple: true,
        directory: false,
      });
      if (selected) {
        if (Array.isArray(selected)) {
          attachedFiles = [...attachedFiles, ...selected];
        } else if (typeof selected === "string") {
          attachedFiles = [...attachedFiles, selected];
        }
      }
    } catch (err) {
      console.error("Attach file error:", err);
    }
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
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
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
</script>

<div 
  class="ai-floating-bar-wrapper"
  ondragover={handleDragOver}
  ondrop={handleDrop}
>
  <!-- Autocomplete drop-down above the input bar -->
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

    <div class="floating-bar-content">
      <!-- Attachment button -->
      <button class="action-btn" onclick={handleAttach} title="Attach Files">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"/>
        </svg>
      </button>

      <div class="bar-divider"></div>

      <!-- Agent selection dropdown -->
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

      <!-- Input text area -->
      <textarea
        bind:this={textareaRef}
        value={input}
        oninput={handleInput}
        onkeydown={handleKeydown}
        placeholder="Ask AI anything... (Type @ to attach files)"
        rows={1}
        class="bar-textarea"
      ></textarea>

      <!-- Send button -->
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

  /* Mention Autocomplete Dropdown styles */
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
</style>
