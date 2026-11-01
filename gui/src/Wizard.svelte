<script>
  // The install wizard and, after an upgrade, the tour cards, in one frame (bragd_core::wizard
  // and ::tour). Six steps, each skippable and naming its Settings area: choices in steps 1, 2
  // and 6 are saved at once, as in Settings; the Knowledge downloads start only with Done.
  // Closed in any way, it is not shown by itself again.
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";
  import Picker from "./Picker.svelte";
  import { guide } from "./guide.svelte.js";
  import FamilyToggles from "./FamilyToggles.svelte";

  /** @type {{ snap: any, save: (name: string, value: any) => Promise<any>, only: (family: string) => Promise<any>, go: (page: any) => void }} */
  let { snap, save, only, go } = $props();

  const tour = !guide.wizard;
  /** @type {any} */
  let data = $state(null);
  let step = $state(0);
  /** @type {string[]} */
  let chosen = $state([]);
  /** @type {any} */
  let info = $state(null);
  let busy = $state(false);
  // Step 2: the full list of switches instead of one button per Family.
  let several = $state(false);
  const shownIds = $derived(snap.families.filter((/** @type {any} */ f) => f.shown).map((/** @type {any} */ f) => f.id));
  // One click: that Family alone, then the next step (whose Engines and downloads follow it).
  const pickOnly = (/** @type {string} */ id) => only(id).then(() => open(true)).then(() => step++);

  // `fresh`: the Families changed, so the downloads ticked are those offered for them now; a
  // download of a Family turned off stays chosen otherwise, unseen.
  const open = (fresh = false) =>
    api.wizardOpen().then((d) => {
      if (!data || fresh) chosen = d.ticked;
      data = d;
    });
  if (!tour) open();
  $effect(() => {
    if (data) api.wizardChosen($state.snapshot(chosen)).then((c) => (info = c));
  });

  const count = $derived(tour ? guide.cards.length : (data?.steps.length ?? 6));
  const current = $derived(tour ? null : data?.steps[step]);
  const opt = (/** @type {string} */ name) => snap.settings.find((/** @type {any} */ o) => o.name === name);
  const last = $derived(step + 1 >= count);

  function close() {
    if (tour) api.tourSeen();
    else if (!busy) api.wizardSeen();
    guide.wizard = false;
    guide.cards = [];
  }
  async function done() {
    if (tour) return close();
    busy = true;
    const failed = await api.wizardDone($state.snapshot(chosen)).catch((e) => [String(e)]);
    guide.notices = [...guide.notices, ...failed.map((text, i) => ({ id: `local:wizard-${i}`, text, settings: "knowledge" }))];
    guide.wizard = false;
  }
  const next = () => (last ? done() : step++);
  function skip() {
    if (current?.id === "knowledge") chosen = [];
    if (last) close();
    else step++;
  }
  function settings(/** @type {string} */ area) {
    close();
    guide.area = area;
    go({ kind: "settings" });
  }
  const theme = (/** @type {string} */ name, /** @type {string} */ value) => save(name, value).then(() => open());
  const tick = (/** @type {string} */ id, /** @type {boolean} */ on) => (chosen = on ? [...chosen, id] : chosen.filter((c) => c !== id));

</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="backdrop">
  <div class="wizard panel" role="dialog" aria-modal="true" aria-labelledby="wizard-title">
    <header>
      <h2 id="wizard-title">
        {#if tour}{t("sidebar.whats_new")} · {t("whats_new.card", { n: step + 1, of: count })}{:else}{current?.title ?? ""}{/if}
      </h2>
      <button class="x" onclick={close} title={t("wizard.close")} aria-label={t("wizard.close")}>×</button>
    </header>

    <div class="body">
      {#if tour}
        {@const c = guide.cards[step]}
        <p class="version">{t("whats_new.version", { version: c.version })}</p>
        <h3>{c.title}</h3>
        <p class="lead">{c.text}</p>
      {:else if current?.id === "welcome"}
        <p class="lead">{t("wizard.welcome")}</p>
        <div class="welcome">
          <div class="col">
            <span>{t("wizard.languages")}</span>
            <Picker value={opt("language").value} choices={opt("language").choices} label={t("wizard.languages")} onchange={(v) => save("language", v).then(() => open())} />
          </div>
          <div class="col">
            <span>{t("setting.theme")}</span>
            <div class="swatches" role="radiogroup" aria-label={t("setting.theme")}>
              {#each data.themes as s (s.id)}
                <button class="sw" role="radio" aria-checked={opt("theme").value === s.id} class:on={opt("theme").value === s.id} style:background={s.bg} style:color={s.fg} onclick={() => theme("theme", s.id)}>
                  <b style:color={s.accent}>Aa</b><small>{s.label}</small>
                </button>
              {/each}
            </div>
            <Picker value={opt("theme").value} choices={opt("theme").choices} label={t("setting.theme")} onchange={(v) => theme("theme", v)} />
            <span>{t("setting.board_theme")}</span>
            <div class="swatches" role="radiogroup" aria-label={t("setting.board_theme")}>
              {#each data.boards as s (s.id)}
                <button class="sw board" role="radio" aria-checked={opt("board_theme").value === s.id} class:on={opt("board_theme").value === s.id} onclick={() => theme("board_theme", s.id)}>
                  <span class="mini" aria-hidden="true">
                    {#each [0, 1, 2, 3, 4, 5, 6, 7, 8] as i}<i style:background={i % 2 ? s.fg : s.bg}></i>{/each}
                  </span>
                  <small>{s.label}</small>
                </button>
              {/each}
            </div>
          </div>
          <div class="col preview" aria-label={t("wizard.preview")}>
            <span>{t("wizard.preview")}</span>
            <div class="sample">
              <b>Bragd</b>
              <span class="dim">{t("tour.families.title")}</span>
              <span class="board" aria-hidden="true">
                {#each Array(16) as _, i}<i class:dark={(i + Math.floor(i / 4)) % 2 === 1}></i>{/each}
              </span>
              <button class="primary" tabindex="-1">{t("wizard.next")}</button>
            </div>
          </div>
        </div>
      {:else if current?.id === "families"}
        <p class="lead">{t("wizard.families")}</p>
        {#if several}
          <p>{t("wizard.several_lead")}</p>
          <FamilyToggles {snap} save={(n, v) => save(n, v).then(() => open(true))} />
        {:else}
          <div class="only">
            {#each snap.families as f (f.id)}
              <button aria-pressed={shownIds.length === 1 && shownIds[0] === f.id} onclick={() => pickOnly(f.id)}>{t("wizard.only", { family: f.name })}</button>
            {/each}
            <button aria-pressed={!guide.first && shownIds.length > 1} onclick={() => (several = true)}>{t("wizard.several")}</button>
          </div>
        {/if}
      {:else if current?.id === "engines"}
        <p class="lead">{t("wizard.engines")}</p>
        {#each data.engines as f}
          <h3>{f.name}</h3>
          {#if !f.engines.length}<p class="dim">{t("wizard.bot")}</p>{/if}
          <ul>
            {#each f.engines as e}
              <li>
                <b>{e.name}</b>:
                {#if e.installed}<span class="ok">{t("wizard.engine_found", { path: e.installed })}</span>{:else}{t("wizard.engine_missing")}.{/if}
                <span class="dim">{e.image ? t("wizard.image_ready") : t("wizard.image_unpublished")}</span>
                {#if !e.installed && e.site}<button class="link" onclick={() => api.openUrl(e.site)}>{t("wizard.engine_get")}</button>{:else if !e.installed}<span class="dim">{t("engines.install_hint_package", { program: e.program })}</span>{/if}
              </li>
            {/each}
          </ul>
        {:else}
          <p>{t("wizard.engines_none")}</p>
        {/each}
      {:else if current?.id === "knowledge"}
        <p class="lead">{t("wizard.knowledge")}</p>
        <ul class="offers">
          {#each data.offers as o (o.id)}
            <li>
              <label>
                <input type="checkbox" checked={chosen.includes(o.id)} onchange={(e) => tick(o.id, e.currentTarget.checked)} />
                <b>{o.name}</b>
                {#if o.recommended}<span class="badge">{t("wizard.recommended")}</span>{/if}
                <span class="dim">{o.size}</span>
              </label>
              {#if o.note}<p class:warn={o.big} class="note">{o.note}</p>{/if}
            </li>
          {/each}
        </ul>
        {#if data.coming}<p class="dim">{data.coming}</p>{/if}
        {#if info}
          <p><b>{info.totals.text}</b></p>
          {#if info.totals.warning}<p class="warn" role="alert">{info.totals.warning}</p>{/if}
        {/if}
      {:else if current?.id === "online"}
        <p class="lead">{t("wizard.online")}</p>
        {#if data.servers.length}<p>{t("wizard.online_sites")}</p>{/if}
        <ul>
          {#each data.servers as url}<li><button class="link" onclick={() => api.openUrl(url)}>{url.replace(/^https:\/\/(www\.)?|\/$/g, "")}</button></li>{/each}
        </ul>
      {:else if current?.id === "helper"}
        <p class="lead">{t("wizard.helper")}</p>
        <label class="check"><input type="checkbox" checked={opt("start_with_session").value} onchange={(e) => save("start_with_session", e.currentTarget.checked)} /> {opt("start_with_session").label}</label>
        <p class="dim">{opt("start_with_session").hint}</p>
        {#if info}
          <p><b>{t("wizard.summary")}</b></p>
          <ul>{#each info.summary as line}<li>{line}</li>{/each}</ul>
          {#if info.totals.warning}<p class="warn" role="alert">{info.totals.warning}</p>{/if}
        {/if}
      {/if}
      {#if current?.later}
        <p><button class="link" onclick={() => settings(current.area)}>{current.later}</button></p>
      {/if}
    </div>

    <footer>
      <ol class="dots" aria-hidden="true">
        {#each Array(count) as _, i}<li class:on={i === step}></li>{/each}
      </ol>
      {#if step > 0}<button onclick={() => step--}>{t("wizard.back")}</button>{/if}
      {#if !tour}<button onclick={skip} disabled={busy}>{t("wizard.skip")}</button>{/if}
      <button class="primary" onclick={next} disabled={busy || (!tour && !data)}>
        {busy ? t("wizard.starting") : last ? t("wizard.done") : t("wizard.next")}
      </button>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.45);
  }
  .wizard {
    display: flex;
    flex-direction: column;
    width: min(46rem, calc(100vw - 1.2rem));
    max-height: calc(100vh - 1.2rem);
    padding: 0;
    box-shadow: var(--shadow);
    overflow: hidden;
  }
  @media (prefers-reduced-motion: no-preference) {
    .wizard {
      animation: rise 0.18s ease-out;
    }
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(0.6rem);
    }
  }
  header,
  footer {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 1rem;
  }
  header {
    border-block-end: 1px solid var(--border);
  }
  footer {
    border-block-start: 1px solid var(--border);
    flex-wrap: wrap;
  }
  h2 {
    flex: 1;
    margin: 0;
    font-size: 1.1rem;
  }
  h3 {
    margin: 0.4rem 0 0.2rem;
    font-size: 1rem;
  }
  .x {
    border: none;
    background: none;
    font-size: 1.3rem;
    padding: 0 0.4rem;
  }
  .body {
    flex: 1;
    overflow: auto;
    padding: 0.8rem 1.1rem;
  }
  .body p {
    margin: 0 0 0.6rem;
  }
  .lead {
    font-size: 1.05em;
  }
  .version {
    color: var(--dim);
  }
  .dim {
    color: var(--dim);
  }
  .ok {
    color: var(--green);
  }
  .warn {
    color: var(--red);
  }
  .note {
    font-size: 0.9em;
    margin: 0.1rem 0 0 1.6rem !important;
    color: var(--dim);
  }
  /* Red darkened towards the text colour: a warning stays readable on the panel. */
  .note.warn {
    color: color-mix(in srgb, var(--red) 65%, var(--fg));
  }
  .link {
    border: none;
    background: none;
    padding: 0;
    color: var(--accent);
    text-decoration: underline;
    text-align: start;
  }
  .only {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr));
    gap: 0.5rem;
    margin-block-end: 0.6rem;
  }
  .only button {
    padding: 0.6rem 0.8rem;
    text-align: start;
  }
  .only button[aria-pressed="true"] {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .welcome {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr));
    gap: 1rem;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .col > span {
    font-weight: 600;
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }
  .sw {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 5.6rem;
    padding: 0.3rem;
    border: 1px solid var(--border);
  }
  .sw small {
    font-size: 0.72em;
    text-align: center;
  }
  .sw.on {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .sw.board {
    background: var(--raised);
    color: var(--fg);
  }
  .mini {
    display: grid;
    grid-template-columns: repeat(3, 0.9rem);
  }
  .mini i,
  .board i {
    display: block;
    aspect-ratio: 1;
  }
  .sample {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    align-items: flex-start;
    padding: 0.7rem;
    background: var(--bg);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: var(--r);
  }
  .sample .board {
    display: grid;
    grid-template-columns: repeat(4, 1.6rem);
    border: 1px solid var(--border);
  }
  .sample .board i {
    background: var(--board-light);
  }
  .sample .board i.dark {
    background: var(--board-dark);
  }
  ul {
    margin: 0 0 0.6rem;
    padding-inline-start: 1.2rem;
  }
  li {
    margin-block-end: 0.3rem;
  }
  .offers {
    list-style: none;
    padding: 0;
  }
  .badge {
    font-size: 0.75em;
    padding: 0.05rem 0.45rem;
    border-radius: var(--r-pill);
    background: var(--accent);
    color: var(--accent-fg);
  }
  .check {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    font-weight: 600;
  }
  .dots {
    flex: 1;
    display: flex;
    gap: 0.35rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .dots li {
    width: 0.55rem;
    height: 0.55rem;
    margin: 0;
    border-radius: 50%;
    background: color-mix(in srgb, var(--fg) 25%, transparent);
  }
  .dots li.on {
    background: var(--accent);
  }
</style>
