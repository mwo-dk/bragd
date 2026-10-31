<script>
  // The Engines item of a Family: each engine Bragd knows or was given, where it stands, the
  // engine each Variant uses, and an engine's options. An image that needs an evaluation network
  // Bragd does not ship (Pikafish's) shows its licence first; agreeing downloads it. Rust finds,
  // checks, downloads and saves all of it.
  import * as api from "./api.js";
  import { t, i18n } from "./i18n.svelte.js";
  import Picker from "./Picker.svelte";
  import { guide } from "./guide.svelte.js";

  /** @type {{ family: any, go: (page: any) => void }} */
  let { family, go } = $props();
  /** @type {any} */
  let page = $state(null);
  let error = $state("");
  let path = $state("");
  let busy = $state(false);
  /** The engine whose options are open, and for which Variant. @type {{key: string, info: any} | null} */
  let opts = $state(null);
  /** The engine whose network's licence is shown. @type {any} */
  let licence = $state(null);

  const fail = (/** @type {any} */ e) => (error = String(e));
  const load = () => api.enginesPage(family.id).then((p) => ((page = p), (error = ""))).catch(fail);
  load();
  // A pull or a network download under way shows its progress; a waiting pull, when it tries again.
  $effect(() => {
    if (!page?.engines.some((/** @type {any} */ e) => e.pulling || e.waiting || (e.network?.job && !e.network.job.finished))) return;
    const id = setInterval(load, 1000);
    return () => clearInterval(id);
  });

  const stateText = (/** @type {any} */ s) =>
    ({
      installed: t("engines.state.installed", { path: s.path }),
      pulled: t("engines.state.pulled"),
      built: t("engines.state.built"),
      pull: t("engines.state.get", { size: s.size }),
      noImage: t("engines.state.no_image"),
      noRuntime: t("engines.state.no_runtime"),
      containersOff: t("engines.state.containers_off"),
      gone: t("engines.state.gone", { path: s.path }),
    })[/** @type {string} */ (s.kind)];
  const ready = (/** @type {any} */ e) => ["installed", "pulled", "built"].includes(e.state.kind);

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

  /** Settings → Engines, where images and the runtime are chosen. */
  const engineSettings = () => ((guide.area = "engines"), go({ kind: "settings" }));

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
                <Picker
                  value={v.chosen}
                  choices={[
                    { value: "", label: t("engines.automatic", { name: page.engines.find((/** @type {any} */ e) => e.key === v.current)?.name ?? "—" }) },
                    ...v.engines.map((/** @type {string} */ k) => ({ value: k, label: page.engines.find((/** @type {any} */ e) => e.key === k)?.name ?? k })),
                  ]}
                  label={v.name}
                  onchange={(k) => act(api.engineDefault(v.key, k))}
                />
              {:else if v.waiting}
                <span class="dim">{t("engines.none_ready")}</span>
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
          {#if e.pulling}
            <progress aria-label={t("engines.pulling", { name: e.name })}></progress>
            <p class="dim" role="status">{t("engines.pulling", { name: e.name })} {e.pulling}</p>
          {:else if e.waiting}
            <p role="status">{t("engines.pull.waiting", { name: e.name, why: e.waiting.why, at: new Date(e.waiting.at * 1000).toLocaleTimeString(i18n.lang, { hour: "2-digit", minute: "2-digit" }) })}</p>
            {#if e.waiting.detail}<details class="dim"><summary>{t("engines.pull.details")}</summary><pre>{e.waiting.detail}</pre></details>{/if}
          {/if}
          {#if e.failed}<p class="error" role="alert">{e.failed}</p>{/if}
          {#if e.network}
            {@const job = e.network.job}
            <p class="dim">
              {#if e.network.present}{t("engines.network.present")}
              {:else if job && !job.finished}{t("engines.network.downloading", { done: Math.round((100 * job.done) / Math.max(job.total, 1)) + "%" })}
              {:else}{t("engines.network.absent", { size: e.network.size })}{/if}
            </p>
            {#if job?.error}<p class="error">{t("engines.network.failed", { why: job.error })}</p>{/if}
          {/if}
          <p class="dim">{t("engines.plays", { variants: e.variants.join(", ") })}</p>
          <div class="actions">
            {#if e.state.kind === "pull"}
              <button class="primary" disabled={busy || !!e.pulling} onclick={() => act(api.enginePull(e.key))}>{e.waiting ? t("engines.pull.retry") : t("engines.get", { size: e.state.size })}</button>
            {/if}
            {#if e.state.kind === "containersOff" || e.state.kind === "noRuntime"}
              <button onclick={engineSettings}>{t("engines.open_settings")}</button>
            {/if}
            {#if !ready(e) && e.site}
              <button onclick={() => api.openUrl(e.site)} title={e.site}>{t("engines.open_site")}</button>
            {/if}
            {#if e.network && !e.network.present}
              <button disabled={busy || (e.network.job && !e.network.job.finished)} onclick={() => (licence = e)}>{t("engines.network.read")}</button>
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

    {#if licence}
      <div class="veil" role="presentation" onclick={(ev) => ev.target === ev.currentTarget && (licence = null)} onkeydown={(ev) => ev.key === "Escape" && (licence = null)}>
        <div class="panel consent" role="dialog" aria-modal="true" aria-labelledby="consent-title">
          <h2 id="consent-title">{t("engines.network.title", { name: licence.name })}</h2>
          <p>{t("engines.network.intro", { name: licence.name, size: licence.network.size })}</p>
          <pre class="licence">{licence.network.licence}</pre>
          <p class="dim">{t("engines.network.from", { url: licence.network.from })}</p>
          <div class="actions">
            <button class="primary" onclick={() => (act(api.engineNetworkDownload(licence.key)), (licence = null))}>{t("engines.network.agree", { size: licence.network.size })}</button>
            <button onclick={() => (licence = null)}>{t("common.cancel")}</button>
          </div>
        </div>
      </div>
    {/if}

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
                <Picker id="eo-{o.name}" value={o.value ?? o.default} choices={o.vars.map((/** @type {string} */ v) => ({ value: v, label: v }))} label={o.name} onchange={(v) => setOption(o.name, v)} />
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
      <button type="button" disabled={busy} onclick={() => api.pickPath("engine").then((p) => p && (path = p), fail)}>{t("files.choose")}</button>
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
  td :global(.picker) {
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
  .veil {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
    background: rgb(0 0 0 / 0.35);
  }
  .consent {
    max-width: 40rem;
    max-height: 100%;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .licence {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font-size: 0.85em;
    background: var(--alt);
    padding: 0.6rem;
    margin: 0;
    max-height: 18rem;
    overflow: auto;
  }
</style>
