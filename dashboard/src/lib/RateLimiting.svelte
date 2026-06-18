<script lang="ts">
  import { onMount } from 'svelte';

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
  let newTierName = "";
  let newLimit = "";
  let newBurst = 0;
  let newPathPattern = "";
  let newDescription = "";

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

  async function handleAddTier() {
    if (!newTierName || !newLimit) return;

    const newPolicy: RateLimitPolicy = {
      name: newTierName,
      limit: newLimit,
      burst: newBurst,
      path: newPathPattern || "/*",
      description: newDescription
    };

    const updated = [...limitTiers, newPolicy];
    await savePolicies(updated);

    // Reset Form
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

  onMount(() => {
    fetchPolicies();
  });
</script>

<div class="rate-limit-panel">
  <div class="panel-header">
    <h3 class="panel-subtitle">Global Rate Limiting Policies</h3>
    <button on:click={() => showModal = true} class="btn btn-primary btn-sm">+ Add Policy Tier</button>
  </div>

  <div class="tier-list">
    {#each limitTiers as tier, index}
      <div class="card tier-card">
        <div class="tier-header">
          <div class="tier-meta">
            <h4>{tier.name}</h4>
            <span class="path-badge font-mono">{tier.path}</span>
          </div>
          <span class="limit-badge">{tier.limit}</span>
        </div>
        <p class="tier-desc">{tier.description}</p>
        <div class="tier-footer" style="display: flex; justify-content: space-between; align-items: center;">
          <span class="detail font-mono">Burst Capacity: {tier.burst > 0 ? `${tier.burst} tokens` : 'N/A'}</span>
          <button on:click={() => handleDeleteTier(index)} class="btn-critical-outline btn-sm">Delete</button>
        </div>
      </div>
    {/each}
  </div>

  <!-- Add Policy Tier Modal -->
  {#if showModal}
    <div class="modal-overlay">
      <div class="modal-content card">
        <h3 class="modal-title">Create Rate Limit Policy Tier</h3>
        
        <div class="form-grid">
          <div class="form-group font-span-2">
            <label for="tier_name">Tier Name</label>
            <input 
              id="tier_name"
              type="text" 
              placeholder="e.g. Nextcloud WebDAV Sync" 
              bind:value={newTierName}
              class="input-field"
            />
          </div>

          <div class="form-group">
            <label for="limit">Request Rate Limit</label>
            <input 
              id="limit"
              type="text" 
              placeholder="e.g. 2000 requests / minute" 
              bind:value={newLimit}
              class="input-field"
            />
          </div>

          <div class="form-group">
            <label for="burst">Burst Token Capacity</label>
            <input 
              id="burst"
              type="number" 
              placeholder="e.g. 200" 
              bind:value={newBurst}
              class="input-field"
            />
          </div>

          <div class="form-group font-span-2">
            <label for="path_pattern">Endpoint Path Pattern</label>
            <input 
              id="path_pattern"
              type="text" 
              placeholder="e.g. /remote.php/dav/*" 
              bind:value={newPathPattern}
              class="input-field"
            />
          </div>

          <div class="form-group font-span-2">
            <label for="description">Tier Description</label>
            <input 
              id="description"
              type="text" 
              placeholder="Brief explanation of this rate threshold application" 
              bind:value={newDescription}
              class="input-field"
            />
          </div>
        </div>

        <div class="modal-actions">
          <button on:click={() => showModal = false} class="btn btn-secondary">Cancel</button>
          <button on:click={handleAddTier} class="btn btn-primary">Add Policy</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1.5rem;
  }

  .panel-subtitle {
    font-size: 1rem;
    font-weight: 600;
    color: #ffffff;
  }

  .tier-list {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .tier-card {
    padding: 1.25rem 1.5rem;
  }

  .tier-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 0.75rem;
  }

  .tier-meta {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .tier-meta h4 {
    font-size: 1.05rem;
    font-weight: 600;
    color: white;
  }

  .path-badge {
    font-size: 0.75rem;
    color: var(--accent-primary);
    background: rgba(99, 102, 241, 0.05);
    border: 1px dashed rgba(99, 102, 241, 0.2);
    padding: 0.1rem 0.4rem;
    border-radius: 4px;
    align-self: flex-start;
  }

  .limit-badge {
    font-size: 0.85rem;
    font-weight: 700;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--border-card);
    padding: 0.3rem 0.75rem;
    border-radius: 6px;
    color: var(--color-high);
  }

  .tier-desc {
    font-size: 0.85rem;
    color: var(--text-muted);
    margin-bottom: 0.75rem;
    line-height: 1.4;
  }

  .tier-footer {
    font-size: 0.75rem;
    color: var(--text-dark);
    border-top: 1px solid var(--border-card);
    padding-top: 0.5rem;
  }

  /* Modal Overlay Styles */
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background-color: rgba(5, 5, 8, 0.85);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal-content {
    width: 100%;
    max-width: 550px;
    padding: 2rem;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .modal-title {
    font-size: 1.25rem;
    font-weight: 700;
    color: white;
    letter-spacing: -0.2px;
    border-bottom: 1px solid var(--border-card);
    padding-bottom: 0.75rem;
  }

  .form-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 1rem;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .form-group.font-span-2 {
    grid-column: span 2;
  }

  .form-group label {
    font-size: 0.75rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    font-weight: 500;
  }

  .form-group .input-field {
    width: 100%;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
    border-top: 1px solid var(--border-card);
    padding-top: 1.25rem;
  }

  .btn-critical-outline {
    background: transparent;
    border: 1px solid rgba(244, 63, 94, 0.4);
    color: var(--color-critical);
    cursor: pointer;
    border-radius: 4px;
    transition: all 0.2s;
  }

  .btn-critical-outline:hover {
    background: rgba(244, 63, 94, 0.1);
    border-color: var(--color-critical);
  }
</style>
