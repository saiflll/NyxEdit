<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { marked } from "marked";
  import { aiSendRequest, type Agent, type ChatMessage, type ChatSession, type AiToolCallEvent, type AiToolResultEvent, currentDir, activeTerminalSessionId, activeFile, fileContent, agents as globalAgents, addToast } from "../stores.svelte";
  import { getIdleState } from "$lib/idle.svelte";
  import { get } from "svelte/store";
  import AIDiffViewer from "./AIDiffViewer.svelte";
  import { getActiveSettings } from "$lib/nyxConfig";

  interface Props {
    onOpenDiff?: (msgIdx: number, changes: { path: string; oldContent: string; newContent: string }[]) => void;
    onMinimize?: () => void;
    onClose?: () => void;
    onFull?: () => void;
    onOpenInTab?: (content: string, label: string) => void;
  }
  let { onOpenDiff, onMinimize, onClose, onFull, onOpenInTab }: Props = $props();

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
  let isDropupOpen = $state(false);
  let isStreaming = $state(false);
  let streamingAgent = $state("");
  let agentList = $state<{ id: string; label: string }[]>([]);
  let autoFallbackQueue = $state<{ agent: Agent; model: string }[]>([]);
  let messagesContainer: HTMLDivElement | undefined = $state(undefined);
  let scrollBtnVisible = $state(false);
  let usageData = $state<AiUsage[]>([]);

  // Auto routing and classification states / helpers
  let currentAutoTier = $state<"simple" | "medium" | "complex" | null>(null);

  type RequestTier = "simple" | "medium" | "complex";

  function parseMessageMeta(content: string) {
    if (!content.startsWith("[")) {
      return { isAuto: false, tier: "", model: "", cleanContent: content };
    }
    
    const closingBracket = content.indexOf("]\n");
    if (closingBracket === -1) {
      return { isAuto: false, tier: "", model: "", cleanContent: content };
    }
    
    const metaStr = content.slice(1, closingBracket);
    const cleanContent = content.slice(closingBracket + 2);
    
    if (metaStr.startsWith("Auto [")) {
      const tierEnd = metaStr.indexOf("]");
      if (tierEnd !== -1) {
        const tier = metaStr.slice(6, tierEnd);
        const model = metaStr.slice(tierEnd + 2).trim();
        return { isAuto: true, tier, model, cleanContent };
      }
    }
    
    return { isAuto: false, tier: "", model: metaStr, cleanContent };
  }

  function cleanUserDisplayContent(content: string): string {
    let clean = content;
    
    if (clean.includes("[Global Custom Instructions]\n")) {
      const startIdx = clean.indexOf("[Global Custom Instructions]\n");
      const endIdx = clean.indexOf("\n\n", startIdx);
      if (endIdx !== -1) {
        clean = clean.slice(endIdx + 2);
      }
    }
    
    if (clean.includes("[Agent Skills Toggles]\n")) {
      const startIdx = clean.indexOf("[Agent Skills Toggles]\n");
      const endIdx = clean.indexOf("\n\n", startIdx);
      if (endIdx !== -1) {
        clean = clean.slice(endIdx + 2);
      }
    }
    
    if (clean.includes("[Active Editor Context - File: ")) {
      const startIdx = clean.indexOf("[Active Editor Context - File: ");
      const endIdx = clean.indexOf("```\n\n", startIdx);
      if (endIdx !== -1) {
        clean = clean.slice(endIdx + 5);
      } else {
        const altEndIdx = clean.indexOf("```", startIdx + 30);
        if (altEndIdx !== -1) {
          clean = clean.slice(altEndIdx + 3);
        }
      }
    }
    
    const attachedIdx = clean.indexOf("\n\n---\n[Attached File:");
    if (attachedIdx !== -1) {
      clean = clean.slice(0, attachedIdx);
    }
    const attachedRefIdx = clean.indexOf("\n\n---\n[Attached File Reference:");
    if (attachedRefIdx !== -1) {
      clean = clean.slice(0, attachedRefIdx);
    }
    
    return clean.trim();
  }

  function getMsgBadges(msg: ChatMessage, isLast: boolean) {
    if (msg.role !== "assistant") return null;
    
    if (isLast && isStreaming) {
      return {
        isAuto: selectedMode === "auto",
        tier: selectedMode === "auto" ? currentAutoTier : null,
        model: streamingAgent ? streamingAgent.replace(/^Auto \[[a-z]+\] /, '') : ""
      };
    }
    
    const meta = parseMessageMeta(msg.content);
    if (meta.model) {
      return {
        isAuto: meta.isAuto,
        tier: meta.isAuto ? meta.tier : null,
        model: meta.model
      };
    }
    
    return null;
  }

  function classifyFrontend(text: string): RequestTier {
    const wordCount = text.trim().split(/\s+/).length;
    const complexKw = /\b(debug|fix|implement|refactor|architect|design|analyze|optimize|reasoning|algorithm|performance|security|database|migration|integration|multi.step|step.by.step)\b/i;
    const reasoningKw = /\b(proof|derive|calculate|theorem|equation|math|statistics|logic|infer|deduce|explain why|why does|how does)\b/i;
    const codeBlock = /```|\bfunction\b|\bclass\b|\bdef\b|\basync\b|\bimport\b|\bexport\b/;
    
    if (wordCount < 12 && !complexKw.test(text) && !codeBlock.test(text)) return "simple";
    if (reasoningKw.test(text) || wordCount > 80 || (codeBlock.test(text) && complexKw.test(text))) return "complex";
    return "medium";
  }

  const TIER_PATTERNS = {
    simple:  /flash|haiku|mini|8b|lite|turbo|fast|small|phi|nano/i,
    medium:  /sonnet|gpt-4o(?!-mini)|mistral-large|70b|medium|llama-3\.1|plus|coder/i,
    complex: /opus|gpt-4(?!o)|o3|reasoner|pro(?!-vision)|deepseek-r|r1|thinking|max/i,
  };

  /**
   * Selects the best agent AND the specific model to use for the given tier.
   * Priority:
   * 1. Scan each agent's `cached_models` for a model matching the tier pattern.
   * 2. Fallback across tiers within cached_models.
   * 3. Last resort: use agent.model (default) with no model override.
   */
  function selectAgentAndModelForTier(agents: Agent[], tier: RequestTier): { agent: Agent; model: string } | null {
    const pool = [...agents].filter(a => a.api_key || ["ollama", "other"].includes(a.provider));
    if (pool.length === 0) return null;

    // 1. Find an agent whose cached_models contains a model matching the preferred tier
    for (const agent of pool) {
      const models = agent.cached_models || [];
      const match = models.find(m => TIER_PATTERNS[tier].test(m));
      if (match) return { agent, model: match };
    }

    // 2. Fallback: try other tiers within cached_models
    const tierOrder: RequestTier[] = ["simple", "medium", "complex"];
    for (const t of tierOrder) {
      if (t === tier) continue;
      for (const agent of pool) {
        const models = agent.cached_models || [];
        const match = models.find(m => TIER_PATTERNS[t].test(m));
        if (match) return { agent, model: match };
      }
    }

    // 3. Last resort: use first agent's default model
    return { agent: pool[0], model: pool[0].model };
  }

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
  let unlistenFileChanged: UnlistenFn | null = null;

  let pendingPermission = $state<{ id: string; command: string; cwd: string } | null>(null);
  let commandOverride = $state("");
  let pendingFilePermission = $state<{ id: string; path: string; is_edit: boolean; diff: any[] } | null>(null);
  let activeDiffIndex = $state<number | null>(null);
  let diffData = $state<Record<number, { path: string; diff: any[] }[]>>({});

  let totalCost = $derived(usageData.reduce((s, u) => s + u.total_cost, 0));
  let totalTokens = $derived(usageData.reduce((s, u) => s + u.total_input_tokens + u.total_output_tokens, 0));
  let totalRequests = $derived(usageData.reduce((s, u) => s + u.total_requests, 0));
  let gaugeFraction = $derived(Math.min(totalCost / 1.0, 1));

  // Quota popover state
  let quotaHover = $state(false);
  let quotaLocked = $state(false);
  let quotaTimer: ReturnType<typeof setTimeout> | null = null;
  function toggleQuotaLock() { quotaLocked = !quotaLocked; if (quotaLocked) quotaHover = true; }
  function showQuota() { if (quotaTimer) clearTimeout(quotaTimer); if (!quotaLocked) quotaHover = true; }
  function hideQuota() { if (quotaTimer) clearTimeout(quotaTimer); quotaTimer = setTimeout(() => { if (!quotaLocked) quotaHover = false; }, 200); }

  async function loadAgents() {
    agents = await invoke<Agent[]>("ai_list_agents");
    globalAgents.set(agents);
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
    unlistenFileChanged?.(); unlistenFileChanged = null;
  }

  async function saveCurrentSession() {
    const msgs = messages.filter(m => m.role !== "system");
    if (msgs.length === 0) return;
    const now = new Date().toISOString();
    const firstUser = msgs.find(m => m.role === "user");
    const existing = currentSessionId ? sessions.find(s => s.id === currentSessionId) : null;
    const name = existing ? existing.name : (firstUser ? summarizeFirstMessage(firstUser.content) : "Chat");
    const session: ChatSession = {
      id: currentSessionId || crypto.randomUUID(),
      name,
      agent_id: selectedAgentId,
      messages: msgs,
      created_at: existing ? existing.created_at : now,
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
    messages = session.messages.map(m => ({ role: m.role, content: m.content, display_content: m.display_content }));
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

  let renameSessionId = $state<string | null>(null);
  let renameSessionValue = $state("");
  function startRename(id: string, currentName: string) {
    renameSessionId = id;
    renameSessionValue = currentName;
  }
  async function commitRename() {
    if (!renameSessionId || !renameSessionValue.trim()) { renameSessionId = null; return; }
    const session = sessions.find(s => s.id === renameSessionId);
    if (session) {
      session.name = renameSessionValue.trim();
      try {
        await invoke("ai_save_session", { session });
        await loadSessions();
      } catch { /* ignore */ }
    }
    renameSessionId = null;
  }
  function cancelRename() { renameSessionId = null; }

  function summarizeFirstMessage(text: string): string {
    let clean = text.replace(/\[.*?\]\s*/g, '').trim();
    clean = clean.split('\n')[0].trim();
    if (clean.length > 45) clean = clean.slice(0, 42) + '...';
    return clean || 'Chat';
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

  async function tryStream(agentId: string, label: string, fallbackQueue: { agent: Agent; model: string }[], modelOverride?: string) {
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

    unlistenFileChanged = await listen<{ id: string; path: string; old_content: string; new_content: string }>("ai:file_changed", (e) => {
      const payload = e.payload;
      const last = messages[messages.length - 1];
      if (last && last.role === "assistant") {
        if (!last.fileChanges) last.fileChanges = [];
        const exists = last.fileChanges.some(x => x.path === payload.path);
        if (!exists) {
          last.fileChanges.push({
            path: payload.path,
            oldContent: payload.old_content,
            newContent: payload.new_content
          });
          messages = [...messages];
        }
      }
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
        const nextLabel = selectedMode === "auto"
          ? `Auto [${currentAutoTier}] ${next.agent.provider}/${next.model}`
          : `${next.agent.provider}/${next.model}`;
        await tryStream(next.agent.id, nextLabel, fallbackQueue, next.model);
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
      activeSessionId: activeSession || null,
      modelOverride: modelOverride || null,
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

    // Read global custom instructions & skills using file-based config system
    const activeSettings = await getActiveSettings();
    const globalInstructions = activeSettings.globalInstructions;
    const skillRead = activeSettings.skillRead;
    const skillWrite = activeSettings.skillWrite;
    const skillTerminal = activeSettings.skillTerminal;

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
        // Step 1: Frontend classification (instant)
        const frontendTier = classifyFrontend(promptText);
        currentAutoTier = frontendTier;
        
        // Step 2: Select agent + model for tier (scans cached_models)
        const chosen = selectAgentAndModelForTier(agents, frontendTier);
        if (!chosen) {
          messages = [...messages, { role: "assistant", content: "No agent with API key configured. Add one in Settings > Agents." }];
          isStreaming = false;
          return;
        }
        
        // Setup fallback queue: other agents or other models from same agent at adjacent tiers
        const candidates = agents
          .filter(a => a.id !== chosen.agent.id && (a.api_key || a.provider === "ollama"))
          .map(a => ({ agent: a, model: a.model }));
        autoFallbackQueue = candidates;
        
        // Step 3: Stream request using the chosen agent + specific model
        const label = `Auto [${frontendTier}] ${chosen.agent.provider}/${chosen.model}`;
        await tryStream(chosen.agent.id, label, candidates, chosen.model);
        
        // Step 4: Backend classification async (does not block UX)
        invoke<any>("ai_classify_request", {
          text: promptText,
          frontendTier
        }).then(res => {
          if (res.overrode_frontend) {
            console.log(`[AutoTier] Backend override: ${frontendTier} -> ${res.tier} (${res.reason})`);
          }
        }).catch(err => {
          console.error("Backend classification error:", err);
        });
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
  export function clearChat() { messages = []; currentSessionId = null; }

  async function undoMessageChanges(msg: ChatMessage) {
    if (!msg.fileChanges || msg.fileChanges.length === 0) return;
    try {
      for (const change of msg.fileChanges) {
        await invoke("fs_write_file", { path: change.path, content: change.oldContent });
      }
      addToast("Successfully reverted file changes!", "success");
    } catch (e) {
      console.error(e);
      addToast("Failed to undo changes", "error");
    }
  }

  async function toggleDiffView(index: number) {
    if (activeDiffIndex === index) {
      activeDiffIndex = null;
      return;
    }
    const msg = messages[index];
    if (!msg.fileChanges || msg.fileChanges.length === 0) return;
    
    if (!diffData[index]) {
      const results: { path: string; diff: any[] }[] = [];
      for (const change of msg.fileChanges) {
        try {
          const diff = await invoke<any[]>("ai_compute_diff", {
            oldContent: change.oldContent,
            newContent: change.newContent
          });
          results.push({ path: change.path, diff });
        } catch (e) {
          console.error("Failed to compute diff:", e);
        }
      }
      diffData[index] = results;
    }
    activeDiffIndex = index;
  }

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
          <div class="ai-session-item" class:active={currentSessionId === session.id} onclick={() => { if (renameSessionId !== session.id) loadSession(session); }}>
            <div class="ai-session-info">
              {#if renameSessionId === session.id}
                <input class="ai-session-rename" bind:value={renameSessionValue}
                  onkeydown={(e) => { if (e.key === 'Enter') commitRename(); if (e.key === 'Escape') cancelRename(); }}
                  onblur={commitRename}
                  autofocus
                  onclick={(e) => e.stopPropagation()}
                />
              {:else}
                <span class="ai-session-name">{session.name}</span>
              {/if}
              <span class="ai-session-date">{formatDate(session.updated_at)}</span>
            </div>
            <button class="ai-session-del ai-session-rename-btn" onclick={(e) => { e.stopPropagation(); startRename(session.id, session.name); }} title="Rename session">
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
            </button>
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
    <div class="ai-header">
      <button class="ai-btn ai-btn-ghost" onclick={() => sessionsOpen = !sessionsOpen} title="Sessions">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
      </button>
      <div class="ai-header-spacer"></div>
      <button class="ai-btn ai-btn-ghost" onclick={clearChat} title="Clear chat">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
      </button>
      <button class="ai-btn ai-btn-ghost" onclick={onMinimize} title="Minimize">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="5" y1="12" x2="19" y2="12"/></svg>
      </button>
      <button class="ai-btn ai-btn-ghost" onclick={onFull} title="Open as Tab">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3"/></svg>
      </button>
      <button class="ai-btn ai-btn-ghost ai-btn-close" onclick={onClose} title="Close">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="6" y1="6" x2="18" y2="18"/><line x1="6" y1="18" x2="18" y2="6"/></svg>
      </button>
    </div>

    <div class="ai-messages" bind:this={messagesContainer}>
      {#each messages as msg, i}
        {@const meta = parseMessageMeta(msg.content)}
        <div class="msg" class:msg-user={msg.role === "user"} class:msg-system={msg.role === "system"} class:msg-assistant={msg.role === "assistant"}>
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
                  <div class="md-content">{@html markedParse(meta.cleanContent)}</div>
                {:else if toolCalls.size === 0}
                  <span class="thinking-text">Thinking<span class="dots"><span>.</span><span>.</span><span>.</span></span></span>
                {/if}
              {:else}
                <div class="md-content">{@html markedParse(meta.cleanContent)}</div>
              {/if}
            {:else}
              {msg.display_content || cleanUserDisplayContent(msg.content)}
            {/if}
          </div>
          {#if msg.role === "assistant"}
            {@const badges = getMsgBadges(msg, i === messages.length - 1)}
            <div class="msg-footer" style="display: flex; align-items: center; justify-content: space-between; margin-top: 1px; gap: 4px;">
              <div class="msg-actions" style="display: flex; align-items: center; gap: 6px;">
                <button class="msg-action-btn" onclick={() => copyMessage(meta.cleanContent, i)} title="Copy Response">
                  {#if copiedMsgIndex === i}
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>
                  {:else}
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
                  {/if}
                </button>
                {#if msg.fileChanges && msg.fileChanges.length > 0}
                  <button class="msg-action-btn" onclick={() => undoMessageChanges(msg)} title="Undo Changes">
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7v6h6"/><path d="M21 17a9 9 0 0 0-9-9 9 9 0 0 0-6 2.3L3 13"/></svg>
                  </button>
                  <button class="msg-action-btn" onclick={() => onOpenDiff?.(i, msg.fileChanges || [])} title="Show Diff">
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="18" rx="1"/><rect x="14" y="3" width="7" height="18" rx="1"/></svg>
                  </button>
                {/if}
                {#if msg.role === "assistant" && !isStreaming}
                  <button class="msg-action-btn" onclick={() => onOpenInTab?.(meta.cleanContent, "AI Plan")} title="Open in Tab">
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3"/></svg>
                  </button>
                {/if}
              </div>
              <div class="msg-footer-badges" style="display: flex; align-items: center; gap: 4px; flex-wrap: wrap;">
                {#if badges}
                  {#if badges.isAuto}
                    <span class="mode-badge auto-badge" style="font-size: var(--fs-9); padding: 2px 6px; border-radius: 4px; background: color-mix(in srgb, var(--accent-blue) 12%, transparent); color: var(--accent-blue); font-weight: 600;">Auto</span>
                    {#if badges.tier === "simple"}
                      <span class="tier-badge simple-badge" style="font-size: var(--fs-9); padding: 2px 6px; border-radius: 4px; background: color-mix(in srgb, var(--accent-green) 12%, transparent); color: var(--accent-green); font-weight: 600;">💡 simple</span>
                    {:else if badges.tier === "medium"}
                      <span class="tier-badge medium-badge" style="font-size: var(--fs-9); padding: 2px 6px; border-radius: 4px; background: color-mix(in srgb, var(--accent-yellow) 12%, transparent); color: var(--accent-yellow); font-weight: 600;">🔧 medium</span>
                    {:else if badges.tier === "complex"}
                      <span class="tier-badge complex-badge" style="font-size: var(--fs-9); padding: 2px 6px; border-radius: 4px; background: color-mix(in srgb, var(--accent-purple) 12%, transparent); color: var(--accent-purple); font-weight: 600;">🧠 complex</span>
                    {/if}
                  {:else}
                    <span class="mode-badge manual-badge" style="font-size: var(--fs-9); padding: 2px 6px; border-radius: 4px; background: var(--bg-hover); color: var(--text-secondary); font-weight: 600;">Manual</span>
                  {/if}
                  {#if badges.model}
                    <span class="model-badge" style="font-size: var(--fs-9); padding: 2px 6px; border-radius: 4px; background: var(--bg-primary); border: 1px solid var(--border-subtle); color: var(--text-muted); font-family: monospace;">{badges.model.split('/').pop()}</span>
                  {/if}
                {/if}
              </div>
            </div>
          {/if}
        </div>
      {/each}
      {#if pendingPermission}
        <div class="permission-panel inline-permission">
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
        <div class="permission-panel inline-permission">
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

      <div class="scroll-sentinel" style="height:1px"></div>
    </div>

    {#if scrollBtnVisible}
      <button class="ai-scroll-btn" onclick={scrollBtnClick}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
        Scroll to bottom
      </button>
    {/if}

    <div class="ai-footer" onclick={() => { isDropupOpen = false; }}>
      <div class="ai-footer-row">
        <button class="ai-mode-btn" onclick={(e) => { e.stopPropagation(); isDropupOpen = !isDropupOpen; }} title="Switch agent">
          <span class="ai-mode-dot" class:auto={selectedMode === "auto"}></span>
          <span class="ai-mode-label">{selectedMode === "auto" ? "Auto" : agentList.find(a => a.id === selectedMode)?.label || selectedMode}</span>
          <svg width="8" height="8" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="6 9 12 15 18 9"/></svg>
        </button>
        <span class="ai-usage-label">{totalTokens < 1000 ? `${totalTokens}` : `${(totalTokens / 1000).toFixed(1)}k`} tok</span>
      </div>
      <div id="ai-mode-dropup" class="ai-mode-dropup" class:open={isDropupOpen}>
        <button class="ai-mode-opt" class:selected={selectedMode === "auto"} onclick={() => { handleModeChange("auto"); isDropupOpen = false; }}>
          <span class="ai-mode-dot auto"></span>
          <span>Auto (default)</span>
        </button>
        {#each agents as agent}
          <button class="ai-mode-opt" class:selected={selectedMode === agent.id} onclick={() => { handleModeChange(agent.id); isDropupOpen = false; }}>
            <span class="ai-mode-dot"></span>
            <span>{agent.name}</span>
            <span class="ai-mode-prov">{agent.provider}</span>
          </button>
        {/each}
      </div>
      <div class="ai-usage-bar-wrap"
        onmouseenter={showQuota}
        onmouseleave={hideQuota}
      >
        <div class="ai-usage-bar-container"
          onclick={toggleQuotaLock}
          role="button"
          tabindex="0"
          title={`$${totalCost.toFixed(4)} · ${totalTokens.toLocaleString()} tokens`}
        >
          <div class="ai-usage-bar-fill" style="width: {gaugeFraction * 100}%"></div>
        </div>
        {#if quotaHover || quotaLocked}
          <div class="ai-quota-popover">
            <div class="ai-quota-title">Active Agents & Usage</div>
            {#each usageData.filter(u => u.total_requests > 0) as usage}
              <div class="ai-quota-item">
                <div class="ai-quota-agent">{usage.agent_name}</div>
                <div class="ai-quota-model">{usage.provider}/{usage.model}</div>
                <div class="ai-quota-detail">{usage.total_requests} req · {usage.total_input_tokens + usage.total_output_tokens} tok · ${usage.total_cost.toFixed(6)}</div>
              </div>
            {:else}
              <div class="ai-quota-empty">No usage data yet</div>
            {/each}
          </div>
        {/if}
      </div>
    </div>

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
  .ai-session-name { display:block; font-size:var(--fs-11); color:var(--text-primary); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; cursor:text; }
  .ai-session-name:hover { color:var(--accent-blue); }
  .ai-session-rename { width:100%; background:var(--bg-primary); color:var(--text-primary); border:1px solid var(--accent-blue); border-radius:3px; padding:1px 4px; font-size:var(--fs-11); font-family:inherit; box-sizing:border-box; }
  .ai-session-rename:focus { outline:none; }
  .ai-session-date { display:block; font-size:var(--fs-9); color:var(--text-muted); }
  .ai-session-del { background:none; border:none; padding:2px; color:var(--text-muted); cursor:pointer; border-radius:3px; flex-shrink:0; opacity:0; transition:all 0.12s ease; }
  .ai-session-item:hover .ai-session-del { opacity:0.6; }
  .ai-session-del:hover { opacity:1 !important; background:var(--bg-hover); }
  .ai-session-del.ai-session-rename-btn:hover { color:var(--accent-blue); }
  .ai-session-del:not(.ai-session-rename-btn):hover { color:var(--accent-red); }
  .ai-sessions-empty { padding:12px 8px; text-align:center; color:var(--text-muted); font-size:var(--fs-10); }
  .ai-chat { flex:1; display:flex; flex-direction:column; min-width:0; background:transparent; color:var(--text-primary); font-size:var(--font-size); position:relative; }
  .ai-header { display:flex; align-items:center; gap:2px; padding:4px 6px; border-bottom:1px solid var(--border-subtle); flex-shrink:0; }
  .ai-header-spacer { flex:1; }
  .ai-btn { display:inline-flex; align-items:center; justify-content:center; gap:3px; border:none; border-radius:3px; padding:3px 6px; font-size:var(--fs-10); cursor:pointer; transition:all 0.12s ease; font-weight:500; }
  .ai-btn-ghost { background:none; color:var(--text-muted); padding:3px; width:22px; height:22px; }
  .ai-btn-ghost:hover { background:var(--bg-hover); color:var(--text-primary); }
  .ai-btn-close:hover { color:var(--accent-red); background:color-mix(in srgb, var(--accent-red) 12%, transparent); }
  .ai-messages { flex:1; overflow-y:auto; overflow-x:hidden; padding:3px 4px; display:flex; flex-direction:column; gap:3px; }
  .msg { width: fit-content; padding:3px 7px 1px 7px; border-radius:5px; max-width:92%; min-width:0; word-wrap:break-word; word-break:break-word; overflow-wrap:anywhere; white-space:pre-wrap; animation:fadeIn 0.12s ease; box-sizing: border-box; }
  @keyframes fadeIn { from { opacity:0; transform:translateY(3px); } to { opacity:1; transform:translateY(0); } }
  .msg-user { background:var(--accent-blue); color:var(--bg-primary); align-self:flex-end; padding-bottom:3px; }
  .msg-assistant { background:var(--bg-surface); border:1px solid var(--border-subtle); align-self:flex-start; }
  .msg-system { background:var(--bg-elevated); color:var(--text-muted); align-self:center; font-size:var(--fs-10); font-style:italic; padding:2px 8px; border-radius:8px; }
  .msg-action-btn { background: none; border: none; padding: 2px 3px; border-radius: 3px; cursor: pointer; color: var(--text-muted); display: inline-flex; align-items: center; justify-content: center; transition: all 0.12s ease; opacity: 0.5; }
  .msg-action-btn:hover { color: var(--text-primary); background: var(--bg-hover); opacity: 1; }
  .msg-text { font-size:var(--font-size); line-height:1.4; user-select: text; -webkit-user-select: text; word-wrap:break-word; word-break:break-word; overflow-wrap:anywhere; }
  .msg-assistant .msg-text { white-space:normal; }
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
  .ai-messages::-webkit-scrollbar { width:4px; }
  .ai-messages::-webkit-scrollbar-thumb { background:var(--bg-hover); border-radius:2px; }
  .ai-usage-label { font-size:var(--fs-9); color:var(--text-muted); white-space:nowrap; flex-shrink:0; }
  .ai-scroll-btn { position:absolute; bottom:48px; left:50%; transform:translateX(-50%); display:inline-flex; align-items:center; gap:4px; background:var(--bg-elevated); border:1px solid var(--border-subtle); border-radius:12px; padding:4px 10px; font-size:var(--fs-10); color:var(--text-muted); cursor:pointer; z-index:10; box-shadow:0 2px 8px rgba(0,0,0,0.15); transition:all 0.12s ease; }
  .ai-scroll-btn:hover { color:var(--text-primary); border-color:var(--accent-blue); }
  .ai-chat { position:relative; }

  /* ─── AI Footer ─── */
  .ai-footer { flex-shrink:0; border-top:1px solid var(--border-subtle); background:var(--bg-surface); position:relative; }
  .ai-footer-row { display:flex; align-items:center; gap:6px; padding:4px 6px; }
  .ai-mode-btn { display:inline-flex; align-items:center; gap:4px; background:var(--bg-primary); border:1px solid var(--border-subtle); border-radius:4px; padding:2px 6px; cursor:pointer; font-size:var(--fs-10); color:var(--text-muted); transition:all 0.12s ease; }
  .ai-mode-btn:hover { border-color:var(--accent-blue); color:var(--text-primary); }
  .ai-mode-dot { width:7px; height:7px; border-radius:50%; background:var(--text-muted); flex-shrink:0; }
  .ai-mode-dot.auto { background:var(--accent-blue); }
  .ai-mode-label { font-weight:500; }
  .ai-mode-dropup { display:none; position:absolute; bottom:100%; left:6px; right:6px; background:var(--bg-elevated); border:1px solid var(--border-primary); border-radius:6px; padding:4px; z-index:20; box-shadow:0 -4px 16px rgba(0,0,0,0.3); max-height:140px; overflow-y:auto; }
  .ai-mode-dropup.open { display:block; }
  .ai-mode-opt { display:flex; align-items:center; gap:6px; width:100%; padding:4px 6px; border:none; background:transparent; color:var(--text-primary); font-size:var(--fs-10); cursor:pointer; border-radius:4px; transition:all 0.08s ease; }
  .ai-mode-opt:hover { background:var(--bg-hover); }
  .ai-mode-opt.selected { background:color-mix(in srgb, var(--accent-blue) 10%, transparent); color:var(--accent-blue); }
  .ai-mode-opt .ai-mode-dot { width:6px; height:6px; }
  .ai-mode-opt.selected .ai-mode-dot { background:var(--accent-blue); }
  .ai-mode-prov { margin-left:auto; font-size:var(--fs-9); color:var(--text-muted); font-family:monospace; }
  .ai-usage-bar-wrap { position:relative; }
  .ai-usage-bar-container { height:3px; background:var(--bg-hover); cursor:pointer; }
  .ai-usage-bar-fill { height:100%; background:var(--accent-blue); border-radius:0 2px 2px 0; transition:width 0.3s ease; min-width:0; }
  .ai-quota-popover { position:absolute; bottom:100%; left:4px; right:4px; background:var(--bg-elevated); border:1px solid var(--border-primary); border-radius:6px; padding:6px 8px; z-index:20; box-shadow:0 -4px 16px rgba(0,0,0,0.3); max-height:160px; overflow-y:auto; animation:floatUp 0.15s ease; }
  .ai-quota-title { font-size:var(--fs-9); font-weight:600; color:var(--text-muted); text-transform:uppercase; letter-spacing:0.5px; margin-bottom:4px; }
  .ai-quota-item { padding:3px 0; border-bottom:1px solid var(--border-subtle); }
  .ai-quota-item:last-child { border-bottom:none; }
  .ai-quota-agent { font-size:var(--fs-10); font-weight:600; color:var(--text-primary); }
  .ai-quota-model { font-size:var(--fs-9); color:var(--accent-blue); font-family:monospace; }
  .ai-quota-detail { font-size:var(--fs-9); color:var(--text-muted); }
  .ai-quota-empty { font-size:var(--fs-10); color:var(--text-muted); text-align:center; padding:8px 0; }
  @keyframes floatUp { from { opacity:0; transform:translateY(4px); } to { opacity:1; transform:translateY(0); } }
  .md-content { line-height:1; user-select:text; padding:0; gap:0; -webkit-user-select:text; word-wrap:break-word; word-break:break-word; overflow-wrap:anywhere; white-space:normal; }
  .md-content :global(p) { margin:0; }
  .md-content :global(> *:first-child) { margin-top:0; }
  .md-content :global(> *:last-child) { margin-bottom:0; }
  .md-content :global(pre) { background:var(--bg-primary); border:1px solid var(--border-subtle); border-radius:4px; padding:4px 6px; overflow-x:auto; margin:2px 0; font-size:var(--fs-10); max-width:100%; white-space:pre; }
  .md-content :global(code) { background:var(--bg-hover); padding:1px 2px; border-radius:2px; font-size:var(--fs-10); }
  .md-content :global(pre code) { background:none; padding:0; border-radius:0; }
  .md-content :global(ul), .md-content :global(ol) { padding-left:14px; margin:1px 0; }
  .md-content :global(li) { margin:0; }
  .md-content :global(h1), .md-content :global(h2), .md-content :global(h3), .md-content :global(h4) { margin:2px 0 1px 0; font-weight:600; color:var(--text-primary); }
  .md-content :global(h1) { font-size:1.05em; } .md-content :global(h2) { font-size:1em; } .md-content :global(h3) { font-size:0.95em; }
  .md-content :global(blockquote) { border-left:3px solid var(--accent-blue); padding-left:6px; margin:3px 0; color:var(--text-muted); }
  .md-content :global(table) { border-collapse:collapse; margin:3px 0; font-size:var(--fs-10); width:100%; }
  .md-content :global(th), .md-content :global(td) { border:1px solid var(--border-subtle); padding:3px 6px; text-align:left; }
  .md-content :global(th) { background:var(--bg-hover); font-weight:600; }
  .md-content :global(a) { color:var(--accent-blue); text-decoration:none; }
  .md-content :global(a:hover) { text-decoration:underline; }
  .md-content :global(img) { max-width:100%; border-radius:4px; margin:2px 0; }
  .md-content :global(hr) { border:none; border-top:1px solid var(--border-subtle); margin:4px 0; }

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
  .inline-permission {
    margin: 8px 12px;
    padding: 8px 10px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    box-shadow: none;
    max-width: 92%;
    align-self: flex-start;
    animation: fadeIn 0.15s ease;
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
