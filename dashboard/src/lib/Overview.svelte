<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { stats, dbSize } from './stores';

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

  onMount(() => {
    fetchSystemSummary();
    summaryInterval = setInterval(fetchSystemSummary, 5000);
    
    updateInterval = setInterval(() => {
      // Calculate reqsPerSec from global stats
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

<div class="overview-panel animate-fade-in">
  <!-- Status Banner -->
  <div class="card status-banner-card bg-pass-glow">
    <div class="banner-left">
      <span class="status-badge-pulse">SECURE</span>
      <div class="banner-text">
        <h4>Aegis Firewall Core Operational</h4>
        <p class="text-muted">All virtual host routing networks are active. Traffic inspected in real time.</p>
      </div>
    </div>
    <div class="banner-right">
      <div class="metric-group">
        <span class="m-val">{reqsPerSec}</span>
        <span class="m-lbl">REQ/SEC</span>
      </div>
    </div>
  </div>

  <!-- Agent Health Status cards -->
  <h3 class="section-title">WAF Node Agent Diagnostics</h3>
  {#if agents.length === 0}
    <div class="card no-agents-card">
      <div class="no-agents-content">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="icon-warning">
          <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path>
          <line x1="12" y1="9" x2="12" y2="13"></line>
          <line x1="12" y1="17" x2="12.01" y2="17"></line>
        </svg>
        <h4>No WAF Node Agents Connected</h4>
        <p>Aegis WAF requires at least one Agent Node to inspect traffic. Install and connect an agent using this command on your target server:</p>
        <div class="code-install-box font-mono">
          <code>curl -sSL http://{window.location.hostname}:8080/install.sh | CONTROLLER_IP={window.location.hostname}:8080 bash</code>
        </div>
      </div>
    </div>
  {:else}
    <div class="grid-cols-2">
      {#each agents as agent}
        <div class="card agent-diagnostic-card">
          <div class="agent-hdr">
            <div class="agent-title-info">
              <span class="dot {agent.status === 'online' ? 'online' : 'offline'}"></span>
              <strong>{agent.hostname}</strong>
              <span class="agent-ip-sub font-mono">({agent.ip})</span>
            </div>
            <div class="agent-uptime-badge font-mono">UPTIME: {agent.uptime}</div>
          </div>

          <div class="agent-diagnostics-grid">
            <!-- CPU Gauge -->
            <div class="diag-group">
              <div class="diag-lbl-row">
                <span class="diag-lbl">CPU Usage</span>
                <span class="diag-val font-mono">{Math.round(agent.cpu)}%</span>
              </div>
              <div class="progress-bar-container">
                <div class="progress-bar-fill fill-cpu" style="width: {agent.cpu}%"></div>
              </div>
            </div>

            <!-- Memory Gauge -->
            <div class="diag-group">
              <div class="diag-lbl-row">
                <span class="diag-lbl">RAM Usage</span>
                <span class="diag-val font-mono">{Math.round(agent.ram)}%</span>
              </div>
              <div class="progress-bar-container">
                <div class="progress-bar-fill fill-ram" style="width: {agent.ram}%"></div>
              </div>
            </div>

            <!-- Disk Gauge -->
            <div class="diag-group">
              <div class="diag-lbl-row">
                <span class="diag-lbl">Disk Storage</span>
                <span class="diag-val font-mono">{Math.round(agent.disk)}%</span>
              </div>
              <div class="progress-bar-container">
                <div class="progress-bar-fill fill-disk" style="width: {agent.disk}%"></div>
              </div>
            </div>

            <!-- OS Platform -->
            <div class="diag-group text-center">
              <span class="diag-lbl">Platform</span>
              <div class="latency-val font-mono text-pass" style="text-transform: capitalize; font-size: 0.9rem;">{agent.os}</div>
              {#if agent.network_interfaces && agent.network_interfaces.length > 0}
                <div style="margin-top: 0.5rem;">
                  <span class="diag-lbl" style="display:block; margin-bottom: 0.2rem; font-size: 0.65rem;">eBPF Interface</span>
                  <select class="interface-select font-mono">
                    {#each agent.network_interfaces as iface}
                      <option value={iface}>{iface}</option>
                    {/each}
                  </select>
                </div>
              {/if}
            </div>
          </div>

          <!-- Discovered Services -->
          {#if agent.discovered_services && agent.discovered_services.length > 0}
            <div class="discovered-services-container">
              <h4 class="services-title">Discovered Services</h4>
              <div class="services-grid">
                {#each agent.discovered_services as svc}
                  <div class="service-card">
                    <span class="service-icon">{svc.source === 'Docker' ? '🐳' : '🔌'}</span>
                    <div class="service-info">
                      <span class="service-name font-mono">{svc.name}</span>
                      <span class="service-port font-mono">{svc.port}/{svc.protocol}</span>
                    </div>
                    <button class="protect-btn" title="Create Virtual Host">
                      Protect
                    </button>
                  </div>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  <!-- Configuration Summary and Disk Storage -->
  <div class="grid-cols-2">
    <!-- WAF Specs summary -->
    <div class="card">
      <h3 class="panel-subtitle">Operational Configuration Summary</h3>
      <div class="spec-summary-list">
        <div class="spec-item">
          <span class="spec-lbl">Active Routing Hostnames:</span>
          <span class="spec-val font-bold">{activeVhostsCount} Domains</span>
        </div>
        <div class="spec-item">
          <span class="spec-lbl">SSL Certificates Enforced:</span>
          <span class="spec-val font-bold">Auto Local CA</span>
        </div>
        <div class="spec-item">
          <span class="spec-lbl">WAF Rules Toggled:</span>
          <span class="spec-val font-bold">{totalRulesToggled} Rules Active</span>
        </div>
        <div class="spec-item">
          <span class="spec-lbl">Distributed Nodes:</span>
          <span class="spec-val font-bold">2 Online Agents</span>
        </div>
      </div>
    </div>

    <!-- Storage and Cap -->
    <div class="card storage-cap-card">
      <h3 class="panel-subtitle">Access Logs Database Capacity</h3>
      <div class="storage-meters">
        <div class="storage-lbl-row">
          <span>Central Database Usage</span>
          <span class="font-mono"><strong>{$dbSize}</strong> / {logLimitMb} MB</span>
        </div>
        <div class="progress-bar-container storage-bar">
          <div class="progress-bar-fill fill-storage" style="width: 1%"></div>
        </div>
        <p class="text-muted storage-desc">
          When the log database exceeds {logLimitMb} MB, the Controller automatically prunes the oldest 1000 requests.
        </p>
      </div>
    </div>
  </div>

  <!-- Active Domain Routing Table -->
  <div class="card">
    <h3 class="panel-subtitle" style="margin-bottom: 1rem;">Active Domain Routing Map</h3>
    <div class="table-card" style="padding: 0; overflow: hidden; border: 1px solid var(--border-card); border-radius: 6px;">
      <table class="routing-table">
        <thead>
          <tr>
            <th>Domain Pattern</th>
            <th>Backend Proxy Address</th>
            <th>SSL Protection</th>
            <th>Custom Rule count</th>
            <th>Geo Lock</th>
          </tr>
        </thead>
        <tbody>
          {#each vhosts as host}
            <tr>
              <td class="font-bold font-mono text-pass">{host.hosts[0]}</td>
              <td class="font-mono">{host.backend}</td>
              <td>
                <span class="ssl-status-badge {host.ssl !== 'Disabled' ? 'ssl-ok' : 'ssl-off'}">
                  {host.ssl}
                </span>
              </td>
              <td class="font-mono">{host.custom_rules ? host.custom_rules.length : 0} Rules</td>
              <td>
                {#if host.blocked_countries.length > 0}
                  <span class="geo-status-badge geo-locked">🔒 {host.geoblock_type} ({host.blocked_countries.length})</span>
                {:else}
                  <span class="geo-status-badge geo-open">🔓 Open Access</span>
                {/if}
              </td>
            </tr>
          {:else}
            <tr>
              <td colspan="5" class="text-center text-muted font-mono" style="padding: 2rem;">No virtual host maps loaded</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>

  <!-- Collaborative Threat Intel Blocklist -->
  <div class="card">
    <div class="blocklist-header" style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;">
      <h3 class="panel-subtitle">Collaborative IP Threat Intelligence (Active Blocks)</h3>
      <span class="threat-intel-badge badge-collab">Reputation Network Active</span>
    </div>
    
    <div class="blocklist-content">
      {#if blockedIps.length > 0}
        <p class="text-muted" style="font-size: 0.8rem; margin: 0 0 1rem 0;">
          The following IPs generated 5+ attacks in 5 minutes and are dynamically blocked at the network entry layer:
        </p>
        <div class="blocked-ips-container" style="display: flex; flex-wrap: wrap; gap: 0.5rem;">
          {#each blockedIps as ip}
            <span class="blocked-ip-tag font-mono">
              <span class="dot offline" style="width: 6px; height: 6px; margin-right: 6px;"></span>
              {ip}
              <span class="block-indicator-label">BLOCKED</span>
            </span>
          {/each}
        </div>
      {:else}
        <div class="empty-blocklist text-center" style="padding: 2rem 1rem;">
          <span style="font-size: 2rem; display: block; margin-bottom: 0.5rem;">🛡️</span>
          <p class="font-bold text-pass" style="margin: 0; font-size: 0.9rem;">No Active IP Threat Blocks</p>
          <p class="text-muted" style="margin: 0.25rem 0 0 0; font-size: 0.78rem;">
            No IPs are currently flagged for collaborative blocklisting. The reputation network is clean.
          </p>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .overview-panel {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .section-title {
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin: 0.5rem 0 0 0;
  }

  .panel-subtitle {
    font-size: 0.95rem;
    font-weight: 700;
    color: #ffffff;
    margin: 0;
  }

  /* Status Banner Card */
  .status-banner-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1.25rem 1.5rem;
    flex-wrap: wrap;
    gap: 1rem;
  }

  .bg-pass-glow {
    background: linear-gradient(135deg, rgba(16, 185, 129, 0.05) 0%, rgba(5, 5, 8, 0.3) 100%);
    border: 1px solid rgba(16, 185, 129, 0.25);
    box-shadow: 0 0 15px rgba(16, 185, 129, 0.05);
  }

  .banner-left {
    display: flex;
    align-items: center;
    gap: 1.25rem;
  }

  .status-badge-pulse {
    background-color: var(--color-pass);
    color: #050508;
    padding: 0.3rem 0.75rem;
    font-size: 0.75rem;
    font-weight: 800;
    border-radius: 4px;
    letter-spacing: 0.8px;
    box-shadow: 0 0 10px rgba(16, 185, 129, 0.4);
  }

  .banner-text h4 {
    color: white;
    font-size: 1.05rem;
    margin: 0 0 0.15rem 0;
    font-weight: 700;
  }

  .banner-text p {
    margin: 0;
    font-size: 0.8rem;
  }

  .banner-right {
    display: flex;
    align-items: center;
  }

  .metric-group {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
  }

  .m-val {
    font-size: 1.8rem;
    font-weight: 800;
    color: var(--color-pass);
    line-height: 1.1;
  }

  .m-lbl {
    font-size: 0.65rem;
    color: var(--text-muted);
    letter-spacing: 0.5px;
    font-weight: 600;
  }

  /* Diagnostic Agent cards */
  .agent-diagnostic-card {
    padding: 1.25rem 1.5rem;
  }

  .agent-hdr {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--border-card);
    padding-bottom: 0.6rem;
    margin-bottom: 0.85rem;
  }

  .agent-title-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .agent-ip-sub {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .agent-uptime-badge {
    font-size: 0.72rem;
    color: var(--text-muted);
    background-color: rgba(255,255,255,0.03);
    padding: 0.15rem 0.4rem;
    border-radius: 4px;
  }

  .agent-diagnostics-grid {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr 120px;
    gap: 1rem;
    align-items: center;
  }

  .diag-group {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .diag-lbl-row {
    display: flex;
    justify-content: space-between;
    font-size: 0.75rem;
  }

  .diag-lbl {
    color: var(--text-muted);
  }

  .diag-val {
    color: white;
    font-weight: 600;
  }

  .latency-val {
    font-size: 1.2rem;
    font-weight: 800;
    margin-top: 0.1rem;
  }

  .text-center {
    text-align: center;
  }

  .progress-bar-container {
    background-color: rgba(255, 255, 255, 0.04);
    height: 6px;
    border-radius: 3px;
    overflow: hidden;
    width: 100%;
  }

  .progress-bar-fill {
    height: 100%;
    border-radius: 3px;
    transition: width 0.4s ease-out;
  }

  .fill-cpu {
    background-color: #3b82f6; /* Blue */
  }

  .fill-ram {
    background-color: #a855f7; /* Purple */
  }

  .fill-disk {
    background-color: #eab308; /* Yellow */
  }

  .fill-storage {
    background-color: #38bdf8; /* Sky blue */
  }

  /* Configuration Specs styling */
  .spec-summary-list {
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
    margin-top: 1rem;
  }

  .spec-item {
    display: flex;
    justify-content: space-between;
    font-size: 0.85rem;
    padding-bottom: 0.5rem;
    border-bottom: 1px dashed rgba(255,255,255,0.03);
  }

  .spec-item:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  .spec-lbl {
    color: var(--text-muted);
  }

  .spec-val {
    color: white;
  }

  /* Capacity and Storage meters */
  .storage-cap-card {
    display: flex;
    flex-direction: column;
  }

  .storage-meters {
    display: flex;
    flex-direction: column;
    margin-top: 1rem;
    gap: 0.5rem;
  }

  .storage-lbl-row {
    display: flex;
    justify-content: space-between;
    font-size: 0.85rem;
    color: white;
  }

  .storage-bar {
    height: 8px;
    border-radius: 4px;
    margin-top: 0.25rem;
  }

  .storage-desc {
    font-size: 0.75rem;
    line-height: 1.4;
    margin: 0.25rem 0 0 0;
  }

  /* Domain Routing Table */
  .routing-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
    text-align: left;
  }

  .routing-table th {
    background-color: rgba(0, 0, 0, 0.25);
    border-bottom: 1px solid var(--border-card);
    padding: 0.8rem 1rem;
    color: var(--text-muted);
    font-weight: 600;
    text-transform: uppercase;
    font-size: 0.72rem;
    letter-spacing: 0.5px;
  }

  .routing-table td {
    padding: 0.8rem 1rem;
    border-bottom: 1px solid var(--border-card);
    color: var(--text-main);
  }

  .routing-table tr:last-child td {
    border-bottom: none;
  }

  .routing-table tr:hover {
    background-color: rgba(255, 255, 255, 0.01);
  }

  .ssl-status-badge {
    font-size: 0.75rem;
    padding: 0.15rem 0.4rem;
    border-radius: 4px;
    font-weight: 700;
  }

  .ssl-ok {
    background-color: rgba(16, 185, 129, 0.1);
    color: var(--color-pass);
  }

  .ssl-off {
    background-color: rgba(255, 255, 255, 0.05);
    color: var(--text-muted);
  }

  .geo-status-badge {
    font-size: 0.75rem;
    padding: 0.15rem 0.4rem;
    border-radius: 4px;
    font-weight: 600;
  }

  .geo-locked {
    background-color: rgba(244, 63, 94, 0.1);
    color: var(--color-critical);
    border: 1px solid rgba(244, 63, 94, 0.15);
  }

  .geo-open {
    background-color: rgba(16, 185, 129, 0.1);
    color: var(--color-pass);
    border: 1px solid rgba(16, 185, 129, 0.15);
  }

  .badge-collab {
    background-color: rgba(59, 130, 246, 0.1);
    color: #3b82f6;
    border: 1px solid rgba(59, 130, 246, 0.25);
    font-size: 0.7rem;
    font-weight: 700;
    padding: 0.15rem 0.4rem;
    border-radius: 4px;
    text-transform: uppercase;
  }

  .blocked-ip-tag {
    display: inline-flex;
    align-items: center;
    background-color: rgba(244, 63, 94, 0.08);
    color: var(--color-critical);
    border: 1px solid rgba(244, 63, 94, 0.18);
    padding: 0.35rem 0.65rem;
    border-radius: 4px;
    font-size: 0.82rem;
    font-weight: 600;
  }

  .block-indicator-label {
    font-size: 0.6rem;
    font-weight: 800;
    background-color: var(--color-critical);
    color: #050508;
    padding: 0.05rem 0.25rem;
    border-radius: 2px;
    margin-left: 8px;
    letter-spacing: 0.5px;
  }

  .no-agents-card {
    grid-column: span 2;
    padding: 3rem 2rem;
    text-align: center;
    border: 1px solid rgba(244, 63, 94, 0.2);
    background: linear-gradient(135deg, rgba(244, 63, 94, 0.03) 0%, rgba(5, 5, 8, 0.4) 100%);
    box-shadow: 0 8px 32px 0 rgba(0, 0, 0, 0.37);
  }

  .no-agents-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    max-width: 600px;
    margin: 0 auto;
  }

  .icon-warning {
    width: 48px;
    height: 48px;
    color: var(--color-critical);
    margin-bottom: 1.5rem;
    filter: drop-shadow(0 0 8px rgba(244, 63, 94, 0.4));
  }

  .no-agents-content h4 {
    font-size: 1.25rem;
    color: #fff;
    margin-bottom: 0.75rem;
    font-weight: 700;
  }

  .no-agents-content p {
    color: var(--text-muted);
    font-size: 0.9rem;
    line-height: 1.5;
    margin-bottom: 1.5rem;
  }

  .code-install-box {
    background-color: #0b0b10;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 1rem 1.5rem;
    width: 100%;
    text-align: left;
    overflow-x: auto;
    position: relative;
  }

  .code-install-box code {
    color: var(--color-pass);
    font-size: 0.85rem;
    white-space: nowrap;
  }

  .interface-select {
    background: rgba(0,0,0,0.3);
    border: 1px solid rgba(255,255,255,0.1);
    color: white;
    font-size: 0.7rem;
    padding: 0.2rem 0.4rem;
    border-radius: 4px;
    width: 100%;
  }

  .discovered-services-container {
    margin-top: 1.25rem;
    padding-top: 1rem;
    border-top: 1px dashed rgba(255,255,255,0.05);
  }

  .services-title {
    font-size: 0.75rem;
    color: var(--text-muted);
    text-transform: uppercase;
    margin: 0 0 0.75rem 0;
    letter-spacing: 0.5px;
  }

  .services-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 0.75rem;
  }

  .service-card {
    display: flex;
    align-items: center;
    background: rgba(255,255,255,0.02);
    border: 1px solid rgba(255,255,255,0.05);
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    gap: 0.75rem;
  }

  .service-icon {
    font-size: 1.2rem;
  }

  .service-info {
    display: flex;
    flex-direction: column;
    flex: 1;
  }

  .service-name {
    font-size: 0.75rem;
    color: white;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100px;
  }

  .service-port {
    font-size: 0.65rem;
    color: var(--text-muted);
  }

  .protect-btn {
    background: rgba(16, 185, 129, 0.1);
    color: var(--color-pass);
    border: 1px solid rgba(16, 185, 129, 0.2);
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.65rem;
    font-weight: 700;
    cursor: pointer;
    transition: all 0.2s;
  }

  .protect-btn:hover {
    background: rgba(16, 185, 129, 0.2);
    border-color: rgba(16, 185, 129, 0.4);
  }
</style>
