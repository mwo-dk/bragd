<script>
  // The Engines item of a Family: each engine Bragd knows or was given, where it stands, the
  // engine each Variant uses, and an engine's options. Rust finds, checks and saves all of it.
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";

  /** @type {{ family: any }} */
  let { family } = $props();
  /** @type {any} */
  let page = $state(null);
  let error = $state("");
  let path = $state("");
  let busy = $state(false);
  /** The engine whose options are open, and for which Variant. @type {{key: string, info: any} | null} */
  let opts = $state(null);

  const fail = (/** @type {any} */ e) => (error = String(e));
  const load = () => api.enginesPage(family.id).then((p) => ((page = p), (error = ""))).catch(fail);
  load();
  // A pull under way shows its progress.
  $effect(() => {
    if (!page?.engines.some((/** @type {any} */ e) => e.pulling)) return;
    const id = setInterval(load, 1000);
    return () => clearInterval(id);
  });

  const stateText = (/** @type {any} */ s) =>
    ({
      installed: t("engines.state.installed", { path: s.path }),
      pulled: t("engines.state.pulled"),
      pull: t("engines.state.pull"),
      noImage: t("engines.state.no_image"),
      noRuntime: t("engines.state.no_runtime"),
      gone: t("engines.state.gone", { path: s.path }),
    })[/** @type {string} */ (s.kind)];
  const ready = (/** @type {any} */ e) => e.state.kind === "installed" || e.state.kind === "pulled";

  async function act(/** @type {Promise<any>} */ p) {
    busy = true;
    try {
      await p;
      await load();
    } catch (e) {
      fail(e);
    }
    busy = false;
  }

  async function showOptions(/** @type {any} */ e) {
    const variant = page.variants.find((/** @type {any} */ v) => v.engines.includes(e.key))?.key;
    if (!variant) return;
    busy = true;
    try {
      opts = { key: e.key, info: await api.engineOptions(e.key, variant) };
      error = "";
    } catch (err) {
      fail(err);
    }
    busy = false;
  }

  async function setOption(/** @type {string} */ name, /** @type {any} */ value) {
    if (!opts) return;
    try {
      opts.info = await api.engineSetOption(opts.key, name, value);
      error = "";
    } catch (e) {
      fail(e);
    }
  }
</script>

<section class="engines">
  <h1>{family.name}: {t("sidebar.engines")}</h1>
  {#if page}
    <h2>{t("engines.by_variant")}</h2>
    <table>
      <tbody>
        {#each page.variants as v (v.key)}
          <tr>
            <th scope="row">{v.name}</th>
            <td>
              {#if v.engines.length}
                <select value={v.chosen} onchange={(e) => act(api.engineDefault(v.key, e.currentTarget.value))} aria-label={v.name}>
                  <option value="">{t("engines.automatic", { name: page.engines.find((/** @type {any} */ e) => e.key === v.current)?.name ?? "—" })}</option>
                  {#each v.engines as k}
                    <option value={k}>{page.engines.find((/** @type {any} */ e) => e.key === k)?.name ?? k}</option>
                  {/each}
                </select>
              {:else}
                <span class="dim">{t("engines.none_for")}</span>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>

    <h2>{t("engines.known")}</h2>
    <div class="list">
      {#each page.engines as e (e.key)}
        <article class="panel engine" class:ready={ready(e)}>
          <header>
            <strong>{e.name}</strong>
            <span class="badge">{ready(e) ? t("engines.ready") : t("engines.not_ready")}</span>
          </header>
          <p class="state">{stateText(e.state)}</p>
          {#if e.hint}<p class="dim">{e.hint}</p>{/if}
          {#if e.pulling}<p class="dim">{e.pulling}</p>{/if}
          <p class="dim">{t("engines.plays", { variants: e.variants.join(", ") })}</p>
          <div class="actions">
            {#if e.state.kind === "pull"}
              <button class="primary" disabled={busy || !!e.pulling} onclick={() => act(api.enginePull(e.key))}>{t("engines.pull")}</button>
            {/if}
            {#if ready(e)}
              <button disabled={busy} onclick={() => showOptions(e)}>{t("engines.options")}</button>
            {/if}
            {#if e.added}
              <button disabled={busy} onclick={() => act(api.engineRemove(e.key))}>{t("engines.remove")}</button>
            {/if}
          </div>
        </article>
      {/each}
    </div>

    {#if opts}
      <section class="panel options">
        <h2>{t("engines.options_of", { name: opts.info.name })}</h2>
        {#each opts.info.options as o (o.name)}
          <div class="row">
            {#if o.kind === "check"}
              <label><input type="checkbox" checked={(o.value ?? o.default) === true || (o.value ?? o.default) === "true"} onchange={(e) => setOption(o.name, e.currentTarget.checked)} /> {o.name}</label>
            {:else}
              <label for="eo-{o.name}">{o.name}</label>
              {#if o.kind === "combo"}
                <select id="eo-{o.name}" value={o.value ?? o.default} onchange={(e) => setOption(o.name, e.currentTarget.value)}>
                  {#each o.vars as v}<option value={v}>{v}</option>{/each}
                </select>
              {:else if o.kind === "spin"}
                <input id="eo-{o.name}" type="number" min={o.min} max={o.max} value={o.value ?? o.default} onchange={(e) => setOption(o.name, e.currentTarget.value)} />
              {:else}
                <input id="eo-{o.name}" type="text" value={o.value ?? o.default} onchange={(e) => setOption(o.name, e.currentTarget.value)} />
              {/if}
            {/if}
            {#if o.value !== null && o.value !== undefined}
              <button class="small" onclick={() => setOption(o.name, null)} title={t("engines.default_value")}>↺</button>
            {/if}
          </div>
        {/each}
        {#each opts.info.ignored as [name, why]}
          <p class="error">{name}: {why}</p>
        {/each}
        <div class="actions">
          <button onclick={async () => opts && (opts.info = await api.engineResetOptions(opts.key))}>{t("engines.reset")}</button>
          <button onclick={() => (opts = null)}>{t("common.close")}</button>
        </div>
      </section>
    {/if}

    <h2>{t("engines.add")}</h2>
    <form class="add" onsubmit={(e) => (e.preventDefault(), act(api.engineAdd(family.id, path)).then(() => (path = "")))}>
      <input type="text" bind:value={path} placeholder={t("engines.add_path")} aria-label={t("engines.add_path")} />
      <button class="primary" disabled={busy || !path.trim()}>{t("engines.add_button")}</button>
    </form>
    <p class="dim">{t("engines.add_hint")}</p>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</section>

<style>
  .engines {
    max-width: 52rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  h2 {
    font-size: 1.05rem;
    margin: 0.8rem 0 0.2rem;
  }
  table {
    width: 100%;
    max-width: 36rem;
  }
  td select {
    width: 100%;
  }
  table th {
    text-align: start;
    font-weight: normal;
    padding-inline-end: 1rem;
  }
  .list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr));
    gap: 0.6rem;
  }
  .engine header {
    display: flex;
    justify-content: space-between;
    gap: 0.4rem;
    align-items: baseline;
  }
  .engine p {
    margin: 0.3rem 0;
    overflow-wrap: anywhere;
  }
  .badge {
    font-size: 0.8em;
    color: var(--dim);
  }
  .ready .badge {
    color: var(--accent, inherit);
    font-weight: 600;
  }
  .dim {
    color: var(--dim);
    font-size: 0.9em;
  }
  .actions,
  .add,
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
  }
  .row {
    margin: 0.25rem 0;
  }
  .row label {
    min-width: 10rem;
  }
  .add input {
    flex: 1;
    min-width: 12rem;
  }
  .error {
    color: var(--red);
  }
</style>
