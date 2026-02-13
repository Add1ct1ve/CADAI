<script lang="ts">
  import { onMount } from 'svelte';
  import { getMechanismStore } from '$lib/stores/mechanisms.svelte';
  import { buildMechanismInsertPrompt } from '$lib/services/mechanisms';
  import type { MechanismItem } from '$lib/types';

  interface Props {
    open: boolean;
    onClose: () => void;
    onStatus?: (msg: string) => void;
  }

  let { open, onClose, onStatus = () => {} }: Props = $props();

  const store = getMechanismStore();

  let query = $state('');
  let importUrl = $state('');
  let selectedId = $state<string | null>(null);
  let busy = $state(false);

  const filtered = $derived(store.mechanisms);
  const selected = $derived(filtered.find((m) => m.id === selectedId) ?? null);

  async function refresh() {
    await store.refresh();
    if (!selectedId && store.mechanisms.length > 0) {
      selectedId = store.mechanisms[0].id;
    }
  }

  async function handleSearch() {
    await store.search(query);
    if (!selectedId && store.mechanisms.length > 0) {
      selectedId = store.mechanisms[0].id;
    }
  }

  function emitInsert(mechanism: MechanismItem) {
    const prompt = buildMechanismInsertPrompt(mechanism);
    window.dispatchEvent(
      new CustomEvent('mechanism:insert-prompt', { detail: { prompt, mechanismId: mechanism.id } })
    );
    onStatus(`Inserted mechanism context: ${mechanism.title}`);
    onClose();
  }

  async function handleInstall() {
    if (!importUrl.trim()) return;
    busy = true;
    try {
      const report = await store.install(importUrl.trim());
      importUrl = '';
      onStatus(`Installed ${report.installed_count} mechanisms from ${report.package_name}`);
    } catch (err) {
      onStatus(`Install failed: ${err}`);
    } finally {
      busy = false;
    }
  }

  async function removePackage(packageId: string) {
    busy = true;
    try {
      const removed = await store.remove(packageId);
      onStatus(removed ? `Removed package ${packageId}` : `Package not found: ${packageId}`);
    } catch (err) {
      onStatus(`Remove failed: ${err}`);
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    if (open) {
      refresh();
    }
  });

  onMount(() => {
    if (open) {
      refresh();
    }
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="overlay" onclick={onClose} role="presentation">
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="panel" role="dialog" aria-label="Mechanism Catalog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <div class="header">
        <h2>Mechanism Catalog</h2>
        <button class="close" onclick={onClose}>&times;</button>
      </div>

      <div class="toolbar">
        <input
          class="search"
          type="text"
          placeholder="Search mechanisms (snap-fit, hinge, o-ring...)"
          bind:value={query}
          onkeydown={(e) => e.key === 'Enter' ? handleSearch() : null}
        />
        <button onclick={handleSearch} disabled={store.loading}>Search</button>
        <button onclick={refresh} disabled={store.loading}>Reset</button>
      </div>

      <div class="import-row">
        <input class="import" type="url" placeholder="Import manifest URL (open-license packs only)" bind:value={importUrl} />
        <button onclick={handleInstall} disabled={busy || !importUrl.trim()}>Import Pack</button>
      </div>

      {#if store.loadError}
        <div class="error">{store.loadError}</div>
      {/if}

      <div class="content">
        <div class="list">
          {#each filtered as m (m.package_id + ':' + m.id)}
            <button class:selected={m.id === selectedId} class="item" onclick={() => selectedId = m.id}>
              <div class="item-title">{m.title}</div>
              <div class="item-meta">{m.category} · {m.package_name}</div>
            </button>
          {/each}
        </div>

        <div class="details">
          {#if selected}
            <h3>{selected.title}</h3>
            <div class="muted">ID: <code>{selected.id}</code></div>
            <p>{selected.summary}</p>
            <div class="tags">
              {#each selected.keywords as keyword}
                <span>{keyword}</span>
              {/each}
            </div>
            <h4>Default Parameters</h4>
            {#if selected.parameters.length === 0}
              <p class="muted">No default parameters defined.</p>
            {:else}
              <ul>
                {#each selected.parameters as p}
                  <li><code>{p.name}</code> = {p.default_value}{p.unit ? ` ${p.unit}` : ''}{p.description ? ` — ${p.description}` : ''}</li>
                {/each}
              </ul>
            {/if}

            <h4>Guidance</h4>
            <pre>{selected.prompt_block}</pre>

            <div class="actions">
              <button class="insert" onclick={() => emitInsert(selected)}>Insert In Chat</button>
            </div>
          {:else}
            <p class="muted">Select a mechanism to view details.</p>
          {/if}

          {#if store.packages.length > 0}
            <h4>Installed Packages</h4>
            <div class="packages">
              {#each store.packages as p (p.package_id)}
                <div class="pkg">
                  <div>
                    <strong>{p.name}</strong>
                    <div class="muted"><code>{p.package_id}</code> · {p.mechanism_count} mechanisms · {p.license}</div>
                  </div>
                  {#if p.is_imported}
                    <button class="danger" onclick={() => removePackage(p.package_id)} disabled={busy}>Remove</button>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    z-index: 200;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .panel {
    width: min(1100px, 92vw);
    max-height: 88vh;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 12px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .header h2 {
    margin: 0;
    font-size: 15px;
  }
  .close {
    background: transparent;
    border: none;
    color: var(--text-primary);
    font-size: 20px;
    cursor: pointer;
  }
  .toolbar, .import-row {
    display: flex;
    gap: 8px;
    padding: 10px 14px;
  }
  .search, .import {
    flex: 1;
    background: var(--bg-mantle);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: 8px 10px;
  }
  .error {
    margin: 0 14px 10px;
    font-size: 12px;
    color: #f38ba8;
  }
  .content {
    min-height: 0;
    display: grid;
    grid-template-columns: 320px 1fr;
    gap: 10px;
    padding: 0 14px 14px;
    overflow: hidden;
  }
  .list {
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    background: var(--bg-mantle);
    overflow: auto;
    min-height: 0;
  }
  .item {
    width: 100%;
    text-align: left;
    background: transparent;
    color: var(--text-primary);
    border: none;
    border-bottom: 1px solid var(--border-subtle);
    padding: 10px;
    cursor: pointer;
  }
  .item:last-child { border-bottom: none; }
  .item.selected { background: rgba(137, 180, 250, 0.14); }
  .item-title { font-weight: 600; font-size: 13px; }
  .item-meta { font-size: 11px; opacity: 0.8; margin-top: 2px; }
  .details {
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    background: var(--bg-mantle);
    padding: 12px;
    overflow: auto;
    min-height: 0;
  }
  h3, h4 { margin: 6px 0; }
  .muted { opacity: 0.75; font-size: 12px; }
  .tags { display: flex; flex-wrap: wrap; gap: 6px; margin: 8px 0; }
  .tags span {
    padding: 3px 8px;
    border-radius: 999px;
    border: 1px solid var(--border-subtle);
    font-size: 11px;
  }
  pre {
    white-space: pre-wrap;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', monospace;
    background: var(--bg-crust);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: 10px;
    font-size: 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 10px;
  }
  .insert {
    background: #2f7d32;
    color: white;
    border: none;
    border-radius: 8px;
    padding: 8px 12px;
    cursor: pointer;
  }
  .packages {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .pkg {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: 8px;
  }
  .danger {
    background: #8b2f2f;
    color: white;
    border: none;
    border-radius: 8px;
    padding: 6px 10px;
    cursor: pointer;
  }
  @media (max-width: 900px) {
    .content {
      grid-template-columns: 1fr;
    }
    .list {
      max-height: 220px;
    }
  }
</style>
