<script lang="ts">
  import { fmtTime } from "../utils";

  let { primaryCwd = "", logs = [] as { type: string }[], showLogs = false, showFloatingRunner = false, showFloatingAi = false, onToggleRunner: onToggleRunner, onToggleAi: onToggleAi, onToggleLogs: onToggleLogs } = $props();

  let now = $state(new Date());
  $effect(() => {
    const id = setInterval(() => now = new Date(), 1000);
    return () => clearInterval(id);
  });

  const particles = Array.from({ length: 20 }, (_, i) => ({
    x: ((i * 37 + 13) % 100) / 100,
    y: ((i * 53 + 7) % 100) / 100,
    s: 0.5 + ((i * 11) % 5) * 0.25,
    d: 3 + (i % 5) * 0.8,
    c: `var(--particle-${i % 5})`,
  }));
</script>

<div class="footer-wrap" role="presentation">
  <div class="footer-particles" aria-hidden="true">
    {#each particles as p}
      <span
        class="particle"
        style="left:{p.x * 100}%; top:{p.y * 100}%; width:{p.s}rem; height:{p.s}rem; animation-duration:{p.d}s; background:{p.c}; color:{p.c};"
      ></span>
    {/each}
  </div>
  <div class="footer-wave" aria-hidden="true">
    <svg viewBox="0 0 1440 6" preserveAspectRatio="none">
      <path d="M0,3 Q360,6 720,3 Q1080,0 1440,3 L1440,6 L0,6 Z" fill="var(--particle-0)" opacity="0.12"/>
    </svg>
  </div>
  <footer class="status-bar">
    <div class="sb-left">
      <button class="sb-btn" class:active={showFloatingRunner} onclick={onToggleRunner} title="Runner Panel" style="margin-right:6px;">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="6 3 20 12 6 21 6 3"/></svg>
      </button>
      {#if primaryCwd}
        <div class="path-breadcrumb">
          {#each primaryCwd.split("\\") as seg, i}
            {#if i > 0}<span class="path-sep">&gt;</span>{/if}
            <span class="path-seg" class:path-seg-last={i === primaryCwd.split("\\").length - 1}>{seg}</span>
          {/each}
        </div>
      {:else}
        <span class="sb-muted">No directory</span>
      {/if}
    </div>

    <div class="sb-center" onclick={onToggleLogs} onkeydown={(e) => e.key === "Enter" && onToggleLogs()} role="button" tabindex="0" title="Toggle logs">
      <span class="sb-clock">{fmtTime(now)}</span>
      <span class="sb-log-badge" class:sb-log-has-error={logs.some(l => l.type === "error")}>{logs.length}</span>
    </div>

    <div class="sb-right">
      <button class="sb-btn" class:active={showFloatingAi} onclick={onToggleAi} title="AI Chat">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2a4 4 0 0 1 4 4c0 2-2 4-4 4s-4-2-4-4a4 4 0 0 1 4-4z"/><path d="M16 14h.2a4 4 0 0 1 3.8 2.8l.8 2.2H3.2l.8-2.2A4 4 0 0 1 7.8 14H8"/></svg>
      </button>
    </div>
  </footer>
</div>

<style>
  .footer-wrap { position:relative; overflow:hidden; flex-shrink:0; }
  .footer-particles { position:absolute; inset:0; pointer-events:none; z-index:1; overflow:hidden; }
  .particle {
    position:absolute;
    border-radius:50%;
    animation-name: float;
    animation-duration: 3s;
    animation-timing-function: ease-in-out;
    animation-iteration-count: infinite;
    animation-direction: alternate;
    opacity:0.5;
    box-shadow:0 0 4px currentColor;
    color:inherit;
  }
  .footer-wave { position:absolute; top:-3px; left:0; right:0; height:6px; z-index:2; pointer-events:none; overflow:hidden; }
  .footer-wave svg { width:100%; height:100%; display:block; }

  @keyframes float {
    0% { transform: translateY(0) translateX(0) scale(1); }
    100% { transform: translateY(-6px) translateX(3px) scale(1.15); }
  }

  .status-bar {
    position:relative; z-index:3;
    display:flex; align-items:center; justify-content:space-between;
    height:var(--status-bar-height); padding:0 8px;
    background:var(--bg-secondary); border-top:1px solid var(--border-primary);
    font-size:var(--fs-11); color:var(--text-muted); flex-shrink:0; user-select:none;
  }
  .sb-left { display:flex; align-items:center; gap:4px; flex:1; min-width:0; overflow-x:auto; scrollbar-width:none; }
  .sb-left::-webkit-scrollbar { display:none; }
  .sb-muted { color:var(--text-muted); font-style:italic; }
  .sb-center { display:flex; align-items:center; justify-content:center; gap:6px; cursor:pointer; padding:0 6px; border-radius:4px; transition:all 0.12s ease; }
  .sb-center:hover { background:var(--bg-hover); }
  .sb-clock { font-family:monospace; font-size:var(--fs-11); letter-spacing:0.5px; }
  .sb-log-badge { font-size:var(--fs-9); background:var(--bg-elevated); color:var(--text-muted); padding:0 5px; border-radius:8px; line-height:14px; font-weight:600; }
  .sb-log-badge.sb-log-has-error { background:var(--accent-red); color:#fff; }
  .sb-right { display:flex; align-items:center; gap:2px; }
  .sb-btn { display:flex; align-items:center; justify-content:center; background:none; border:none; color:var(--text-muted); padding:3px 6px; cursor:pointer; border-radius:4px; transition:all 0.12s ease; }
  .sb-btn:hover { color:var(--text-primary); background:var(--bg-hover); }
  .sb-btn.active { color:var(--accent-blue); }

  .path-breadcrumb { display:flex; align-items:center; gap:2px; font-size:var(--fs-10); }
  .path-sep { margin:0 1px; opacity:0.5; }
  .path-seg { max-width:80px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .path-seg-last { color:var(--text-secondary); font-weight:500; }
</style>
