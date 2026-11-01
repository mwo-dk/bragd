<script>
  // Disk watch: everything Bragd keeps on disk, with its size, where it is and its
  // limit; the limits for all of it and for the free space; the limits crossed; pruning the
  // Library by the Sources' Strip rules, previewed first; and what Disk watch may do by itself
  // for each Source; and the Position store's Export and Import. Sizes are typed as shown (5 GB);
  // Rust reads them.
  import * as k from "./knowledge.js";
  import { t } from "./i18n.svelte.js";
  import JobLine from "./JobLine.svelte";

  /** @type {any} */
  let r = $state(null);
  let total = $state("");
  let free = $state("");
  /** @type {Record<string, string>} */
  let items = $state({});
  /** @type {number | null} */
  let job = $state(null);
  let error = $state("");
  let message = $state("");

  async function load() {
    try {
      r = await k.diskReport();
      total = r.totalLimit;
      free = r.freeBelow;
      items = Object.fromEntries(r.items.map((/** @type {any} */ i) => [i.id, i.limit]));
      error = "";
    } catch (e) {
      error = k.why(e);
    }
  }
  $effect(() => {
    load();
  });

  async function save() {
    error = message = "";
    try {
      await k.diskLimitsSet(total, free, items);
      message = t("disk.saved");
      await load();
    } catch (e) {
      error = k.why(e);
    }
  }

  async function prune(/** @type {boolean} */ dryRun) {
    error = "";
    try {
      job = await k.libraryPrune(dryRun);
    } catch (e) {
      error = k.why(e);
    }
  }

  let stored = $state("");
  async function positions(/** @type {() => Promise<string|null>} */ run) {
    error = stored = "";
    try {
      stored = (await run()) ?? "";
    } catch (e) {
      error = k.why(e);
    }
  }

  async function auto(/** @type {string} */ id, /** @type {any} */ options, /** @type {string} */ what, /** @type {boolean} */ on) {
    try {
      await k.sourceSet(id, { ...options, auto: { ...options.auto, [what]: on } });
      await load();
    } catch (e) {
      error = k.why(e);
    }
  }
</script>

<section class="disk">
  <h2>{t("disk.title")}</h2>
  <p class="hint">{t("disk.intro")}</p>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if r}
    {#each r.notices as n}<p class="notice" role="alert">{n}</p>{/each}
    <div class="scroll">
      <table>
        <thead>
          <tr><th>{t("disk.item")}</th><th>{t("disk.size")}</th><th>{t("disk.where")}</th><th>{t("disk.limit")}</th></tr>
        </thead>
        <tbody>
          {#each r.items as i (i.id)}
            <tr class:over={i.over}>
              <td>{i.label}{#if i.external} <span class="tag" title={t("disk.external_hint")}>{t("disk.external")}</span>{/if}</td>
              <td class="num">{i.size}</td>
              <td class="path" title={i.path}>{i.path}</td>
              <td><input type="text" bind:value={items[i.id]} placeholder={t("disk.no_limit")} size="8" disabled={i.external} aria-label={t("disk.limit_of", { item: i.label })} /></td>
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr>
            <th>{t("disk.total")}</th>
            <td class="num">{r.total}</td>
            <td></td>
            <td><input type="text" bind:value={total} placeholder={t("disk.no_limit")} size="8" aria-label={t("disk.total_limit")} /></td>
          </tr>
        </tfoot>
      </table>
    </div>
    <div class="row">
      <label>{t("disk.free_below")} <input type="text" bind:value={free} placeholder={t("disk.no_limit")} size="8" /></label>
      <span class="hint">{r.free.map((/** @type {[string, string]} */ [p, b]) => t("disk.free", { path: p, free: b })).join(" · ")}</span>
    </div>
    <div class="row">
      <button class="primary" onclick={save}>{t("disk.save")}</button>
      <span class="hint">{t("disk.size_hint")}</span>
    </div>
    {#if message}<p role="status">{message}</p>{/if}

    <h3>{t("disk.prune_title")}</h3>
    <p class="hint">{t("disk.prune_hint")}</p>
    <div class="row">
      <button onclick={() => prune(true)}>{t("strip.preview")}</button>
      <button onclick={() => prune(false)}>{t("disk.prune")}</button>
    </div>
    {#if job !== null}<JobLine id={job} ondone={load} />{/if}

    <h3>{t("positions.title")}</h3>
    <p class="hint">{t("positions.hint")}</p>
    <div class="row">
      <button onclick={() => positions(k.positionsExport)}>{t("positions.export")}</button>
      <button onclick={() => positions(k.positionsImport)}>{t("positions.import")}</button>
    </div>
    {#if stored}<p role="status">{stored}</p>{/if}

    <h3>{t("disk.auto_title")}</h3>
    <p class="hint">{t("disk.auto_hint")}</p>
    {#each r.sources as [id, name, o] (id)}
      <div class="row">
        <strong class="src">{name}</strong>
        {#each ["strip", "delete_oldest", "pause"] as what}
          <label class="check"><input type="checkbox" checked={o.auto[what]} onchange={(e) => auto(id, o, what, e.currentTarget.checked)} /> {t(`disk.auto.${what}`)}</label>
        {/each}
      </div>
    {/each}
  {:else if !error}
    <p class="hint">{t("helper.connecting")}</p>
  {/if}
</section>

<style>
  .disk {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-width: 56rem;
  }
  h2 {
    font-size: 1.15rem;
    margin: 0;
  }
  h3 {
    font-size: 1rem;
    margin: 0.6rem 0 0;
  }
  .scroll {
    overflow-x: auto;
  }
  table {
    border-collapse: collapse;
    width: 100%;
  }
  th,
  td {
    text-align: start;
    padding: 0.25rem 0.5rem;
    border-bottom: 1px solid var(--border);
  }
  .num {
    text-align: end;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .path {
    max-width: 18rem;
    overflow-wrap: anywhere;
    color: var(--dim);
    font-size: 0.85em;
  }
  tr.over td {
    color: var(--red);
  }
  .tag {
    font-size: 0.75em;
    padding: 0 0.3rem;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    color: var(--dim);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem 0.8rem;
    align-items: center;
  }
  label {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }
  .src {
    min-width: 8rem;
  }
  .hint {
    color: var(--dim);
    font-size: 0.85em;
    margin: 0;
  }
  .notice {
    margin: 0;
    padding: 0.4rem 0.6rem;
    border-inline-start: 3px solid var(--red);
    background: var(--alt);
  }
  .error {
    color: var(--red);
  }
  input[type="text"] {
    width: 6.5rem;
  }
  @media (max-width: 760px) {
    th:nth-child(3),
    td:nth-child(3) {
      display: none;
    }
  }
</style>
