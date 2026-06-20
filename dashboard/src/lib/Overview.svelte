<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { stats, dbSize, logs } from './stores';

  export let controllerUrl = '';

  interface AgentNode {
    hostname: string;
    ip: string;
    os: string;
    status: 'online' | 'offline';
    uptime: string;
    cpu: number;
    ram: number;
    disk: number;
    network_interfaces: string[];
    discovered_services: {
      name: string;
      port: number;
      protocol: string;
      source: string;
    }[];
  }

  interface VHost {
    name: string;
    hosts: string[];
    backend: string;
    ssl: string;
    geoblock_type: string;
    blocked_countries: string[];
    custom_rules: any[];
    rules: any[];
  }

  let agents: AgentNode[] = [];
  let vhosts: VHost[] = [];
  let totalRulesToggled = 0;
  let activeVhostsCount = 0;
  let logLimitMb = 500;
  let reqsPerSec = 0;
  let blockedIps: string[] = [];

  let updateInterval: ReturnType<typeof setInterval>;
  let summaryInterval: ReturnType<typeof setInterval>;
  let lastTotalRequests = 0;

  async function fetchSystemSummary() {
    try {
      const resVhosts = await fetch(`${controllerUrl}/api/v1/vhosts`);
      if (resVhosts.ok) {
        vhosts = await resVhosts.json();
        activeVhostsCount = vhosts.length;
        
        let rulesCount = 0;
        vhosts.forEach(v => {
          rulesCount += (v.rules ? v.rules.length : 0) + (v.custom_rules ? v.custom_rules.length : 0);
        });
        totalRulesToggled = rulesCount;
      }

      const resConfig = await fetch(`${controllerUrl}/api/v1/config`);
      if (resConfig.ok) {
        const cfg = await resConfig.json();
        logLimitMb = cfg.log_limit_mb;
      }

      const resBlocklist = await fetch(`${controllerUrl}/api/v1/reputation/blocklist`);
      if (resBlocklist.ok) {
        blockedIps = await resBlocklist.json();
      }

      const resAgents = await fetch(`${controllerUrl}/api/v1/agents`);
      if (resAgents.ok) {
        agents = await resAgents.json();
      }
    } catch (e) {
      console.error("Failed to fetch system summary:", e);
    }
  }

  function formatCount(num: number): string {
    if (num < 1000) return num.toString();
    if (num < 1000000) {
      return (num / 1000).toFixed(1).replace('.0', '') + 'k';
    }
    return (num / 1000000).toFixed(1).replace('.0', '') + 'M';
  }

  function formatTime(timestamp: string): string {
    try {
      if (timestamp.includes('T')) {
        return timestamp.split('T')[1].split('.')[0];
      }
      return timestamp;
    } catch {
      return timestamp;
    }
  }

  onMount(() => {
    fetchSystemSummary();
    summaryInterval = setInterval(fetchSystemSummary, 5000);
    
    updateInterval = setInterval(() => {
      const currentTotal = $stats.total_requests;
      if (lastTotalRequests > 0) {
        reqsPerSec = Math.max(0, Math.floor((currentTotal - lastTotalRequests) / 2.5));
      }
      lastTotalRequests = currentTotal;
    }, 2500);
  });

  onDestroy(() => {
    if (updateInterval) clearInterval(updateInterval);
    if (summaryInterval) clearInterval(summaryInterval);
  });
</script>

<div class="overview-panel flex flex-col gap-lg">
  <!-- Header Section -->
  <div class="flex justify-between items-end">
    <div>
      <div class="flex items-center gap-2 text-on-surface-variant text-xs mb-1">
        <span>Aegis WAF</span>
        <span class="material-symbols-outlined text-[12px]">chevron_right</span>
        <span class="text-primary">Dashboard</span>
      </div>
      <h2 class="font-headline-md text-headline-md font-bold text-on-surface">Network Overview</h2>
      <p class="text-on-surface-variant font-body-sm text-body-sm">Real-time traffic telemetry and threat mitigation status.</p>
    </div>
    <div class="flex gap-sm">
      <div class="flex flex-col items-end">
        <span class="text-[10px] text-on-surface-variant uppercase font-bold">Sampling Rate</span>
        <span class="font-code-md text-code-md text-primary">1:1 (Real-time)</span>
      </div>
    </div>
  </div>

  <!-- Metric Grid -->
  <div class="grid grid-cols-1 md:grid-cols-3 gap-lg">
    <!-- Total Requests -->
    <div class="glass-panel p-lg rounded-lg cyan-glow">
      <div class="flex justify-between items-start mb-sm">
        <span class="text-on-surface-variant text-[12px] font-bold uppercase tracking-widest">Total Requests</span>
        <span class="material-symbols-outlined text-primary">dynamic_feed</span>
      </div>
      <div class="flex items-baseline gap-sm">
        <h3 class="font-metric-lg text-metric-lg text-primary">{formatCount($stats.total_requests)}</h3>
        <span class="text-primary/60 text-[11px] font-code-md font-mono">({reqsPerSec} req/sec)</span>
      </div>
      <div class="mt-md h-1 bg-surface-container rounded-full overflow-hidden">
        <div class="h-full bg-primary w-full"></div>
      </div>
    </div>

    <!-- Threats Blocked -->
    <div class="glass-panel p-lg rounded-lg">
      <div class="flex justify-between items-start mb-sm">
        <span class="text-on-surface-variant text-[12px] font-bold uppercase tracking-widest">Threats Blocked</span>
        <span class="material-symbols-outlined text-secondary">gpp_maybe</span>
      </div>
      <div class="flex items-baseline gap-sm">
        <h3 class="font-metric-lg text-metric-lg text-secondary">{formatCount($stats.blocked)}</h3>
        {#if $stats.total_requests > 0}
          <span class="text-secondary/60 text-body-sm font-code-md font-mono">({(($stats.blocked / $stats.total_requests) * 100).toFixed(2)}%)</span>
        {/if}
      </div>
      <div class="mt-md h-1 bg-surface-container rounded-full overflow-hidden">
        <div class="h-full bg-secondary" style="width: {$stats.total_requests > 0 ? ($stats.blocked / $stats.total_requests) * 100 : 0}%"></div>
      </div>
    </div>

    <!-- Rate Limited -->
    <div class="glass-panel p-lg rounded-lg">
      <div class="flex justify-between items-start mb-sm">
        <span class="text-on-surface-variant text-[12px] font-bold uppercase tracking-widest">Rate Limited</span>
        <span class="material-symbols-outlined text-tertiary">speed</span>
      </div>
      <div class="flex items-baseline gap-sm">
        <h3 class="font-metric-lg text-metric-lg text-tertiary">{formatCount($stats.rate_limited)}</h3>
        {#if $stats.total_requests > 0}
          <span class="text-tertiary/60 text-body-sm font-code-md font-mono">({(($stats.rate_limited / $stats.total_requests) * 100).toFixed(2)}%)</span>
        {/if}
      </div>
      <div class="mt-md h-1 bg-surface-container rounded-full overflow-hidden">
        <div class="h-full bg-tertiary" style="width: {$stats.total_requests > 0 ? ($stats.rate_limited / $stats.total_requests) * 100 : 0}%"></div>
      </div>
    </div>
  </div>

  <!-- Content Bento Grid -->
  <div class="grid grid-cols-12 gap-lg">
    <!-- Agent Nodes (Live Health) -->
    <section class="col-span-12 lg:col-span-4 glass-panel rounded-lg overflow-hidden flex flex-col">
      <div class="px-md py-sm border-b border-outline-variant flex justify-between items-center bg-surface-container-low">
        <h4 class="text-body-sm font-bold flex items-center gap-xs text-on-surface">
          <span class="material-symbols-outlined text-[18px]">dns</span>
          Agent Nodes
        </h4>
        <span class="text-[10px] font-code-md text-on-surface-variant">Active: {agents.filter(a => a.status === 'online').length}/{agents.length}</span>
      </div>

      <div class="p-md space-y-md flex-1 overflow-y-auto">
        {#if agents.length === 0}
          <div class="text-center py-8 text-on-surface-variant font-code-md text-xs">
            No WAF agents connected.<br/>
            Run the install command on your agent server:
            <div class="mt-2 p-2 bg-surface-container-lowest border border-outline-variant/30 rounded text-left overflow-x-auto text-[10px] whitespace-nowrap">
              <code>curl -sSL {controllerUrl}/install.sh | bash</code>
            </div>
          </div>
        {:else}
          {#each agents as agent}
            <div class="space-y-sm bg-surface-container-lowest/30 p-3 rounded border border-outline-variant/20">
              <div class="flex justify-between items-center">
                <div class="flex items-center gap-1.5">
                  <span class="w-2 h-2 rounded-full {agent.status === 'online' ? 'bg-primary pulse-dot' : 'bg-secondary'}"></span>
                  <span class="text-xs font-bold text-on-surface">{agent.hostname}</span>
                  <span class="text-[10px] font-mono text-on-surface-variant">({agent.ip})</span>
                </div>
                <span class="text-[9px] font-mono bg-surface-container px-1.5 py-0.5 rounded text-on-surface-variant uppercase">{agent.os}</span>
              </div>

              <!-- Metrics -->
              <div class="space-y-xs">
                <div class="flex justify-between text-[11px] font-code-md uppercase">
                  <span class="text-primary">{agent.hostname}</span>
                  <span class="text-on-surface-variant font-mono">{Math.round(agent.cpu)}% CPU</span>
                </div>
                <!-- CPU bar -->
                <div class="h-1.5 bg-surface-container rounded-full overflow-hidden flex">
                  <div class="h-full {agent.cpu > 80 ? 'bg-error' : 'bg-primary'}" style="width: {agent.cpu}%"></div>
                </div>
                <!-- RAM bar -->
                <div class="h-1 bg-surface-container rounded-full overflow-hidden flex">
                  <div class="h-full bg-primary/40" style="width: {agent.ram}%"></div>
                </div>
              </div>

              <div class="text-[9px] font-code-md text-on-surface-variant flex justify-between pt-1">
                <span>Uptime: {agent.uptime}</span>
                {#if agent.network_interfaces && agent.network_interfaces.length > 0}
                  <span>Interface: {agent.network_interfaces[0]}</span>
                {/if}
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </section>

    <!-- Active VHosts Table -->
    <section class="col-span-12 lg:col-span-8 glass-panel rounded-lg overflow-hidden flex flex-col">
      <div class="px-md py-sm border-b border-outline-variant flex justify-between items-center bg-surface-container-low">
        <h4 class="text-body-sm font-bold flex items-center gap-xs text-on-surface">
          <span class="material-symbols-outlined text-[18px]">public</span>
          Active VHosts
        </h4>
        <span class="text-[10px] font-code-md text-on-surface-variant">Active Routers: {vhosts.length}</span>
      </div>

      <div class="overflow-x-auto flex-1">
        <table class="w-full text-left font-body-sm text-body-sm">
          <thead class="bg-surface-container text-on-surface-variant text-[11px] uppercase font-bold">
            <tr>
              <th class="px-md py-sm">VHost Name</th>
              <th class="px-md py-sm">Backend Address</th>
              <th class="px-md py-sm">SSL Status</th>
              <th class="px-md py-sm">Rules</th>
              <th class="px-md py-sm text-right">Geo Lock</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-outline-variant/30">
            {#each vhosts as host}
              <tr class="hover:bg-surface-container-low/50 transition-colors">
                <td class="px-md py-md font-bold text-on-surface font-mono">{host.hosts[0]}</td>
                <td class="px-md py-md font-code-md text-on-surface-variant">{host.backend}</td>
                <td class="px-md py-md">
                  <div class="flex items-center gap-xs">
                    {#if host.ssl !== 'Disabled'}
                      <span class="material-symbols-outlined text-[14px] text-primary" style="font-variation-settings: 'FILL' 1;">verified_user</span>
                      <span class="text-[11px] uppercase font-bold text-primary">{host.ssl}</span>
                    {:else}
                      <span class="material-symbols-outlined text-[14px] text-on-surface-variant">lock_open</span>
                      <span class="text-[11px] uppercase font-bold text-on-surface-variant">Disabled</span>
                    {/if}
                  </div>
                </td>
                <td class="px-md py-md font-code-md text-on-surface-variant">{(host.rules ? host.rules.length : 0) + (host.custom_rules ? host.custom_rules.length : 0)} rules</td>
                <td class="px-md py-md text-right">
                  {#if host.blocked_countries && host.blocked_countries.length > 0}
                    <span class="text-[10px] font-bold text-secondary uppercase bg-secondary-container/10 border border-secondary/20 px-1.5 py-0.5 rounded">
                      🔒 {host.geoblock_type} ({host.blocked_countries.length})
                    </span>
                  {:else}
                    <span class="text-[10px] font-bold text-primary uppercase bg-primary-container/10 border border-primary/20 px-1.5 py-0.5 rounded">
                      🔓 Open
                    </span>
                  {/if}
                </td>
              </tr>
            {:else}
              <tr>
                <td colspan="5" class="px-md py-md text-center text-on-surface-variant font-code-md col-span-5">No Virtual Hosts configured</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>
  </div>

  <!-- Security Event Log (Live Stream) -->
  <section class="glass-panel rounded-lg overflow-hidden">
    <div class="px-md py-sm border-b border-outline-variant flex justify-between items-center bg-surface-container-low">
      <h4 class="text-body-sm font-bold flex items-center gap-xs text-on-surface">
        <span class="material-symbols-outlined text-[18px]">terminal</span>
        Security Event Log (Live Stream)
      </h4>
      <div class="flex gap-md items-center">
        <div class="flex items-center gap-xs">
          <span class="w-1.5 h-1.5 rounded-full bg-primary pulse-dot"></span>
          <span class="text-[10px] font-code-md uppercase text-primary">Connected</span>
        </div>
        <span class="text-[10px] font-code-md text-on-surface-variant">DB Size: {$dbSize}</span>
      </div>
    </div>
    
    <div class="p-md font-code-md text-code-md h-48 overflow-y-auto terminal-scroll bg-surface-container-lowest" id="event-log">
      {#if $logs.length === 0}
        <div class="text-on-surface-variant text-center py-12 text-xs italic">
          Waiting for security event activity...
        </div>
      {:else}
        {#each $logs.slice(0, 50) as log}
          <div class="flex gap-md {log.action === 'ALLOW' || log.action === 'PASS' ? 'opacity-70' : 'opacity-100'} mb-1.5 leading-tight">
            <span class="text-on-surface-variant shrink-0">[{formatTime(log.timestamp)}]</span>
            <span class="shrink-0 font-bold {log.action === 'ALLOW' || log.action === 'PASS' ? 'text-primary' : log.action === 'BLOCK' ? 'text-secondary' : 'text-tertiary'}">{log.action}</span>
            <span class="text-on-surface truncate">
              {log.method} <span class="text-on-surface-variant">{log.path}</span> - <span class="font-bold">{log.client_ip}</span> 
              {#if log.reason}
                <span class="text-on-surface-variant">({log.reason})</span>
              {/if}
            </span>
          </div>
        {/each}
      {/if}
    </div>
  </section>
</div>

<style>
  .glass-panel {
    background: #0d1117;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-top: 1px solid rgba(255, 255, 255, 0.12);
    position: relative;
  }

  .cyan-glow {
    box-shadow: 0 0 20px rgba(0, 212, 255, 0.05);
  }

  .terminal-scroll::-webkit-scrollbar {
    width: 4px;
  }
  .terminal-scroll::-webkit-scrollbar-track {
    background: transparent;
  }
  .terminal-scroll::-webkit-scrollbar-thumb {
    background: #33353a;
    border-radius: 2px;
  }

  @keyframes pulse-cyan {
    0% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.5; transform: scale(1.1); }
    100% { opacity: 1; transform: scale(1); }
  }
  .pulse-dot {
    animation: pulse-cyan 2s infinite ease-in-out;
  }
</style>
