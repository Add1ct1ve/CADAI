<script lang="ts">
  import { getDrawingStore } from '$lib/stores/drawing.svelte';
  import { generateView, regenerateAllViews, exportPdf, exportDxf } from '$lib/services/drawing-service';
  import type { PaperSize, PaperOrientation } from '$lib/types/drawing';

  const store = getDrawingStore();

  const drawing = $derived(store.activeDrawing);
  const selectedView = $derived(
    drawing && store.selectedViewId
      ? drawing.views.find((v) => v.id === store.selectedViewId) ?? null
      : null
  );
  const selectedDim = $derived(
    drawing && store.selectedDimensionId
      ? drawing.dimensions.find((d) => d.id === store.selectedDimensionId) ?? null
      : null
  );
  const selectedNote = $derived(
    drawing && store.selectedNoteId
      ? drawing.notes.find((n) => n.id === store.selectedNoteId) ?? null
      : null
  );

  let statusMessage = $state('');
  let isBusy = $state(false);

  function showStatus(msg: string) {
    statusMessage = msg;
    setTimeout(() => { statusMessage = ''; }, 3000);
  }

  function updatePaperSize(size: PaperSize) {
    if (!drawing) return;
    store.updateDrawingMeta(drawing.id, { paperSize: size });
  }

  function toggleOrientation() {
    if (!drawing) return;
    const newOrientation: PaperOrientation = drawing.orientation === 'landscape' ? 'portrait' : 'landscape';
    store.updateDrawingMeta(drawing.id, { orientation: newOrientation });
  }

  async function handleRegenerate() {
    if (!drawing || !store.selectedViewId) return;
    isBusy = true;
    try {
      await generateView(drawing.id, store.selectedViewId);
      showStatus('View regenerated');
    } catch (err) {
      showStatus(`Error: ${err}`);
    } finally {
      isBusy = false;
    }
  }

  async function handleRegenerateAll() {
    if (!drawing) return;
    isBusy = true;
    try {
      await regenerateAllViews(drawing.id);
      showStatus('All views regenerated');
    } catch (err) {
      showStatus(`Error: ${err}`);
    } finally {
      isBusy = false;
    }
  }

  async function handleExportPdf() {
    if (!drawing) return;
    isBusy = true;
    try {
      const result = await exportPdf(drawing.id);
      if (result) showStatus(result);
    } catch (err) {
      showStatus(`PDF export failed: ${err}`);
    } finally {
      isBusy = false;
    }
  }

  async function handleExportDxf() {
    if (!drawing) return;
    isBusy = true;
    try {
      const result = await exportDxf(drawing.id);
      if (result) showStatus(result);
    } catch (err) {
      showStatus(`DXF export failed: ${err}`);
    } finally {
      isBusy = false;
    }
  }
</script>

<div class="drawing-properties">
  {#if drawing}
    <!-- Drawing Info -->
    <div class="section">
      <div class="section-header">Drawing</div>
      <div class="prop-row">
        <label>Name</label>
        <input
          type="text"
          class="prop-input"
          value={drawing.name}
          oninput={(e) => store.updateDrawingMeta(drawing.id, { name: (e.target as HTMLInputElement).value })}
        />
      </div>
      <div class="prop-row">
        <label>Paper</label>
        <select
          class="prop-select"
          value={drawing.paperSize}
          onchange={(e) => updatePaperSize((e.target as HTMLSelectElement).value as PaperSize)}
        >
          <option value="A4">A4</option>
          <option value="A3">A3</option>
          <option value="A2">A2</option>
          <option value="A1">A1</option>
          <option value="Letter">Letter</option>
          <option value="Tabloid">Tabloid</option>
        </select>
      </div>
      <div class="prop-row">
        <label>Orient</label>
        <button class="prop-btn" onclick={toggleOrientation}>
          {drawing.orientation === 'landscape' ? 'Landscape' : 'Portrait'}
        </button>
      </div>
      <div class="prop-row">
        <label>Views</label>
        <span class="prop-value">{drawing.views.length}</span>
      </div>
      <div class="prop-actions">
        <button class="action-btn regen-btn" onclick={handleRegenerateAll} disabled={isBusy || drawing.views.length === 0}>
          Regen All
        </button>
      </div>
    </div>

    <!-- Selected View -->
    {#if selectedView}
      <div class="section">
        <div class="section-header">Selected View</div>
        <div class="prop-row">
          <label>Direction</label>
          <span class="prop-badge">{selectedView.direction}</span>
        </div>
        <div class="prop-row">
          <label>Label</label>
          <input
            type="text"
            class="prop-input"
            value={selectedView.label}
            oninput={(e) => store.updateView(drawing.id, selectedView.id, { label: (e.target as HTMLInputElement).value })}
          />
        </div>
        <div class="prop-row">
          <label>Scale</label>
          <input
            type="number"
            class="prop-input narrow"
            value={selectedView.scale}
            step="0.1"
            min="0.01"
            oninput={(e) => store.updateView(drawing.id, selectedView.id, { scale: parseFloat((e.target as HTMLInputElement).value) || 1 })}
          />
        </div>
        <div class="prop-row">
          <label>X</label>
          <input
            type="number"
            class="prop-input narrow"
            value={selectedView.x.toFixed(1)}
            step="1"
            oninput={(e) => store.updateView(drawing.id, selectedView.id, { x: parseFloat((e.target as HTMLInputElement).value) || 0 })}
          />
          <label>Y</label>
          <input
            type="number"
            class="prop-input narrow"
            value={selectedView.y.toFixed(1)}
            step="1"
            oninput={(e) => store.updateView(drawing.id, selectedView.id, { y: parseFloat((e.target as HTMLInputElement).value) || 0 })}
          />
        </div>
        <div class="prop-row">
          <label>Hidden</label>
          <input
            type="checkbox"
            checked={selectedView.showHidden}
            onchange={(e) => store.updateView(drawing.id, selectedView.id, { showHidden: (e.target as HTMLInputElement).checked })}
          />
        </div>
        <div class="prop-actions">
          <button class="action-btn regen-btn" onclick={handleRegenerate} disabled={isBusy}>
            Regenerate
          </button>
          <button class="action-btn delete-btn" onclick={() => store.removeView(drawing.id, selectedView.id)}>
            Delete
          </button>
        </div>
      </div>
    {/if}

    <!-- Selected Dimension -->
    {#if selectedDim}
      <div class="section">
        <div class="section-header">Selected Dimension</div>
        <div class="prop-row">
          <label>Type</label>
          <span class="prop-badge">{selectedDim.type}</span>
        </div>
        <div class="prop-row">
          <label>Value</label>
          <span class="prop-value">{selectedDim.value.toFixed(2)}</span>
        </div>
        <div class="prop-row">
          <label>Text</label>
          <input
            type="text"
            class="prop-input"
            value={selectedDim.text ?? ''}
            placeholder="Auto"
            oninput={(e) => {
              const val = (e.target as HTMLInputElement).value;
              store.updateDimension(drawing.id, selectedDim.id, { text: val || undefined });
            }}
          />
        </div>
        <div class="prop-row">
          <label>Offset</label>
          <input
            type="number"
            class="prop-input narrow"
            value={selectedDim.offsetDistance}
            step="1"
            oninput={(e) => store.updateDimension(drawing.id, selectedDim.id, { offsetDistance: parseFloat((e.target as HTMLInputElement).value) || 8 })}
          />
        </div>
        <div class="prop-actions">
          <button class="action-btn delete-btn" onclick={() => store.removeDimension(drawing.id, selectedDim.id)}>
            Delete
          </button>
        </div>
      </div>
    {/if}

    <!-- Selected Note -->
    {#if selectedNote}
      <div class="section">
        <div class="section-header">Selected Note</div>
        <div class="prop-row">
          <label>Text</label>
          <input
            type="text"
            class="prop-input"
            value={selectedNote.text}
            oninput={(e) => store.updateNote(drawing.id, selectedNote.id, { text: (e.target as HTMLInputElement).value })}
          />
        </div>
        <div class="prop-row">
          <label>Size</label>
          <input
            type="number"
            class="prop-input narrow"
            value={selectedNote.fontSize}
            step="1"
            min="6"
            oninput={(e) => store.updateNote(drawing.id, selectedNote.id, { fontSize: parseFloat((e.target as HTMLInputElement).value) || 10 })}
          />
        </div>
        <div class="prop-row">
          <label>Bold</label>
          <input
            type="checkbox"
            checked={selectedNote.bold}
            onchange={(e) => store.updateNote(drawing.id, selectedNote.id, { bold: (e.target as HTMLInputElement).checked })}
          />
        </div>
        <div class="prop-actions">
          <button class="action-btn delete-btn" onclick={() => store.removeNote(drawing.id, selectedNote.id)}>
            Delete
          </button>
        </div>
      </div>
    {/if}

    <!-- Title Block -->
    <div class="section">
      <div class="section-header">Title Block</div>
      {#each [
        ['title', 'Title'],
        ['author', 'Author'],
        ['date', 'Date'],
        ['scale', 'Scale'],
        ['sheetNumber', 'Sheet'],
        ['revision', 'Rev'],
        ['material', 'Material'],
        ['company', 'Company'],
      ] as [field, label]}
        <div class="prop-row">
          <label>{label}</label>
          <input
            type="text"
            class="prop-input"
            value={drawing.titleBlock[field as keyof typeof drawing.titleBlock]}
            oninput={(e) => store.updateTitleBlock(drawing.id, { [field]: (e.target as HTMLInputElement).value })}
          />
        </div>
      {/each}
    </div>

    <!-- Export -->
    <div class="section">
      <div class="section-header">Export</div>
      <div class="prop-actions">
        <button class="action-btn export-btn" onclick={handleExportPdf} disabled={isBusy}>
          Export PDF
        </button>
        <button class="action-btn export-btn" onclick={handleExportDxf} disabled={isBusy}>
          Export DXF
        </button>
      </div>
    </div>

    {#if statusMessage}
      <div class="status-msg">{statusMessage}</div>
    {/if}
  {:else}
    <div class="empty">No drawing active</div>
  {/if}
</div>

<style>
  .drawing-properties {
    height: 100%;
    overflow-y: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .section {
    background: var(--bg-mantle);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .section-header {
    font-size: 10px;
    font-weight: 700;
    color: var(--accent);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 2px;
  }

  .prop-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .prop-row label {
    font-size: 11px;
    color: var(--text-secondary);
    width: 48px;
    flex-shrink: 0;
  }

  .prop-input {
    flex: 1;
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 3px 6px;
    font-size: 11px;
    color: var(--text-primary);
    font-family: var(--font-mono);
    min-width: 0;
  }

  .prop-input.narrow {
    width: 50px;
    flex: 0 0 50px;
  }

  .prop-input:focus {
    border-color: var(--accent);
    outline: none;
  }

  .prop-select {
    flex: 1;
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 3px 6px;
    font-size: 11px;
    color: var(--text-primary);
  }

  .prop-btn {
    flex: 1;
    background: var(--bg-overlay);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 3px 8px;
    font-size: 11px;
    color: var(--text-secondary);
    cursor: pointer;
  }

  .prop-btn:hover {
    color: var(--text-primary);
    border-color: var(--accent);
  }

  .prop-value {
    font-size: 11px;
    color: var(--text-primary);
    font-family: var(--font-mono);
  }

  .prop-badge {
    font-size: 10px;
    font-weight: 600;
    color: var(--accent);
    background: rgba(137, 180, 250, 0.1);
    border: 1px solid rgba(137, 180, 250, 0.3);
    border-radius: 3px;
    padding: 1px 6px;
    text-transform: uppercase;
  }

  .prop-actions {
    display: flex;
    gap: 4px;
    margin-top: 4px;
  }

  .action-btn {
    flex: 1;
    border-radius: 3px;
    padding: 4px 8px;
    font-size: 10px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid;
    transition: all 0.12s ease;
  }

  .action-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .regen-btn {
    color: #a6e3a1;
    border-color: rgba(166, 227, 161, 0.3);
    background: rgba(166, 227, 161, 0.1);
  }

  .regen-btn:hover:not(:disabled) {
    background: rgba(166, 227, 161, 0.2);
  }

  .delete-btn {
    color: #f38ba8;
    border-color: rgba(243, 139, 168, 0.3);
    background: rgba(243, 139, 168, 0.1);
  }

  .delete-btn:hover:not(:disabled) {
    background: rgba(243, 139, 168, 0.2);
  }

  .export-btn {
    color: #89dceb;
    border-color: rgba(137, 220, 235, 0.3);
    background: rgba(137, 220, 235, 0.1);
  }

  .export-btn:hover:not(:disabled) {
    background: rgba(137, 220, 235, 0.2);
  }

  .status-msg {
    font-size: 11px;
    color: var(--text-muted);
    padding: 4px 8px;
    background: var(--bg-overlay);
    border-radius: 3px;
    text-align: center;
  }

  .empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
