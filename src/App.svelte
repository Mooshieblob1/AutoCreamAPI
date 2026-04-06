<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  interface SteamGame {
    app_id: number;
    name: string;
    install_dir: string;
    has_x86: boolean;
    has_x64: boolean;
    cream_api_applied: boolean;
  }

  interface DlcInfo {
    app_id: number;
    name: string;
    from_store: boolean;
  }

  interface ApplyResult {
    success: boolean;
    message: string;
  }

  interface DllStatus {
    dll_dir: string;
    has_x86: boolean;
    has_x64: boolean;
  }

  let games: SteamGame[] = $state([]);
  let selectedGame: SteamGame | null = $state(null);
  let dlcList: DlcInfo[] = $state([]);
  let selectedDlcs: Set<number> = $state(new Set());
  let loading = $state(false);
  let dlcLoading = $state(false);
  let statusMessage = $state("");
  let searchQuery = $state("");
  let dllStatus: DllStatus | null = $state(null);

  // Settings
  let offlineMode = $state(false);
  let extraProtection = $state(false);

  async function loadGames() {
    loading = true;
    statusMessage = "Scanning Steam libraries...";
    try {
      games = await invoke<SteamGame[]>("get_installed_games");
      statusMessage = `Found ${games.length} installed games`;
    } catch (e) {
      statusMessage = `Error: ${e}`;
    }
    loading = false;
  }

  async function selectGame(game: SteamGame) {
    selectedGame = game;
    dlcList = [];
    selectedDlcs = new Set();
    dlcLoading = true;
    statusMessage = `Fetching DLCs for ${game.name}...`;
    try {
      dlcList = await invoke<DlcInfo[]>("get_dlc_list", {
        appId: game.app_id,
      });
      selectedDlcs = new Set(dlcList.map((d) => d.app_id));
      statusMessage = `Found ${dlcList.length} DLCs for ${game.name}`;
    } catch (e) {
      statusMessage = `Error fetching DLCs: ${e}`;
    }
    dlcLoading = false;
  }

  function toggleDlc(appId: number) {
    const next = new Set(selectedDlcs);
    if (next.has(appId)) {
      next.delete(appId);
    } else {
      next.add(appId);
    }
    selectedDlcs = next;
  }

  function toggleAll() {
    if (selectedDlcs.size === dlcList.length) {
      selectedDlcs = new Set();
    } else {
      selectedDlcs = new Set(dlcList.map((d) => d.app_id));
    }
  }

  async function applyCreamApi() {
    if (!selectedGame) return;
    loading = true;
    statusMessage = "Applying CreamAPI...";
    try {
      const dlcs = dlcList
        .filter((d) => selectedDlcs.has(d.app_id))
        .map((d) => ({ app_id: d.app_id, name: d.name }));
      const result = await invoke<ApplyResult>("apply_cream_api", {
        installDir: selectedGame.install_dir,
        hasX86: selectedGame.has_x86,
        hasX64: selectedGame.has_x64,
        appId: selectedGame.app_id,
        dlcs: dlcs,
        offline: offlineMode,
        extraProtection: extraProtection,
      });
      statusMessage = result.message;
    } catch (e) {
      statusMessage = `Error: ${e}`;
    }
    loading = false;
  }

  async function removeCreamApi() {
    if (!selectedGame) return;
    loading = true;
    statusMessage = "Removing CreamAPI...";
    try {
      const result = await invoke<ApplyResult>("remove_cream_api", {
        installDir: selectedGame.install_dir,
      });
      statusMessage = result.message;
      if (selectedGame) selectedGame.cream_api_applied = false;
    } catch (e) {
      statusMessage = `Error: ${e}`;
    }
    loading = false;
  }

  let filteredGames = $derived(
    searchQuery.trim()
      ? games.filter((g) =>
          g.name.toLowerCase().includes(searchQuery.toLowerCase()),
        )
      : games,
  );

  // Load games on mount
  loadGames();
  checkDllStatus();

  async function checkDllStatus() {
    try {
      dllStatus = await invoke<DllStatus>("check_dll_status");
    } catch (_) {}
  }

  let dllsMissing = $derived(
    dllStatus != null && (!dllStatus.has_x86 || !dllStatus.has_x64),
  );
</script>

<header>
  <h1>AutoCreamAPI</h1>
  <span class="status">{statusMessage}</span>
</header>

{#if dllsMissing}
  <div class="dll-warning">
    <strong>CreamAPI DLLs not found.</strong> Place
    <code>cream_api.dll</code> and <code>cream_api64.dll</code> in:
    <code>{dllStatus?.dll_dir}</code>
  </div>
{/if}

<main>
  <section class="game-panel">
    <div class="panel-header">
      <h2>Installed Games</h2>
      <button onclick={loadGames} disabled={loading}>Refresh</button>
    </div>
    <input
      type="text"
      placeholder="Search games..."
      bind:value={searchQuery}
      name="game-search"
      id="game-search"
      class="search-input"
    />
    <div class="game-list scrollable">
      {#each filteredGames as game}
        <button
          class="game-item"
          class:selected={selectedGame?.app_id === game.app_id}
          class:applied={game.cream_api_applied}
          onclick={() => selectGame(game)}
        >
          <span class="game-name">{game.name}</span>
          <span class="game-id">#{game.app_id}</span>
          {#if game.cream_api_applied}
            <span class="badge applied-badge">Applied</span>
          {/if}
        </button>
      {/each}
      {#if filteredGames.length === 0 && !loading}
        <p class="empty-state">
          {games.length === 0
            ? "No Steam games found. Click Refresh."
            : "No matches."}
        </p>
      {/if}
      {#if loading && games.length === 0}
        <p class="empty-state">Scanning...</p>
      {/if}
    </div>
  </section>

  <section class="dlc-panel">
    {#if selectedGame}
      <div class="panel-header">
        <h2>{selectedGame.name}</h2>
        <div class="header-actions">
          {#if dlcList.length > 0}
            <button onclick={toggleAll}>
              {selectedDlcs.size === dlcList.length
                ? "Deselect All"
                : "Select All"}
            </button>
          {/if}
        </div>
      </div>

      <div class="dlc-list scrollable">
        {#if dlcLoading}
          <p class="empty-state">Fetching DLCs...</p>
        {:else if dlcList.length === 0}
          <p class="empty-state">No DLCs found for this game.</p>
        {:else}
          {#each dlcList as dlc}
            <label class="checkbox-row dlc-item">
              <input
                type="checkbox"
                checked={selectedDlcs.has(dlc.app_id)}
                onchange={() => toggleDlc(dlc.app_id)}
                name="dlc-{dlc.app_id}"
                id="dlc-{dlc.app_id}"
              />
              <span class="dlc-name">{dlc.name}</span>
              <span class="dlc-id">#{dlc.app_id}</span>
              {#if !dlc.from_store}
                <span class="badge hidden-badge">Hidden</span>
              {/if}
            </label>
          {/each}
        {/if}
      </div>

      <div class="settings-bar">
        <label class="checkbox-row">
          <input
            type="checkbox"
            bind:checked={offlineMode}
            name="offline-mode"
            id="offline-mode"
          />
          <span>Offline Mode</span>
        </label>
        <label class="checkbox-row">
          <input
            type="checkbox"
            bind:checked={extraProtection}
            name="extra-protection"
            id="extra-protection"
          />
          <span>Extra Protection</span>
        </label>
      </div>

      <div class="action-bar">
        <button
          class="primary"
          onclick={applyCreamApi}
          disabled={loading || selectedDlcs.size === 0 || dllsMissing}
        >
          Apply CreamAPI ({selectedDlcs.size} DLCs)
        </button>
        {#if selectedGame.cream_api_applied}
          <button onclick={removeCreamApi} disabled={loading}>
            Remove CreamAPI
          </button>
        {/if}
      </div>
    {:else}
      <div class="empty-state full-center">
        <p>Select a game from the list to manage DLCs</p>
      </div>
    {/if}
  </section>
</main>

<style>
  .dll-warning {
    background: #6b2d00;
    color: #ffb86c;
    padding: 8px 20px;
    font-size: 0.85rem;
    border-bottom: 1px solid #a44200;
  }
  .dll-warning code {
    background: rgba(0, 0, 0, 0.3);
    padding: 1px 5px;
    border-radius: 3px;
    font-size: 0.8rem;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 20px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    -webkit-app-region: drag;
  }

  header h1 {
    font-size: 1rem;
    font-weight: 700;
    color: var(--primary);
  }

  .status {
    font-size: 0.8rem;
    color: var(--text-muted);
    -webkit-app-region: no-drag;
  }

  main {
    display: grid;
    grid-template-columns: 320px 1fr;
    flex: 1;
    min-height: 0;
  }

  .game-panel,
  .dlc-panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .game-panel {
    border-right: 1px solid var(--border);
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }

  .panel-header h2 {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .header-actions {
    display: flex;
    gap: 8px;
  }

  .search-input {
    margin: 8px 12px;
    width: calc(100% - 24px);
  }

  .game-list,
  .dlc-list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 8px;
  }

  .game-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: left;
    padding: 10px 12px;
    margin: 2px 0;
    border-radius: 6px;
    border: 1px solid transparent;
  }

  .game-item.selected {
    background: var(--surface-hover);
    border-color: var(--primary);
  }

  .game-item.applied {
    border-left: 3px solid var(--success);
  }

  .game-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .game-id,
  .dlc-id {
    font-size: 0.75rem;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .dlc-item {
    padding: 8px 12px;
    border-radius: 6px;
    cursor: pointer;
  }

  .dlc-item:hover {
    background: var(--surface-hover);
  }

  .dlc-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badge {
    font-size: 0.65rem;
    padding: 2px 6px;
    border-radius: 4px;
    font-weight: 600;
    text-transform: uppercase;
    flex-shrink: 0;
  }

  .applied-badge {
    background: var(--success);
    color: var(--bg);
  }

  .hidden-badge {
    background: var(--warning);
    color: var(--bg);
  }

  .settings-bar {
    display: flex;
    gap: 20px;
    padding: 10px 16px;
    border-top: 1px solid var(--border);
  }

  .action-bar {
    display: flex;
    gap: 10px;
    padding: 12px 16px;
    border-top: 1px solid var(--border);
    background: var(--surface);
  }

  .empty-state {
    padding: 24px;
    text-align: center;
    color: var(--text-muted);
    font-size: 0.875rem;
  }

  .full-center {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }
</style>
