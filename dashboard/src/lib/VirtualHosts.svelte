<script lang="ts">
  import { onMount } from 'svelte';

  export let controllerUrl = '';
  let vhosts: any[] = [];

  let showModal = false;
  let isEditing = false;
  let editIndex: number | null = null;

  let newServerName = "";
  let newUpstream = "";
  let newSsl = "Auto (Local CA)";
  let newMaxBody = "10MB";
  let newRateLimit = "600 req/min";
  let blockedCountries: string[] = [];
  let geoblockType = "Blocklist";

  // Batching Mode Category Selection
  let selectedCategories = {
    sqli: true,
    xss: true,
    lfi: true,
    cmdi: false,
    ssrf: false,
    bot: false
  };

  const availableCountries = [
    { code: 'US', name: 'United States', flag: '🇺🇸' },
    { code: 'CN', name: 'China', flag: '🇨🇳' },
    { code: 'RU', name: 'Russia', flag: '🇷🇺' },
    { code: 'DE', name: 'Germany', flag: '🇩🇪' },
    { code: 'SG', name: 'Singapore', flag: '🇸🇬' },
    { code: 'ID', name: 'Indonesia', flag: '🇮🇩' },
    { code: 'BR', name: 'Brazil', flag: '🇧🇷' },
    { code: 'AU', name: 'Australia', flag: '🇦🇺' }
  ];

  const flags: { [code: string]: string } = {
    'US': '🇺🇸',
    'DE': '🇩🇪',
    'RU': '🇷🇺',
    'CN': '🇨🇳',
    'SG': '🇸🇬',
    'ID': '🇮🇩',
    'BR': '🇧🇷',
    'AU': '🇦🇺'
  };

  async function fetchVhosts() {
    try {
      const res = await fetch(`${controllerUrl}/api/v1/vhosts`);
      if (res.ok) {
        const raw = await res.json();
        vhosts = raw.map((item: any) => ({
          server_name: item.hosts && item.hosts.length > 0 ? item.hosts[0] : item.name,
          upstream: item.backend,
          ssl: item.ssl || "Disabled",
          max_body: item.max_body || "10MB",
          rate_limit: item.rate_limit || "600 req/min",
          rules: item.rules || [],
          blocked_countries: item.blocked_countries || [],
          geoblock_type: item.geoblock_type || "Blocklist",
          custom_rules: item.custom_rules || [],
          rate_limit_tiers: item.rate_limit_tiers || [],
          logging: item.logging || null,
          status: "online"
        }));
      }
    } catch (e) {
      console.error("Failed to fetch virtual hosts:", e);
    }
  }

  async function saveVhosts(updatedVhosts: any[]) {
    const payload = updatedVhosts.map((item: any) => ({
      name: item.server_name.replace(/[^a-zA-Z0-9.-]/g, '_').toLowerCase(),
      hosts: [item.server_name],
      backend: item.upstream,
      ssl: item.ssl,
      max_body: item.max_body,
      rate_limit: item.rate_limit,
      rules: item.rules,
      blocked_countries: item.blocked_countries || [],
      geoblock_type: item.geoblock_type || "Blocklist",
      custom_rules: item.custom_rules || [],
      rate_limit_tiers: item.rate_limit_tiers || [],
      logging: item.logging || { enabled: true, db_path: "logs/aegis-waf.db" }
    }));

    try {
      const res = await fetch(`${controllerUrl}/api/v1/vhosts`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        await fetchVhosts();
      } else {
        console.error("Failed to save virtual hosts");
      }
    } catch (e) {
      console.error("Error saving virtual hosts:", e);
    }
  }

  onMount(() => {
    fetchVhosts();
  });

  function openCreateModal() {
    isEditing = false;
    editIndex = null;
    newServerName = "";
    newUpstream = "";
    newSsl = "Auto (Local CA)";
    newMaxBody = "10MB";
    newRateLimit = "600 req/min";
    blockedCountries = [];
    geoblockType = "Blocklist";
    selectedCategories = {
      sqli: true,
      xss: true,
      lfi: true,
      cmdi: false,
      ssrf: false,
      bot: false
    };
    showModal = true;
  }

  function openEditModal(index: number) {
    isEditing = true;
    editIndex = index;
    const host = vhosts[index];
    
    newServerName = host.server_name;
    newUpstream = host.upstream;
    newSsl = host.ssl;
    newMaxBody = host.max_body;
    newRateLimit = host.rate_limit;
    blockedCountries = host.blocked_countries || [];
    geoblockType = host.geoblock_type || "Blocklist";
    
    selectedCategories = {
      sqli: host.rules.includes("SQLI-*"),
      xss: host.rules.includes("XSS-*"),
      lfi: host.rules.includes("LFI-*"),
      cmdi: host.rules.includes("CMDI-*"),
      ssrf: host.rules.includes("SSRF-*"),
      bot: host.rules.includes("BOT-*")
    };
    showModal = true;
  }

  function handleDeleteVhost(index: number) {
    const updated = vhosts.filter((_, i) => i !== index);
    saveVhosts(updated);
  }

  function toggleCountry(code: string, checked: boolean) {
    if (checked) {
      blockedCountries = [...blockedCountries, code];
    } else {
      blockedCountries = blockedCountries.filter(c => c !== code);
    }
  }

  function handleSaveVhost() {
    if (!newServerName || !newUpstream) return;
    
    // Compile active wildcard pattern rules based on batch checkboxes
    let activeRules: string[] = [];
    if (selectedCategories.sqli) activeRules.push("SQLI-*");
    if (selectedCategories.xss) activeRules.push("XSS-*");
    if (selectedCategories.lfi) {
      activeRules.push("LFI-*");
      activeRules.push("RFI-*");
    }
    if (selectedCategories.cmdi) activeRules.push("CMDI-*");
    if (selectedCategories.ssrf) activeRules.push("SSRF-*");
    if (selectedCategories.bot) activeRules.push("BOT-*");

    const hostData = {
      server_name: newServerName,
      upstream: newUpstream,
      ssl: newSsl,
      max_body: newMaxBody,
      rate_limit: newRateLimit,
      rules: activeRules,
      blocked_countries: blockedCountries,
      geoblock_type: geoblockType,
      custom_rules: isEditing && editIndex !== null ? vhosts[editIndex].custom_rules : [],
      rate_limit_tiers: isEditing && editIndex !== null ? vhosts[editIndex].rate_limit_tiers : [],
      logging: isEditing && editIndex !== null ? vhosts[editIndex].logging : null,
      status: "online"
    };

    let updated = [...vhosts];
    if (isEditing && editIndex !== null) {
      updated[editIndex] = hostData;
    } else {
      updated.push(hostData);
    }

    saveVhosts(updated);
    showModal = false;
  }
</script>

<div class="vhosts-panel animate-fade-in">
  <div class="panel-header">
    <h3 class="panel-subtitle">Virtual Host Routing Rules</h3>
    <button on:click={openCreateModal} class="btn btn-primary btn-sm">+ Add Virtual Host</button>
  </div>

  <div class="vhost-cards">
    {#if vhosts.length === 0}
      <div class="empty-state card">
        <span class="empty-icon">🌐</span>
        <h4>No Virtual Hosts Configured</h4>
        <p>Click "+ Add Virtual Host" above to route your domains through the WAF.</p>
      </div>
    {/if}

    {#each vhosts as host, i}
      <div class="card vhost-card">
        <div class="vhost-header">
          <div class="vhost-title">
            <span class="dot online"></span>
            <h4>{host.server_name}</h4>
          </div>
          <div class="header-actions">
            <span class="badge ssl-badge {host.ssl !== 'Disabled' ? 'ssl-active' : 'ssl-disabled'}">
              🔒 {host.ssl}
            </span>
            <button on:click={() => openEditModal(i)} class="action-btn edit-btn" title="Edit Host">✏️</button>
            <button on:click={() => handleDeleteVhost(i)} class="action-btn delete-btn" title="Delete Host">🗑️</button>
          </div>
        </div>

        <div class="vhost-details">
          <div class="detail-row">
            <span class="label">Upstream Proxy:</span>
            <span class="val font-mono">{host.upstream}</span>
          </div>
          <div class="detail-row">
            <span class="label">Max Body Size:</span>
            <span class="val font-mono">{host.max_body}</span>
          </div>
          <div class="detail-row">
            <span class="label">Rate Limiter:</span>
            <span class="val">{host.rate_limit}</span>
          </div>
          
          <div class="rules-section">
            <span class="label">Enabled Rules:</span>
            <div class="rules-tags">
              {#each host.rules as rule}
                <span class="rule-tag font-mono">{rule}</span>
              {:else}
                <span class="val text-muted">All rules disabled</span>
              {/each}
            </div>
          </div>

          <div class="geoblock-section">
            <span class="label">Geoblocking ({host.geoblock_type}):</span>
            <div class="blocked-countries-tags">
              {#each host.blocked_countries as code}
                <span class="country-tag {host.geoblock_type === 'Allowlist' ? 'allowlist-tag' : 'blocklist-tag'}">{flags[code] || '🌐'} {code}</span>
              {:else}
                {#if host.geoblock_type === 'Allowlist'}
                  <span class="val text-muted font-bold" style="color: var(--color-critical)">Lockdown (Block All Countries)</span>
                {:else}
                  <span class="val text-muted font-bold" style="color: var(--color-pass)">None (Open Access)</span>
                {/if}
              {/each}
            </div>
          </div>
        </div>
      </div>
    {/each}
  </div>

  <!-- Modal Form Overlay -->
  {#if showModal}
    <div class="modal-overlay">
      <div class="modal-content card">
        <h3 class="modal-title">{isEditing ? 'Edit Virtual Host' : 'Create Virtual Host'}</h3>
        
        <div class="form-grid">
          <div class="form-group">
            <label for="server_name">Server Name (Domain / Wildcard)</label>
            <input 
              id="server_name"
              type="text" 
              placeholder="e.g. *.domainsaya.my.id" 
              bind:value={newServerName}
              class="input-field"
            />
          </div>

          <div class="form-group">
            <label for="upstream">Upstream Backend Port</label>
            <input 
              id="upstream"
              type="text" 
              placeholder="e.g. 127.0.0.1:8080" 
              bind:value={newUpstream}
              class="input-field"
            />
          </div>

          <div class="form-group">
            <label for="ssl">SSL Encryption Mode</label>
            <select id="ssl" bind:value={newSsl} class="input-field select-input">
              <option value="Auto (Local CA)">Auto (Local CA)</option>
              <option value="Manual Cert">Manual Certificate</option>
              <option value="Disabled">Disabled (HTTP Only)</option>
            </select>
          </div>

          <div class="form-group">
            <label for="max_body">Max Request Body Size</label>
            <input 
              id="max_body"
              type="text" 
              placeholder="e.g. 10MB" 
              bind:value={newMaxBody}
              class="input-field"
            />
          </div>

          <div class="form-group font-span-2">
            <label for="rate_limit">Rate Limiter Threshold</label>
            <input 
              id="rate_limit"
              type="text" 
              placeholder="e.g. 600 req/min" 
              bind:value={newRateLimit}
              class="input-field"
            />
          </div>

          <!-- Batching Checkbox Selector for rules -->
          <div class="form-group font-span-2">
            <span class="form-section-label">Enable WAF Rule Modules (Batching Mode)</span>
            <div class="rules-checkbox-grid">
              <label class="checkbox-label-vhost">
                <input type="checkbox" bind:checked={selectedCategories.sqli} />
                <span>SQL Injection (SQLI-*)</span>
              </label>
              <label class="checkbox-label-vhost">
                <input type="checkbox" bind:checked={selectedCategories.xss} />
                <span>Cross-Site Scripting (XSS-*)</span>
              </label>
              <label class="checkbox-label-vhost">
                <input type="checkbox" bind:checked={selectedCategories.lfi} />
                <span>File Inclusion (LFI-* / RFI-*)</span>
              </label>
              <label class="checkbox-label-vhost">
                <input type="checkbox" bind:checked={selectedCategories.cmdi} />
                <span>Command Injection (CMDI-*)</span>
              </label>
              <label class="checkbox-label-vhost">
                <input type="checkbox" bind:checked={selectedCategories.ssrf} />
                <span>Request Forgery (SSRF-*)</span>
              </label>
              <label class="checkbox-label-vhost">
                <input type="checkbox" bind:checked={selectedCategories.bot} />
                <span>Bots & Scanners (BOT-*)</span>
              </label>
            </div>
          </div>

          <!-- Geoblocking Strategy Selection -->
          <div class="form-group font-span-2">
            <label for="geoblock_type">Geoblocking Strategy</label>
            <select id="geoblock_type" bind:value={geoblockType} class="input-field select-input">
              <option value="Blocklist">Blocklist (Block selected countries, allow all others)</option>
              <option value="Allowlist">Allowlist (Allow ONLY selected countries, block all others)</option>
            </select>
          </div>

          <!-- Geoblocking Checkboxes -->
          <div class="form-group font-span-2">
            <span class="form-section-label">Geoblocking Countries ({geoblockType === 'Allowlist' ? 'Allow List' : 'Block List'})</span>
            <div class="countries-checkbox-grid">
              {#each availableCountries as country}
                <label class="checkbox-label-vhost">
                  <input
                    type="checkbox"
                    value={country.code}
                    checked={blockedCountries.includes(country.code)}
                    on:change={(e) => toggleCountry(country.code, (e.target as HTMLInputElement).checked)}
                  />
                  <span>{country.flag} {country.name}</span>
                </label>
              {/each}
            </div>
          </div>
        </div>

        <div class="modal-actions">
          <button on:click={() => showModal = false} class="btn btn-secondary">Cancel</button>
          <button on:click={handleSaveVhost} class="btn btn-primary font-bold">
            {isEditing ? 'Save Changes' : 'Create Host'}
          </button>
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

  .vhost-cards {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .vhost-card {
    padding: 1.25rem 1.5rem;
  }

  .vhost-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--border-card);
    padding-bottom: 0.75rem;
    margin-bottom: 1rem;
  }

  .vhost-title {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .vhost-title h4 {
    font-size: 1.1rem;
    font-weight: 600;
    color: white;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .action-btn {
    background: transparent;
    border: none;
    cursor: pointer;
    font-size: 0.95rem;
    padding: 0.2rem;
    border-radius: 4px;
    transition: all 0.2s;
    user-select: none;
  }

  .action-btn:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .delete-btn:hover {
    color: var(--color-critical);
    filter: drop-shadow(0 0 4px rgba(244, 63, 94, 0.5));
  }

  .ssl-badge {
    font-size: 0.75rem;
    margin-right: 0.25rem;
  }

  .ssl-active {
    background-color: rgba(16, 185, 129, 0.1);
    color: var(--color-pass);
  }

  .ssl-disabled {
    background-color: rgba(244, 63, 94, 0.1);
    color: var(--color-critical);
  }

  .vhost-details {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1rem;
  }

  .detail-row {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .label {
    font-size: 0.75rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .val {
    font-size: 0.85rem;
    color: var(--text-main);
  }

  .rules-section, .geoblock-section {
    grid-column: span 3;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin-top: 0.5rem;
    border-top: 1px dashed rgba(255, 255, 255, 0.05);
    padding-top: 0.75rem;
  }

  .rules-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .rule-tag {
    font-size: 0.75rem;
    background-color: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--border-card);
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    color: var(--text-muted);
  }

  /* Blocked countries tags */
  .blocked-countries-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .country-tag {
    font-size: 0.75rem;
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    font-weight: 600;
  }
  
  .country-tag.blocklist-tag {
    background-color: rgba(244, 63, 94, 0.15);
    color: var(--color-critical);
    border: 1px solid rgba(244, 63, 94, 0.2);
  }

  .country-tag.allowlist-tag {
    background-color: rgba(16, 185, 129, 0.15);
    color: var(--color-pass);
    border: 1px solid rgba(16, 185, 129, 0.2);
  }

  /* Modal Styles */
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
    max-width: 580px;
    padding: 2rem;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    max-height: 90vh;
    overflow-y: auto;
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

  .select-input {
    background-color: var(--bg-darker);
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
    border-top: 1px solid var(--border-card);
    padding-top: 1.25rem;
  }

  /* Rules Checkbox Grid styling */
  .rules-checkbox-grid, .countries-checkbox-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 0.75rem;
    background-color: rgba(0, 0, 0, 0.2);
    border: 1px solid var(--border-card);
    padding: 1rem;
    border-radius: 6px;
    margin-top: 0.25rem;
  }

  .form-section-label {
    font-size: 0.75rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    font-weight: 500;
  }

  .checkbox-label-vhost {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    font-size: 0.8rem;
    color: var(--text-main);
    user-select: none;
  }

  .checkbox-label-vhost input {
    cursor: pointer;
    width: 15px;
    height: 15px;
    accent-color: var(--accent-primary);
  }

  /* Empty State */
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 4rem 2rem;
    text-align: center;
    color: var(--text-muted);
    gap: 0.5rem;
  }

  .empty-icon {
    font-size: 2.5rem;
    margin-bottom: 0.5rem;
  }

  .empty-state h4 {
    color: white;
    font-weight: 600;
  }
</style>
