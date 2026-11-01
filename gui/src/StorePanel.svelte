<script>
  // The Position store's housekeeping in Disk watch, below its Export and Import: what each
  // Engine and Edition takes, and one action each to
  // prune the Editions older than the newest and the Engines removed from this machine.
  import * as k from "./knowledge.js";
  import { t, num } from "./i18n.svelte.js";

  /** @type {any} */
  let s = $state(null);
  let error = $state("");
  let message = $state("");
  let busy = $state(false);

  async function load() {
    try {
      s = await k.storeReport();
    } catch (e) {
      error = k.why(e);
    }
  }
  $effect(() => {
    load();
  });

  async function prune(/** @type {string} */ what) {
    error = message = "";
    busy = true;
    try {
      message = await k.storePrune(what);
      await load();
    } catch (e) {
      error = k.why(e);
    } finally {
      busy = false;
    }
  }
</script>

<p class="hint">{t("store.hint")}</p>
{#if error}<p class="error" role="alert">{error}</p>{/if}
{#if s}
  {#if !s.editions.length}
    <p class="hint">{t("store.empty")}</p>
  {:else}
    <div class="scroll">
      <table>
        <thead>
          <tr><th>{t("store.engine")}</th><th>{t("store.edition")}</th><th>{t("store.positions")}</th><th>{t("disk.size")}</th></tr>
        </thead>
        <tbody>
          {#each s.editions as e}
            <tr>
              <td>{e.engine}{#if e.removed} <span class="tag">{t("store.removed")}</span>{/if}</td>
              <td>{e.version}{#if e.older} <span class="tag">{t("store.older")}</span>{/if}</td>
              <td class="num">{num(e.positions)}</td>
              <td class="num">{e.size}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <div class="row">
      <button onclick={() => prune("older")} disabled={busy || !s.editions.some((/** @type {any} */ e) => e.older)}>{t("store.prune_older", { positions: s.older })}</button>
      <button onclick={() => prune("removed")} disabled={busy || !s.editions.some((/** @type {any} */ e) => e.removed)}>{t("store.prune_removed", { positions: s.removed })}</button>
    </div>
  {/if}
  {#if message}<p role="status">{message}</p>{/if}
{/if}

<style>
  .hint {
    color: var(--dim);
    font-size: 0.9em;
    margin: 0;
  }
  .scroll {
    overflow-x: auto;
  }
  table {
    border-collapse: collapse;
  }
  th,
  td {
    text-align: start;
    padding: 0.2rem 0.6rem 0.2rem 0;
  }
  .num {
    text-align: end;
    white-space: nowrap;
  }
  .tag {
    font-size: 0.8em;
    padding: 0 0.3rem;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    color: var(--dim);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
  }
  .error {
    color: var(--red);
  }
</style>
