<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onDestroy } from "svelte";

  // ─── Types ──────────────────────────────────────────────────────────
  type NodeStatus = "pending" | "active" | "completed" | "error";

  export type DagNode = {
    id: string;
    label: string;
    level: number;
    status: NodeStatus;
    elapsed_ms?: number;
    agent?: string;
  };

  // ─── Props ──────────────────────────────────────────────────────────
  interface Props {
    /** External nodes array — can be passed from parent (AIChat) */
    nodes?: DagNode[];
    /** If true, also listen to ai:route_progress events internally */
    listenMode?: boolean;
  }
  let { nodes: externalNodes = [], listenMode = false }: Props = $props();

  // ─── State ──────────────────────────────────────────────────────────
  let internalNodes = $state<DagNode[]>([]);
  let unlistenProgress: UnlistenFn | null = null;
  let currentLevelIdx = $state(0);
  let isRunning = $state(false);

  const nodes = $derived(externalNodes.length > 0 ? externalNodes : internalNodes);

  // Group by level
  const levelMap = $derived.by(() => {
    const map = new Map<number, DagNode[]>();
    for (const node of nodes) {
      const arr = map.get(node.level) ?? [];
      arr.push(node);
      map.set(node.level, arr);
    }
    const sorted = Array.from(map.keys()).sort((a, b) => a - b);
    return sorted.map(k => ({ level: k, nodes: map.get(k)! }));
  });

  const totalNodes = $derived(nodes.length);
  const completedNodes = $derived(nodes.filter(n => n.status === "completed").length);
  const activeNodes = $derived(nodes.filter(n => n.status === "active").length);
  const errorNodes = $derived(nodes.filter(n => n.status === "error").length);
  const progressPct = $derived(totalNodes > 0 ? (completedNodes / totalNodes) * 100 : 0);

  // ─── Listen Mode ────────────────────────────────────────────────────
  $effect(() => {
    if (!listenMode) return;

    listen<string>("ai:route_progress", (e) => {
      const msg = e.payload;

      // DAG start
      const dagMatch = msg.match(/^Starting DAG with (\d+) nodes across (\d+) levels$/);
      if (dagMatch) {
        internalNodes = [];
        currentLevelIdx = 0;
        isRunning = true;
        return;
      }

      // Level transition
      const levelMatch = msg.match(/^DAG level (\d+)\/(\d+) \((\d+) parallel nodes\)$/);
      if (levelMatch) {
        currentLevelIdx = parseInt(levelMatch[1]) - 1;
        internalNodes = internalNodes.map(n =>
          n.level < currentLevelIdx && n.status === "active" ? { ...n, status: "completed" as const } : n
        );
        return;
      }

      // Node starting
      const nodeStart = msg.match(/^Starting DAG\[([^\]]+)\] (.+)$/);
      if (nodeStart) {
        const [, nodeId, nodeLabel] = nodeStart;
        const exists = internalNodes.some(n => n.id === nodeId);
        if (!exists) {
          let lvl = currentLevelIdx;
          if (nodeId === "scan") lvl = 0;
          else if (["review", "arch", "security", "style", "perf"].includes(nodeId)) lvl = 1;
          else lvl = 2;
          internalNodes = [...internalNodes, { id: nodeId, label: nodeLabel, level: lvl, status: "active" }];
        } else {
          internalNodes = internalNodes.map(n => n.id === nodeId ? { ...n, status: "active" as const } : n);
        }
        return;
      }

      // Progress update
      const progressMatch = msg.match(/^DAG progress: (\d+)\/(\d+) nodes complete$/);
      if (progressMatch) {
        const done = parseInt(progressMatch[1]);
        const total = parseInt(progressMatch[2]);
        if (done === total) {
          internalNodes = internalNodes.map(n => n.status === "active" ? { ...n, status: "completed" as const } : n);
          isRunning = false;
        }
        return;
      }
    }).then(fn => { unlistenProgress = fn; });

    listen<any>("ai:done", () => {
      internalNodes = internalNodes.map(n => ({ ...n, status: "completed" as const }));
      isRunning = false;
    });

    listen<any>("ai:error", () => {
      internalNodes = internalNodes.map(n =>
        n.status === "active" ? { ...n, status: "error" as const } : n
      );
      isRunning = false;
    });

    return () => { unlistenProgress?.(); };
  });

  onDestroy(() => { unlistenProgress?.(); });

  // ─── Node styling ────────────────────────────────────────────────────
  function nodeClass(status: NodeStatus): string {
    switch (status) {
      case "active": return "node-active";
      case "completed": return "node-done";
      case "error": return "node-error";
      default: return "node-pending";
    }
  }

  function nodeIcon(status: NodeStatus): string {
    switch (status) {
      case "active": return "⟳";
      case "completed": return "✓";
      case "error": return "✗";
      default: return "·";
    }
  }
</script>

{#if nodes.length > 0}
  <div class="dag-panel">
    <!-- Summary bar -->
    <div class="dag-summary">
      <div class="dag-title">
        {#if isRunning}
          <span class="dag-spinner"></span>
        {/if}
        <span>DAG Execution</span>
        <span class="dag-counts">
          {#if activeNodes > 0}<span class="cnt-active">{activeNodes} running</span>{/if}
          {#if completedNodes > 0}<span class="cnt-done">{completedNodes}/{totalNodes} done</span>{/if}
          {#if errorNodes > 0}<span class="cnt-error">{errorNodes} failed</span>{/if}
        </span>
      </div>
      <!-- Progress bar -->
      <div class="dag-progress-bg">
        <div
          class="dag-progress-fg"
          class:progress-error={errorNodes > 0}
          style="width: {progressPct}%"
        ></div>
      </div>
    </div>

    <!-- Level grid -->
    <div class="dag-levels">
      {#each levelMap as levelGroup, li}
        <div class="dag-level">
          <span class="level-label">L{li}</span>
          <div class="level-nodes">
            {#each levelGroup.nodes as node}
              <div class="dag-node {nodeClass(node.status)}" title={node.label}>
                <span class="node-icon">{nodeIcon(node.status)}</span>
                <span class="node-label">{node.label}</span>
              </div>
            {/each}
          </div>
          {#if li < levelMap.length - 1}
            <div class="level-arrow">↓</div>
          {/if}
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .dag-panel {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    overflow: hidden;
    margin: 6px 0;
  }

  /* Summary */
  .dag-summary {
    padding: 8px 12px 6px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .dag-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-10);
    font-weight: 700;
    color: var(--text-secondary);
    margin-bottom: 6px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .dag-counts {
    display: flex;
    gap: 8px;
    margin-left: auto;
  }
  .cnt-active { color: var(--accent-blue); font-size: var(--fs-9); }
  .cnt-done   { color: var(--accent-green); font-size: var(--fs-9); }
  .cnt-error  { color: var(--accent-red); font-size: var(--fs-9); }

  /* Spinner */
  .dag-spinner {
    width: 10px;
    height: 10px;
    border: 2px solid var(--border-subtle);
    border-top-color: var(--accent-blue);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    display: inline-block;
    flex-shrink: 0;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* Progress bar */
  .dag-progress-bg {
    width: 100%;
    height: 4px;
    background: var(--bg-primary);
    border-radius: 2px;
    overflow: hidden;
  }
  .dag-progress-fg {
    height: 100%;
    background: linear-gradient(90deg, var(--accent-blue), var(--accent-green));
    border-radius: 2px;
    transition: width 0.4s ease;
  }
  .dag-progress-fg.progress-error {
    background: linear-gradient(90deg, var(--accent-orange, var(--accent-red)), var(--accent-red));
  }

  /* Levels */
  .dag-levels {
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 0;
  }
  .dag-level {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .level-label {
    font-size: var(--fs-9);
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .level-nodes {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .level-arrow {
    text-align: center;
    color: var(--text-muted);
    font-size: var(--fs-12);
    line-height: 1;
    padding: 2px 0;
    opacity: 0.5;
  }

  /* Node */
  .dag-node {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    border-radius: 5px;
    font-size: var(--fs-10);
    font-weight: 600;
    border: 1px solid transparent;
    transition: all 0.2s ease;
    white-space: nowrap;
  }
  .node-pending {
    background: var(--bg-primary);
    border-color: var(--border-subtle);
    color: var(--text-muted);
  }
  .node-active {
    background: color-mix(in srgb, var(--accent-blue) 10%, var(--bg-primary));
    border-color: color-mix(in srgb, var(--accent-blue) 35%, transparent);
    color: var(--accent-blue);
    box-shadow: 0 0 8px color-mix(in srgb, var(--accent-blue) 15%, transparent);
    animation: pulse-blue 1.5s ease-in-out infinite;
  }
  .node-done {
    background: color-mix(in srgb, var(--accent-green) 10%, var(--bg-primary));
    border-color: color-mix(in srgb, var(--accent-green) 25%, transparent);
    color: var(--accent-green);
  }
  .node-error {
    background: color-mix(in srgb, var(--accent-red) 10%, var(--bg-primary));
    border-color: color-mix(in srgb, var(--accent-red) 25%, transparent);
    color: var(--accent-red);
  }

  @keyframes pulse-blue {
    0%, 100% { box-shadow: 0 0 6px color-mix(in srgb, var(--accent-blue) 15%, transparent); }
    50% { box-shadow: 0 0 14px color-mix(in srgb, var(--accent-blue) 30%, transparent); }
  }

  .node-icon { font-size: 11px; flex-shrink: 0; }
  .node-label { overflow: hidden; text-overflow: ellipsis; max-width: 120px; }
</style>
