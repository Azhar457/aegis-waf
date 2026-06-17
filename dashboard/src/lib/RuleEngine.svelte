<script lang="ts">
  import { onMount } from 'svelte';

  export let controllerUrl = '';

  interface CustomRule {
    id: string;
    name: string;
    condition_type: string; // "path", "query", "body", "header:<name>"
    operator: string;       // "equals", "contains", "starts_with"
    condition_value: string;
    action: string;         // "block", "redirect"
    action_value: string;   // Target redirect URL
    enabled: boolean;
  }

  interface VHost {
    name: string;
    hosts: string[];
    backend: string;
    rules: string[];
    blocked_countries: string[];
    geoblock_type: string;
    custom_rules: CustomRule[];
    ssl: string;
    max_body: string;
    rate_limit: string;
  }

  let vhosts: VHost[] = [];
  let selectedVhostIndex = 0;
  
  // Custom Rules form state
  let showModal = false;
  let newRuleId = "";
  let newRuleName = "";
  let conditionFieldType = "path"; // "path", "query", "body", "header"
  let customHeaderName = "User-Agent";
  let newOperator = "contains";    // "equals", "contains", "starts_with"
  let newConditionValue = "";
  let newAction = "block";         // "block", "redirect"
  let newRedirectUrl = "";

  // Preset signatures lists (read-only reference for user)
  let presets = [
    { id: "SQLI-001", name: "SQL Injection (Basic)", category: "SQL Injection", severity: "Critical", description: "Classic SQL injection pattern (OR 1=1, UNION SELECT)" },
    { id: "SQLI-002", name: "SQL Injection (Blind/Time)", category: "SQL Injection", severity: "Critical", description: "Time-based blind SQL injection (SLEEP, WAITFOR)" },
    { id: "SQLI-003", name: "SQL Injection (Union)", category: "SQL Injection", severity: "Critical", description: "UNION SELECT queries to extract DB schema" },
    { id: "XSS-001", name: "XSS - Script Tag", category: "Cross-Site Scripting", severity: "High", description: "Injecting script block or external javascript sources" },
    { id: "XSS-002", name: "XSS - Event Handler", category: "Cross-Site Scripting", severity: "High", description: "HTML event handlers execution (onload, onerror, alert)" },
    { id: "LFI-001", name: "Local File Inclusion", category: "File Inclusion", severity: "High", description: "Path traversal access (/etc/passwd, ../)" },
    { id: "RFI-001", name: "Remote File Inclusion", category: "File Inclusion", severity: "Critical", description: "External script execution via URL inclusion" },
    { id: "BOT-001", name: "Bad User-Agent", category: "Bots & Scanners", severity: "Medium", description: "Known security scanners (sqlmap, nmap, gobuster, wfuzz)" }
  ];

  async function fetchVhosts() {
    try {
      const res = await fetch(`${controllerUrl}/api/v1/vhosts`);
      if (res.ok) {
        vhosts = await res.json();
      }
    } catch (e) {
      console.error("Failed to fetch virtual hosts:", e);
    }
  }

  async function saveVhosts() {
    try {
      const res = await fetch(`${controllerUrl}/api/v1/vhosts`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(vhosts)
      });
      if (res.ok) {
        await fetchVhosts();
      }
    } catch (e) {
      console.error("Failed to save custom rules:", e);
    }
  }

  onMount(() => {
    fetchVhosts();
  });

  function openCreateModal() {
    newRuleId = "CR-" + Math.floor(100 + Math.random() * 900);
    newRuleName = "";
    conditionFieldType = "path";
    customHeaderName = "User-Agent";
    newOperator = "contains";
    newConditionValue = "";
    newAction = "block";
    newRedirectUrl = "";
    showModal = true;
  }

  function handleAddRule() {
    if (!newRuleName || !newConditionValue) return;
    if (newAction === 'redirect' && !newRedirectUrl) return;

    let finalConditionType = conditionFieldType;
    if (conditionFieldType === 'header') {
      finalConditionType = `header:${customHeaderName.trim().toLowerCase()}`;
    }

    const newRule: CustomRule = {
      id: newRuleId,
      name: newRuleName,
      condition_type: finalConditionType,
      operator: newOperator,
      condition_value: newConditionValue,
      action: newAction,
      action_value: newAction === 'redirect' ? newRedirectUrl : "",
      enabled: true
    };

    if (!vhosts[selectedVhostIndex].custom_rules) {
      vhosts[selectedVhostIndex].custom_rules = [];
    }

    vhosts[selectedVhostIndex].custom_rules.push(newRule);
    vhosts = [...vhosts]; // trigger reactivity

    saveVhosts();
    showModal = false;
  }

  function toggleCustomRule(ruleId: string) {
    vhosts[selectedVhostIndex].custom_rules = vhosts[selectedVhostIndex].custom_rules.map(r => {
      if (r.id === ruleId) {
        return { ...r, enabled: !r.enabled };
      }
      return r;
    });
    vhosts = [...vhosts];
    saveVhosts();
  }

  function deleteCustomRule(ruleId: string) {
    if (!confirm("Are you sure you want to delete this custom rule?")) return;
    vhosts[selectedVhostIndex].custom_rules = vhosts[selectedVhostIndex].custom_rules.filter(r => r.id !== ruleId);
    vhosts = [...vhosts];
    saveVhosts();
  }

  function displayCondition(rule: CustomRule): string {
    let field = rule.condition_type;
    if (field.startsWith('header:')) {
      field = `Header [${field.substring(7).toUpperCase()}]`;
    } else {
      field = field.toUpperCase();
    }
    
    let op = rule.operator;
    if (op === 'contains') op = 'contains';
    else if (op === 'equals') op = '=';
    else if (op === 'starts_with') op = 'starts with';

    return `${field} ${op} "${rule.condition_value}"`;
  }
</script>

<div class="rule-panel">
  <!-- Virtual Host Selector -->
  <div class="vhost-selector-bar card">
    <span class="selector-txt font-bold">Manage Rules for Domain:</span>
    {#if vhosts.length > 0}
      <select bind:value={selectedVhostIndex} class="input-field select-vhost font-bold">
        {#each vhosts as host, index}
          <option value={index}>{host.hosts[0]} ({host.name})</option>
        {/each}
      </select>
    {:else}
      <span class="text-muted font-mono">No virtual hosts available</span>
    {/if}
  </div>

  <!-- Main section Grid: Custom Rules and Preset signatures -->
  <div class="rules-grid">
    <!-- Custom Rules Section -->
    <div class="custom-rules-area">
      <div class="area-header">
        <h3 class="panel-subtitle">Custom Enforcements (Conditions & Actions)</h3>
        {#if vhosts.length > 0}
          <button on:click={openCreateModal} class="btn btn-primary btn-sm">+ Add Custom Rule</button>
        {/if}
      </div>

      <div class="table-card card">
        <table class="rules-table">
          <thead>
            <tr>
              <th style="width: 100px;">ID</th>
              <th>Rule Name</th>
              <th>Condition Match</th>
              <th style="width: 100px;">Action</th>
              <th style="width: 80px; text-align: center;">Active</th>
              <th style="width: 80px; text-align: center;">Delete</th>
            </tr>
          </thead>
          <tbody>
            {#if vhosts[selectedVhostIndex] && vhosts[selectedVhostIndex].custom_rules && vhosts[selectedVhostIndex].custom_rules.length > 0}
              {#each vhosts[selectedVhostIndex].custom_rules as rule}
                <tr class={!rule.enabled ? 'rule-disabled' : ''}>
                  <td class="font-mono">{rule.id}</td>
                  <td class="rule-name">{rule.name}</td>
                  <td class="rule-cond font-mono">{displayCondition(rule)}</td>
                  <td>
                    {#if rule.action === 'redirect'}
                      <span class="action-badge redirect-badge" title={rule.action_value}>↪️ REDIRECT</span>
                    {:else}
                      <span class="action-badge block-badge">🔒 BLOCK (403)</span>
                    {/if}
                  </td>
                  <td style="text-align: center;">
                    <input 
                      type="checkbox" 
                      checked={rule.enabled} 
                      on:change={() => toggleCustomRule(rule.id)}
                      class="toggle-checkbox"
                    />
                  </td>
                  <td style="text-align: center;">
                    <button on:click={() => deleteCustomRule(rule.id)} class="delete-icon-btn">🗑️</button>
                  </td>
                </tr>
              {/each}
            {:else}
              <tr>
                <td colspan="6" class="empty-rules-row font-mono text-muted">
                  No custom rules defined for this virtual host. Click "+ Add Custom Rule" to build one.
                </td>
              </tr>
            {/if}
          </tbody>
        </table>
      </div>
    </div>

    <!-- Read-Only Presets Reference -->
    <div class="preset-rules-area">
      <div class="area-header">
        <h3 class="panel-subtitle">Default Rule Modules Reference</h3>
      </div>
      
      <div class="presets-list">
        {#each presets as p}
          <div class="preset-card card">
            <div class="preset-hdr">
              <span class="font-mono badge preset-id">{p.id}</span>
              <strong class="preset-name">{p.name}</strong>
            </div>
            <p class="preset-desc text-muted">{p.description}</p>
          </div>
        {/each}
      </div>
    </div>
  </div>

  <!-- Add Custom Rule Modal -->
  {#if showModal}
    <div class="modal-overlay">
      <div class="modal-content card">
        <h3 class="modal-title">Create Custom Rule Condition</h3>
        
        <div class="form-grid">
          <div class="form-group">
            <label for="rule_id">Rule Signature ID</label>
            <input 
              id="rule_id"
              type="text" 
              bind:value={newRuleId}
              class="input-field font-mono"
              readonly
            />
          </div>

          <div class="form-group">
            <label for="rule_name">Rule Name / Description</label>
            <input 
              id="rule_name"
              type="text" 
              placeholder="e.g. Block admin access" 
              bind:value={newRuleName}
              class="input-field"
            />
          </div>

          <!-- Matching condition fields -->
          <div class="form-group">
            <label for="match_field">Check Target Field</label>
            <select id="match_field" bind:value={conditionFieldType} class="input-field select-input">
              <option value="path">URL Path (e.g. /wp-admin)</option>
              <option value="query">Query Parameter (e.g. id=1)</option>
              <option value="body">Request Body Content</option>
              <option value="header">HTTP Request Header</option>
            </select>
          </div>

          {#if conditionFieldType === 'header'}
            <div class="form-group">
              <label for="header_name">HTTP Header Name</label>
              <input 
                id="header_name"
                type="text" 
                placeholder="e.g. User-Agent or Referer" 
                bind:value={customHeaderName}
                class="input-field"
              />
            </div>
          {:else}
            <div class="form-group">
              <label for="operator">Operator</label>
              <select id="operator" bind:value={newOperator} class="input-field select-input">
                <option value="contains">Contains substring</option>
                <option value="equals">Equals exactly</option>
                <option value="starts_with">Starts with prefix</option>
              </select>
            </div>
          {/if}

          {#if conditionFieldType === 'header'}
            <div class="form-group">
              <label for="operator_hdr">Operator</label>
              <select id="operator_hdr" bind:value={newOperator} class="input-field select-input">
                <option value="contains">Contains substring</option>
                <option value="equals">Equals exactly</option>
                <option value="starts_with">Starts with prefix</option>
              </select>
            </div>
          {/if}

          <div class="form-group font-span-2">
            <label for="match_value">Value to Match</label>
            <input 
              id="match_value"
              type="text" 
              placeholder="e.g. /wp-admin or bad_bot" 
              bind:value={newConditionValue}
              class="input-field"
            />
          </div>

          <!-- Action Fields -->
          <div class="form-group {newAction === 'redirect' ? '' : 'font-span-2'}">
            <label for="action_select">Rule Action</label>
            <select id="action_select" bind:value={newAction} class="input-field select-input">
              <option value="block">Block (Return 403 Forbidden)</option>
              <option value="redirect">Redirect (Return 302 Temporary Redirect)</option>
            </select>
          </div>

          {#if newAction === 'redirect'}
            <div class="form-group">
              <label for="redirect_url">Redirect Target URL</label>
              <input 
                id="redirect_url"
                type="text" 
                placeholder="e.g. http://localhost/blocked.html" 
                bind:value={newRedirectUrl}
                class="input-field"
              />
            </div>
          {/if}
        </div>

        <div class="modal-actions">
          <button on:click={() => showModal = false} class="btn btn-secondary">Cancel</button>
          <button on:click={handleAddRule} class="btn btn-primary font-bold">Add Custom Rule</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .vhost-selector-bar {
    display: flex;
    align-items: center;
    gap: 1.5rem;
    padding: 0.8rem 1.25rem;
    background-color: var(--bg-card);
    border: 1px solid var(--border-card);
    margin-bottom: 1.5rem;
  }

  .selector-txt {
    font-size: 0.9rem;
    color: var(--text-muted);
  }

  .select-vhost {
    background-color: var(--bg-darker);
    padding: 0.4rem 1rem;
    font-size: 0.9rem;
    color: white;
    min-width: 250px;
    border: 1px solid var(--border-card);
  }

  .rules-grid {
    display: grid;
    grid-template-columns: 2.2fr 1fr;
    gap: 1.5rem;
  }

  .area-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .panel-subtitle {
    font-size: 1rem;
    font-weight: 700;
    color: #ffffff;
    margin: 0;
  }

  .table-card {
    padding: 0;
    overflow: hidden;
  }

  .rules-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
    text-align: left;
  }

  .rules-table th {
    background-color: rgba(0, 0, 0, 0.25);
    border-bottom: 1px solid var(--border-card);
    padding: 0.85rem 1rem;
    color: var(--text-muted);
    font-weight: 600;
    text-transform: uppercase;
    font-size: 0.72rem;
    letter-spacing: 0.5px;
  }

  .rules-table td {
    padding: 0.85rem 1rem;
    border-bottom: 1px solid var(--border-card);
  }

  .rules-table tr:last-child td {
    border-bottom: none;
  }

  .rules-table tr:hover {
    background-color: rgba(255, 255, 255, 0.015);
  }

  .rule-name {
    font-weight: 600;
    color: white;
  }

  .rule-cond {
    color: var(--text-muted);
    font-size: 0.8rem;
  }

  .action-badge {
    font-size: 0.75rem;
    padding: 0.15rem 0.4rem;
    border-radius: 4px;
    font-weight: 700;
  }

  .block-badge {
    background-color: rgba(244, 63, 94, 0.15);
    color: var(--color-critical);
    border: 1px solid rgba(244, 63, 94, 0.2);
  }

  .redirect-badge {
    background-color: rgba(59, 130, 246, 0.15);
    color: #3b82f6;
    border: 1px solid rgba(59, 130, 246, 0.2);
  }

  .rule-disabled {
    opacity: 0.4;
  }

  .toggle-checkbox {
    cursor: pointer;
    width: 15px;
    height: 15px;
    accent-color: var(--accent-primary);
  }

  .delete-icon-btn {
    background: transparent;
    border: none;
    cursor: pointer;
    font-size: 0.95rem;
    opacity: 0.6;
    transition: all 0.2s;
  }

  .delete-icon-btn:hover {
    opacity: 1;
    color: var(--color-critical);
  }

  .empty-rules-row {
    text-align: center;
    padding: 3rem 1rem;
  }

  /* Preset signatures styling */
  .presets-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .preset-card {
    padding: 0.85rem 1.25rem;
    background-color: rgba(255, 255, 255, 0.01);
  }

  .preset-hdr {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.25rem;
  }

  .preset-id {
    font-size: 0.7rem;
    background-color: rgba(255,255,255,0.05);
    border: 1px solid var(--border-card);
    color: white;
  }

  .preset-name {
    font-size: 0.85rem;
    color: white;
  }

  .preset-desc {
    font-size: 0.75rem;
    margin: 0;
  }

  /* Modal Overlay */
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
</style>
