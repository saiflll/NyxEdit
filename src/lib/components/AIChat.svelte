<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { marked } from "marked";
  import { aiSendRequest, type Agent, type ChatMessage, type ChatSession, type AiToolCallEvent, type AiToolResultEvent, currentDir, activeTerminalSessionId, activeFile, fileContent } from "../stores.svelte";
  import { getIdleState } from "$lib/idle.svelte";
  import { get } from "svelte/store";
  import AIDiffViewer from "./AIDiffViewer.svelte";

  type AiUsage = {
    agent_id: string; agent_name: string; provider: string; model: string;
    total_requests: number; total_input_tokens: number; total_output_tokens: number; total_cost: number;
  };

  let agents = $state<Agent[]>([]);
  let messages = $state<ChatMessage[]>([]);
  let toolCalls = $state<Map<string, { name: string; args: string; result: string }>>(new Map());
  let input = $state("");
  let selectedMode = $state("auto");
  let selectedAgentId = $state("");
  let isStreaming = $state(false);
  let streamingAgent = $state("");
  let agentList = $state<{ id: string; label: string }[]>([]);
  let autoFallbackQueue = $state<Agent[]>([]);
  let messagesContainer: HTMLDivElement | undefined = $state(undefined);
  let scrollBtnVisible = $state(false);
  let usageData = $state<AiUsage[]>([]);

  // Session state
  let sessions = $state<ChatSession[]>([]);
  let currentSessionId = $state<string | null>(null);
  let sessionsOpen = $state(false);

  let unlistenChunk: UnlistenFn | null = null;
  let unlistenDone: UnlistenFn | null = null;
  let unlistenError: UnlistenFn | null = null;
  let unlistenToolCall: UnlistenFn | null = null;
  let unlistenToolResult: UnlistenFn | null = null;
  let unlistenRequestPermission: UnlistenFn | null = null;
  let unlistenRequestFilePermission: UnlistenFn | null = null;

  let pendingPermission = $state<{ id: string; command: string; cwd: string } | null>(null);
  let commandOverride = $state("");
  let pendingFilePermission = $state<{ id: string; path: string; is_edit: boolean; diff: any[] } | null>(null);

  let totalCost = $derived(usageData.reduce((s, u) => s + u.total_cost, 0));
  let totalTokens = $derived(usageData.reduce((s, u) => s + u.total_input_tokens + u.total_output_tokens, 0));
  let totalRequests = $derived(usageData.reduce((s, u) => s + u.total_requests, 0));
  let gaugeFraction = $derived(Math.min(totalCost / 1.0, 1));
  let gaugeCircumference = $derived(2 * Math.PI * 18);

  async function loadAgents() {
    agents = await invoke<Agent[]>("ai_list_agents");
    agentList = [
      { id: "auto", label: "Auto (default)" },
      ...agents.map((a) => ({ id: a.id, label: `${a.name} (${a.provider})` })),
    ];
    if (selectedMode === "auto" && agents.length > 0) {
      selectedAgentId = agents[0].id;
    }
  }

  async function loadUsage() {
    try {
      usageData = await invoke<AiUsage[]>("ai_get_usage");
    } catch { /* ignore */ }
  }

  async function loadSessions() {
    try {
      sessions = await invoke<ChatSession[]>("ai_list_sessions");
    } catch { /* ignore */ }
  }

  function cleanup() {
    unlistenChunk?.(); unlistenChunk = null;
    unlistenDone?.(); unlistenDone = null;
    unlistenError?.(); unlistenError = null;
    unlistenToolCall?.(); unlistenToolCall = null;
    unlistenToolResult?.(); unlistenToolResult = null;
    unlistenRequestPermission?.(); unlistenRequestPermission = null;
    unlistenRequestFilePermission?.(); unlistenRequestFilePermission = null;
  }

  async function saveCurrentSession() {
    const msgs = messages.filter(m => m.role !== "system");
    if (msgs.length === 0) return;
    const now = new Date().toISOString();
    const firstUser = msgs.find(m => m.role === "user");
    const name = firstUser ? firstUser.content.slice(0, 60) + (firstUser.content.length > 60 ? "..." : "") : "Chat";
    const session: ChatSession = {
      id: currentSessionId || crypto.randomUUID(),
      name,
      agent_id: selectedAgentId,
      messages: msgs,
      created_at: currentSessionId ? sessions.find(s => s.id === currentSessionId)?.created_at || now : now,
      updated_at: now,
    };
    currentSessionId = session.id;
    try {
      await invoke("ai_save_session", { session });
      await loadSessions();
    } catch { /* ignore */ }
  }

  async function loadSession(session: ChatSession) {
    sessionsOpen = false;
    currentSessionId = session.id;
    messages = session.messages.map(m => ({ role: m.role, content: m.content }));
    selectedAgentId = session.agent_id;
    const found = agentList.find(a => a.id === session.agent_id || a.id === "auto");
    if (found) selectedMode = found.id;
  }

  async function deleteSession(id: string) {
    try {
      await invoke("ai_delete_session", { sessionId: id });
      if (currentSessionId === id) { currentSessionId = null; }
      await loadSessions();
    } catch { /* ignore */ }
  }

  function newChat() {
    currentSessionId = null;
    messages = [];
    toolCalls = new Map();
  }

  function formatDate(iso: string): string {
    try {
      const d = new Date(iso);
      return d.toLocaleDateString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
    } catch { return iso; }
  }

  async function tryStream(agentId: string, label: string, fallbackQueue: Agent[]) {
    cleanup();
    messages = [...messages, { role: "assistant", content: "" }];
    toolCalls = new Map();
    streamingAgent = label;

    unlistenChunk = await listen<{delta: string}>("ai:chunk", (e) => {
      const last = messages[messages.length - 1];
      if (last) last.content += e.payload.delta;
      messages = [...messages];
    });

    unlistenToolCall = await listen<AiToolCallEvent>("ai:tool_call", (e) => {
      const tc = e.payload;
      toolCalls = new Map(toolCalls).set(tc.id, { name: tc.name, args: JSON.stringify(tc.arguments, null, 2), result: "..." });
    });

    unlistenRequestPermission = await listen<{ id: string; command: string; cwd: string }>("ai:request_tool_permission", (e) => {
      pendingPermission = e.payload;
      commandOverride = e.payload.command;
    });

    unlistenRequestFilePermission = await listen<any>("ai:request_file_permission", (e) => {
      pendingFilePermission = e.payload;
    });

    unlistenToolResult = await listen<AiToolResultEvent>("ai:tool_result", (e) => {
      const tr = e.payload;
      const existing = toolCalls.get(tr.id);
      if (existing) {
        const maxLen = 500;
        const result = tr.result.length > maxLen ? tr.result.slice(0, maxLen) + "\n..." : tr.result;
        toolCalls = new Map(toolCalls).set(tr.id, { ...existing, result });
      }
    });

    unlistenDone = await listen<{content: string; provider: string; model: string; input_tokens: number; output_tokens: number; cost: number}>("ai:done", async (e) => {
      const p = e.payload;
      const prefix = streamingAgent ? `[${streamingAgent}]` : `[${p.provider}/${p.model}]`;
      messages[messages.length - 1] = { role: "assistant", content: prefix + "\n" + p.content };
      messages = [...messages];
      isStreaming = false;
      streamingAgent = "";
      toolCalls = new Map();
      cleanup();
      loadUsage();
      await saveCurrentSession();
    });

    unlistenError = await listen<{error: string}>("ai:error", async (e) => {
      cleanup();
      messages = messages.slice(0, -1);
      toolCalls = new Map();
      if (fallbackQueue.length > 0) {
        const next = fallbackQueue.shift()!;
        await tryStream(next.id, `${next.provider}/${next.model}`, fallbackQueue);
      } else {
        messages = [...messages, { role: "assistant", content: `Error: ${e.payload.error}` }];
        messages = [...messages];
        isStreaming = false;
        streamingAgent = "";
        loadUsage();
      }
    });

    const chatMessages = messages.slice(0, -1).map((m) => ({ role: m.role, content: m.content }));
    const workspaceRoot = get(currentDir) || "";
    const activeSession = get(activeTerminalSessionId);
    await invoke("ai_chat_stream", {
      agentId,
      messages: chatMessages,
      workspaceRoot: workspaceRoot || null,
      activeSessionId: activeSession || null
    });
  }

  async function sendMessage(text: string = "", files: string[] = []) {
    const promptText = text || input;
    if (!promptText.trim() || isStreaming) return;
    const idle = getIdleState();
    if (idle.isIdle) {
      idle.setActive();
      messages = [...messages, { role: "assistant", content: "_App was idle. Woke up to process your request._" }];
    }

    // Read global custom instructions & skills from localStorage
    const globalInstructions = localStorage.getItem("nyxedit-global-instructions") || "";
    const skillRead = localStorage.getItem("nyxedit-skill-read") !== "false";
    const skillWrite = localStorage.getItem("nyxedit-skill-write") !== "false";
    const skillTerminal = localStorage.getItem("nyxedit-skill-terminal") !== "false";

    let contextPrefix = "";
    if (globalInstructions.trim()) {
      contextPrefix += `[Global Custom Instructions]\n${globalInstructions.trim()}\n\n`;
    }
    contextPrefix += `[Agent Skills Toggles]\n`;
    contextPrefix += `- Reading Files: ${skillRead ? "ENABLED" : "DISABLED (Do not use read_file, glob, grep, list_directory. Inform user if requested)"}\n`;
    contextPrefix += `- Writing/Editing Files: ${skillWrite ? "ENABLED" : "DISABLED (Do not use write_file, edit. Inform user if requested)"}\n`;
    contextPrefix += `- Terminal Command Execution: ${skillTerminal ? "ENABLED" : "DISABLED (Do not use bash_run. Inform user if requested)"}\n\n`;

    const activeFilePath = get(activeFile);
    const activeFileVal = get(fileContent);
    if (activeFilePath && activeFileVal) {
      contextPrefix += `[Active Editor Context - File: ${activeFilePath}]\n\`\`\`\n${activeFileVal}\n\`\`\`\n\n`;
    }

    let finalContent = contextPrefix + promptText;
    if (files && files.length > 0) {
      let fileContexts = [];
      for (const file of files) {
        try {
          const content = await invoke<string>("fs_read_file", { path: file });
          fileContexts.push(`\n\n---\n[Attached File: ${file}]\n${content}\n---`);
        } catch {
          fileContexts.push(`\n\n---\n[Attached File Reference: ${file} (Failed to read)]\n---`);
        }
      }
      finalContent = finalContent + "\n" + fileContexts.join("\n");
    }

    const displayLabel = files && files.length > 0
      ? `${promptText}\n\n📎 Attached files:\n` + files.map(f => ` - ${f.split(/[\\/]/).pop()}`).join("\n")
      : promptText;
    const userMsg: ChatMessage = {
      role: "user",
      content: finalContent,
      display_content: displayLabel
    };
    messages = [...messages, userMsg];
    input = "";
    isStreaming = true;

    try {
      if (selectedMode === "auto") {
        const candidates = agents.filter(a => a.api_key);
        if (candidates.length === 0) {
          messages = [...messages, { role: "assistant", content: "No agent with API key configured. Add one in Settings > Agents." }];
          isStreaming = false;
          return;
        }
        autoFallbackQueue = candidates.slice(1);
        await tryStream(candidates[0].id, `${candidates[0].provider}/${candidates[0].model}`, autoFallbackQueue);
      } else if (selectedAgentId) {
        await tryStream(selectedAgentId, "", []);
      }
    } catch (e: any) {
      if (isStreaming) {
        messages = [...messages, { role: "assistant", content: `Error: ${e}` }];
        isStreaming = false;
        streamingAgent = "";
        cleanup();
      }
    }
  }

  function handleModeChange(val: string) {
    selectedMode = val;
    if (val !== "auto") selectedAgentId = val;
  }

  function markedParse(content: string): string {
    try { return marked.parse(content) as string; } catch { return content; }
  }
  function clearChat() { messages = []; currentSessionId = null; }

  async function respondPermission(approved: boolean) {
    if (!pendingPermission) return;
    try {
      await invoke("ai_respond_bash_permission", {
        id: pendingPermission.id,
        approved,
        modifiedCommand: approved ? commandOverride : null
      });
    } catch (e) {
      console.error("Failed to respond to permission request:", e);
    }
    pendingPermission = null;
    commandOverride = "";
  }

  async function respondFilePermission(approved: boolean) {
    if (!pendingFilePermission) return;
    try {
      await invoke("ai_respond_file_permission", {
        id: pendingFilePermission.id,
        approved
      });
    } catch (e) {
      console.error("Failed to respond to file permission request:", e);
    }
    pendingFilePermission = null;
  }

  let copiedMsgIndex = $state<number | null>(null);
  async function copyMessage(content: string, index: number) {
    try {
      await navigator.clipboard.writeText(content);
      copiedMsgIndex = index;
      setTimeout(() => {
        if (copiedMsgIndex === index) { copiedMsgIndex = null; }
      }, 2000);
    } catch { /* ignore */ }
  }
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); sendMessage(); }
  }

  function scrollToBottom(smooth = true) {
    if (messagesContainer) {
      messagesContainer.scrollTo({ top: messagesContainer.scrollHeight, behavior: smooth ? "smooth" : "auto" });
    }
  }

  function scrollBtnClick() {
    scrollToBottom(true);
    scrollBtnVisible = false;
  }

  $effect(() => { loadAgents(); loadUsage(); loadSessions(); });

  $effect(() => {
    if (messagesContainer) {
      const el = messagesContainer;
      const observer = new IntersectionObserver(
        ([entry]) => { scrollBtnVisible = !entry.isIntersecting; },
        { root: el, threshold: 0 }
      );
      const sentinel = el.querySelector(".scroll-sentinel");
      if (sentinel) observer.observe(sentinel);
      return () => observer.disconnect();
    }
  });

  $effect(() => {
    const unsub = aiSendRequest.subscribe(async (req) => {
      if (req) {
        const text = req.content;
        const files = req.files || [];
        const agentId = req.agentId;
        aiSendRequest.set(null);
        if (agentId) {
          if (agentId === "auto") {
            selectedMode = "auto";
            if (agents.length > 0) {
              selectedAgentId = agents[0].id;
            }
          } else {
            selectedMode = agentId;
            selectedAgentId = agentId;
          }
        }
        await sendMessage(text, files);
      }
    });
    return unsub;
  });

  $effect(() => {
    messages;
    if (!scrollBtnVisible && messagesContainer) { scrollToBottom(false); }
  });
</script>

<div class="ai-wrapper">
  {#if sessionsOpen}
    <div class="ai-sessions">
      <div class="ai-sessions-header">
        <span class="ai-sessions-title">Sessions</span>
        <button class="ai-btn ai-btn-ghost" onclick={() => sessionsOpen = false} title="Close">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </button>
      </div>
      <button class="ai-session-new" onclick={newChat}>
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        New Chat
      </button>
      <div class="ai-sessions-list">
        {#each sessions as session}
          <div class="ai-session-item" class:active={currentSessionId === session.id} onclick={() => loadSession(session)}>
            <div class="ai-session-info">
              <span class="ai-session-name">{session.name}</span>
              <span class="ai-session-date">{formatDate(session.updated_at)}</span>
            </div>
            <button class="ai-session-del" onclick={(e) => { e.stopPropagation(); deleteSession(session.id); }} title="Delete session">
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
            </button>
          </div>
        {/each}
        {#if sessions.length === 0}
          <div class="ai-sessions-empty">No saved sessions</div>
        {/if}
      </div>
    </div>
  {/if}

  <div class="ai-chat" class:ai-chat-shrunk={sessionsOpen}>
    <div class="ai-toolbar">
      <button class="ai-btn ai-btn-ghost" onclick={() => sessionsOpen = !sessionsOpen} title="Sessions">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
      </button>
      <div class="ai-agent-select">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="8" r="4"/><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/></svg>
        <select bind:value={selectedMode} onchange={(e) => handleModeChange((e.target as HTMLSelectElement).value)}>
          <option value="auto">Auto (default)</option>
          {#each agents as agent}
            <option value={agent.id}>{agent.name} ({agent.provider})</option>
          {/each}
        </select>
      </div>
      <div class="ai-usage-gauge" title={`$${totalCost.toFixed(4)} · ${totalTokens.toLocaleString()} tokens · ${totalRequests} requests`}>
        <svg width="22" height="22" viewBox="0 0 44 44">
          <circle cx="22" cy="22" r="18" fill="none" stroke="var(--bg-hover)" stroke-width="4"/>
          <circle cx="22" cy="22" r="18" fill="none" stroke="var(--accent-blue)" stroke-width="4"
            stroke-dasharray={gaugeCircumference} stroke-dashoffset={gaugeCircumference * (1 - gaugeFraction)}
            transform="rotate(-90 22 22)" stroke-linecap="round"/>
        </svg>
        <span class="ai-usage-label">{totalTokens < 1000 ? `${totalTokens}` : `${(totalTokens / 1000).toFixed(1)}k`} tok</span>
      </div>
      <button class="ai-btn ai-btn-ghost" onclick={clearChat} title="Clear chat">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
      </button>
    </div>

    <div class="ai-messages" bind:this={messagesContainer}>
      {#each messages as msg, i}
        <div class="msg" class:msg-user={msg.role === "user"} class:msg-system={msg.role === "system"} class:msg-assistant={msg.role === "assistant"}>
          <div class="msg-header">
            <div class="msg-badge">{msg.role}</div>
            {#if msg.role === "assistant" && !(isStreaming && msg === messages[messages.length - 1] && msg.content === "")}
              <button class="msg-copy-btn" onclick={() => copyMessage(msg.content, i)} title="Copy message">
                {#if copiedMsgIndex === i}
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>
                {:else}
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
                {/if}
              </button>
            {/if}
          </div>
          <div class="msg-text" class:streaming={isStreaming && msg === messages[messages.length - 1] && msg.role === "assistant"}>
            {#if msg.role === "assistant"}
              {#if isStreaming && msg === messages[messages.length - 1]}
                {#if toolCalls.size > 0}
                  <div class="tool-calls">
                    {#each Array.from(toolCalls.entries()) as [id, tc]}
                      <div class="tool-call-item">
                        <span class="tool-call-name">{tc.name}</span>
                        <pre class="tool-call-args">{tc.args}</pre>
                        {#if tc.result !== "..."}
                          <pre class="tool-call-result">{tc.result}</pre>
                        {:else}
                          <span class="tool-call-pending">Executing...</span>
                        {/if}
                      </div>
                    {/each}
                  </div>
                {/if}
                {#if msg.content}
                  <div class="md-content">{@html markedParse(msg.content)}</div>
                {:else if toolCalls.size === 0}
                  <span class="thinking-text">Thinking<span class="dots"><span>.</span><span>.</span><span>.</span></span></span>
                {/if}
              {:else}
                <div class="md-content">{@html markedParse(msg.content)}</div>
              {/if}
            {:else}
              {msg.display_content || msg.content}
            {/if}
          </div>
        </div>
      {/each}
      <div class="scroll-sentinel" style="height:1px"></div>
    </div>

    {#if scrollBtnVisible}
      <button class="ai-scroll-btn" onclick={scrollBtnClick}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
        Scroll to bottom
      </button>
    {/if}

    {#if pendingPermission}
      <div class="permission-panel">
        <div class="permission-header">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent-yellow)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
          <span class="permission-title">Execute Command?</span>
        </div>
        <div class="permission-desc">
          Agent wants to run command in <span class="cwd-tag">{pendingPermission.cwd || "workspace root"}</span>:
        </div>
        <div class="command-box">
          <textarea bind:value={commandOverride} class="command-editor" rows={3}></textarea>
        </div>
        <div class="permission-actions">
          <button class="perm-btn perm-reject" onclick={() => respondPermission(false)}>Reject</button>
          <button class="perm-btn perm-accept" onclick={() => respondPermission(true)}>Accept & Run</button>
        </div>
      </div>
    {/if}

    {#if pendingFilePermission}
      <div class="permission-panel">
        <div class="permission-header">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 1 1 3 3L12 15l-4 1 1-4Z"/></svg>
          <span class="permission-title">{pendingFilePermission.is_edit ? "Approve Edit?" : "Approve Write File?"}</span>
        </div>
        <div class="permission-desc">
          Agent wants to {pendingFilePermission.is_edit ? "edit" : "create"} file:
        </div>
        <AIDiffViewer path={pendingFilePermission.path} isEdit={pendingFilePermission.is_edit} diff={pendingFilePermission.diff} />
        <div class="permission-actions">
          <button class="perm-btn perm-reject" onclick={() => respondFilePermission(false)}>Reject</button>
          <button class="perm-btn perm-accept" onclick={() => respondFilePermission(true)}>Accept & Write</button>
        </div>
      </div>
    {/if}

  </div>
</div>

<style>
  .ai-wrapper { display:flex; height:100%; position:relative; }
  .ai-sessions { width:220px; flex-shrink:0; border-right:1px solid var(--border-subtle); display:flex; flex-direction:column; background:var(--bg-primary); }
  .ai-sessions-header { display:flex; align-items:center; justify-content:space-between; padding:6px 8px; border-bottom:1px solid var(--border-subtle); }
  .ai-sessions-title { font-size:var(--fs-10); font-weight:600; text-transform:uppercase; letter-spacing:0.5px; color:var(--text-muted); }
  .ai-session-new { display:flex; align-items:center; gap:4px; padding:5px 8px; margin:4px 6px; background:var(--bg-hover); border:none; border-radius:4px; color:var(--text-primary); font-size:var(--fs-11); cursor:pointer; transition:all 0.12s ease; }
  .ai-session-new:hover { background:var(--accent-blue); color:var(--bg-primary); }
  .ai-sessions-list { flex:1; overflow-y:auto; padding:2px 0; }
  .ai-session-item { display:flex; align-items:center; gap:4px; padding:5px 8px; margin:1px 6px; border-radius:4px; cursor:pointer; transition:all 0.12s ease; }
  .ai-session-item:hover { background:var(--bg-hover); }
  .ai-session-item.active { background:color-mix(in srgb, var(--accent-blue) 12%, transparent); }
  .ai-session-info { flex:1; min-width:0; }
  .ai-session-name { display:block; font-size:var(--fs-11); color:var(--text-primary); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .ai-session-date { display:block; font-size:var(--fs-9); color:var(--text-muted); }
  .ai-session-del { background:none; border:none; padding:2px; color:var(--text-muted); cursor:pointer; border-radius:3px; flex-shrink:0; opacity:0; transition:all 0.12s ease; }
  .ai-session-item:hover .ai-session-del { opacity:0.6; }
  .ai-session-del:hover { opacity:1 !important; color:var(--accent-red); background:var(--bg-hover); }
  .ai-sessions-empty { padding:12px 8px; text-align:center; color:var(--text-muted); font-size:var(--fs-10); }
  .ai-chat { flex:1; display:flex; flex-direction:column; min-width:0; background:transparent; color:var(--text-primary); font-size:var(--font-size); }
  .ai-toolbar { display:flex; align-items:center; gap:6px; padding:6px 10px; border-bottom:1px solid var(--border-subtle); flex-shrink:0; }
  .ai-agent-select { display:flex; align-items:center; gap:6px; flex:1; color:var(--text-muted); }
  .ai-agent-select select { flex:1; background:var(--bg-primary); color:var(--text-primary); border:1px solid var(--border-subtle); border-radius:4px; padding:3px 6px; font-size:var(--fs-11); }
  .ai-btn { display:inline-flex; align-items:center; justify-content:center; gap:4px; border:none; border-radius:4px; padding:4px 10px; font-size:var(--fs-11); cursor:pointer; transition:all 0.12s ease; font-weight:500; }
  .ai-btn-ghost { background:none; color:var(--text-muted); padding:4px; }
  .ai-btn-ghost:hover { background:var(--bg-hover); }
  .ai-btn-send { background:var(--accent-blue); color:var(--bg-primary); padding:6px; border-radius:6px; align-self:flex-end; }
  .ai-btn-send:disabled { opacity:0.3; cursor:not-allowed; }
  .ai-btn-send:hover:not(:disabled) { filter:brightness(1.1); }
  .ai-messages { flex:1; overflow-y:auto; padding:8px 10px; display:flex; flex-direction:column; gap:8px; }
  .msg { padding:8px 12px; border-radius:8px; max-width:92%; word-wrap:break-word; white-space:pre-wrap; animation:fadeIn 0.15s ease; }
  @keyframes fadeIn { from { opacity:0; transform:translateY(4px); } to { opacity:1; transform:translateY(0); } }
  .msg-user { background:var(--accent-blue); color:var(--bg-primary); align-self:flex-end; }
  .msg-assistant { background:var(--bg-surface); border:1px solid var(--border-subtle); align-self:flex-start; }
  .msg-system { background:var(--bg-elevated); color:var(--text-muted); align-self:center; font-size:var(--fs-10); font-style:italic; padding:4px 12px; border-radius:12px; }
  .msg-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 4px; gap: 8px; }
  .msg-header .msg-badge { margin-bottom: 0; }
  .msg-copy-btn { background: none; border: none; padding: 2px 4px; border-radius: 4px; cursor: pointer; color: var(--text-muted); display: inline-flex; align-items: center; justify-content: center; transition: all 0.12s ease; opacity: 0.6; }
  .msg-copy-btn:hover { color: var(--text-primary); background: var(--bg-hover); opacity: 1; }
  .msg-badge { font-size:var(--fs-9); text-transform:uppercase; opacity:0.5; margin-bottom:4px; font-weight:600; letter-spacing:0.5px; }
  .msg-text { font-size:var(--font-size); line-height:1.6; user-select: text; -webkit-user-select: text; }
  .streaming { opacity:0.7; }
  .thinking-text { opacity:0.7; }
  .dots span { animation:dotAnim 1.4s infinite; opacity:0; }
  .dots span:nth-child(2) { animation-delay:0.2s; }
  .dots span:nth-child(3) { animation-delay:0.4s; }
  @keyframes dotAnim { 0%,60%,100% { opacity:0; } 30% { opacity:1; } }
  .tool-calls { display:flex; flex-direction:column; gap:6px; }
  .tool-call-item { background:var(--bg-primary); border:1px solid var(--border-subtle); border-radius:6px; padding:6px 8px; font-size:var(--fs-10); }
  .tool-call-name { font-weight:600; color:var(--accent-blue); display:block; margin-bottom:2px; }
  .tool-call-args { font-size:var(--fs-9); margin:2px 0; padding:4px; background:var(--bg-hover); border-radius:4px; max-height:100px; overflow:auto; white-space:pre-wrap; }
  .tool-call-result { font-size:var(--fs-9); margin:2px 0; padding:4px; background:color-mix(in srgb, var(--accent-green) 6%, transparent); border-radius:4px; max-height:100px; overflow:auto; white-space:pre-wrap; }
  .tool-call-pending { color:var(--text-muted); font-style:italic; font-size:var(--fs-9); }
  .ai-input { display:flex; gap:6px; padding:8px 10px; border-top:1px solid var(--border-subtle); flex-shrink:0; }
  .ai-input textarea { flex:1; background:var(--bg-primary); color:var(--text-primary); border:1px solid var(--border-subtle); border-radius:8px; padding:8px; font-size:var(--font-size); resize:none; }
  .ai-input textarea:focus { outline:none; border-color:var(--accent-blue); }
  .ai-messages::-webkit-scrollbar { width:4px; }
  .ai-messages::-webkit-scrollbar-thumb { background:var(--bg-hover); border-radius:2px; }
  .ai-usage-gauge { display:flex; align-items:center; gap:4px; cursor:default; flex-shrink:0; }
  .ai-usage-label { font-size:var(--fs-9); color:var(--text-muted); white-space:nowrap; }
  .ai-scroll-btn { position:absolute; bottom:48px; left:50%; transform:translateX(-50%); display:inline-flex; align-items:center; gap:4px; background:var(--bg-elevated); border:1px solid var(--border-subtle); border-radius:12px; padding:4px 10px; font-size:var(--fs-10); color:var(--text-muted); cursor:pointer; z-index:10; box-shadow:0 2px 8px rgba(0,0,0,0.15); transition:all 0.12s ease; }
  .ai-scroll-btn:hover { color:var(--text-primary); border-color:var(--accent-blue); }
  .ai-chat { position:relative; }
  .md-content { line-height:1.6; user-select: text; -webkit-user-select: text; }
  .md-content p { margin:0 0 6px 0; }
  .md-content p:last-child { margin-bottom:0; }
  .md-content pre { background:var(--bg-primary); border:1px solid var(--border-subtle); border-radius:6px; padding:8px 10px; overflow-x:auto; margin:6px 0; font-size:var(--fs-11); }
  .md-content code { background:var(--bg-hover); padding:1px 4px; border-radius:3px; font-size:var(--fs-11); }
  .md-content pre code { background:none; padding:0; border-radius:0; }
  .md-content ul, .md-content ol { padding-left:18px; margin:4px 0; }
  .md-content li { margin:2px 0; }
  .md-content h1, .md-content h2, .md-content h3, .md-content h4 { margin:8px 0 4px 0; font-weight:600; color:var(--text-primary); }
  .md-content h1 { font-size:1.1em; } .md-content h2 { font-size:1.05em; } .md-content h3 { font-size:1em; }
  .md-content blockquote { border-left:3px solid var(--accent-blue); padding-left:8px; margin:6px 0; color:var(--text-muted); }
  .md-content table { border-collapse:collapse; margin:6px 0; font-size:var(--fs-11); width:100%; }
  .md-content th, .md-content td { border:1px solid var(--border-subtle); padding:4px 8px; text-align:left; }
  .md-content th { background:var(--bg-hover); font-weight:600; }
  .md-content a { color:var(--accent-blue); text-decoration:none; }
  .md-content a:hover { text-decoration:underline; }
  .md-content img { max-width:100%; border-radius:4px; margin:4px 0; }
  .md-content hr { border:none; border-top:1px solid var(--border-subtle); margin:8px 0; }

  .permission-panel {
    margin: 8px 10px;
    padding: 10px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-primary);
    border-radius: 8px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.35);
    display: flex;
    flex-direction: column;
    gap: 6px;
    animation: slideUp 0.2s ease;
  }
  @keyframes slideUp { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }
  .permission-header { display: flex; align-items: center; gap: 6px; }
  .permission-title { font-weight: 600; font-size: var(--fs-11); color: var(--text-primary); }
  .permission-desc { font-size: var(--fs-10); color: var(--text-muted); }
  .cwd-tag { font-family: monospace; background: var(--bg-hover); padding: 1px 4px; border-radius: 3px; color: var(--accent-cyan); }
  .command-box { width: 100%; }
  .command-editor {
    width: 100%;
    background: var(--bg-primary);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    padding: 6px;
    font-family: monospace;
    font-size: var(--fs-10);
    resize: none;
    outline: none;
    box-sizing: border-box;
  }
  .command-editor:focus { border-color: var(--accent-blue); }
  .permission-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 2px; }
  .perm-btn {
    border: none;
    border-radius: 4px;
    padding: 5px 12px;
    font-size: var(--fs-10);
    font-weight: 600;
    cursor: pointer;
    transition: all 0.12s ease;
  }
  .perm-reject { background: var(--bg-hover); color: var(--text-primary); }
  .perm-reject:hover { background: var(--accent-red); color: var(--bg-primary); }
  .perm-accept { background: var(--accent-blue); color: var(--bg-primary); }
  .perm-accept:hover { filter: brightness(1.15); }
</style>
