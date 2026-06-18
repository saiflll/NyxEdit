<script lang="ts">
  import type { Tab, TabType } from "$lib/stores/pageState";
  import { TAB_LABELS, TAB_ICONS } from "$lib/utils/tabHelpers";

  let { 
    tabs = [], 
    activeTabId = "", 
    onSetActive = () => {}, 
    onClose = () => {}, 
    onCloseAll = () => {}, 
    onCloseOther = () => {}, 
    onAdd = () => {} 
  }: {
    tabs?: Tab[];
    activeTabId?: string;
    onSetActive?: (id: string) => void;
    onClose?: (id: string) => void;
    onCloseAll?: () => void;
    onCloseOther?: (id: string) => void;
    onAdd?: (type: TabType) => void;
  } = $props();

  let tabsInternal = $state<Tab[]>(tabs);
  let activeTabIdInternal = $state(activeTabId);

  function handleContextMenu(e: MouseEvent, tab: Tab) {
    e.preventDefault();
    // TODO: Implement context menu for tabs
  }
</script>

<div class="tab-bar">
  <div class="tabs-list">
    {#each tabs as tab (tab.id)}
      {#if true}
        {@const icon = TAB_ICONS[tab.type]}
        {@const label = tab.label || TAB_LABELS[tab.type]}
        <div
          class="tab-item {tab.id === activeTabId ? 'active' : ''}"
          onclick={() => onSetActive(tab.id)}
          oncontextmenu={(e) => handleContextMenu(e, tab)}
        >
          <span class="tab-icon">{@html icon}</span>
          <span class="tab-label">{label}</span>
          
          {#if tabs.length > 1}
            <button 
              class="tab-close" 
              onclick={(e) => { e.stopPropagation(); onClose(tab.id); }}
            >
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          {/if}
        </div>
      {/if}
    {/each}
  </div>
  
  <div class="tab-actions">
    <button class="tab-add" onclick={() => onAdd("file")} title="New Tab">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="12" y1="5" x2="12" y2="19"></line>
        <line x1="5" y1="12" x2="19" y2="12"></line>
      </svg>
    </button>
    
    {#if tabs.length > 1}
      <button class="tab-close-all" onclick={onCloseAll} title="Close All">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="4" y1="4" x2="20" y2="20"></line>
          <line x1="20" y1="4" x2="4" y2="20"></line>
        </svg>
      </button>
    {/if}
  </div>
</div>

<style>
  .tab-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-color);
    padding: 0 8px;
    height: 36px;
    overflow-x: auto;
  }
  
  .tabs-list {
    display: flex;
    gap: 2px;
    flex: 1;
    overflow-x: auto;
  }
  
  .tab-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    font-size: 12px;
    cursor: pointer;
    border-radius: 4px 4px 0 0;
    transition: background 0.15s;
    white-space: nowrap;
    max-width: 200px;
  }
  
  .tab-item:hover {
    background: var(--bg-hover);
  }
  
  .tab-item.active {
    background: var(--bg-primary);
    border-top: 2px solid var(--accent-color);
  }
  
  .tab-icon {
    display: flex;
    align-items: center;
    opacity: 0.8;
  }
  
  .tab-label {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  
  .tab-close, .tab-close-all, .tab-add {
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 2px;
    border-radius: 3px;
    opacity: 0.6;
    transition: all 0.15s;
  }
  
  .tab-close:hover, .tab-close-all:hover, .tab-add:hover {
    opacity: 1;
    background: var(--bg-hover);
  }
  
  .tab-actions {
    display: flex;
    gap: 4px;
    margin-left: 8px;
  }
</style>
