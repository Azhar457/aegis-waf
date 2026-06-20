<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import Overview from './lib/Overview.svelte';
  import LiveLogs from './lib/LiveLogs.svelte';
  import VirtualHosts from './lib/VirtualHosts.svelte';
  import RuleEngine from './lib/RuleEngine.svelte';
  import RateLimiting from './lib/RateLimiting.svelte';
  import { initGlobalStore, cleanupGlobalStore, connectionStatus, stats } from './lib/stores';

  const controllerUrl = typeof window !== 'undefined' ? window.location.origin : 'http://localhost:8080';
  let activeTab = 'overview';

  onMount(() => {
    initGlobalStore(controllerUrl);
  });

  function formatCount(num: number): string {
    if (num < 1000) return num.toString();
    if (num < 1000000) {
      return (num / 1000).toFixed(1).replace('.0', '') + 'k';
    }
    return (num / 1000000).toFixed(1).replace('.0', '') + 'M';
  }

  onDestroy(() => {
    cleanupGlobalStore();
  });
</script>

<div class="min-h-screen bg-surface-container-lowest text-on-surface font-body-sm flex">
  <!-- Side Navigation Shell -->
  <aside class="w-[240px] h-screen fixed left-0 top-0 bg-surface-container-lowest border-r border-outline-variant flex flex-col py-lg z-50">
    <div class="px-lg mb-xl">
      <h1 class="font-headline-md text-headline-md font-bold text-primary">Aegis WAF</h1>
      <p class="text-[10px] text-outline font-code-md opacity-60 font-mono">v2.4.0-prod</p>
    </div>
    <nav class="flex-1 space-y-1">
      <button 
        type="button"
        class="w-full flex items-center px-lg py-sm transition-colors text-left cursor-pointer {activeTab === 'overview' ? 'text-primary font-bold border-r-2 border-primary bg-primary-container/10' : 'text-on-surface-variant hover:bg-surface-container-high hover:text-on-surface'}" 
        on:click={() => activeTab = 'overview'}
      >
        <span class="material-symbols-outlined mr-md text-[20px]">dashboard</span>
        <span class="font-body-sm text-body-sm">Overview</span>
      </button>

      <button 
        type="button"
        class="w-full flex items-center px-lg py-sm transition-colors text-left cursor-pointer {activeTab === 'logs' ? 'text-primary font-bold border-r-2 border-primary bg-primary-container/10' : 'text-on-surface-variant hover:bg-surface-container-high hover:text-on-surface'}" 
        on:click={() => activeTab = 'logs'}
      >
        <span class="material-symbols-outlined mr-md text-[20px]">security</span>
        <span class="font-body-sm text-body-sm">Attack Logs</span>
      </button>

      <button 
        type="button"
        class="w-full flex items-center px-lg py-sm transition-colors text-left cursor-pointer {activeTab === 'vhosts' ? 'text-primary font-bold border-r-2 border-primary bg-primary-container/10' : 'text-on-surface-variant hover:bg-surface-container-high hover:text-on-surface'}" 
        on:click={() => activeTab = 'vhosts'}
      >
        <span class="material-symbols-outlined mr-md text-[20px]">dns</span>
        <span class="font-body-sm text-body-sm">Virtual Hosts</span>
      </button>

      <button 
        type="button"
        class="w-full flex items-center px-lg py-sm transition-colors text-left cursor-pointer {activeTab === 'rules' ? 'text-primary font-bold border-r-2 border-primary bg-primary-container/10' : 'text-on-surface-variant hover:bg-surface-container-high hover:text-on-surface'}" 
        on:click={() => activeTab = 'rules'}
      >
        <span class="material-symbols-outlined mr-md text-[20px]">rule</span>
        <span class="font-body-sm text-body-sm">Rule Engine</span>
      </button>

      <button 
        type="button"
        class="w-full flex items-center px-lg py-sm transition-colors text-left cursor-pointer {activeTab === 'rate_limits' ? 'text-primary font-bold border-r-2 border-primary bg-primary-container/10' : 'text-on-surface-variant hover:bg-surface-container-high hover:text-on-surface'}" 
        on:click={() => activeTab = 'rate_limits'}
      >
        <span class="material-symbols-outlined mr-md text-[20px]">speed</span>
        <span class="font-body-sm text-body-sm">Rate Limiting</span>
      </button>
    </nav>

    <div class="mt-auto px-lg space-y-4">
      <div class="py-sm px-md bg-surface-container border border-outline-variant rounded flex items-center gap-2">
        <span class="w-2 h-2 rounded-full {$connectionStatus === 'online' ? 'bg-primary animate-pulse' : 'bg-error'}"></span>
        <span class="text-[10px] uppercase font-bold tracking-widest {$connectionStatus === 'online' ? 'text-primary' : 'text-error'}">
          {$connectionStatus === 'online' ? 'System Online' : 'System Offline'}
        </span>
      </div>
      <div class="pt-lg border-t border-outline-variant space-y-2">
        <a class="flex items-center text-on-surface-variant hover:text-primary transition-colors py-1" href="https://azhar457.github.io/note/" target="_blank">
          <span class="material-symbols-outlined mr-sm text-[18px]">menu_book</span>
          <span class="text-xs font-body-sm">Documentation</span>
        </a>
      </div>
    </div>
  </aside>

  <!-- Top App Bar & Main Content Canvas -->
  <div class="flex-1 ml-[240px] flex flex-col min-h-screen">
    <!-- Top App Bar -->
    <header class="flex justify-between items-center h-16 px-lg bg-surface/80 backdrop-blur-md sticky top-0 z-40 border-b border-outline-variant">
      <div class="flex items-center gap-xl">
        <span class="font-headline-md text-headline-md font-black tracking-tighter text-primary">Aegis WAF</span>
        <div class="flex items-center gap-lg">
          <span class="text-primary border-b-2 border-primary pb-base text-sm font-bold">CONSOLE</span>
        </div>
      </div>
      <div class="flex items-center gap-md">
        <div class="flex items-center gap-sm ml-lg text-on-surface-variant">
          <span class="stat-pill text-xs px-3 py-1 bg-surface-container rounded border border-outline-variant">
            Attacks Prevented: <strong class="text-error">{formatCount($stats.blocked)}</strong>
          </span>
        </div>
      </div>
    </header>

    <!-- Main Content -->
    <main class="p-lg flex-1 overflow-y-auto bg-background">
      {#if activeTab === 'overview'}
        <Overview {controllerUrl} />
      {/if}
      {#if activeTab === 'logs'}
        <LiveLogs {controllerUrl} />
      {/if}
      {#if activeTab === 'vhosts'}
        <VirtualHosts {controllerUrl} />
      {/if}
      {#if activeTab === 'rules'}
        <RuleEngine {controllerUrl} />
      {/if}
      {#if activeTab === 'rate_limits'}
        <RateLimiting {controllerUrl} />
      {/if}
    </main>
  </div>
</div>
