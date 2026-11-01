<script>
  // Clubs: the Club files known here, one open with its members, ratings and audit log, and
  // players found in the rating lists. bragd-clubs decides and checks every change; this page
  // shows it and passes the choices back.
  import * as api from "./api.js";
  import { t, num, i18n } from "./i18n.svelte.js";
  import Tournaments from "./Tournaments.svelte";
  import Leagues from "./Leagues.svelte";

  /** @type {{ snap: any, go: (page: any) => void }} */
  let { snap, go } = $props();
  /** @type {any} */
  let page = $state(null);
  /** @type {any} */
  let club = $state(null);
  let error = $state("");
  /** The director or arbiter the user acts as. */
  let by = $state(/** @type {number|null} */ (null));

  let fresh = $state({ name: "", director: "", system: "glicko2" });
  const empty = () => ({ name: "", role: "member", fide_id: "", national_id: "", federation: "", lichess: "" });
  /** The member form: a new member (id null) or one being edited. */
  let form = $state(/** @type {any} */ (null));
  let start = $state(/** @type {number|null} */ (null));
  let query = $state("");
  /** @type {any[]|null} */
  let found = $state(null);
  /** The rating being set by hand. */
  let rate = $state(/** @type {any} */ (null));
  let system = $state({ value: "glicko2", why: "" });

  const chess = $derived(snap.families.find((/** @type {any} */ f) => f.id === "chess" && f.shown));
  const officials = $derived(club ? club.members.filter((/** @type {any} */ m) => m.role !== "member") : []);
  /** The role of whoever the user acts as: directors change the Club, arbiters run Tournaments. */
  const role = $derived(officials.find((/** @type {any} */ m) => m.id === by)?.role ?? null);
  const director = $derived(role === "director" && !club?.held_by);

  function show(/** @type {any} */ v) {
    club = v;
    if (v && !v.members.some((/** @type {any} */ m) => m.id === by && m.role !== "member")) {
      const ds = v.members.filter((/** @type {any} */ m) => m.role !== "member");
      by = ds.length === 1 ? ds[0].id : null;
    }
    if (v) system.value = v.system;
  }

  const load = () =>
    api.clubsPage().then(
      (p) => {
        page = p;
        show(p.open);
      },
      (e) => (error = String(e)),
    );
  load();

  async function run(/** @type {() => Promise<any>} */ f) {
    try {
      const v = await f();
      error = "";
      return v;
    } catch (e) {
      error = String(e);
    }
  }
  const change = (/** @type {() => Promise<any>} */ f) => run(f).then((v) => v && show(v));

  const when = (/** @type {number} */ at) => new Date(at * 1000).toLocaleString(i18n.lang, { dateStyle: "medium", timeStyle: "short" });
  const rating = (/** @type {any} */ m) =>
    club.system === "glicko2" ? t("clubs.rating_rd", { rating: num(Math.round(m.rating.rating)), rd: num(Math.round(m.rating.deviation)) }) : num(Math.round(m.rating.rating));

  async function create(/** @type {SubmitEvent} */ e) {
    e.preventDefault();
    const v = await run(() => api.clubCreate(fresh.name, fresh.system, fresh.director));
    if (v) {
      fresh = { name: "", director: "", system: "glicko2" };
      await load();
    }
  }
  async function open(/** @type {() => Promise<any>} */ f) {
    const v = await run(f);
    if (v) await load();
  }
  async function close() {
    await api.clubClose();
    club = null;
    form = rate = null;
    await load();
  }

  function edit(/** @type {any} */ m) {
    form = m ? { id: m.id, name: m.name, role: m.role, fide_id: m.fide_id, national_id: m.national_id, federation: m.federation, lichess: m.lichess } : { id: null, ...empty() };
    start = null;
    found = null;
    query = "";
  }
  async function saveMember(/** @type {SubmitEvent} */ e) {
    e.preventDefault();
    const { id, ...details } = form;
    const v = await run(() => (id === null ? api.clubAdd(by, details, start) : api.clubUpdate(by, id, details)));
    if (v) {
      show(v);
      form = null;
    }
  }
  async function search(/** @type {SubmitEvent} */ e) {
    e.preventDefault();
    found = (await run(() => api.ratingSearch(query))) ?? [];
  }
  /** A player found in a rating list fills the form. */
  function use(/** @type {any} */ p) {
    const fide = p.list.startsWith("FIDE");
    form = {
      ...form,
      name: p.name,
      fide_id: fide ? p.id : p.fide_id || form.fide_id,
      national_id: fide ? form.national_id : p.id,
      federation: fide ? form.federation : p.federation,
    };
    start = p.rating ?? null;
    found = null;
  }
  async function saveRating(/** @type {SubmitEvent} */ e) {
    e.preventDefault();
    const v = await run(() => api.clubRating(by, rate.id, Number(rate.value), rate.why));
    if (v) {
      show(v);
      rate = null;
    }
  }
  function takeOver() {
    if (confirm(t("clubs.take_over_confirm", { user: club.held_by.user, host: club.held_by.host }))) change(() => api.clubTakeOver(by));
  }
</script>

<section class="clubs">
  <h1>{t("sidebar.clubs")}</h1>
  <p>{t("clubs.intro")}</p>

  {#if club}
    <div class="panel head">
      <div>
        <h2>{club.name}</h2>
        <p class="dim">{club.path}</p>
      </div>
      <button onclick={close}>{t("clubs.close")}</button>
    </div>

    {#if club.held_by}
      <div class="panel warn" role="status">
        <p>{t("clubs.held_by", { user: club.held_by.user, host: club.held_by.host, since: when(club.held_by.since) })}</p>
        <button disabled={role !== "director"} onclick={takeOver}>{t("clubs.take_over")}</button>
      </div>
    {/if}

    <div class="panel row">
      <label for="club-by">{t("clubs.acting_as")}</label>
      <select id="club-by" bind:value={by}>
        <option value={null}>{t("clubs.acting_none")}</option>
        {#each officials as d (d.id)}<option value={d.id}>{d.name} ({t(`clubs.role.${d.role}`)})</option>{/each}
      </select>
      <span class="dim">{t("clubs.acting_hint")}</span>
    </div>

    <div class="panel">
      <h3>{t("clubs.members")}</h3>
      <div class="scroll">
        <table>
          <thead>
            <tr>
              <th>{t("clubs.name")}</th>
              <th>{t("clubs.role")}</th>
              <th>{t("clubs.fide_id")}</th>
              <th>{t("clubs.national_id")}</th>
              <th>{t("clubs.lichess")}</th>
              <th>{t("clubs.rating")}</th>
              <th><span class="sr">{t("clubs.actions")}</span></th>
            </tr>
          </thead>
          <tbody>
            {#each club.members as m (m.id)}
              <tr>
                <td>{m.name}</td>
                <td>{t(`clubs.role.${m.role}`)}</td>
                <td>{m.fide_id}</td>
                <td>{m.national_id}{m.federation ? ` (${m.federation})` : ""}</td>
                <td>{m.lichess}</td>
                <td class="num">{rating(m)}</td>
                <td class="acts">
                  <button disabled={!director} onclick={() => edit(m)}>{t("clubs.edit")}</button>
                  <button disabled={!director} onclick={() => (rate = { id: m.id, name: m.name, value: Math.round(m.rating.rating), why: "" })}>{t("clubs.set_rating")}</button>
                  <button disabled={!director} onclick={() => confirm(t("clubs.remove_confirm", { name: m.name })) && change(() => api.clubRemove(by, m.id))}>{t("clubs.remove")}</button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      <button class="primary" disabled={!director} onclick={() => edit(null)}>{t("clubs.add")}</button>
    </div>

    {#if rate}
      <form class="panel" onsubmit={saveRating}>
        <h3>{t("clubs.set_rating_of", { name: rate.name })}</h3>
        <label>{t("clubs.new_rating")} <input type="number" min="0" max="4000" required bind:value={rate.value} /></label>
        <label>{t("clubs.why")} <input type="text" required bind:value={rate.why} placeholder={t("clubs.why_hint")} /></label>
        <div class="row">
          <button class="primary">{t("clubs.save")}</button>
          <button type="button" onclick={() => (rate = null)}>{t("clubs.cancel")}</button>
        </div>
      </form>
    {/if}

    {#if form}
      <form class="panel" onsubmit={saveMember}>
        <h3>{form.id === null ? t("clubs.add") : t("clubs.edit_of", { name: form.name })}</h3>
        {#if form.id === null}
          <div class="find">
            <label for="club-find">{t("clubs.find")}</label>
            <div class="row">
              <input id="club-find" type="search" bind:value={query} placeholder={t("clubs.find_hint")} onkeydown={(e) => e.key === "Enter" && search(/** @type {any} */ (e))} />
              <button type="button" onclick={(e) => search(/** @type {any} */ (e))}>{t("clubs.search")}</button>
            </div>
            {#if !page?.lists.some((/** @type {any} */ l) => l.players)}
              <p class="dim">
                {t("clubs.no_lists")}
                {#if chess}<button type="button" class="link" onclick={() => go({ kind: "item", family: "chess", item: "databases" })}>{t("clubs.to_sources")}</button>{/if}
              </p>
            {/if}
            {#if found}
              {#if found.length === 0}<p class="dim">{t("clubs.none_found")}</p>{/if}
              <ul class="found">
                {#each found as p (p.list + p.id)}
                  <li>
                    <button type="button" class="link" onclick={() => use(p)}>{p.name}</button>
                    <span class="dim">{p.list} {p.id}{p.federation ? ` · ${p.federation}` : ""}{p.title ? ` · ${p.title}` : ""}{p.rating ? ` · ${num(p.rating)}` : ""}{p.born ? ` · ${p.born}` : ""}</span>
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
        {/if}
        <div class="grid">
          <label>{t("clubs.name")} <input type="text" required bind:value={form.name} /></label>
          <label
            >{t("clubs.role")}
            <select bind:value={form.role}>
              {#each ["director", "arbiter", "member"] as r}<option value={r}>{t(`clubs.role.${r}`)}</option>{/each}
            </select>
          </label>
          <label>{t("clubs.fide_id")} <input type="text" inputmode="numeric" bind:value={form.fide_id} /></label>
          <label>{t("clubs.national_id")} <input type="text" bind:value={form.national_id} /></label>
          <label>{t("clubs.federation")} <input type="text" maxlength="3" bind:value={form.federation} placeholder="DEN" /></label>
          <label>{t("clubs.lichess")} <input type="text" bind:value={form.lichess} /></label>
        </div>
        {#if form.id === null && start !== null}<p class="dim">{t("clubs.starts_at", { rating: num(start) })}</p>{/if}
        <div class="row">
          <button class="primary">{t("clubs.save")}</button>
          <button type="button" onclick={() => (form = null)}>{t("clubs.cancel")}</button>
        </div>
      </form>
    {/if}

    <Tournaments {club} {by} {role} onclub={show} />
    <Leagues {club} {by} {role} onclub={show} />

    <form class="panel row" onsubmit={(e) => (e.preventDefault(), change(() => api.clubSystem(by, system.value, system.why)))}>
      <label for="club-system">{t("clubs.system")}</label>
      <select id="club-system" bind:value={system.value} disabled={!director}>
        <option value="glicko2">{t("clubs.system.glicko2")}</option>
        <option value="elo">{t("clubs.system.elo")}</option>
      </select>
      {#if system.value !== club.system}
        <input type="text" required bind:value={system.why} placeholder={t("clubs.why_hint")} aria-label={t("clubs.why")} />
        <button class="primary">{t("clubs.save")}</button>
      {/if}
    </form>

    <div class="panel">
      <h3>{t("clubs.log")}</h3>
      {#if club.log.length === 0}
        <p class="dim">{t("clubs.log_empty")}</p>
      {:else}
        <div class="scroll">
          <table>
            <thead>
              <tr><th>{t("clubs.when")}</th><th>{t("clubs.who")}</th><th>{t("clubs.role.member")}</th><th>{t("clubs.change")}</th><th>{t("clubs.why")}</th></tr>
            </thead>
            <tbody>
              {#each club.log as e, i (i)}
                <tr>
                  <td>{when(e.at)}</td>
                  <td>{e.who}</td>
                  <td>{e.member}</td>
                  <td>{t(`clubs.log.${e.what}`)}: {e.old} → {e.new}</td>
                  <td>{e.why}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  {:else if page}
    <div class="panel">
      <h2>{t("clubs.yours")}</h2>
      {#if page.clubs.length === 0}<p class="dim">{t("clubs.none_yet")}</p>{/if}
      <ul class="known">
        {#each page.clubs as c (c.path)}
          <li>
            <button class="link" disabled={!c.there} onclick={() => open(() => api.clubOpen(c.path))}>{c.name}</button>
            <span class="dim">{c.there ? c.path : t("clubs.gone", { path: c.path })}</span>
            <button onclick={() => run(() => api.clubForget(c.path)).then((p) => p && (page = p))}>{t("clubs.forget")}</button>
          </li>
        {/each}
      </ul>
      <button onclick={() => open(api.clubPick)}>{t("clubs.open_file")}</button>
    </div>

    <form class="panel" onsubmit={create}>
      <h2>{t("clubs.new")}</h2>
      <div class="grid">
        <label>{t("clubs.club_name")} <input type="text" required bind:value={fresh.name} /></label>
        <label>{t("clubs.your_name")} <input type="text" required bind:value={fresh.director} /></label>
        <label
          >{t("clubs.system")}
          <select bind:value={fresh.system}>
            <option value="glicko2">{t("clubs.system.glicko2")}</option>
            <option value="elo">{t("clubs.system.elo")}</option>
          </select>
        </label>
      </div>
      <p class="dim">{t("clubs.new_hint")}</p>
      <button class="primary">{t("clubs.create")}</button>
    </form>

    {#if chess}
      <div class="panel">
        <h2>{t("clubs.lists")}</h2>
        <ul class="known">
          {#each page.lists as l (l.id)}
            <li>{l.name}: <span class="dim">{l.players ? t("clubs.list_have", { n: num(l.players), when: when(l.at) }) : t("clubs.list_none")}</span></li>
          {/each}
        </ul>
        <button class="link" onclick={() => go({ kind: "item", family: "chess", item: "databases" })}>{t("clubs.to_sources")}</button>
      </div>
    {/if}
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</section>

<style>
  .clubs {
    max-width: 60rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  h2,
  h3 {
    font-size: 1.05rem;
    margin: 0 0 0.4rem;
  }
  p {
    margin: 0.3rem 0;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: start;
    gap: 0.6rem;
  }
  .row {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
    align-items: center;
  }
  .row input[type="text"],
  .row input[type="search"] {
    flex: 1;
    min-width: 10rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(13rem, 1fr));
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .row label {
    display: inline;
  }
  form.panel > label {
    margin-bottom: 0.4rem;
  }
  .scroll {
    overflow-x: auto;
    margin-bottom: 0.5rem;
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
    vertical-align: top;
  }
  .num {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .acts {
    white-space: nowrap;
  }
  .known,
  .found {
    list-style: none;
    padding: 0;
    margin: 0.3rem 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .known li,
  .found li {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: baseline;
  }
  .find {
    margin-bottom: 0.6rem;
  }
  .dim {
    color: var(--dim);
    font-size: 0.9em;
    overflow-wrap: anywhere;
  }
  .warn {
    border-color: var(--red);
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
  .error {
    color: var(--red);
  }
</style>
