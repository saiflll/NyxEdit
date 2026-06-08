<script lang="ts">
  import { toasts, type Toast as TToast } from "../stores.svelte";
</script>

{#each $toasts as toast (toast.id)}
  <div class="toast toast-{toast.type}" role="alert">
    {#if toast.type === "error"}
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>
    {:else if toast.type === "success"}
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
    {:else}
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
    {/if}
    <span>{toast.message}</span>
  </div>
{/each}

<style>
  .toast {
    position:fixed; bottom:32px; left:50%; transform:translateX(-50%);
    display:flex; align-items:center; gap:8px;
    padding:8px 16px; border-radius:8px; font-size:var(--fs-11);
    z-index:500; animation:toastIn 0.25s ease; pointer-events:none;
    box-shadow:0 4px 20px rgba(0,0,0,0.5);
  }
  .toast-info { background:var(--bg-elevated); color:var(--text-primary); border:1px solid var(--border-primary); }
  .toast-success { background:#065f46; color:#a7f3d0; border:1px solid #059669; }
  .toast-error { background:#7f1d1d; color:#fecaca; border:1px solid #dc2626; }
  @keyframes toastIn { from { opacity:0; transform:translateX(-50%) translateY(12px); } to { opacity:1; transform:translateX(-50%) translateY(0); } }
</style>
