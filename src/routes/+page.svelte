<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  if (import.meta.env.MODE === "test") import("../lib/ui-fixture.js");
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
  /** @type {"import" | "export" | null} */
  let resultKind = $state(null);
  let osSep = $state("/");
  let healthFilter = $state("All");
  let page = $state(0);
  let elapsed = $state("");
  let scanned = $state(false);
  let importName = $state("");
  let previewError = $state("");
  let previewBusy = $state(false);
  let incompleteMapping = $derived(
    mappings.some(
      (item) => Boolean(item.from.trim()) !== Boolean(item.to.trim()),
    ),
  );
  const pageSize = 100;
  let filtered = $derived(filteredEntries());
  let sorted = $derived(sortedEntries());
  let pageCount = $derived(Math.max(1, Math.ceil(sorted.length / pageSize)));
  let visible = $derived(
    sorted.slice(
      Math.min(page, pageCount - 1) * pageSize,
      (Math.min(page, pageCount - 1) + 1) * pageSize,
    ),
  );
  let health = $derived({
    OK: scanData.entries.filter((e) => e.status === "OK").length,
    Broken: scanData.entries.filter((e) => e.status === "Broken").length,
    Unreadable: scanData.entries.filter((e) => e.status === "Unreadable")
      .length,
  });
  $effect(() => {
    scanQuery;
    healthFilter;
    sortKey;
    sortDir;
    page = 0;
  });
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
    const candidates = scanData.entries.filter(
      (entry) => healthFilter === "All" || entry.status === healthFilter,
    );
    return includeTerms.length || excludeTerms.length
      ? candidates.filter((entry) => {
          const rel = (entry.relative || "").toString().toLowerCase();
          const tgt = (entry.target || "").toString().toLowerCase();
          const st = (entry.status || "").toString().toLowerCase();
          const matchesInclude =
            includeTerms.length === 0 ||
            includeTerms.some(
              (term) =>
                rel.includes(term) || tgt.includes(term) || st.includes(term),
            );
          const matchesExclude = excludeTerms.some(
            (term) =>
              rel.includes(term) || tgt.includes(term) || st.includes(term),
          );
          return matchesInclude && !matchesExclude;
        })
      : [...candidates];
  }

  /** @param {SymlinkEntry} entry */
  function getSortValue(entry) {
    if (sortKey === "target") return entry.target || "";
    if (sortKey === "status") return entry.status || "";
    return entry.relative || "";
  }

  /** @returns {SymlinkEntry[]} */
  function sortedEntries() {
    const entries = [...filtered];
    const dir = sortDir === "asc" ? 1 : -1;
    const keys = new Map(
      entries.map((entry) => [entry, getSortValue(entry).toLowerCase()]),
    );
    entries.sort((a, b) => {
      const left = keys.get(a) || "";
      const right = keys.get(b) || "";
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

  /** @param {ResultData} data @param {"import" | "export"} [kind] */
  function openResult(data, kind = "import") {
    resultKind = kind;
    resultData = data;
    resultOpen = true;
  }

  function closeResult() {
    resultOpen = false;
    resultData = null;
    resultKind = null;
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
    return () => {
      if (previewTimer) clearTimeout(previewTimer);
      previewSeq++;
    };
    return () => {
      if (previewTimer) clearTimeout(previewTimer);
      previewSeq++;
    };
  });

  /** @param {string} root */
  function addMappingFromRoot(root) {
    const trimmed = displayPath(root).trim();
    if (!trimmed) return;
    const index = mappings.findIndex((item) => !item.from.trim());
    if (index >= 0) {
      mappings = mappings.map((item, idx) =>
        idx == index ? { ...item, from: trimmed } : item,
      );
      return;
    }
    mappings = [...mappings, { from: trimmed, to: "" }];
  }

  /** @returns {MappingRule[]} */
  function cleanMappings() {
    return mappings
      .map((item) => ({ from: item.from.trim(), to: item.to.trim() }))
      .filter((item) => item.from && item.to);
  }

  /** @param {string} value */
  function normalizedPath(value) {
    let path = value.replaceAll("\\", "/").replace(/\/{2,}/g, "/");
    if (path.length > 1) path = path.replace(/\/+$/, "");
    return osSep === "\\" ? path.toLowerCase() : path;
  }

  /** @param {string} target @param {string} from @param {string} to */
  function replacePreviewRoot(target, from, to) {
    const source = normalizedPath(from);
    const value = normalizedPath(target);
    if (!source || !to.trim()) return target;
    if (value === source) return to;
    if (!value.startsWith(source + "/")) return target;
    const suffix = target.replaceAll("\\", "/").split("/").slice(source.split("/").length).join(osSep);
    return `${to.replace(/[\\/]$/, "")}${osSep}${suffix}`;
  }

  /** @param {string} target */
  function previewTarget(target) {
    let mapped = target;
    for (const rule of cleanMappings()) {
      const next = replacePreviewRoot(mapped, rule.from, rule.to);
      if (next !== mapped) {
        mapped = next;
        break;
      }
    }
    return replacePreviewRoot(mapped, importData?.src_root || "", dstRoot);
  }

  function localPreview() {
    const data = importData;
    if (!data || !dstRoot.trim() || !preview.root_samples.length) return preview;
    return {
      roots: preview.roots,
      sample: [],
      root_samples: preview.root_samples.map((item) => {
        const root = normalizedPath(item.root);
        const entry = data.entries.find((candidate) => {
          const target = normalizedPath(candidate.target);
          return target === root || target.startsWith(root + "/");
        });
        if (!entry) return item;
        const relative = entry.relative.replaceAll("\\", osSep).replaceAll("/", osSep);
        return {
          root: item.root,
          link: `${dstRoot.replace(/[\\/]$/, "")}${osSep}${relative}`,
          target: previewTarget(entry.target),
        };
      }),
    };
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
    try {
      const selected = await open({ directory: true, multiple: false });
      if (selected) scanRoot = selected;
    } catch (err) {
      setStatus(`Folder selection failed: ${err}`);
    }
  }

  async function browseTarget() {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (selected) dstRoot = selected;
    } catch (err) {
      setStatus(`Folder selection failed: ${err}`);
    }
  }

  async function runScan() {
    if (working) return;
    if (!scanRoot) {
      setStatus("Select a folder to scan.");
      return;
    }
    working = true;
    setStatus("Scanning folders and checking targets…");
    const started = performance.now();
    try {
      const result = await invoke("scan_symlinks", { root: scanRoot });
      scanData = result;
      scanned = true;
      page = 0;
      elapsed = ((performance.now() - started) / 1000).toFixed(2);
      const skippedNote = result.skipped
        ? ` Skipped ${result.skipped} entries.`
        : "";
      setStatus(
        `Scan complete. ${result.entries.length} symlinks found.${skippedNote}`,
      );
    } catch (err) {
      setStatus(`Scan failed: ${err}`);
    } finally {
      working = false;
    }
  }

  async function exportJson() {
    if (working) return;
    const entries = [...filtered];
    if (!entries.length) {
      setStatus("Run a scan before exporting.");
      return;
    }
    working = true;
    try {
      const path = await save({
        filters: [{ name: "JSON", extensions: ["json"] }],
        defaultPath: "symlinks.json",
      });
      if (!path) return;
      await invoke("export_symlinks", {
        path,
        data: {
          src_root: scanData.src_root,
          entries,
        },
      });
      setStatus(`Exported JSON to ${path}`);
      openResult(
        { created: entries.length, failed: 0, sample_links: [path] },
        "export",
      );
    } catch (err) {
      setStatus(`Export failed: ${err}`);
    } finally {
      working = false;
    }
  }

  async function loadJson() {
    if (working) return;
    working = true;
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!selected) return;
      const result = await invoke("load_export", { path: selected });
      importData = result;
      importName = selected.split(/[\\/]/).pop() || selected;
      preview = { roots: [], sample: [], root_samples: [] };
      lastFailures = [];
      setStatus(`Loaded ${result.entries.length} entries.`);
    } catch (err) {
      setStatus(`Load failed: ${err}`);
    } finally {
      working = false;
    }
  }

  function scheduleAutoPreview() {
    previewSeq++;
    previewError = "";
    previewBusy = false;
    if (previewTimer) {
      clearTimeout(previewTimer);
      previewTimer = null;
    }
    if (activeTab !== "import") return;
    if (!importData || !dstRoot) return;
    preview = localPreview();
    previewBusy = true;
    previewTimer = setTimeout(() => {
      previewRecreate(true);
    }, 300);
  }

  $effect(() => {
    activeTab;
    importData;
    dstRoot;
    const mappingKey = mappings
      .map((item) => `${item.from}|${item.to}`)
      .join("||");
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
    previewBusy = true;
    const seq = (previewSeq += 1);
    try {
      const result = await invoke("preview_recreate", {
        data: importData,
        dstRoot,
        mappings: cleanMappings(),
        maxPreview: 20,
      });
      if (seq !== previewSeq) return;
      preview = {
        roots: result.roots,
        sample: result.sample,
        root_samples: result.root_samples || [],
      };
      if (!silent) {
        setStatus(
          `Preview ready. ${result.roots.length} target roots detected.`,
        );
      }
    } catch (err) {
      if (seq === previewSeq) previewError = `${err}`;
      if (!silent) {
        setStatus(`Preview failed: ${err}`);
      }
    } finally {
      if (seq === previewSeq) previewBusy = false;
      if (!silent) {
        working = false;
      }
    }
  }

  async function recreate() {
    if (working) return;
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
        mappings: cleanMappings(),
      });
      if (conflicts.total > 0) {
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
        missingAsDir: false,
      });
      openResult({
        created: result.created,
        failed: result.failed.length,
        sample_links: result.sample_links || [],
      });
      lastFailures = result.failed || [];
      const failures = result.failed.length;
      if (failures) {
        const needsAdmin = result.failed.some((/** @type {string} */ item) =>
          item.includes("os error 1314"),
        );
        if (needsAdmin) {
          closeResult();
          try {
            lastFailures = [];
            const jobId = await invoke("start_admin_recreate", {
              data: importData,
              dstRoot,
              mappings: cleanMappings(),
              missingAsDir: false,
            });
            setStatus(
              "Admin prompt opened. Approve to continue recreating symlinks.",
            );
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
            missingAsDir: false,
          });
          lastFailures = [];
          setStatus(
            "Admin prompt opened. Approve to continue recreating symlinks.",
          );
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
          lastFailures = result.errors || [];
          openResult({
            created: result.created,
            failed: result.failed,
            sample_links: result.sample_links || [],
          });
          setStatus(
            `Created ${result.created} symlinks. ${result.failed} failed.`,
          );
          return;
        }
      } catch (err) {
        setStatus(`Admin result check failed: ${err}`);
        return;
      }
    }
    setStatus(
      "Admin recreate still running. Check the destination folder in a moment.",
    );
  }

  /** @param {HTMLDialogElement} node */
  function showDialog(node) {
    node.showModal();
  }
</script>

<main class="app">
  <aside class="sidebar">
    <a
      class="brand"
      href="/"
      onclick={(e) => {
        e.preventDefault();
        activeTab = "scan";
      }}
      ><img src="/logo.svg" alt="" /><span
        >Symlink<span class="brand-sub">MANAGER</span></span
      ></a
    >
    <div class="nav-label">WORKSPACE</div>
    <nav aria-label="Main navigation">
      <button
        class:active={activeTab === "scan"}
        onclick={() => (activeTab = "scan")}
        aria-current={activeTab === "scan" ? "page" : undefined}
        ><span class="nav-icon">⌕</span><span
          >Scan & export<small>Explore your connections</small></span
        ></button
      >
      <button
        class:active={activeTab === "import"}
        onclick={() => (activeTab = "import")}
        aria-current={activeTab === "import" ? "page" : undefined}
        ><span class="nav-icon">↗</span><span
          >Import & recreate<small>Move links, keep connections</small></span
        ></button
      >
    </nav>
    <div class="platform">
      v1.1.0
    </div>
  </aside>
  <div class="workspace">
    <div class="content">
      <header class="page-heading">
        <div>
          <h1>
            {activeTab === "scan"
              ? "Discover & Preserve"
              : "A new home for your links."}
          </h1>
          <p class="subtitle">
            {activeTab === "scan"
              ? "Explore symbolic links, check their health, and take a portable snapshot."
              : "Load a snapshot, map your paths, and recreate your connections."}
          </p>
        </div>
        <span class="heading-symbol" aria-hidden="true"
          >{activeTab === "scan" ? "⌕" : "↗"}</span
        >
      </header>
      {#if activeTab === "scan"}
        <section class="scan-control panel" aria-label="Scan folder">
          <form
            onsubmit={(e) => {
              e.preventDefault();
              runScan();
            }}
            class="row"
          >
            <label class="sr-only" for="scan-root">Folder to scan</label>
            <div class="path-input">
              <span aria-hidden="true">⌑</span><input
                id="scan-root"
                placeholder={osSep === "/"
                  ? "/home/you/library"
                  : "D:\\Media\\Library"}
                bind:value={scanRoot}
                disabled={working}
              />
            </div>
            <button
              type="button"
              class="ghost"
              onclick={browseScan}
              disabled={working}>Browse</button
            ><button class="accent" disabled={working || !scanRoot.trim()}
              >{working ? "Working…" : "Scan symlinks"}<span>→</span></button
            >
          </form>
        </section>
        <section class="health-grid" aria-label="Link health filters">
          {#each ["All", "OK", "Broken", "Unreadable"] as kind}
            <button
              class="health-card"
              class:selected={healthFilter === kind}
              class:healthy={kind === "OK"}
              class:broken={kind === "Broken"}
              class:unreadable={kind === "Unreadable"}
              onclick={() => (healthFilter = kind)}
              aria-pressed={healthFilter === kind}
              ><span class="health-label"
                ><span class="dot"></span>{kind === "All"
                  ? "Total links"
                  : kind === "OK"
                    ? "Healthy"
                    : kind}</span
              ><strong
                >{kind === "All"
                  ? scanData.entries.length.toLocaleString()
                  : health[
                      /** @type {'OK'|'Broken'|'Unreadable'} */ (kind)
                    ].toLocaleString()}</strong
              ></button
            >
          {/each}
        </section>
        <section class="panel results">
          <div class="toolbar">
            <label class="search"
              ><span aria-hidden="true">⌕</span><input
                aria-label="Search links"
                placeholder="Search paths, targets, or status…"
                bind:value={scanQuery}
              /></label
            ><span class="search-help"
              >Comma = OR <span>·</span> −term = exclude</span
            >{#if scanQuery || healthFilter !== "All"}<button
                class="text-button"
                onclick={() => {
                  scanQuery = "";
                  healthFilter = "All";
                }}>Clear filters</button
              >{/if}<button
              class="ghost export-button"
              onclick={exportJson}
              disabled={working || !filtered.length}
              >↓ &nbsp; Export JSON</button
            >
          </div>
          <div class="table-wrap" aria-busy={working}>
            <table>
              <thead
                ><tr
                  >{#each [["relative", "LINK PATH"], ["target", "TARGET DESTINATION"], ["status", "HEALTH"]] as [key, label]}<th
                      aria-sort={sortKey === key
                        ? sortDir === "asc"
                          ? "ascending"
                          : "descending"
                        : "none"}
                      ><button
                        class="sort"
                        onclick={() =>
                          setSort(
                            /** @type {'relative'|'target'|'status'} */ (key),
                          )}
                        >{label}<span
                          >{sortKey === key
                            ? sortDir === "asc"
                              ? "↑"
                              : "↓"
                            : "↕"}</span
                        ></button
                      ></th
                    >{/each}</tr
                ></thead
              ><tbody
                >{#each visible as entry}<tr
                    ><td title={displayPath(entry.relative)}
                      ><span class="link-glyph" aria-hidden="true">↗</span
                      >{displayPath(entry.relative)}</td
                    ><td title={displayPath(entry.target)} class="target-cell"
                      >{displayPath(entry.target)}</td
                    ><td
                      ><span
                        class="badge"
                        class:healthy={entry.status === "OK"}
                        class:broken={entry.status === "Broken"}
                        class:unreadable={entry.status === "Unreadable"}
                        ><span class="dot"></span>{entry.status}</span
                      ></td
                    ></tr
                  >{/each}</tbody
              >
            </table>
            {#if !visible.length}<div class="empty-state">
                <div class="empty-icon" aria-hidden="true">
                  {scanned ? "⌕" : "↗"}
                </div>
                <h3>
                  {working
                    ? "Following your connections…"
                    : !scanned
                      ? "Your next connection starts here"
                      : scanData.entries.length
                        ? "No matching links"
                        : "No symbolic links found"}
                </h3>
                <p>
                  {!scanned
                    ? "Choose a folder above to discover and inspect its symbolic links."
                    : scanData.entries.length
                      ? "Try a different search or clear your health filter."
                      : "Try another folder. Regular files are not included in the inventory."}
                </p>
                {#if !scanned}<span class="empty-flow"
                    >SCAN <span>→</span> INSPECT <span>→</span> EXPORT</span
                  >{/if}
              </div>{/if}
          </div>
          <footer class="table-footer">
            <span
              >{filtered.length
                ? Math.min(page, pageCount - 1) * pageSize + 1
                : 0}–{Math.min(
                (Math.min(page, pageCount - 1) + 1) * pageSize,
                filtered.length,
              )} of {filtered.length.toLocaleString()} links{#if elapsed}
                <span class="slash">·</span> Scan {elapsed}s{/if}{#if scanData.skipped}
                <span class="warning">
                  · {scanData.skipped} skipped</span
                >{/if}</span
            >
            <div class="pagination">
              <button
                aria-label="Previous page"
                disabled={page === 0}
                onclick={() => page--}>←</button
              ><span>{Math.min(page + 1, pageCount)} / {pageCount}</span><button
                aria-label="Next page"
                disabled={page >= pageCount - 1}
                onclick={() => page++}>→</button
              >
            </div>
          </footer>
        </section>
      {:else}
        <fieldset class="import-fields" disabled={working}>
          <div class="import-grid">
            <section class="panel">
              <div class="section-label">
                <span class="step">01</span>
                <div>
                  <h2>Load your snapshot</h2>
                  <p>A JSON export from Symlink Manager.</p>
                </div>
              </div>
              <button class="upload-zone" onclick={loadJson}
                ><span class="upload-icon">↓</span><strong
                  >{importName || "Choose a JSON export"}</strong
                ><span
                  >{importData
                    ? importData.entries.length.toLocaleString() +
                      " links ready to reconnect · Click to change"
                    : "Browse your files to get started"}</span
                ></button
              >
            </section>
            <section class="panel">
              <div class="section-label">
                <span class="step">02</span>
                <div>
                  <h2>Set the destination</h2>
                  <p>The folder where your new links will live.</p>
                </div>
              </div>
              <label for="target-root">Destination folder</label>
              <div class="row">
                <input
                  id="target-root"
                  placeholder={osSep === "/"
                    ? "/home/you/links"
                    : "D:\\Media\\Links"}
                  bind:value={dstRoot}
                /><button class="ghost" onclick={browseTarget}>Browse</button>
              </div>
              <p class="hint">
                The original folder structure is preserved. Target files are not
                copied.
              </p>
            </section>
          </div>
          <section class="panel mapping-panel">
            <div class="results-heading">
              <div class="section-label">
                <span class="step">03</span>
                <div>
                  <h2>
                    Reconnect target paths <span class="optional">OPTIONAL</span
                    >
                  </h2>
                  <p>
                    Moving drives? Replace a target prefix. The first matching
                    rule wins.
                  </p>
                </div>
              </div>
              <button class="ghost" onclick={addMapping}>+ Add rule</button>
            </div>
            <div class="mapping-labels">
              <span>ORIGINAL TARGET PREFIX</span><span>NEW TARGET PREFIX</span>
            </div>
            {#each mappings as mapping, index}<div class="mapping-row">
                <input
                  aria-label={"Original prefix, rule " + (index + 1)}
                  placeholder={osSep === "/" ? "/old/media" : "W:\\Shows"}
                  bind:value={mapping.from}
                /><span class="arrow">→</span><input
                  aria-label={"New prefix, rule " + (index + 1)}
                  placeholder={osSep === "/" ? "/mnt/media" : "E:\\Shows"}
                  bind:value={mapping.to}
                /><button
                  class="remove"
                  aria-label={"Remove rule " + (index + 1)}
                  onclick={() => removeMapping(index)}>×</button
                >
              </div>{/each}
            {#if incompleteMapping}<p class="warning hint">
                Complete both prefixes in each rule, or remove the unfinished
                rule.
              </p>{/if}
            <p class="hint">
              Targets inside the original scan root also follow the new
              destination.
            </p>
          </section>
          <section class="panel preview-panel">
            <div class="results-heading">
              <div>
                <h2>Review the connections</h2>
                <p>
                  {previewBusy
                    ? "Updating preview…"
                    : "Preview updates automatically as you edit."}
                </p>
              </div>
            </div>
            {#if previewError}<p class="warning preview-warning" role="alert">
                {previewError}
              </p>{/if}
            {#if preview.roots.length}<div class="preview-grid">
                <div>
                  <p class="eyebrow">DETECTED ROOTS · CLICK TO MAP</p>
                  {#each preview.roots as item}<button
                      class="root-link"
                      onclick={() => addMappingFromRoot(item.root)}
                      ><span>{displayPath(item.root)}</span><span class="count"
                        >{item.count}</span
                      ></button
                    >{/each}
                </div>
                <div>
                  <p class="eyebrow">ONE EXAMPLE PER ROOT</p>
                  {#each preview.root_samples as item}<div class="preview-link">
                      <span>{displayPath(item.link)}</span><span
                        class="preview-target"
                        >↳ {displayPath(item.target)}</span
                      >
                    </div>{/each}
                </div>
              </div>{:else}<div class="preview-empty">
                Load a snapshot and choose a destination to preview your links.
              </div>{/if}
          </section>
          <div class="recreate-bar">
            <p>
              <strong>Ready to reconnect?</strong><span
                >Existing items require confirmation before replacement.</span
              >
            </p>
            <button
              class="accent"
              onclick={recreate}
              disabled={!importData?.entries.length ||
                !dstRoot.trim() ||
                previewBusy ||
                !!previewError ||
                incompleteMapping}
              >{working ? "Working…" : "Recreate links"} <span>↗</span></button
            >
          </div>
        </fieldset>
        {#if lastFailures.length}<section class="panel failures">
            <h2>{lastFailures.length} links need attention</h2>
            <p>First 10 errors</p>
            {#each lastFailures.slice(0, 10) as item}<div>{item}</div>{/each}
          </section>{/if}
      {/if}
    </div>
    <footer class="statusbar" role="status" aria-live="polite">
      <span class="dot" class:pulse={working}></span><span
        >{status ||
          "Ready when you are. Choose a folder or load a snapshot."}</span
      ><span class="status-end">SYMLINK MANAGER</span>
    </footer>
  </div>
  {#if confirmOpen}<dialog
      use:showDialog
      oncancel={(/** @type {Event} */ e) => {
        e.preventDefault();
        closeConfirm(false);
      }}
      aria-labelledby="confirm-title"
    >
      <span class="dialog-icon warning">↻</span>
      <h2 id="confirm-title">Replace existing items?</h2>
      <p>
        {confirmData?.total} destination items already exist and will be replaced.
      </p>
      {#if (confirmData?.non_symlink ?? 0) > 0}<p class="warning">
          Includes {confirmData?.non_symlink} real files or folders. Non-empty folders
          will be preserved.
        </p>{/if}
      <div class="modal-list">
        {#each confirmData?.sample ?? [] as item}<div>
            {displayPath(item)}
          </div>{/each}
      </div>
      <div class="modal-actions">
        <button class="ghost" onclick={() => closeConfirm(false)}>Cancel</button
        ><button class="danger" onclick={() => closeConfirm(true)}
          >Replace & recreate</button
        >
      </div>
    </dialog>{/if}
  {#if resultOpen}<dialog
      use:showDialog
      oncancel={closeResult}
      aria-labelledby="result-title"
    >
      <span class="dialog-icon">{resultData?.failed ? "!" : "✓"}</span>
      <h2 id="result-title">
        {resultKind === "export" ? "Snapshot saved" : "Recreation complete"}
      </h2>
      <p>
        {resultKind === "export"
          ? "Your connections are ready to travel."
          : "Review the results of your operation below."}
      </p>
      <div class="result-summary">
        <div>
          <strong>{resultData?.created ?? 0}</strong><span
            >{resultKind === "export" ? "Exported" : "Created"}</span
          >
        </div>
        <div><strong>{resultData?.failed ?? 0}</strong><span>Failed</span></div>
      </div>
      <div class="modal-list">
        {#each resultData?.sample_links ?? [] as item}<div>
            {displayPath(item)}
          </div>{/each}
      </div>
      <div class="modal-actions">
        <button class="accent" onclick={closeResult}>Done</button>
      </div>
    </dialog>{/if}
</main>

<style>
  :global(:root) {
    font-family: "Segoe UI", system-ui, sans-serif;
    color: #e7eeec;
    background: #0d1214;
    color-scheme: dark;
    font-synthesis: none;
    --muted: #8e9e9e;
    --line: #273234;
    --accent: #83e3be;
  }
  :global(*) {
    box-sizing: border-box;
  }
  :global(body) {
    margin: 0;
  }
  :global(button),
  :global(input) {
    font: inherit;
  }
  :global(button) {
    cursor: pointer;
  }
  :global(button:disabled) {
    opacity: 0.4;
    cursor: not-allowed;
  }
  :global(button:focus-visible),
  :global(a:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 4px;
  }
  :global(input:focus) {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px #83e3be13;
  }
  :global(::selection) {
    background: #3c7460;
    color: white;
  }
  :global(::-webkit-scrollbar) {
    width: 8px;
    height: 8px;
  }
  :global(::-webkit-scrollbar-thumb) {
    background: #3a494b;
    border-radius: 8px;
  }
  .app {
    min-height: 100vh;
  }
  .sidebar {
    width: 228px;
    position: fixed;
    inset: 0 auto 0 0;
    padding: 30px 18px 20px;
    background: #101719;
    border-right: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    z-index: 2;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 12px;
    text-decoration: none;
    color: #eff8f5;
    font-size: 23px;
    font-weight: 650;
    letter-spacing: -0.7px;
    padding: 0 10px;
  }
  .brand img {
    width: 42px;
    height: 42px;
  }
  .brand-sub {
    display: block;
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 3.2px;
    color: var(--muted);
    margin-top: 1px;
  }
  .nav-label {
    font-size: 10px;
    letter-spacing: 1.6px;
    color: #718484;
    margin: 48px 14px 14px;
  }
  nav {
    display: grid;
    gap: 8px;
  }
  nav button {
    display: flex;
    text-align: left;
    align-items: center;
    gap: 12px;
    padding: 14px 12px;
    border: 1px solid transparent;
    border-radius: 9px;
    background: none;
    color: #a3b0b0;
    font-size: 13px;
    font-weight: 600;
  }
  nav button.active {
    background: #83e3be0d;
    border-color: #83e3be27;
    color: var(--accent);
  }
  nav small {
    display: block;
    font-size: 10px;
    font-weight: 400;
    margin-top: 5px;
    color: #7f9590;
  }
  .nav-icon {
    font-size: 25px;
    line-height: 1;
  }
  .platform {
    margin-top: auto;
    border-top: 1px solid var(--line);
    padding: 16px 10px 0;
    font-size: 10px;
    letter-spacing: 0.8px;
    color: #718484;
  }
  .dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    background: currentColor;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .workspace {
    margin-left: 228px;
    min-width: 0;
  }
  .content {
    padding: 24px 36px 52px;
    max-width: 1600px;
    margin: auto;
  }
  .page-heading {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 18px;
  }
  .eyebrow {
    font-size: 9px;
    letter-spacing: 1.8px;
    color: var(--accent);
    font-weight: 600;
    margin: 0 0 10px;
  }
  h1 {
    font-size: 32px;
    letter-spacing: -1.1px;
    line-height: 1.2;
    margin: 0;
    font-weight: 600;
  }
  .subtitle {
    font-size: 12px;
    color: var(--muted);
    margin: 8px 0 0;
    line-height: 1.6;
  }
  .heading-symbol {
    font-size: 58px;
    font-weight: 200;
    color: #3b6356;
    margin-right: 12px;
  }
  .panel {
    background: #131b1e;
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 16px 20px;
  }
  h2 {
    font-size: 14px;
    font-weight: 600;
    letter-spacing: -0.15px;
    margin: 0;
  }
  p {
    line-height: 1.6;
  }
  .section-label {
    display: flex;
    gap: 13px;
    align-items: center;
    margin-bottom: 20px;
  }
  .step {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border: 1px solid #345247;
    border-radius: 8px;
    font-size: 11px;
    color: var(--accent);
    flex-shrink: 0;
  }
  .section-label p,
  .results-heading p {
    font-size: 11px;
    color: var(--muted);
    margin: 5px 0 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .row input {
    flex: 1;
    min-width: 0;
  }
  .path-input {
    display: flex;
    align-items: center;
    flex: 1;
    min-width: 0;
    position: relative;
  }
  .path-input > span {
    position: absolute;
    left: 14px;
    color: #819a90;
    font-size: 20px;
  }
  .path-input input {
    padding-left: 42px;
    width: 100%;
  }
  input {
    background: #0e1517;
    color: #d6e2de;
    border: 1px solid #2e3d3e;
    border-radius: 7px;
    padding: 11px 13px;
    font-size: 12px;
    min-width: 0;
    transition: border-color 0.15s;
  }
  input::placeholder {
    color: #677d7a;
  }
  button {
    border-radius: 7px;
    border: 1px solid transparent;
    padding: 10px 15px;
    font-size: 12px;
    transition:
      background 0.15s,
      color 0.15s;
  }
  .accent {
    background: var(--accent);
    color: #10281f;
    font-weight: 650;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 24px;
  }
  .accent:hover:not(:disabled) {
    background: #a5f0d1;
  }
  .ghost {
    background: #192326;
    border-color: #344244;
    color: #d4dfdb;
    white-space: nowrap;
  }
  .ghost:hover:not(:disabled) {
    background: #263431;
    border-color: #557266;
  }
  .health-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 14px;
    margin: 14px 0;
  }
  .health-card {
    background: #131b1e;
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 13px 18px;
    text-align: left;
    position: relative;
    color: #bacac5;
  }
  .health-card.selected {
    border-color: #668d7c;
    background: #182521;
  }
  .health-label {
    font-size: 11px;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .health-card strong {
    display: block;
    font-size: 29px;
    font-weight: 550;
    letter-spacing: -1px;
    margin: 6px 0 0;
    color: #edf4f0;
    font-variant-numeric: tabular-nums;
  }
  .healthy {
    color: #83e3be;
  }
  .broken {
    color: #ef929a;
  }
  .unreadable,
  .warning {
    color: #e9bb78;
  }
  .results {
    padding: 0;
    overflow: hidden;
  }
  .results-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
  }
  .results-heading > div {
    min-width: 0;
  }
  .results-heading p {
    overflow-wrap: anywhere;
  }
  .count {
    font-size: 10px;
    background: #263833;
    color: #a6c5b8;
    padding: 3px 7px;
    border-radius: 5px;
    margin-left: 8px;
    font-weight: 500;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 15px;
    border-bottom: 1px solid var(--line);
    padding: 12px 22px;
    background: #11191b;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
  }
  .search > span {
    font-size: 22px;
    color: #728c83;
  }
  .search input {
    background: transparent;
    padding: 6px 0;
    border: none;
    box-shadow: none;
    width: 100%;
    font-size: 11px;
  }
  .search:focus-within {
    outline: 1px solid var(--accent);
    outline-offset: 5px;
    border-radius: 3px;
  }
  .search-help {
    font-size: 10px;
    color: #768c85;
    white-space: nowrap;
  }
  .search-help > span {
    margin: 0 6px;
  }
  .text-button {
    background: none;
    color: var(--accent);
    padding: 0;
    font-size: 10px;
  }
  .export-button {
    margin-left: auto;
  }
  .table-wrap {
    overflow: auto;
    max-height: min(620px, calc(100vh - 500px));
    min-height: min(260px, calc(100vh - 500px));
  }
  table {
    border-collapse: collapse;
    width: 100%;
    table-layout: fixed;
  }
  th {
    position: sticky;
    top: 0;
    background: #182124;
    z-index: 1;
    text-align: left;
  }
  th:first-child {
    width: 46%;
  }
  th:nth-child(2) {
    width: 46%;
  }
  th:last-child {
    width: 8%;
    min-width: 84px;
    text-align: right;
  }
  .sort {
    background: none;
    color: #8ba099;
    padding: 12px 22px;
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    font-size: 9px;
    font-weight: 500;
    letter-spacing: 0.8px;
    white-space: nowrap;
  }
  .sort span {
    color: #627e70;
  }
  th:last-child .sort {
    justify-content: flex-end;
  }
  td {
    font-family: Consolas, "Cascadia Code", monospace;
    font-size: 11px;
    color: #c3d1cc;
    padding: 13px 22px;
    border-bottom: 1px solid #223032;
    overflow-wrap: anywhere;
    vertical-align: top;
    line-height: 1.7;
  }
  tr:hover td {
    background: #1a2727;
  }
  .target-cell {
    color: #80988e;
  }
  .link-glyph {
    margin-right: 9px;
    color: #789f8b;
  }
  .badge {
    font:
      10px "Segoe UI",
      sans-serif;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 7px;
    border-radius: 5px;
    background: #7c9e8d0b;
    white-space: nowrap;
    border: 1px solid #7c9e8d20;
  }
  td:last-child {
    text-align: right;
  }
  .empty-state {
    text-align: center;
    padding: 38px 20px 32px;
  }
  .empty-icon {
    font-size: 30px;
    color: var(--accent);
    margin: 0 auto 18px;
    background: #83e3be08;
    border: 1px solid #83e3be20;
    border-radius: 14px;
    width: 55px;
    height: 55px;
    display: grid;
    place-items: center;
  }
  .empty-state h3 {
    font-size: 15px;
    font-weight: 500;
    margin: 0 0 8px;
  }
  .empty-state p {
    font-size: 11px;
    color: #81988e;
    margin: 0;
  }
  .empty-flow {
    display: inline-flex;
    gap: 15px;
    font-size: 8px;
    letter-spacing: 1.7px;
    color: #708a7d;
    margin-top: 26px;
  }
  .empty-flow > span {
    color: #416e59;
  }
  .table-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 11px 22px;
    font-size: 10px;
    color: #83988f;
    border-top: 1px solid var(--line);
  }
  .pagination {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .pagination button {
    background: #1b2927;
    color: #c3d4cb;
    border-color: #2d4138;
    padding: 4px 10px;
  }
  .statusbar {
    position: fixed;
    bottom: 0;
    left: 228px;
    right: 0;
    min-height: 34px;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 9px 25px;
    border-top: 1px solid var(--line);
    background: #101819;
    font-size: 10px;
    color: #8b9e95;
    z-index: 3;
  }
  .statusbar > .dot {
    color: var(--accent);
  }
  .statusbar > span:nth-child(2) {
    overflow-wrap: anywhere;
  }
  .status-end {
    margin-left: auto;
    font-size: 8px;
    letter-spacing: 1.3px;
    white-space: nowrap;
    color: #627e70;
  }
  .pulse {
    animation: pulse 1s infinite alternate;
  }
  @keyframes pulse {
    to {
      opacity: 0.2;
    }
  }
  .import-fields {
    border: 0;
    padding: 0;
    margin: 0;
    min-width: 0;
  }
  .import-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 18px;
    margin-bottom: 18px;
  }
  .upload-zone {
    background: #101c1b;
    border: 1px dashed #47685a;
    display: flex;
    align-items: center;
    flex-direction: column;
    gap: 5px;
    width: 100%;
    padding: 12px;
    color: #c8ded2;
  }
  .upload-icon {
    font-size: 18px;
    color: var(--accent);
  }
  .upload-zone strong {
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .upload-zone > span:last-child {
    font-size: 10px;
    color: #7d9c8b;
  }
  .import-grid .section-label {
    margin-bottom: 14px;
  }
  .import-grid label {
    display: block;
    font-size: 11px;
    color: #aec1b6;
    margin: 14px 0 8px;
  }
  .hint {
    font-size: 10px;
    color: #839b8e;
    margin: 15px 0 0;
  }
  .import-grid .hint {
    margin-top: 10px;
  }
  .mapping-panel {
    margin-bottom: 18px;
  }
  .mapping-panel .results-heading {
    padding: 0;
    margin-bottom: 22px;
  }
  .mapping-panel .section-label {
    margin: 0;
  }
  .optional {
    font-size: 8px;
    letter-spacing: 1px;
    color: #819a8c;
    margin-left: 8px;
  }
  .mapping-labels {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 38px;
    padding-right: 40px;
    font-size: 8px;
    letter-spacing: 1px;
    color: #749383;
    margin-bottom: 9px;
  }
  .mapping-row {
    display: grid;
    grid-template-columns: 1fr 18px 1fr 30px;
    gap: 10px;
    margin-top: 8px;
    align-items: center;
  }
  .arrow {
    color: var(--accent);
    text-align: center;
  }
  .remove {
    padding: 2px;
    background: transparent;
    color: #a6b9ae;
    font-size: 22px;
  }
  .remove:hover {
    color: #ef929a;
  }
  .preview-panel {
    padding: 0;
    overflow: hidden;
  }
  .preview-grid {
    display: grid;
    grid-template-columns: 1fr 1.5fr;
    gap: 25px;
    padding: 20px 22px;
    border-top: 1px solid var(--line);
    max-height: 360px;
    overflow: auto;
  }
  .preview-grid .eyebrow {
    color: #859f91;
    font-size: 8px;
  }
  .root-link {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    width: 100%;
    padding: 10px 0;
    border-bottom: 1px solid #24372c;
    border-radius: 0;
    background: none;
    color: #bfd2c5;
    text-align: left;
    font-size: 11px;
    overflow-wrap: anywhere;
  }
  .root-link:hover {
    color: var(--accent);
  }
  .preview-link {
    display: grid;
    gap: 5px;
    padding: 10px 0;
    border-bottom: 1px solid #24372c;
    font:
      10px Consolas,
      monospace;
    overflow-wrap: anywhere;
  }
  .preview-target {
    color: var(--accent);
  }
  .preview-empty {
    padding: 30px;
    text-align: center;
    border-top: 1px solid var(--line);
    font-size: 12px;
    color: #7d9888;
  }
  .preview-warning {
    margin: 0;
    padding: 10px 22px 0;
    font-size: 10px;
  }
  .recreate-bar {
    display: flex;
    justify-content: space-between;
    gap: 20px;
    align-items: center;
    padding: 22px 0;
  }
  .recreate-bar p {
    margin: 0;
  }
  .recreate-bar strong {
    display: block;
    font-size: 12px;
    font-weight: 500;
  }
  .recreate-bar p span {
    display: block;
    font-size: 10px;
    color: #89a091;
    margin-top: 5px;
  }
  .failures {
    color: #ef929a;
    font-size: 11px;
    overflow-wrap: anywhere;
  }
  .failures > div {
    padding: 8px 0;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
  }
  dialog {
    background: #182320;
    color: #e2eee7;
    border: 1px solid #466150;
    border-radius: 16px;
    padding: 28px;
    width: min(520px, 90vw);
    max-height: 85vh;
    overflow: auto;
    box-shadow: 0 30px 100px #0009;
  }
  dialog::backdrop {
    background: #040b09bf;
    backdrop-filter: blur(4px);
  }
  dialog h2 {
    font-size: 22px;
    margin: 16px 0 10px;
  }
  dialog p {
    font-size: 12px;
    color: #a3b7aa;
  }
  .dialog-icon {
    font-size: 28px;
    color: var(--accent);
  }
  .modal-list {
    max-height: 180px;
    overflow: auto;
    font:
      11px Consolas,
      monospace;
    background: #101a15;
    border-radius: 7px;
    margin: 16px 0;
    padding: 12px;
    overflow-wrap: anywhere;
  }
  .modal-list > div {
    padding: 5px 0;
  }
  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 20px;
  }
  .danger {
    background: #edaaa9;
    color: #381515;
    font-weight: 600;
  }
  .result-summary {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 15px;
    padding: 20px;
    background: #101a15;
    border-radius: 9px;
  }
  .result-summary strong {
    display: block;
    font-size: 28px;
    color: var(--accent);
  }
  .result-summary span {
    font-size: 11px;
    color: #a3b7aa;
  }
  @media (min-width: 1500px) {
    .content {
      padding-top: 40px;
    }
  }
  @media (max-width: 1100px) {
    .sidebar {
      width: 190px;
      padding-inline: 12px;
    }
    .workspace {
      margin-left: 190px;
    }
    .statusbar {
      left: 190px;
    }
    .content {
      padding: 24px 22px 65px;
    }
    .search-help {
      display: none;
    }
    .health-card {
      padding: 15px;
    }
    th:first-child {
      width: 44%;
    }
    th:nth-child(2) {
      width: 44%;
    }
    th:last-child {
      width: 12%;
    }
    .sort,
    td {
      padding: 12px 14px;
    }
    .brand {
      font-size: 21px;
      gap: 8px;
    }
    .brand img {
      width: 36px;
    }
  }
  @media (max-width: 760px) {
    .sidebar {
      position: static;
      width: auto;
      padding: 15px;
      flex-direction: row;
      align-items: center;
      justify-content: space-between;
    }
    .brand-sub,
    .nav-label,
    .platform,
    nav small,
    .nav-icon {
      display: none;
    }
    nav {
      display: flex;
    }
    nav button {
      padding: 10px;
      font-size: 11px;
    }
    .brand {
      font-size: 17px;
    }
    .brand img {
      width: 28px;
      height: 28px;
    }
    .workspace {
      margin-left: 0;
    }
    .statusbar {
      left: 0;
    }
    .status-end,
    .heading-symbol,
    .content {
      padding: 22px 16px 70px;
    }
    h1 {
      font-size: 27px;
    }
    .health-grid {
      grid-template-columns: 1fr 1fr;
      gap: 10px;
    }
    .row {
      flex-wrap: wrap;
    }
    .path-input {
      flex-basis: 100%;
    }
    .scan-control .row > .accent {
      flex: 1;
    }
    .import-grid {
      grid-template-columns: 1fr;
    }
    .preview-grid {
      grid-template-columns: 1fr;
    }
    .table-wrap table {
      min-width: 610px;
    }
    .results-heading {
      padding: 17px;
    }
    .mapping-labels {
      display: none;
    }
    .mapping-row {
      grid-template-columns: 1fr 18px 1fr 24px;
      gap: 5px;
    }
    .panel {
      padding: 17px;
    }
    .results,
    .preview-panel {
      padding: 0;
    }
    .recreate-bar {
      align-items: flex-start;
    }
    .recreate-bar .accent {
      gap: 10px;
      white-space: nowrap;
    }
  }
  @media (min-width: 761px) and (max-height: 800px) {
    .content {
      padding-top: 20px;
    }
    .page-heading {
      margin-bottom: 18px;
    }
    h1 {
      font-size: 28px;
    }
    .subtitle {
      margin-top: 8px;
    }
    .health-grid {
      margin: 16px 0;
    }
    .health-card {
      padding: 13px 18px;
    }
    .health-card strong {
      font-size: 26px;
      margin: 5px 0;
    }
    .empty-state {
      padding: 24px 20px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .pulse {
      animation: none;
    }
    :global(*) {
      transition: none !important;
    }
  }
</style>
