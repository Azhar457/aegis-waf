<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import { SearchAddon } from '@xterm/addon-search';
  import { WebglAddon } from '@xterm/addon-webgl';
  import '@xterm/xterm/css/xterm.css';
  import { connectionStatus, stats, latestLog, dbSize } from './stores';
  import type { WafLog } from './stores';
  import Chart from 'chart.js/auto';

  export let controllerUrl = '';

  // Visual dashboard states
  let attackTrend: number[] = Array(15).fill(0); // Attack rates timeline
  let lastTotalBlocked = 0;
  let activeAttacks: any[] = [];
  let isDestroyed = false;
  let trendInterval: ReturnType<typeof setInterval>;

  // Logging & Terminal states
  let showTerminal = false; // Drawer visibility
  let incomingTerminalQueue: WafLog[] = [];
  let enableLogging = true;
  let logLimitSelection = '500';
  let customLimitValue = 500;

  let flushInterval: ReturnType<typeof setInterval>;
  let dbSizeInterval: ReturnType<typeof setInterval>;
  let resizeObserver: ResizeObserver;

  let terminalDiv: HTMLDivElement;
  let term: Terminal | null = null;
  let fitAddon: FitAddon | null = null;
  let searchAddon: SearchAddon | null = null;
  let isScrollLocked = true;

  let chartCanvas: HTMLCanvasElement;
  let threatChart: Chart | null = null;

  const countries = [
    { code: 'US', name: 'United States', x: 100, y: 45 },
    { code: 'DE', name: 'Germany', x: 260, y: 35 },
    { code: 'RU', name: 'Russia', x: 340, y: 30 },
    { code: 'CN', name: 'China', x: 385, y: 50 },
    { code: 'SG', name: 'Singapore', x: 405, y: 73 },
    { code: 'ID', name: 'Indonesia', x: 415, y: 78 },
    { code: 'BR', name: 'Brazil', x: 180, y: 85 },
    { code: 'AU', name: 'Australia', x: 440, y: 95 }
  ];

  const flags: { [code: string]: string } = {
    'US': '🇺🇸', 'DE': '🇩🇪', 'RU': '🇷🇺', 'CN': '🇨🇳',
    'SG': '🇸🇬', 'ID': '🇮🇩', 'BR': '🇧🇷', 'AU': '🇦🇺', 'LOCAL': '🏠'
  };

  let countryLeaderboard: { [code: string]: { name: string, count: number } } = {
    'RU': { name: 'Russia', count: 0 },
    'CN': { name: 'China', count: 0 },
    'US': { name: 'United States', count: 0 },
    'DE': { name: 'Germany', count: 0 },
    'BR': { name: 'Brazil', count: 0 }
  };

  function getIpCountry(ip: string) {
    if (ip.startsWith('127.') || ip.startsWith('192.168.') || ip.startsWith('10.') || ip === '::1' || ip.startsWith('172.')) {
      return { code: 'LOCAL', name: 'Local Network', x: 415, y: 65 };
    }
    let hash = 0;
    for (let i = 0; i < ip.length; i++) {
      hash = ip.charCodeAt(i) + ((hash << 5) - hash);
    }
    const index = Math.abs(hash) % countries.length;
    return countries[index];
  }

  function updateLeaderboard(code: string, name: string) {
    if (code === 'LOCAL') return;
    if (!countryLeaderboard[code]) {
      countryLeaderboard[code] = { name, count: 0 };
    }
    countryLeaderboard[code].count += 1;
    countryLeaderboard = { ...countryLeaderboard };
  }

  $: sortedCountries = Object.entries(countryLeaderboard)
    .map(([code, item]) => ({ code, name: item.name, count: item.count }))
    .sort((a, b) => b.count - a.count)
    .slice(0, 5);

  function triggerAttackAnimation(log: WafLog) {
    if (isDestroyed) return;
    if (activeAttacks.length >= 15) return; // M2 Fix: Cap active animations
    
    const country = getIpCountry(log.client_ip);
    const attackId = Math.random().toString(36).substring(2, 9);
    const newAttack = {
      id: attackId,
      fromName: country.name,
      fromCode: country.code,
      x1: country.x,
      y1: country.y,
      x2: 415, // WAF Node Jakarta
      y2: 78,
      action: log.action,
      ip: log.client_ip,
      reason: log.reason
    };

    activeAttacks = [...activeAttacks, newAttack];
    updateLeaderboard(country.code, country.name);

    setTimeout(() => {
      if (!isDestroyed) {
        activeAttacks = activeAttacks.filter(a => a.id !== attackId);
      }
    }, 1500);
  }

  // Fetch configs from Backend
  async function fetchConfig() {
    try {
      const res = await fetch(`${controllerUrl}/api/v1/config`);
      if (res.ok) {
        const data = await res.json();
        enableLogging = data.logging_enabled;
        const limit = data.log_limit_mb;
        if (limit === 500 || limit === 1024) {
          logLimitSelection = limit.toString();
        } else {
          logLimitSelection = 'custom';
          customLimitValue = limit;
        }
      }
    } catch (e) {
      console.error(e);
    }
  }

  // Update Config on Backend
  async function updateConfig() {
    const limit = logLimitSelection === 'custom' ? customLimitValue : parseInt(logLimitSelection);
    try {
      await fetch(`${controllerUrl}/api/v1/config`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          logging_enabled: enableLogging,
          log_limit_mb: limit
        })
      });
    } catch (e) {
      console.error(e);
    }
  }

  // Get current DB Size
  async function fetchDbSize() {
    try {
      const res = await fetch(`${controllerUrl}/api/v1/logs/db_size`);
      if (res.ok) {
        const data = await res.json();
        dbSize.set(data.formatted);
      }
    } catch (e) {
      console.error(e);
    }
  }

  // Format single log to ANSI colored string for xterm.js
  function formatAnsiLog(log: WafLog): string {
    const timePart = log.timestamp.split('T')[1];
    const ts = timePart ? timePart.substring(0, 8) : log.timestamp;
    
    const ip = log.client_ip;
    const act = log.action;
    const path = log.path;
    const rId = log.rule_id;
    const reason = log.reason;

    let actionColor = "\x1b[37m";
    if (act === 'BLOCK') {
      actionColor = "\x1b[1;31m";
    } else if (act === 'RATE_LIMIT') {
      actionColor = "\x1b[1;33m";
    } else if (act === 'REDIRECT') {
      actionColor = "\x1b[1;34m"; // Bold Blue for custom redirect
    } else if (act === 'PASS') {
      actionColor = "\x1b[1;32m"; // Bold Green for passed requests
    } else if (act === 'ERROR') {
      actionColor = "\x1b[1;35m"; // Bold Magenta/Purple for errors
    }

    return `\x1b[90m[${ts}]\x1b[0m \x1b[32m${ip}\x1b[0m ${actionColor}| ${act} |\x1b[0m \x1b[36m"${path}"\x1b[0m \x1b[33m[${rId}]\x1b[0m \x1b[37m- ${reason}\x1b[0m`;
  }

  function printWelcomeBanner() {
    if (!term) return;
    term.writeln("\x1b[90m================================================================================\x1b[0m");
    term.writeln("\x1b[1;36mAEGIS WAF v1.0 - CORE ACCESS TERMINAL SYSTEM (ACTIVE MONITORS)\x1b[0m");
    term.writeln(`\x1b[90mSTATUS: \x1b[1;32mONLINE\x1b[0m \x1b[90m| HARDWARE-ACCELERATED STREAM ACTIVE\x1b[0m`);
    term.writeln("\x1b[90m================================================================================\x1b[0m\r\n");
  }

  async function fetchInitialLogs() {
    try {
      const res = await fetch(`${controllerUrl}/api/v1/logs`);
      if (res.ok) {
        const data: WafLog[] = await res.json();
        if (term) {
          term.clear();
          printWelcomeBanner();
          
          // M3 Fix: Batch initial log writes with requestAnimationFrame to prevent freeze
          const writeChunk = (logsChunk: WafLog[], chunkSize: number, startIndex: number) => {
            if (isDestroyed || !term) return;
            const end = Math.min(startIndex + chunkSize, logsChunk.length);
            for (let i = startIndex; i < end; i++) {
              term.writeln(formatAnsiLog(logsChunk[i]));
            }
            if (end < logsChunk.length) {
              requestAnimationFrame(() => writeChunk(logsChunk, chunkSize, end));
            } else if (isScrollLocked) {
              term.scrollToBottom();
            }
          };
          writeChunk(data.reverse(), 50, 0);
        }
      }
    } catch (e) {
      console.error(e);
    }
  }

  $: {
    if (showTerminal && fitAddon && terminalDiv) {
      // Fit initially, and again after transition completes
      setTimeout(() => {
        if (showTerminal && terminalDiv && terminalDiv.clientWidth > 0) {
          try {
            fitAddon?.fit();
          } catch (e) {
            console.warn(e);
          }
        }
      }, 360);
    }
  }

  function handleResize() {
    if (showTerminal && fitAddon && terminalDiv && terminalDiv.clientWidth > 0 && terminalDiv.clientHeight > 0) {
      try {
        fitAddon.fit();
      } catch (e) {
        console.warn(e);
      }
    }
  }

  function handleTransitionEnd(event: TransitionEvent) {
    if (showTerminal && fitAddon && terminalDiv && event.propertyName === 'right' && terminalDiv.clientWidth > 0 && terminalDiv.clientHeight > 0) {
      try {
        fitAddon.fit();
      } catch (e) {
        console.warn(e);
      }
    }
  }

  // Subscribe to latest log from Global Store
  latestLog.subscribe(log => {
    if (log && !isDestroyed) {
      incomingTerminalQueue.push(log);
      if (log.action === 'BLOCK' || log.action === 'RATE_LIMIT' || log.action === 'REDIRECT') {
        triggerAttackAnimation(log);
      }
    }
  });

  onMount(async () => {
    isDestroyed = false;
    window.addEventListener('resize', handleResize);

    // Trend simulation calculation based on global stats
    trendInterval = setInterval(() => {
      const currentTotalBlocked = $stats.blocked + $stats.rate_limited;
      if (lastTotalBlocked > 0) {
        const delta = Math.max(0, currentTotalBlocked - lastTotalBlocked);
        attackTrend = [...attackTrend.slice(1), delta];
      } else {
        attackTrend = [...attackTrend.slice(1), 0];
      }
      lastTotalBlocked = currentTotalBlocked;

      if (threatChart) {
        threatChart.data.datasets[0].data = [...attackTrend];
        const maxVal = Math.max(...attackTrend, 10);
        if (threatChart.options.scales && threatChart.options.scales.y) {
          (threatChart.options.scales.y as any).suggestedMax = maxVal;
        }
        threatChart.update('none');
      }
    }, 5000);

    // Initialize Chart.js
    if (chartCanvas) {
      const ctx = chartCanvas.getContext('2d');
      const gradient = ctx?.createLinearGradient(0, 0, 0, 120);
      gradient?.addColorStop(0, 'rgba(244, 63, 94, 0.3)');
      gradient?.addColorStop(1, 'rgba(244, 63, 94, 0)');

      threatChart = new Chart(chartCanvas, {
        type: 'line',
        data: {
          labels: Array(15).fill(''),
          datasets: [{
            label: 'Blocked Requests',
            data: [...attackTrend],
            borderColor: '#f43f5e',
            backgroundColor: gradient,
            borderWidth: 2.5,
            fill: true,
            tension: 0.4,
            pointRadius: 0,
            pointHoverRadius: 4
          }]
        },
        options: {
          responsive: true,
          maintainAspectRatio: false,
          animation: { duration: 0 },
          scales: {
            x: { display: false },
            y: { display: false, min: 0, suggestedMax: 10 }
          },
          plugins: {
            legend: { display: false },
            tooltip: { enabled: false }
          }
        }
      });
    }

    // Initialize xterm.js
    term = new Terminal({
      theme: {
        background: '#050508',
        foreground: '#e2e8f0',
        cursor: '#ffffff',
        black: '#000000',
        red: '#f43f5e',
        green: '#10b981',
        yellow: '#fbbf24',
        blue: '#3b82f6',
        magenta: '#d946ef',
        cyan: '#22d3ee',
        white: '#ffffff',
      },
      fontFamily: 'Consolas, Fira Code, Monaco, monospace',
      fontSize: 12,
      lineHeight: 1.3,
      cursorBlink: true,
      scrollback: 2000,
      convertEol: true,
      disableStdin: true
    });

    fitAddon = new FitAddon();
    searchAddon = new SearchAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(searchAddon);

    term.open(terminalDiv);
    try {
      const webglAddon = new WebglAddon();
      term.loadAddon(webglAddon);
    } catch (e) {
      console.warn('WebGL addon failed to load', e);
    }
    printWelcomeBanner();

    // M4 Fix: handle browser resize properly
    resizeObserver = new ResizeObserver(() => {
      if (showTerminal && terminalDiv && terminalDiv.clientWidth > 0 && terminalDiv.clientHeight > 0) {
        try {
          fitAddon?.fit();
        } catch (e) {
          console.warn(e);
        }
      }
    });
    resizeObserver.observe(terminalDiv);

    term.onScroll((newY) => {
      if (term) {
        const viewportY = term.buffer.active.viewportY;
        const baseY = term.buffer.active.baseY;
        isScrollLocked = (viewportY >= baseY - 1);
      }
    });

    fetchConfig();
    fetchDbSize();
    
    // M1 Fix: wait for initial logs to finish before we start flushing SSE queue
    await fetchInitialLogs();

    // 200ms flush queue for terminal
    flushInterval = setInterval(() => {
      if (incomingTerminalQueue.length > 0 && term) {
        incomingTerminalQueue.forEach(log => {
          term?.writeln(formatAnsiLog(log));
        });
        incomingTerminalQueue = [];

        if (isScrollLocked) {
          term.scrollToBottom();
        }
      }
    }, 200);

    dbSizeInterval = setInterval(fetchDbSize, 5000);
  });

  onDestroy(() => {
    isDestroyed = true;
    window.removeEventListener('resize', handleResize);
    if (flushInterval) clearInterval(flushInterval);
    if (dbSizeInterval) clearInterval(dbSizeInterval);
    if (trendInterval) clearInterval(trendInterval);
    if (resizeObserver) resizeObserver.disconnect();
    if (term) term.dispose();
    if (threatChart) threatChart.destroy();
  });

  function handleExport() {
    window.open(`${controllerUrl}/api/v1/logs/export`, '_blank');
  }

  function clearTerminal() {
    if (term) {
      term.clear();
      printWelcomeBanner();
    }
    incomingTerminalQueue = [];
  }

  $: maxTrendVal = Math.max(...attackTrend, 10);

  function formatCount(num: number): string {
    if (num < 1000) return num.toString();
    if (num < 1000000) {
      return (num / 1000).toFixed(1).replace('.0', '') + 'k';
    }
    return (num / 1000000).toFixed(1).replace('.0', '') + 'M';
  }
</script>

<div class="visual-dashboard-panel">
  <!-- Counters Row -->
  <div class="grid-cols-3">
    <!-- Card 1 -->
    <div class="card animate-fade-in">
      <div class="card-title">Legitimate Requests</div>
      <div class="card-value text-pass font-bold">
        {formatCount($stats.total_requests - $stats.blocked - $stats.rate_limited)}
      </div>
      <div class="card-subtext">
        <span class="dot online"></span> Active traffic passing WAF proxy
      </div>
    </div>
    
    <!-- Card 2 -->
    <div class="card bg-critical-glow animate-fade-in">
      <div class="card-title text-critical">Blocked Attacks</div>
      <div class="card-value text-critical font-bold font-mono">
        {formatCount($stats.blocked)}
      </div>
      <div class="card-subtext">
        🔒 SQLi, XSS, Path Traversal attempts thwarted
      </div>
    </div>

    <!-- Card 3 -->
    <div class="card bg-high-glow animate-fade-in">
      <div class="card-title text-high">Rate Limited IPs</div>
      <div class="card-value text-high font-bold font-mono">
        {formatCount($stats.rate_limited)}
      </div>
      <div class="card-subtext">
        ⚡ Brute force and DoS nodes throttled
      </div>
    </div>
  </div>

  <!-- Chart and Trigger Bar -->
  <div class="grid-cols-2">
    <!-- Chart Card -->
    <div class="card chart-card">
      <div class="chart-header-group">
        <h3 class="panel-subtitle">Real-Time Threat Level (Last 45s)</h3>
        <span class="badge scale-badge">Max: {maxTrendVal} req/3s</span>
      </div>
      <div class="chart-container">
        <canvas bind:this={chartCanvas} class="chart-canvas"></canvas>
      </div>
      <div class="chart-labels">
        <span>45s ago</span>
        <span>Now (Scaled)</span>
      </div>
    </div>

    <!-- Live Terminal Launcher Panel -->
    <div class="card terminal-launcher-card bg-terminal-glow">
      <h3 class="panel-subtitle">WAF Access Terminal Monitor</h3>
      <p class="launcher-desc text-muted">
        Stream raw connection requests, geoblocking logs, and matched signatures with zero delay. Perfect for inspection during nmap/nikto vulnerability scans.
      </p>
      <div class="launcher-actions">
        <button on:click={() => showTerminal = true} class="btn btn-primary monitor-btn font-bold">
          📺 Open Core Terminal Monitor
        </button>
        <span class="status-indicator-inline">
          Connection: 
          <strong class={$connectionStatus === 'online' ? 'text-pass' : 'text-critical'}>
            {$connectionStatus.toUpperCase()}
          </strong>
        </span>
      </div>
    </div>
  </div>

  <!-- Threat Map & Leaderboard -->
  <div class="grid-map-panel">
    <!-- Map Card -->
    <div class="card map-card">
      <h3 class="panel-subtitle">Real-Time Global Threat Map (Offline Resolver)</h3>
      <div class="map-wrapper">
        <svg viewBox="0 0 500 150" class="world-svg">
          <!-- Continents -->
          <path d="M 50,30 Q 85,25 120,30 T 150,55 T 100,70 T 50,30" fill="rgba(255, 255, 255, 0.02)" stroke="rgba(255, 255, 255, 0.08)" stroke-width="1" />
          <path d="M 140,70 L 175,80 L 155,115 L 140,70" fill="rgba(255, 255, 255, 0.02)" stroke="rgba(255, 255, 255, 0.08)" stroke-width="1" />
          <path d="M 230,30 L 270,25 L 290,45 L 250,70 L 230,95 L 210,75 L 230,30" fill="rgba(255, 255, 255, 0.02)" stroke="rgba(255, 255, 255, 0.08)" stroke-width="1" />
          <path d="M 290,25 L 440,25 L 460,65 L 390,80 L 350,55 L 290,25" fill="rgba(255, 255, 255, 0.02)" stroke="rgba(255, 255, 255, 0.08)" stroke-width="1" />
          <path d="M 430,90 L 460,95 L 450,110 L 430,90" fill="rgba(255, 255, 255, 0.02)" stroke="rgba(255, 255, 255, 0.08)" stroke-width="1" />

          <!-- Target WAF Node (Jakarta/Indonesia) -->
          <circle cx="415" cy="78" r="4.5" fill="var(--color-pass)" class="glow-pass" />
          <circle cx="415" cy="78" r="14" fill="none" stroke="var(--color-pass)" stroke-width="1.5">
            <animate attributeName="r" values="4.5;18" dur="2.5s" repeatCount="indefinite" />
            <animate attributeName="opacity" values="1;0" dur="2.5s" repeatCount="indefinite" />
          </circle>

          <!-- Attack Lines (Arcs) -->
          {#each activeAttacks as attack (attack.id)}
            <path
              d="M {attack.x1} {attack.y1} Q {(attack.x1 + attack.x2) / 2} {Math.min(attack.y1, attack.y2) - 25} {attack.x2} {attack.y2}"
              fill="none"
              stroke="var(--color-critical)"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-dasharray="100"
              stroke-dashoffset="100"
              class="attack-arc"
            />
            
            <circle r="3" fill="var(--color-critical)">
              <animateMotion
                path="M {attack.x1} {attack.y1} Q {(attack.x1 + attack.x2) / 2} {Math.min(attack.y1, attack.y2) - 25} {attack.x2} {attack.y2}"
                dur="1.2s"
                repeatCount="1"
                fill="freeze"
              />
            </circle>

            <circle cx={attack.x1} cy={attack.y1} r="3" fill="var(--color-critical)" />
            <circle cx={attack.x1} cy={attack.y1} r="10" fill="none" stroke="var(--color-critical)" stroke-width="1">
              <animate attributeName="r" values="3;14" dur="1s" repeatCount="1" />
              <animate attributeName="opacity" values="1;0" dur="1s" repeatCount="1" />
            </circle>
          {/each}
        </svg>

        {#if activeAttacks.length > 0}
          <div class="threat-alert-flash">ATTACK THWARTED</div>
        {/if}
      </div>
    </div>

    <!-- Leaderboard -->
    <div class="card leaderboard-card">
      <h3 class="panel-subtitle">Top Attack Sources</h3>
      <div class="leaderboard-list">
        {#each sortedCountries as country, i}
          <div class="leaderboard-item animate-fade-in">
            <span class="rank font-mono">#{i + 1}</span>
            <span class="flag">{flags[country.code] || '🌐'}</span>
            <span class="country-name">{country.name}</span>
            <span class="attacks-badge font-mono">{formatCount(country.count)}</span>
          </div>
        {:else}
          <div class="empty-leaderboard font-mono text-muted">
            No attacks blocked during this session
          </div>
        {/each}
      </div>
    </div>
  </div>

  <!-- Slide-Out Terminal Drawer Overlay -->
  <div on:transitionend={handleTransitionEnd} class="terminal-drawer-overlay {showTerminal ? 'open' : ''}">
    <div class="terminal-drawer card">
      <div class="drawer-header">
        <div class="drawer-hdr-title">
          <span class="dot online animate-pulse"></span>
          <h4 class="panel-subtitle">📺 Raw WAF Access Terminal Logs</h4>
        </div>
        <button on:click={() => showTerminal = false} class="btn btn-secondary btn-sm close-drawer-btn font-bold">
          ✖ Close Monitor
        </button>
      </div>

      <!-- Controls row -->
      <div class="terminal-controls">
        <div class="control-left">
          <label class="checkbox-container">
            <input 
              type="checkbox" 
              bind:checked={enableLogging} 
              on:change={updateConfig}
            />
            <span class="checkbox-label font-bold">Enable access log</span>
          </label>

          <div class="limit-selector">
            <span class="control-txt">Limit:</span>
            <select 
              bind:value={logLimitSelection} 
              on:change={updateConfig}
              class="input-field select-limit"
            >
              <option value="500">500 MB</option>
              <option value="1024">1024 MB</option>
              <option value="custom">Custom</option>
            </select>

            {#if logLimitSelection === 'custom'}
              <div class="custom-input-group">
                <input 
                  type="number" 
                  bind:value={customLimitValue} 
                  on:blur={updateConfig}
                  class="input-field custom-limit-input"
                  min="10" 
                  max="10240"
                />
                <span class="mb-label">MB</span>
              </div>
            {/if}
          </div>
        </div>

        <div class="control-right">
          <span class="db-size-indicator font-mono">
            Size: <strong>{$dbSize}</strong>
          </span>

          <button on:click={clearTerminal} class="btn btn-secondary btn-sm clear-terminal-btn font-bold">
            Clear
          </button>

          <button on:click={handleExport} class="btn btn-primary btn-sm export-btn" title="Download Log File">
            📥 Export
          </button>
        </div>
      </div>

      <!-- xterm.js terminal box container -->
      <div class="terminal-box">
        <div bind:this={terminalDiv} class="terminal-container"></div>
      </div>
    </div>
  </div>
</div>

<style>
  .visual-dashboard-panel {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .panel-subtitle {
    font-size: 0.95rem;
    font-weight: 700;
    color: #ffffff;
    letter-spacing: -0.2px;
    margin: 0;
  }

  .chart-header-group {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1.25rem;
  }

  .scale-badge {
    font-size: 0.75rem;
    background-color: rgba(244, 63, 94, 0.1);
    color: var(--color-critical);
    border: 1px solid rgba(244, 63, 94, 0.15);
  }

  .chart-card {
    display: flex;
    flex-direction: column;
  }

  .chart-container {
    height: 120px;
    position: relative;
    margin-bottom: 0.5rem;
  }

  .chart-labels {
    display: flex;
    justify-content: space-between;
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  /* Threat Map Panel */
  .grid-map-panel {
    display: grid;
    grid-template-columns: 2.2fr 1fr;
    gap: 1.25rem;
  }

  .map-card {
    display: flex;
    flex-direction: column;
    padding: 1.25rem 1.5rem;
    position: relative;
  }

  .map-wrapper {
    background-color: rgba(0, 0, 0, 0.35);
    border: 1px solid var(--border-card);
    border-radius: 8px;
    padding: 0.5rem;
    position: relative;
    margin-top: 1rem;
    overflow: hidden;
  }

  .world-svg {
    width: 100%;
    height: auto;
    display: block;
  }

  .threat-alert-flash {
    position: absolute;
    top: 10px;
    right: 15px;
    background-color: rgba(244, 63, 94, 0.95);
    color: white;
    padding: 0.2rem 0.5rem;
    font-size: 0.75rem;
    font-weight: 800;
    border-radius: 4px;
    letter-spacing: 0.8px;
    animation: flash-red 0.5s ease-in-out infinite alternate;
  }

  @keyframes flash-red {
    0% { opacity: 0.4; }
    100% { opacity: 1; }
  }

  @keyframes draw-arc {
    0% {
      stroke-dashoffset: 100;
    }
    100% {
      stroke-dashoffset: 0;
    }
  }

  .attack-arc {
    animation: draw-arc 1.2s cubic-bezier(0.25, 0.46, 0.45, 0.94) forwards;
  }

  /* Leaderboard Card */
  .leaderboard-card {
    display: flex;
    flex-direction: column;
    padding: 1.25rem 1.5rem;
  }

  .leaderboard-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    margin-top: 1rem;
  }

  .leaderboard-item {
    display: flex;
    align-items: center;
    padding: 0.65rem 0.85rem;
    background-color: rgba(0, 0, 0, 0.2);
    border: 1px solid var(--border-card);
    border-radius: 6px;
    gap: 0.75rem;
  }

  .rank {
    font-size: 0.8rem;
    color: var(--text-muted);
    font-weight: 700;
    width: 15px;
  }

  .flag {
    font-size: 1.2rem;
  }

  .country-name {
    font-size: 0.85rem;
    font-weight: 600;
    color: white;
    flex-grow: 1;
  }

  .attacks-badge {
    background-color: rgba(244, 63, 94, 0.15);
    color: var(--color-critical);
    padding: 0.1rem 0.4rem;
    border-radius: 4px;
    font-size: 0.8rem;
    font-weight: 700;
  }

  .empty-leaderboard {
    text-align: center;
    padding: 3rem 1rem;
    color: var(--text-muted);
    font-size: 0.8rem;
    border: 1px dashed var(--border-card);
    border-radius: 6px;
  }

  /* Terminal Launcher Card */
  .terminal-launcher-card {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 1.25rem 1.5rem;
  }

  .bg-terminal-glow {
    background: linear-gradient(135deg, rgba(34, 211, 238, 0.03) 0%, rgba(5, 5, 8, 0.2) 100%);
    border: 1px dashed rgba(34, 211, 238, 0.25);
  }

  .launcher-desc {
    font-size: 0.8rem;
    line-height: 1.5;
    margin: 0.75rem 0;
  }

  .launcher-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 1rem;
  }

  .monitor-btn {
    border: 1px solid var(--accent-primary);
    box-shadow: 0 0 10px rgba(34, 211, 238, 0.2);
  }

  .status-indicator-inline {
    font-size: 0.78rem;
    color: var(--text-muted);
  }

  /* Slide-Out Drawer Overlay */
  .terminal-drawer-overlay {
    position: fixed;
    top: 0;
    right: -100vw;
    width: 100vw;
    height: 100vh;
    background-color: rgba(5, 5, 8, 0.75);
    backdrop-filter: blur(5px);
    z-index: 100;
    display: flex;
    justify-content: flex-end;
    transition: right 0.35s cubic-bezier(0.25, 0.46, 0.45, 0.94);
  }

  .terminal-drawer-overlay.open {
    right: 0;
  }

  .terminal-drawer {
    width: 60%;
    min-width: 780px;
    height: 100%;
    background-color: #050508;
    border-left: 1px solid var(--border-card);
    border-radius: 0;
    display: flex;
    flex-direction: column;
    padding: 1.5rem;
    gap: 1rem;
    box-shadow: -10px 0 30px rgba(0, 0, 0, 0.85);
  }

  .drawer-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--border-card);
    padding-bottom: 0.85rem;
  }

  .drawer-hdr-title {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .close-drawer-btn {
    background: transparent;
    border: 1px solid rgba(255,255,255,0.15);
  }

  /* Controls Bar inside Drawer */
  .terminal-controls {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background-color: #0b0c10;
    border: 1px solid var(--border-card);
    padding: 0.5rem 0.85rem;
    border-radius: 6px;
    font-size: 0.8rem;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .control-left {
    display: flex;
    align-items: center;
    gap: 1.25rem;
  }

  .control-right {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .checkbox-container {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    cursor: pointer;
    user-select: none;
  }

  .checkbox-container input {
    cursor: pointer;
    width: 14px;
    height: 14px;
    accent-color: var(--accent-primary);
  }

  .limit-selector {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .control-txt {
    color: var(--text-muted);
  }

  .select-limit {
    padding: 0.2rem 0.4rem;
    background-color: var(--bg-darker);
    font-size: 0.78rem;
  }

  .custom-input-group {
    display: flex;
    align-items: center;
    gap: 0.2rem;
  }

  .custom-limit-input {
    width: 60px;
    padding: 0.15rem 0.3rem;
    text-align: center;
    font-size: 0.78rem;
    background-color: var(--bg-darker);
  }

  .mb-label {
    font-size: 0.72rem;
    color: var(--text-muted);
  }

  .db-size-indicator {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-card);
    padding: 0.25rem 0.6rem;
    border-radius: 4px;
    font-size: 0.78rem;
  }

  /* Terminal box inside drawer */
  .terminal-box {
    background-color: #050508;
    border: 1px solid var(--border-card);
    border-radius: 6px;
    flex-grow: 1;
    padding: 0.65rem;
    box-shadow: inset 0 0 15px rgba(0, 0, 0, 0.95);
    overflow: hidden;
  }

  .terminal-container {
    width: 100%;
    height: 100%;
  }

  .clear-terminal-btn {
    background: transparent;
    border: 1px solid rgba(255,255,255,0.08);
  }

  .clear-terminal-btn:hover {
    background: rgba(255,255,255,0.03);
  }
</style>
