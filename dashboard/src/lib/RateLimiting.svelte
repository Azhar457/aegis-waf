<script lang="ts">
  import { onMount } from 'svelte';
  import { logs, stats } from './stores';

  export let controllerUrl = '';

  interface RateLimitPolicy {
    name: string;
    limit: string;
    burst: number;
    path: string;
    description: string;
  }

  let limitTiers: RateLimitPolicy[] = [];

  let showModal = false;
  let isEditing = false;
  let editIndex: number | null = null;

  let newTierName = "";
  let newLimit = "";
  let newBurst = 0;
  let newPathPattern = "";
  let newDescription = "";

  // Circular gauge values (telemetry)
  let reservedCapacity = 76.2;
  let maxRps = 12400;
  let avgLoad = 8120;
  let rejectRate = 1.2;

  async function fetchPolicies() {
    try {
      const res = await fetch(`${controllerUrl}/api/v1/rate-limits`);
      if (res.ok) {
        limitTiers = await res.json();
      }
    } catch (e) {
      console.error("Failed to fetch rate limit policies:", e);
    }
  }

  async function savePolicies(updatedTiers: RateLimitPolicy[]) {
    try {
      const res = await fetch(`${controllerUrl}/api/v1/rate-limits`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify(updatedTiers)
      });
      if (res.ok) {
        limitTiers = updatedTiers;
      } else {
        console.error("Failed to save policies on controller:", res.statusText);
      }
    } catch (e) {
      console.error("Error saving policies:", e);
    }
  }

  onMount(() => {
    fetchPolicies();
    
    // Animate circular gauge randomly to simulate real monitoring activity
    const timer = setInterval(() => {
      reservedCapacity = Math.min(100, Math.max(30, Number((reservedCapacity + (Math.random() * 4 - 2)).toFixed(1))));
      maxRps = Math.floor(maxRps + (Math.random() * 200 - 100));
      avgLoad = Math.floor(avgLoad + (Math.random() * 150 - 75));
      rejectRate = Math.min(10, Math.max(0.1, Number((rejectRate + (Math.random() * 0.2 - 0.1)).toFixed(2))));
    }, 3000);

    return () => clearInterval(timer);
  });

  function openCreateModal() {
    isEditing = false;
    editIndex = null;
    newTierName = "";
    newLimit = "";
    newBurst = 0;
    newPathPattern = "/*";
    newDescription = "";
    showModal = true;
  }

  function openEditModal(index: number) {
    isEditing = true;
    editIndex = index;
    const tier = limitTiers[index];
    newTierName = tier.name;
    newLimit = tier.limit;
    newBurst = tier.burst;
    newPathPattern = tier.path;
    newDescription = tier.description;
    showModal = true;
  }

  async function handleAddTier() {
    if (!newTierName || !newLimit) return;

    const newPolicy: RateLimitPolicy = {
      name: newTierName,
      limit: newLimit,
      burst: newBurst,
      path: newPathPattern || "/*",
      description: newDescription
    };

    let updated = [...limitTiers];
    if (isEditing && editIndex !== null) {
      updated[editIndex] = newPolicy;
    } else {
      updated.push(newPolicy);
    }

    await savePolicies(updated);

    newTierName = "";
    newLimit = "";
    newBurst = 0;
    newPathPattern = "";
    newDescription = "";
    showModal = false;
  }

  async function handleDeleteTier(index: number) {
    if (confirm(`Are you sure you want to delete policy: ${limitTiers[index].name}?`)) {
      const updated = limitTiers.filter((_, idx) => idx !== index);
      await savePolicies(updated);
    }
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

  $: rateLimitEvents = $logs.filter(log => 
    log.action.toUpperCase() === 'LIMIT' || 
    log.action.toUpperCase() === 'RATE_LIMIT' || 
    log.action.toUpperCase() === 'RATE' || 
    log.reason.toLowerCase().includes('rate limit') ||
    log.reason.toLowerCase().includes('throttle')
  );
</script>

<div class="rate-limiting-panel flex flex-col gap-lg">
  <!-- Title & Actions -->
  <div class="flex justify-between items-end">
    <div>
      <div class="flex items-center gap-2 text-on-surface-variant text-xs mb-1">
        <span>Aegis WAF</span>
        <span class="material-symbols-outlined text-[12px]">chevron_right</span>
        <span>Configuration</span>
        <span class="material-symbols-outlined text-[12px]">chevron_right</span>
        <span class="text-primary">Rate Limiting</span>
      </div>
      <h1 class="font-headline-md text-headline-md text-on-surface">Rate Limiting Policies</h1>
    </div>
    <div class="flex gap-md">
      <button 
        on:click={openCreateModal}
        class="px-md py-sm bg-primary-container text-on-primary font-bold rounded flex items-center gap-sm hover:brightness-110 active:scale-95 transition-all cursor-pointer border-none"
      >
        <span class="material-symbols-outlined text-sm">add</span>
        Create Policy
      </button>
    </div>
  </div>

  <!-- Bento Telemetry Grid -->
  <div class="grid grid-cols-12 gap-lg">
    <!-- Global Token Bucket Visualizer -->
    <div class="col-span-12 md:col-span-4 glass-panel p-lg rounded-xl flex flex-col items-center justify-center relative overflow-hidden h-[340px]">
      <div class="absolute top-4 left-4 z-10">
        <h3 class="text-on-surface-variant text-xs font-bold uppercase tracking-widest">Global Token Bucket</h3>
        <p class="text-on-surface-variant/60 text-[10px]">Real-time availability</p>
      </div>
      <!-- Circle SVG -->
      <div class="relative flex items-center justify-center">
        <svg class="w-56 h-56 -rotate-90">
          <circle class="text-surface-container-highest" cx="112" cy="112" fill="transparent" r="90" stroke="currentColor" stroke-width="10"></circle>
          <circle 
            class="text-primary transition-all duration-700 ease-out" 
            cx="112" 
            cy="112" 
            fill="transparent" 
            r="90" 
            stroke="currentColor" 
            stroke-dasharray="565" 
            stroke-dashoffset={565 - (565 * reservedCapacity) / 100} 
            stroke-width="10"
          ></circle>
          <circle 
            class="text-primary/30 blur-[4px] transition-all duration-700 ease-out" 
            cx="112" 
            cy="112" 
            fill="transparent" 
            r="90" 
            stroke="currentColor" 
            stroke-dasharray="565" 
            stroke-dashoffset={565 - (565 * reservedCapacity) / 100} 
            stroke-width="10"
          ></circle>
        </svg>
        <div class="absolute text-center flex flex-col items-center">
          <span class="font-metric-lg text-3xl text-primary font-bold">{reservedCapacity}%</span>
          <span class="text-[9px] text-on-surface-variant uppercase tracking-widest mt-1 opacity-60">Reserved Capacity</span>
        </div>
      </div>
      <div class="mt-md w-full flex justify-between px-md text-xs">
        <div class="text-center">
          <p class="text-[9px] text-on-surface-variant mb-1 uppercase font-semibold">Max RPS</p>
          <p class="font-mono text-primary font-bold">{maxRps.toLocaleString()}</p>
        </div>
        <div class="text-center border-x border-outline-variant/30 px-md">
          <p class="text-[9px] text-on-surface-variant mb-1 uppercase font-semibold">Dropped</p>
          <p class="font-mono text-error font-bold">{(rejectRate * 12).toFixed(0)}/sec</p>
        </div>
        <div class="text-center">
          <p class="text-[9px] text-on-surface-variant mb-1 uppercase font-semibold">Avg Load</p>
          <p class="font-mono text-on-surface font-bold">{avgLoad.toLocaleString()}</p>
        </div>
      </div>
    </div>

    <!-- Trends Chart -->
    <div class="col-span-12 md:col-span-8 glass-panel p-lg rounded-xl flex flex-col justify-between h-[340px]">
      <div class="flex justify-between items-center mb-md">
        <div>
          <h3 class="text-on-surface-variant text-xs font-bold uppercase tracking-widest">Rate Limit Exceedance Trend</h3>
          <p class="text-on-surface-variant/60 text-[10px]">Aggregated across all policies (last 60m)</p>
        </div>
        <div class="flex gap-2">
          <span class="flex items-center gap-1 text-[10px] text-on-surface-variant">
            <span class="w-2 h-2 rounded-full bg-primary/40"></span> Active limit
          </span>
          <span class="flex items-center gap-1 text-[10px] text-on-surface-variant">
            <span class="w-2 h-2 rounded-full bg-error"></span> Rejections
          </span>
        </div>
      </div>

      <!-- Chart Bars -->
      <div class="flex-1 w-full bg-surface-container-lowest/40 rounded p-md flex items-end gap-1.5 overflow-hidden">
        <div class="w-full h-full flex items-end justify-between gap-[2px]">
          {#each Array(30) as _, i}
            {@const heightVal = Math.floor(20 + Math.sin(i * 0.3) * 30 + Math.random() * 40)}
            {@const isError = heightVal > 75}
            <div 
              class="flex-1 transition-all duration-300 rounded-t-sm {isError ? 'bg-error' : 'bg-primary/35 hover:bg-primary'}" 
              style="height: {heightVal}%"
              title="Interval {i}: {heightVal}% usage"
            ></div>
          {/each}
        </div>
      </div>

      <div class="grid grid-cols-4 gap-md mt-md">
        <div class="p-sm bg-surface-container/50 rounded border-l-2 border-primary">
          <p class="text-[9px] text-on-surface-variant uppercase font-bold mb-0.5">Global Requests</p>
          <p class="font-mono text-sm font-bold">{$stats.total_requests.toLocaleString()}</p>
        </div>
        <div class="p-sm bg-surface-container/50 rounded border-l-2 border-primary">
          <p class="text-[9px] text-on-surface-variant uppercase font-bold mb-0.5">Rate Limited</p>
          <p class="font-mono text-sm font-bold">{$stats.rate_limited.toLocaleString()}</p>
        </div>
        <div class="p-sm bg-surface-container/50 rounded border-l-2 border-error">
          <p class="text-[9px] text-on-surface-variant uppercase font-bold mb-0.5">Reject Rate</p>
          <p class="font-mono text-sm font-bold text-error">{rejectRate}%</p>
        </div>
        <div class="p-sm bg-surface-container/50 rounded border-l-2 border-outline">
          <p class="text-[9px] text-on-surface-variant uppercase font-bold mb-0.5">Health Score</p>
          <p class="font-mono text-sm font-bold text-primary">99.8/100</p>
        </div>
      </div>
    </div>
  </div>

  <!-- Header for policy list -->
  <div class="flex items-center justify-between mt-xl mb-md">
    <h2 class="text-on-surface font-headline-md text-headline-md flex items-center gap-sm">
      Active Policy Matrix
      <span class="text-xs font-normal text-on-surface-variant px-2 py-0.5 bg-surface-container rounded border border-outline-variant font-mono">
        {limitTiers.length} policies deployed
      </span>
    </h2>
  </div>

  <!-- Policy Matrix Grid -->
  <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-lg">
    {#each limitTiers as tier, index}
      <div class="glass-panel rounded-xl hover:border-primary/40 transition-all group cursor-pointer relative overflow-hidden flex flex-col h-[280px]">
        <div class="p-md flex justify-between items-start border-b border-outline-variant/50">
          <div>
            <div class="flex items-center gap-xs mb-1">
              <span class="w-2 h-2 rounded-full bg-primary animate-pulse shadow-[0_0_8px_rgba(168,232,255,0.4)]"></span>
              <span class="text-xs font-bold text-on-surface tracking-tight truncate max-w-[150px]" title={tier.name}>{tier.name}</span>
            </div>
            <span class="text-[10px] font-mono bg-primary/10 border border-primary/20 text-primary px-1.5 py-0.5 rounded">
              {tier.path}
            </span>
          </div>
          <div class="flex items-center gap-1">
            <button on:click={() => openEditModal(index)} class="text-on-surface-variant hover:text-primary transition-colors cursor-pointer bg-transparent border-none" title="Edit Tier">
              <span class="material-symbols-outlined text-[18px]">edit</span>
            </button>
            <button on:click={() => handleDeleteTier(index)} class="text-on-surface-variant hover:text-error transition-colors cursor-pointer bg-transparent border-none" title="Delete Tier">
              <span class="material-symbols-outlined text-[18px]">delete</span>
            </button>
          </div>
        </div>
        
        <div class="p-md flex-1 flex flex-col justify-between">
          <p class="text-[11px] text-on-surface-variant leading-relaxed line-clamp-3">
            {tier.description || 'Global rate limiting rules for matches in paths.'}
          </p>
          
          <div class="space-y-sm mt-sm">
            <div class="flex justify-between items-center text-xs">
              <span class="text-outline uppercase text-[10px] font-bold">Throttling Level</span>
              <span class="font-mono text-primary font-bold">{tier.limit}</span>
            </div>
            
            <div class="flex justify-between items-center text-xs border-t border-outline-variant/20 pt-sm">
              <span class="text-outline uppercase text-[10px] font-bold font-mono">Burst Capacity</span>
              <span class="font-mono text-on-surface font-semibold">
                {tier.burst > 0 ? `${tier.burst} tokens` : 'N/A'}
              </span>
            </div>
          </div>
        </div>
        
        <div class="p-md bg-surface-container-high/30 border-t border-outline-variant flex justify-between items-center text-[10px] text-on-surface-variant">
          <span class="flex items-center gap-1">
            <span class="material-symbols-outlined text-[12px]">timer</span> Rate Window
          </span>
          <span class="px-2 py-0.5 bg-primary/10 border border-primary/20 text-primary text-[10px] font-bold rounded uppercase">
            Limit & Deny
          </span>
        </div>
      </div>
    {/each}

    <!-- Add New Policy Card Placeholder -->
    <button 
      on:click={openCreateModal}
      class="border-2 border-dashed border-outline-variant rounded-xl hover:border-primary/40 hover:bg-surface-container-low/10 transition-all group cursor-pointer flex flex-col items-center justify-center h-[280px] w-full text-left bg-transparent"
    >
      <div class="w-12 h-12 rounded-full border border-outline-variant flex items-center justify-center group-hover:bg-primary/10 group-hover:border-primary transition-all">
        <span class="material-symbols-outlined text-on-surface-variant group-hover:text-primary">add</span>
      </div>
      <p class="mt-md text-xs font-bold text-on-surface-variant group-hover:text-primary">Create New Policy</p>
      <p class="text-[10px] text-on-surface-variant/60 mt-1">Deploy a new rate-limit rule</p>
    </button>
  </div>

  <!-- Terminal Output / Live Logs Section -->
  <div class="glass-panel rounded-xl overflow-hidden mt-lg border border-outline-variant">
    <div class="bg-surface-container px-lg py-sm border-b border-outline-variant flex justify-between items-center">
      <div class="flex items-center gap-md">
        <span class="text-xs font-bold uppercase tracking-widest flex items-center gap-xs text-on-surface">
          <span class="material-symbols-outlined text-sm text-primary">terminal</span>
          Live Policy Action Log
        </span>
      </div>
      <div class="flex gap-lg">
        <span class="text-[10px] text-on-surface-variant font-mono">Real-time alerts</span>
      </div>
    </div>
    <div class="p-md h-40 bg-[#040508] font-mono text-xs overflow-y-auto space-y-1">
      {#each rateLimitEvents as log}
        <div class="flex gap-lg opacity-80 hover:opacity-100 py-0.5 border-b border-outline-variant/10">
          <span class="text-on-surface-variant/40 shrink-0">{formatTime(log.timestamp)}</span>
          <span class="text-error font-bold w-12">{log.action}</span>
          <span class="text-on-surface-variant truncate w-48">Path: <span class="text-on-surface">{log.path}</span></span>
          <span class="text-on-surface-variant">Source: <span class="text-primary underline">{log.client_ip}</span></span>
          <span class="text-on-surface-variant flex-1 text-right truncate text-outline text-[11px]">{log.reason}</span>
        </div>
      {:else}
        <div class="flex gap-lg opacity-40 italic py-2 justify-center">
          <span class="material-symbols-outlined text-sm">hourglass_empty</span>
          <span class="text-on-surface-variant">Listening for rate-limit violations on active endpoints...</span>
        </div>
      {/each}
    </div>
  </div>
</div>

<!-- Modal Form Overlay -->
{#if showModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-lg overflow-y-auto">
    <div class="glass-panel rounded-xl max-w-lg w-full p-lg shadow-2xl flex flex-col gap-md my-auto border border-outline-variant">
      <div class="flex justify-between items-center border-b border-outline-variant pb-md">
        <h3 class="font-headline-md text-headline-md text-on-surface">{isEditing ? 'Edit Rate Policy' : 'Create Rate Policy'}</h3>
        <button on:click={() => showModal = false} class="text-outline hover:text-primary transition-colors cursor-pointer bg-transparent border-none">
          <span class="material-symbols-outlined">close</span>
        </button>
      </div>

      <div class="flex flex-col gap-sm">
        <div class="flex flex-col gap-1">
          <label for="tier_name" class="text-xs font-bold text-on-surface-variant uppercase tracking-wider">Tier Name</label>
          <input 
            id="tier_name"
            type="text" 
            placeholder="e.g. API Gateway Sync" 
            bind:value={newTierName}
            class="bg-surface-container-low border border-outline-variant rounded p-sm text-sm outline-none focus:border-primary text-on-surface"
          />
        </div>

        <div class="flex flex-col gap-1">
          <label for="path_pattern" class="text-xs font-bold text-on-surface-variant uppercase tracking-wider">Target URL Path Pattern</label>
          <input 
            id="path_pattern"
            type="text" 
            placeholder="e.g. /api/* or /login" 
            bind:value={newPathPattern}
            class="bg-surface-container-low border border-outline-variant rounded p-sm text-sm outline-none focus:border-primary text-on-surface font-mono"
          />
        </div>

        <div class="grid grid-cols-2 gap-sm">
          <div class="flex flex-col gap-1">
            <label for="limit" class="text-xs font-bold text-on-surface-variant uppercase tracking-wider">Rate Limit String</label>
            <input 
              id="limit"
              type="text" 
              placeholder="e.g. 200 requests/minute" 
              bind:value={newLimit}
              class="bg-surface-container-low border border-outline-variant rounded p-sm text-sm outline-none focus:border-primary text-on-surface"
            />
          </div>

          <div class="flex flex-col gap-1">
            <label for="burst" class="text-xs font-bold text-on-surface-variant uppercase tracking-wider">Burst Token Capacity</label>
            <input 
              id="burst"
              type="number" 
              placeholder="e.g. 50" 
              bind:value={newBurst}
              class="bg-surface-container-low border border-outline-variant rounded p-sm text-sm outline-none focus:border-primary text-on-surface font-mono"
            />
          </div>
        </div>

        <div class="flex flex-col gap-1">
          <label for="description" class="text-xs font-bold text-on-surface-variant uppercase tracking-wider">Policy Description</label>
          <textarea 
            id="description" 
            placeholder="Describe what this rate limiting tier is enforced for..." 
            bind:value={newDescription}
            class="bg-surface-container-low border border-outline-variant rounded p-sm text-sm outline-none focus:border-primary text-on-surface h-20 resize-none"
          ></textarea>
        </div>
      </div>

      <div class="flex justify-end gap-md border-t border-outline-variant pt-md mt-md">
        <button 
          on:click={() => showModal = false} 
          class="px-lg py-sm bg-surface-container border border-outline-variant hover:bg-surface-container-high rounded text-sm text-on-surface transition-colors cursor-pointer"
        >
          Cancel
        </button>
        <button 
          on:click={handleAddTier} 
          class="px-lg py-sm bg-primary text-background font-bold rounded text-sm hover:brightness-110 active:scale-95 transition-all cursor-pointer border-none"
        >
          {isEditing ? 'Save Changes' : 'Create Policy'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .glass-panel {
    background: rgba(13, 17, 23, 0.7);
    backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-top: 1px solid rgba(255, 255, 255, 0.15);
  }
</style>
