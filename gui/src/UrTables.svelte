<script>
  // Ur's solved tables, one a rule set of each Ur Variant that is on: their size, the memory
  // making one takes, and Make or Delete. The Helper makes them on this machine; Rust says how
  // far it has come.
  import * as api from "./api.js";
  import { jobCancel } from "./knowledge.js";
  import { t } from "./i18n.svelte.js";

  /** @type {any[]} */
  let tables = $state([]);
  let error = $state("");
  const load = () => api.urTables().then((v) => ((tables = v), (error = "")), (e) => (error = String(e)));
  load();
  // A table being made shows how far it has come.
  $effect(() => {
    if (!tables.some((x) => x.job !== null)) return;
    const id = setInterval(load, 2000);
    return () => clearInterval(id);
  });
  const act = (/** @type {Promise<any>} */ p) => p.then(load, (e) => (error = String(e)));
</script>

<h2>{t("ur.tables")}</h2>
<p class="dim">{t("ur.tables_intro")}</p>
<table>
  <tbody>
    {#each tables as x (x.variant)}
      <tr>
        <th scope="row">{x.name}</th>
        <td>
          {#if x.ready}{t("ur.table_ready", { size: x.size })}
          {:else if x.job !== null}
            <progress value={x.progress ?? 0} aria-label={t("ur.table_making", { name: x.name })}></progress>
            <span role="status">{t("ur.table_making", { name: x.name })} {Math.round((x.progress ?? 0) * 100)}%</span>
          {:else}{t("ur.table_absent", { size: x.size, memory: x.memory })}{/if}
          {#if x.error}<p class="error">{x.error}</p>{/if}
        </td>
        <td class="actions">
          {#if x.ready}
            <button onclick={() => act(api.urTableDelete(x.variant))}>{t("ur.table_delete")}</button>
          {:else if x.job !== null}
            <button onclick={() => act(jobCancel(x.job))}>{t("common.cancel")}</button>
          {:else}
            <button class="primary" onclick={() => act(api.urTableMake(x.variant))}>{t("ur.table_make")}</button>
          {/if}
        </td>
      </tr>
    {/each}
  </tbody>
</table>
{#if error}<p class="error" role="alert">{error}</p>{/if}

<style>
  h2 {
    font-size: 1.05rem;
    margin: 0.8rem 0 0.2rem;
  }
  table {
    border-collapse: collapse;
  }
  th,
  td {
    text-align: left;
    padding: 0.3rem 0.6rem 0.3rem 0;
    vertical-align: top;
  }
  .actions {
    white-space: nowrap;
  }
  progress {
    width: 8rem;
  }
</style>
