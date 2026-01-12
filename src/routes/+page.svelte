<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";

  /** @typedef {{ relative: string, target: string, status: string, link_is_dir?: boolean }} SymlinkEntry */
  /** @typedef {{ src_root: string, entries: SymlinkEntry[] }} ExportData */
  /** @typedef {{ src_root: string, entries: SymlinkEntry[], skipped: number }} ScanResult */
  /** @typedef {{ from: string, to: string }} MappingRule */
  /** @typedef {{ root: string, count: number }} PreviewRoot */
  /** @typedef {{ link: string, target: string }} PreviewItem */
  /** @typedef {{ root: string, link: string, target: string }} PreviewRootSample */
  /** @typedef {{ roots: PreviewRoot[], sample: PreviewItem[], root_samples: PreviewRootSample[] }} PreviewState */
  /** @typedef {{ total: number, non_symlink: number, sample: string[] }} ConflictReport */
  /** @typedef {{ created: number, failed: number, sample_links: string[] }} ResultData */

  let scanRoot = $state("");
  let dstRoot = $state("");
  /** @type {ScanResult} */
  let scanData = $state({ src_root: "", entries: [], skipped: 0 });
  /** @type {ExportData | null} */
  let importData = $state(null);
  /** @type {MappingRule[]} */
  let mappings = $state([{ from: "", to: "" }]);
  /** @type {PreviewState} */
  let preview = $state({ roots: [], sample: [], root_samples: [] });
  let status = $state("");
  let working = $state(false);
  let activeTab = $state("scan");
  /** @type {string[]} */
  let lastFailures = $state([]);
  /** @type {"relative" | "target" | "status"} */
  let sortKey = $state("relative");
  let sortDir = $state("asc");
  let scanQuery = $state("");
  let confirmOpen = $state(false);
  /** @type {ConflictReport | null} */
  let confirmData = $state(null);
  let resultOpen = $state(false);
  /** @type {ResultData | null} */
  let resultData = $state(null);
  let osSep = $state("/");
  let previewSeq = 0;
  /** @type {ReturnType<typeof setTimeout> | null} */
  let previewTimer = null;
  /** @type {((accepted: boolean) => void) | null} */
  let confirmResolve = null;

  /** @param {"relative" | "target" | "status"} key */
  function setSort(key) {
    if (sortKey === key) {
      sortDir = sortDir === "asc" ? "desc" : "asc";
      return;
    }
    sortKey = key;
    sortDir = "asc";
  }

  /** @returns {SymlinkEntry[]} */
  function filteredEntries() {
    const query = scanQuery.trim().toLowerCase();
    const rawTerms = query
      .split(",")
      .map((term) => term.trim())
      .filter((term) => term.length > 0);
    const includeTerms = rawTerms.filter((term) => !term.startsWith("-"));
    const excludeTerms = rawTerms
      .filter((term) => term.startsWith("-"))
      .map((term) => term.slice(1))
      .filter((term) => term.length > 0);
    return includeTerms.length || excludeTerms.length
      ? scanData.entries.filter((entry) => {
          const rel = (entry.relative || "").toString().toLowerCase();
          const tgt = (entry.target || "").toString().toLowerCase();
          const st = (entry.status || "").toString().toLowerCase();
          const matchesInclude =
            includeTerms.length === 0 ||
            includeTerms.some(
              (term) => rel.includes(term) || tgt.includes(term) || st.includes(term)
            );
          const matchesExclude = excludeTerms.some(
            (term) => rel.includes(term) || tgt.includes(term) || st.includes(term)
          );
          return matchesInclude && !matchesExclude;
        })
      : [...scanData.entries];
  }

  /** @param {SymlinkEntry} entry */
  function getSortValue(entry) {
    if (sortKey === "target") return entry.target || "";
    if (sortKey === "status") return entry.status || "";
    return entry.relative || "";
  }

  /** @returns {SymlinkEntry[]} */
  function sortedEntries() {
    const entries = filteredEntries();
    const dir = sortDir === "asc" ? 1 : -1;
    entries.sort((a, b) => {
      const left = getSortValue(a).toString().toLowerCase();
      const right = getSortValue(b).toString().toLowerCase();
      if (left < right) return -1 * dir;
      if (left > right) return 1 * dir;
      return 0;
    });
    return entries;
  }

  /** @param {string} message */
  function setStatus(message) {
    status = message;
  }

  /** @param {ConflictReport} data */
  function openConfirm(data) {
    confirmData = data;
    confirmOpen = true;
    return new Promise((resolve) => {
      confirmResolve = resolve;
    });
  }

  /** @param {boolean} accepted */
  function closeConfirm(accepted) {
    confirmOpen = false;
    const resolve = confirmResolve;
    confirmResolve = null;
    if (resolve) {
      resolve(accepted);
    }
  }

  /** @param {ResultData} data */
  function openResult(data) {
    resultData = data;
    resultOpen = true;
  }

  function closeResult() {
    resultOpen = false;
    resultData = null;
  }

  /** @param {string} value */
  function displayPath(value) {
    if (!value) return "";
    if (osSep === "\\") {
      return value.replaceAll("/", "\\");
    }
    return value.replaceAll("\\", "/");
  }

  onMount(() => {
    const ua = navigator.userAgent || "";
    osSep = ua.includes("Windows") ? "\\" : "/";
  });

  /** @returns {MappingRule[]} */
  function cleanMappings() {
    return mappings
      .map((item) => ({ from: item.from.trim(), to: item.to.trim() }))
      .filter((item) => item.from && item.to);
  }

  function addMapping() {
    mappings = [...mappings, { from: "", to: "" }];
  }

  /** @param {number} index */
  function removeMapping(index) {
    mappings = mappings.filter((_, idx) => idx !== index);
    if (mappings.length === 0) {
      mappings = [{ from: "", to: "" }];
    }
  }

  async function browseScan() {
    const selected = await open({ directory: true, multiple: false });
    if (selected) scanRoot = selected;
  }

  async function browseTarget() {
    const selected = await open({ directory: true, multiple: false });
    if (selected) dstRoot = selected;
  }

  async function runScan() {
    if (!scanRoot) {
      setStatus("Select a folder to scan.");
      return;
    }
    working = true;
    try {
      const result = await invoke("scan_symlinks", { root: scanRoot });
      scanData = result;
      const skippedNote = result.skipped ? ` Skipped ${result.skipped} entries.` : "";
      setStatus(`Scan complete. ${result.entries.length} symlinks found.${skippedNote}`);
    } catch (err) {
      setStatus(`Scan failed: ${err}`);
    } finally {
      working = false;
    }
  }

  async function exportJson() {
    const entries = filteredEntries();
    if (!entries.length) {
      setStatus("Run a scan before exporting.");
      return;
    }
    const path = await save({
      filters: [{ name: "JSON", extensions: ["json"] }],
      defaultPath: "symlinks.json"
    });
    if (!path) return;
    working = true;
    try {
      await invoke("export_symlinks", {
        path,
        data: {
          src_root: scanData.src_root,
          entries
        }
      });
      setStatus(`Exported JSON to ${path}`);
    } catch (err) {
      setStatus(`Export failed: ${err}`);
    } finally {
      working = false;
    }
  }

  async function loadJson() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }]
    });
    if (!selected) return;
    working = true;
    try {
      const result = await invoke("load_export", { path: selected });
      importData = result;
      setStatus(`Loaded ${result.entries.length} entries.`);
    } catch (err) {
      setStatus(`Load failed: ${err}`);
    } finally {
      working = false;
    }
  }

  function scheduleAutoPreview() {
    if (previewTimer) {
      clearTimeout(previewTimer);
      previewTimer = null;
    }
    if (activeTab !== "import") return;
    if (!importData || !dstRoot) return;
    previewTimer = setTimeout(() => {
      previewRecreate(true);
    }, 300);
  }

  $effect(() => {
    activeTab;
    importData;
    dstRoot;
    const mappingKey = mappings.map((item) => `${item.from}|${item.to}`).join("||");
    mappingKey;
    scheduleAutoPreview();
  });

  /** @param {boolean} [silent=false] */
  async function previewRecreate(silent = false) {
    if (!importData) {
      if (!silent) {
        setStatus("Load an export file first.");
      }
      return;
    }
    if (!dstRoot) {
      if (!silent) {
        setStatus("Select a target root.");
      }
      return;
    }
    if (!silent) {
      working = true;
    }
    const seq = (previewSeq += 1);
    try {
      const result = await invoke("preview_recreate", {
        data: importData,
        dstRoot,
        mappings: cleanMappings(),
        maxPreview: importData.entries.length
      });
      if (seq !== previewSeq) return;
      preview = {
        roots: result.roots,
        sample: result.sample,
        root_samples: result.root_samples || []
      };
      if (!silent) {
        setStatus(`Preview ready. ${result.roots.length} target roots detected.`);
      }
    } catch (err) {
      if (!silent) {
        setStatus(`Preview failed: ${err}`);
      }
    } finally {
      if (!silent) {
        working = false;
      }
    }
  }

  async function recreate() {
    if (!importData) {
      setStatus("Load an export file first.");
      return;
    }
    if (!dstRoot) {
      setStatus("Select a target root.");
      return;
    }
    working = true;
    try {
      const conflicts = await invoke("check_recreate_conflicts", {
        data: importData,
        dstRoot,
        mappings: cleanMappings()
      });
      if (conflicts.total > 0) {
        working = false;
        const accepted = await openConfirm(conflicts);
        if (!accepted) {
          setStatus("Recreate cancelled.");
          return;
        }
        working = true;
      }
      const result = await invoke("recreate_symlinks", {
        data: importData,
        dstRoot,
        mappings: cleanMappings(),
        missingAsDir: false
      });
      lastFailures = result.failed || [];
      const failures = result.failed.length;
      if (failures) {
        const needsAdmin = result.failed.some((/** @type {string} */ item) => item.includes("os error 1314"));
        if (needsAdmin) {
          try {
            lastFailures = [];
            const jobId = await invoke("start_admin_recreate", {
              data: importData,
              dstRoot,
              mappings: cleanMappings(),
              missingAsDir: false
            });
            setStatus("Admin prompt opened. Approve to continue recreating symlinks.");
            await pollAdminResult(jobId);
            return;
          } catch (adminErr) {
            setStatus(`Admin recreate failed: ${adminErr}`);
            return;
          }
        }
        setStatus(`Created ${result.created} symlinks. ${failures} failed.`);
      } else {
        setStatus(`Created ${result.created} symlinks successfully.`);
      }
    } catch (err) {
      const message = `${err}`;
      if (message.includes("os error 1314")) {
        try {
          const jobId = await invoke("start_admin_recreate", {
            data: importData,
            dstRoot,
            mappings: cleanMappings(),
            missingAsDir: false
          });
          lastFailures = [];
          setStatus("Admin prompt opened. Approve to continue recreating symlinks.");
          await pollAdminResult(jobId);
          return;
        } catch (adminErr) {
          setStatus(`Admin recreate failed: ${adminErr}`);
        }
      } else {
        lastFailures = [];
        setStatus(`Recreate failed: ${err}`);
      }
    } finally {
      working = false;
    }
  }

  /** @param {string} jobId */
  async function pollAdminResult(jobId) {
    const maxTries = 60;
    for (let i = 0; i < maxTries; i += 1) {
      await new Promise((resolve) => setTimeout(resolve, 1000));
      try {
        const result = await invoke("poll_admin_result", { jobId });
        if (result) {
          openResult({
            created: result.created,
            failed: result.failed,
            sample_links: result.sample_links || []
          });
          setStatus(`Created ${result.created} symlinks. ${result.failed} failed.`);
          return;
        }
      } catch (err) {
        setStatus(`Admin result check failed: ${err}`);
        return;
      }
    }
    setStatus("Admin recreate still running. Check the destination folder in a moment.");
  }
</script>

<main class="app">
  <header class="hero">
    <div>
      <h1 class="hero-title">Symlink Manager</h1>
      <p class="subtitle">
        Scan, export, and recreate symlinks across Windows and Linux with bulk root remapping.
      </p>
    </div>
    <div class="stats">
      <div>
        <span class="label">Scan root</span>
        <span class="value">{scanData.src_root || "Not scanned"}</span>
      </div>
      <div class="stats-row">
        <div>
          <span class="label">Entries</span>
          <span class="value">{scanData.entries.length}</span>
        </div>
        <span class="stat-sep" aria-hidden="true"></span>
        <div>
          <span class="label">Import loaded</span>
          <span class="value">{importData ? importData.entries.length : 0}</span>
        </div>
      </div>
    </div>
  </header>

  <section class="tabs">
    <button
      class:active={activeTab === "scan"}
      type="button"
      onclick={() => (activeTab = "scan")}
    >
      Scan & Export
    </button>
    <button
      class:active={activeTab === "import"}
      type="button"
      onclick={() => (activeTab = "import")}
    >
      Import & Recreate
    </button>
  </section>

  {#if activeTab === "scan"}
    <section class="grid">
      <div class="panel lift-1">
        <div class="panel-header">
          <h2>Scan & Export</h2>
          <span class="muted">Find symlinks in a folder tree.</span>
        </div>
        <div class="field">
          <label for="scan-root">Folder to scan</label>
          <div class="row">
            <input
              type="text"
              placeholder="D:\Media\Library"
              bind:value={scanRoot}
              id="scan-root"
            />
            <button type="button" class="ghost" onclick={browseScan}>Browse</button>
          </div>
        </div>
        <div class="row">
          <button type="button" class="accent" onclick={runScan} disabled={working}>
            Scan symlinks
          </button>
          <button type="button" class="ghost" onclick={exportJson} disabled={working}>
            Export JSON
          </button>
        </div>
      </div>
    </section>

    <section class="panel table-panel lift-3">
      <div class="panel-header">
        <div>
          <h2>Scan Results</h2>
          {#if scanData.skipped}
            <span class="muted meta">Skipped: {scanData.skipped}</span>
          {/if}
        </div>
        <input
          class="search-input"
          type="text"
          placeholder="Search..."
          bind:value={scanQuery}
        />
      </div>
      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>
                <button type="button" class="sort" onclick={() => setSort("relative")}>
                  Relative path
                  <span class="caret" aria-hidden="true">{sortKey === "relative" ? (sortDir === "asc" ? "▲" : "▼") : ""}</span>
                </button>
              </th>
              <th>
                <button type="button" class="sort" onclick={() => setSort("target")}>
                  Target
                  <span class="caret" aria-hidden="true">{sortKey === "target" ? (sortDir === "asc" ? "▲" : "▼") : ""}</span>
                </button>
              </th>
              <th>
                <button type="button" class="sort" onclick={() => setSort("status")}>
                  Status
                  <span class="caret" aria-hidden="true">{sortKey === "status" ? (sortDir === "asc" ? "▲" : "▼") : ""}</span>
                </button>
              </th>
            </tr>
          </thead>
          <tbody>
            {#if scanData.entries.length === 0}
              <tr>
                <td colspan="3" class="muted">No scan data yet.</td>
              </tr>
            {:else}
              {#each sortedEntries() as entry}
                <tr class={entry.status === "Broken" ? "broken" : entry.status === "Unreadable" ? "unreadable" : ""}>
                  <td>{entry.relative}</td>
                  <td class="target-cell">
                    <span class="target-text">{entry.target}</span>
                  </td>
                  <td class="status-cell">
                    <span class="status-dot {entry.status === "Broken" ? "bad" : entry.status === "Unreadable" ? "warn" : "ok"}"></span>
                  </td>
                </tr>
              {/each}
            {/if}
          </tbody>
        </table>
      </div>
    </section>
  {:else}
    <section class="grid">
      <div class="panel lift-2">
        <div class="panel-header">
          <h2>Import & Recreate</h2>
          <span class="muted">Remap roots, then recreate in bulk.</span>
        </div>
        <div class="row">
          <button type="button" class="ghost" onclick={loadJson} disabled={working}>
            Load JSON export
          </button>
        </div>
        <div class="field field-gap">
          <label for="target-root">Target root (links destination)</label>
          <div class="row">
            <input
              type="text"
              placeholder="D:\Media\Links"
              bind:value={dstRoot}
              id="target-root"
            />
            <button type="button" class="ghost" onclick={browseTarget}>Browse</button>
          </div>
        </div>
        <div class="field">
          <span class="field-label">Root remap rules</span>
          <p class="hint">Replace target prefixes when moving between drives or OS roots.</p>
        {#each mappings as mapping, index}
            <div class="row mapping-row">
              <input
                type="text"
                placeholder="W:\Shows"
                bind:value={mapping.from}
              />
              <span class="arrow">-&gt;</span>
              <input
                type="text"
                placeholder="/mnt/media/shows"
                bind:value={mapping.to}
              />
              <button
                type="button"
                class="ghost small remove"
                aria-label="Remove mapping"
                onclick={() => removeMapping(index)}
              >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <circle cx="12" cy="12" r="8" fill="none" stroke="currentColor" stroke-width="2" />
                  <line x1="8" y1="16" x2="16" y2="8" stroke="currentColor" stroke-width="2" />
                </svg>
              </button>
            </div>
        {/each}
        <button type="button" class="ghost small add-mapping" onclick={addMapping}>
          Add mapping
        </button>
      </div>
      <div class="row">
        <button type="button" class="ghost" onclick={() => previewRecreate()} disabled={working}>
          Preview
        </button>
        <button type="button" class="accent" onclick={recreate} disabled={working}>
          Recreate
        </button>
      </div>
      {#if lastFailures.length}
        <div class="failures">
          <p class="label">Failures (first 10)</p>
          {#each lastFailures.slice(0, 10) as item}
            <div class="failure-row">{item}</div>
          {/each}
        </div>
      {/if}
      {#if preview.roots.length || preview.sample.length}
        <div class="preview-grid">
          <div class="preview bubble">
            <p class="label">Target roots detected</p>
              {#each preview.roots as item}
                <div class="preview-row">
                  <span>{displayPath(item.root)}</span>
                  <span class="count">{item.count}</span>
                </div>
              {/each}
          </div>
          <div class="preview bubble">
            <p class="label">Preview links</p>
              {#if preview.root_samples?.length}
                {#each preview.root_samples as item}
                  <div class="preview-row two-col">
                    <span>{displayPath(item.link)}</span>
                    <span class="muted">-&gt;</span>
                    <span>{displayPath(item.target)}</span>
                  </div>
                {/each}
              {:else}
                {#each preview.sample as item}
                  <div class="preview-row two-col">
                    <span>{displayPath(item.link)}</span>
                    <span class="muted">-&gt;</span>
                    <span>{displayPath(item.target)}</span>
                  </div>
                {/each}
              {/if}
          </div>
        </div>
      {/if}
      </div>
    </section>
  {/if}

  {#if confirmOpen}
    <div class="modal-backdrop" role="dialog" aria-modal="true">
      <div class="modal">
        <h3>Replace existing items?</h3>
        <p class="muted">
          {confirmData?.total} items already exist in the target folder and will be replaced.
        </p>
        {#if (confirmData?.non_symlink ?? 0) > 0}
          <p class="warning">Includes {confirmData?.non_symlink ?? 0} real files or folders.</p>
        {/if}
        {#if confirmData?.sample?.length}
          <div class="modal-list">
            {#each confirmData?.sample ?? [] as item}
              <div class="modal-row">{item}</div>
            {/each}
          </div>
        {/if}
        <div class="row modal-actions">
          <button type="button" class="ghost" onclick={() => closeConfirm(false)}>Cancel</button>
          <button type="button" class="accent" onclick={() => closeConfirm(true)}>Replace and recreate</button>
        </div>
      </div>
    </div>
  {/if}

  {#if resultOpen}
    <div class="modal-backdrop" role="dialog" aria-modal="true">
      <div class="modal">
        <h3>Import Complete</h3>
        <p class="muted">
          Recreate complete.
        </p>
        <div class="result-grid">
          <div>Created</div>
          <div>{resultData?.created ?? 0}</div>
          <div>Failed</div>
          <div>{resultData?.failed ?? 0}</div>
        </div>
        {#if resultData?.sample_links?.length}
          <div class="modal-list">
            {#each resultData?.sample_links ?? [] as item}
              <div class="modal-row">{item}</div>
            {/each}
          </div>
        {/if}
        <div class="row modal-actions">
          <button type="button" class="accent" onclick={closeResult}>OK</button>
        </div>
      </div>
    </div>
  {/if}

  <div class="status-bubble">
    <span>{status}</span>
  </div>
</main>

<style>
  :global(:root) {
    --bg: #141312;
    --panel: rgba(20, 20, 22, 0.88);
    --panel-strong: rgba(28, 28, 32, 0.96);
    --ink: #f5f3f0;
    --muted: #b1aba3;
    --accent: #f26430;
    --accent-strong: #ff9b73;
    --ok: #27f178;
    color-scheme: dark;
  }

  :global(body) {
    margin: 0;
    font-family: "Space Grotesk", "IBM Plex Sans", "Segoe UI", sans-serif;
    background: var(--bg);
    color: var(--ink);
    overflow-y: scroll;
  }

  .app {
    min-height: 100vh;
    padding: 32px 32px 64px;
    box-sizing: border-box;
    background:
      radial-gradient(circle at 15% 20%, rgba(255, 155, 115, 0.15), transparent 55%),
      radial-gradient(circle at 80% 5%, rgba(242, 100, 48, 0.18), transparent 45%),
      linear-gradient(160deg, rgba(20, 19, 18, 0.9) 0%, rgba(10, 10, 10, 0.98) 100%);
    position: relative;
    overflow: hidden;
  }

  .app::before {
    content: "";
    position: absolute;
    inset: -20% 10% auto auto;
    width: 480px;
    height: 480px;
    background: radial-gradient(circle, rgba(255, 155, 115, 0.25), transparent 60%);
    filter: blur(10px);
    opacity: 0.6;
    pointer-events: none;
  }

  .hero {
    display: flex;
    justify-content: space-between;
    gap: 32px;
    align-items: flex-start;
    margin-bottom: 28px;
    animation: rise 0.6s ease both;
  }

  .hero-title {
    margin: 0 0 12px;
    font-size: clamp(38px, 5vw, 56px);
    color: var(--accent);
  }

  .subtitle {
    max-width: 520px;
    color: var(--muted);
    margin: 0;
  }

  .stats {
    display: grid;
    gap: 12px;
    min-width: 240px;
    background: var(--panel);
    border: 1px solid rgba(255, 255, 255, 0.06);
    padding: 16px;
    border-radius: 16px;
    backdrop-filter: blur(12px);
  }

  .stats > div {
    text-align: center;
  }

  .stats-row {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 14px;
  }

  .stats-row > div {
    text-align: center;
  }

  .stat-sep {
    width: 1px;
    height: 26px;
    background: rgba(255, 255, 255, 0.12);
    flex: 0 0 1px;
  }

  .stats .label {
    display: block;
    font-size: 12px;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.12em;
  }

  .stats .value {
  .meta {
    display: block;
    font-size: 12px;
    margin-top: 4px;
  }


    font-size: 14px;
    word-break: break-all;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 20px;
    margin-bottom: 24px;
  }

  .tabs {
    display: inline-flex;
    gap: 8px;
    padding: 6px;
    border-radius: 999px;
    background: rgba(20, 20, 22, 0.7);
    border: 1px solid rgba(255, 255, 255, 0.08);
    margin-bottom: 20px;
  }

  .tabs button {
    border-radius: 999px;
    padding: 8px 18px;
    background: transparent;
    color: var(--muted);
    border: 1px solid transparent;
    font-weight: 600;
  }

  .tabs button.active {
    background: var(--accent);
    color: #1b0f0a;
    border-color: transparent;
  }

  .panel {
    background: var(--panel);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 18px;
    padding: 20px;
    backdrop-filter: blur(12px);
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    margin-bottom: 16px;
  }

  .panel-header h2 {
    margin: 0;
    font-size: 20px;
  }

  .search-input {
    flex: 0 0 300px !important;
    width: 300px !important;
    max-width: 300px;
    background: var(--panel-strong);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: var(--ink);
    padding: 8px 12px;
    border-radius: 999px;
    font-size: 13px;
  }

  .search-input:focus {
    outline: 1px solid var(--accent);
  }

  .muted {
    color: var(--muted);
  }

  .field {
    margin-bottom: 16px;
  }

  .field label,
  .field .field-label {
    display: block;
    font-size: 13px;
    margin-bottom: 8px;
    color: var(--muted);
  }

  .field-gap {
    margin-top: 8px;
  }

  input[type="text"] {
    flex: 1;
    background: var(--panel-strong);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: var(--ink);
    padding: 10px 12px;
    border-radius: 10px;
    font-size: 14px;
  }

  input[type="text"]:focus {
    outline: 1px solid var(--accent);
  }

  .row {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
  }

  button {
    border: none;
    border-radius: 10px;
    padding: 10px 14px;
    font-family: inherit;
    font-size: 14px;
    cursor: pointer;
    transition: transform 0.2s ease, background 0.2s ease, opacity 0.2s ease;
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  button.accent {
    background: var(--accent);
    color: #1b0f0a;
    font-weight: 600;
  }

  button.accent:hover {
    transform: translateY(-1px);
    background: var(--accent-strong);
  }

  button.ghost {
    background: transparent;
    color: var(--ink);
    border: 1px solid rgba(255, 255, 255, 0.12);
  }

  button.ghost:hover {
    border-color: var(--accent);
  }

  button.small {
    padding: 6px 10px;
    font-size: 12px;
  }

  button.remove {
    width: 26px;
    height: 26px;
    padding: 0;
    border-radius: 50%;
    font-weight: 700;
    line-height: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  button.remove svg {
    width: 14px;
    height: 14px;
  }

  .hint {
    font-size: 12px;
    color: var(--muted);
    margin: -6px 0 10px;
  }

  .mapping-row {
    align-items: center;
  }

  .mapping-row .arrow {
    color: var(--accent);
    font-size: 18px;
  }

  .add-mapping {
    margin-top: 8px;
  }

  .preview {
    margin-top: 16px;
    padding-top: 12px;
    display: grid;
    gap: 6px;
  }

  .preview.bubble {
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 14px;
    padding: 12px 14px;
    background: rgba(20, 20, 22, 0.6);
  }

  .preview-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 16px;
  }

  .failures {
    margin-top: 16px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    padding-top: 12px;
    display: grid;
    gap: 6px;
    font-size: 12px;
    color: #ff6b6b;
    word-break: break-all;
  }

  .failure-row {
    background: rgba(255, 107, 107, 0.08);
    border: 1px solid rgba(255, 107, 107, 0.2);
    border-radius: 8px;
    padding: 8px 10px;
  }

  .preview-row {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 12px;
    font-size: 12px;
    color: var(--ink);
    word-break: break-all;
  }

  .preview-row .count {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .preview-row.two-col {
    grid-template-columns: 1fr auto 1fr;
    gap: 8px;
  }

  .table-panel {
    padding-bottom: 12px;
  }

  .table-wrap {
    max-height: 360px;
    overflow: auto;
    border-radius: 12px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    table-layout: fixed;
  }

  th,
  td {
    padding: 10px 12px;
    text-align: left;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    vertical-align: top;
  }

  th {
    position: sticky;
    top: 0;
    background: rgba(20, 20, 22, 0.96);
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    padding: 0;
  }

  .sort {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    background: transparent;
    border: none;
    color: var(--ink);
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    padding: 10px 12px;
  }

  .sort:hover {
    color: var(--accent);
  }

  .caret {
    font-size: 10px;
    color: var(--accent);
  }

  tr.broken td {
    color: #ff6b6b;
  }

  tr.unreadable td {
    color: #ffb36b;
  }

  .status-dot {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    box-shadow: 0 0 10px rgba(39, 241, 120, 0.5);
    flex-shrink: 0;
  }

  .status-dot.ok {
    background: #27f178;
    box-shadow: 0 0 10px rgba(39, 241, 120, 0.6);
  }

  .status-dot.bad {
    background: #ff6b6b;
    box-shadow: 0 0 10px rgba(255, 107, 107, 0.5);
  }

  .status-dot.warn {
    background: #ffb36b;
    box-shadow: 0 0 10px rgba(255, 179, 107, 0.5);
  }

  .target-text,
  td {
    word-break: break-word;
    white-space: normal;
  }

  th:first-child,
  td:first-child {
    width: 42%;
  }

  th:nth-child(2),
  td:nth-child(2) {
    width: 52%;
  }

  th:last-child,
  td:last-child {
    width: 6%;
    text-align: right;
  }

  .status-cell {
    text-align: right;
    vertical-align: middle;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(8, 8, 8, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 30;
  }

  .modal {
    width: min(92vw, 520px);
    background: var(--panel-strong);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    padding: 18px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.45);
  }

  .modal h3 {
    margin: 0 0 10px;
    font-size: 18px;
  }

  .modal p {
    margin: 0 0 10px;
  }

  .modal-list {
    margin: 8px 0 12px;
    max-height: 180px;
    overflow: auto;
    border-radius: 12px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(10, 10, 12, 0.6);
    padding: 10px;
    font-size: 12px;
    color: var(--ink);
  }

  .modal-row {
    padding: 4px 0;
    word-break: break-all;
  }

  .modal-actions {
    justify-content: flex-end;
  }

  .result-grid {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 16px;
    margin: 10px 0 12px;
    font-size: 13px;
  }

  .result-grid div:nth-child(odd) {
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-size: 11px;
  }

  .warning {
    color: #ffb36b;
  }

  .status-bubble {
    position: fixed;
    left: 50%;
    transform: translateX(-50%);
    bottom: 18px;
    padding: 8px 12px;
    font-size: 12px;
    color: var(--muted);
    background: rgba(20, 20, 22, 0.85);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 999px;
    backdrop-filter: blur(12px);
    z-index: 20;
    max-width: 55%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .lift-1 {
    animation: rise 0.7s ease 0.1s both;
  }

  .lift-2 {
    animation: rise 0.7s ease 0.2s both;
  }

  .lift-3 {
    animation: rise 0.7s ease 0.3s both;
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(12px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @media (max-width: 900px) {
    .hero {
      flex-direction: column;
    }

    .stats {
      width: 100%;
    }
  }
</style>




