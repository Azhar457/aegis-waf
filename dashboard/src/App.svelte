<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import Overview from './lib/Overview.svelte';
  import LiveLogs from './lib/LiveLogs.svelte';
  import VirtualHosts from './lib/VirtualHosts.svelte';
  import RuleEngine from './lib/RuleEngine.svelte';
  import RateLimiting from './lib/RateLimiting.svelte';
  import { initGlobalStore, cleanupGlobalStore, connectionStatus, stats } from './lib/stores';

  const controllerUrl = 'http://localhost:8080';
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

<div class="app-container">
  <!-- Sidebar Menu (Left panel) -->
  <aside class="sidebar">
    <div>
      <div class="logo-section">
        <div class="logo-icon">Æ</div>
        <div class="logo-text">AEGIS WAF</div>
      </div>

      <nav class="nav-links">
        <div 
          class="nav-item {activeTab === 'overview' ? 'active' : ''}" 
          on:click={() => activeTab = 'overview'}
        >
          <span>📊</span> Overview
        </div>
        <div 
          class="nav-item {activeTab === 'logs' ? 'active' : ''}" 
          on:click={() => activeTab = 'logs'}
        >
          <span>🛡️</span> Attack Logs
        </div>
        <div 
          class="nav-item {activeTab === 'vhosts' ? 'active' : ''}" 
          on:click={() => activeTab = 'vhosts'}
        >
          <span>🌐</span> Virtual Hosts
        </div>
        <div 
          class="nav-item {activeTab === 'rules' ? 'active' : ''}" 
          on:click={() => activeTab = 'rules'}
        >
          <span>⚙️</span> Rule Engine
        </div>
        <div 
          class="nav-item {activeTab === 'rate_limits' ? 'active' : ''}" 
          on:click={() => activeTab = 'rate_limits'}
        >
          <span>🚨</span> Rate Limit Tiers
        </div>
      </nav>
    </div>

    <!-- Active Connection Status -->
    <div class="system-status">
      <span class="label">Controller connection</span>
      <div class="status-indicator">
        <span class="dot {$connectionStatus === 'online' ? 'online' : 'offline'}"></span>
        <span style="color: {$connectionStatus === 'online' ? 'var(--color-pass)' : 'var(--color-critical)'}">
          {$connectionStatus === 'online' ? 'CONNECTED' : 'DISCONNECTED'}
        </span>
      </div>
    </div>
  </aside>

  <!-- Workstation Panel (Right panel) -->
  <main class="main-content">
    <header class="top-bar">
      <h2 class="page-title">
        {#if activeTab === 'overview'}Overview Dashboard{/if}
        {#if activeTab === 'logs'}Real-Time Attack Logging{/if}
        {#if activeTab === 'vhosts'}Virtual Hosts Routing{/if}
        {#if activeTab === 'rules'}WAF Rule Signatures{/if}
        {#if activeTab === 'rate_limits'}Rate Limiting Tiers{/if}
      </h2>

      <div class="stats-summary">
        <span class="stat-pill">Attacks Prevented: <strong class="text-critical">{formatCount($stats.blocked)}</strong></span>
        <span class="stat-pill font-mono">Tiers: <strong>4 Tiers</strong></span>
      </div>
    </header>

    <section class="content-body">
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
        <RateLimiting />
      {/if}
    </section>
  </main>
</div>
