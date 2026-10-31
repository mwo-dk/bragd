<script>
  // Chess's Tablebases item: the Syzygy folder, what it holds, each set's download (its size
  // asked for first, nothing fetched until you say so) and the Lichess fallback. Rust plans,
  // fetches and counts; this page shows it and passes the choices back.
  import * as api from "./api.js";
  import { t, num } from "./i18n.svelte.js";

  /** @type {{ snap: any, save: (name: string, value: any) => Promise<any> }} */
  let { snap, save } = $props();
  /** @type {any} */
  let page = $state(null);
  let error = $state("");
  let folder = $state("");

  const load = () =>
    api
      .tablebasesPage()
      .then((p) => {
        page = p;
        if (!folder) folder = p.custom ? p.folder : "";
      })
      .catch((e) => (error = String(e)));
  load();
  $effect(() => {
    if (!page?.job.set || page.job.finished) return;
    const id = setInterval(load, 1000);
    return () => clearInterval(id);
  });

  const gb = (/** @type {number} */ b) => t("tablebases.gb", { n: num(Math.round((b / 1e9) * 10) / 10) });
  const setName = (/** @type {string} */ s) => t(`tablebases.set.${s}`);

  async function run(/** @type {() => Promise<any>} */ f) {
    try {
      await f();
      error = "";
    } catch (e) {
      error = String(e);
    }
    await load();
  }
  const online = $derived(snap.settings.find((/** @type {any} */ o) => o.name === "tablebases_online")?.value ?? true);
</script>

<section class="tablebases">
  <h1>{t("sidebar.tablebases")}</h1>
  <p>{t("tablebases.intro")}</p>
  {#if page}
    <div class="panel">
      <label for="tb-folder">{t("tablebases.folder")}</label>
      <form class="row" onsubmit={(e) => (e.preventDefault(), run(() => save("tablebases_path", folder.trim())))}>
        <input id="tb-folder" type="text" bind:value={folder} placeholder={page.folder} />
        <button type="button" onclick={() => api.pickPath("tablebases").then((p) => p && (folder = p), (e) => (error = String(e)))}>{t("files.choose")}</button>
        <button>{t("tablebases.use_folder")}</button>
      </form>
      <p class="dim">{page.custom ? t("tablebases.folder_set") : t("tablebases.folder_default", { path: page.folder })}</p>
    </div>

    <div class="sets">
      {#each page.sets as s (s.set)}
        <article class="panel">
          <h2>{setName(s.set)}</h2>
          <p>{t(`tablebases.about.${s.set}`)}</p>
          <p class="dim">{t("tablebases.have", { n: num(s.have) })}</p>
          {#if s.download}
            {#if s.size !== null}
              <p>{t("tablebases.size", { size: gb(s.size) })}</p>
              <button class="primary" disabled={page.offline || (page.job.set && !page.job.finished)} onclick={() => run(() => api.tablebaseDownload(s.set))}>{t("tablebases.download")}</button>
            {:else}
              <button disabled={page.offline} onclick={() => run(() => api.tablebasePlan(s.set))}>{t("tablebases.plan")}</button>
            {/if}
          {:else}
            <p class="dim">{t("tablebases.folder_only")}</p>
          {/if}
        </article>
      {/each}
    </div>

    {#if page.job.set}
      <div class="panel job" role="status">
        <p>{setName(page.job.set)}: {gb(page.job.done)} / {gb(page.job.total)}</p>
        <progress max={page.job.total || 1} value={page.job.done}></progress>
        {#if page.job.error}<p class="error">{page.job.error}</p>{:else if page.job.finished}<p>{t("tablebases.done")}</p>{/if}
        {#if !page.job.finished}<button onclick={() => run(api.tablebaseStop)}>{t("tablebases.stop")}</button>{/if}
      </div>
    {/if}

    <div class="panel">
      <label><input type="checkbox" checked={online} onchange={(e) => run(() => save("tablebases_online", e.currentTarget.checked))} /> {t("tablebases.online")}</label>
      <p class="dim">{t("tablebases.online_hint")}</p>
      {#if page.offline}<p class="dim">{t("tablebases.offline")}</p>{/if}
    </div>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</section>

<style>
  .tablebases {
    max-width: 52rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .sets {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(14rem, 1fr));
    gap: 0.6rem;
  }
  h2 {
    font-size: 1.05rem;
    margin: 0 0 0.3rem;
  }
  p {
    margin: 0.3rem 0;
  }
  .row {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
  }
  .row input {
    flex: 1;
    min-width: 12rem;
  }
  .dim {
    color: var(--dim);
    font-size: 0.9em;
    overflow-wrap: anywhere;
  }
  progress {
    width: 100%;
  }
  .error {
    color: var(--red);
  }
</style>
