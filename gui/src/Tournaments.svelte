<script>
  // A Club's Tournaments: set one up, then on the night register and check in players, pair,
  // show the pairings on a second screen, enter and correct results, and publish. Rust decides
  // every pairing, warning, tiebreak and export (bragd_clubs::tournament); this page draws it.
  import * as api from "./api.js";
  import { t, num, i18n } from "./i18n.svelte.js";

  /** @type {{ club: any, by: number|null, role: string|null, onclub: (v: any) => void }} */
  let { club, by, role, onclub } = $props();

  /** The open Tournament's view. @type {any} */
  let tv = $state(null);
  let tab = $state("players");
  let error = $state("");
  let note = $state("");
  const today = () => new Date().toISOString().slice(0, 10);
  let fresh = $state({ name: "", system: "dutch", rounds: 7, date: today(), place: "" });
  /** The guest being added: typed in, or found in a rating list. */
  let guest = $state({ name: "", rating: "", fide_id: "", federation: "", title: "" });
  let member = $state(/** @type {number|null} */ (null));
  let here = $state(true);
  let query = $state("");
  /** @type {any[]|null} */
  let found = $state(null);
  /** The pairing being changed by hand: boards as [white, black], the bye, the reason. @type {any} */
  let edit = $state(null);
  /** @type {string[]|null} */
  let warnings = $state(null);
  /** The tiebreak list being edited, by code. @type {string[]|null} */
  let order = $state(null);
  let adding = $state("");

  const official = $derived(by !== null && (role === "director" || role === "arbiter") && !club.held_by);
  const director = $derived(by !== null && role === "director" && !club.held_by);
  const T = $derived(tv?.tournament);
  const last = $derived(T && T.played.length ? T.played.length - 1 : -1);
  const name = (/** @type {number} */ p) => T.players[p]?.name ?? "";
  const registered = $derived(new Set(T ? T.players.map((/** @type {any} */ p) => p.member).filter((/** @type {any} */ m) => m !== null) : []));
  const free = $derived(club.members.filter((/** @type {any} */ m) => !registered.has(m.id)));
  const systemKey = (/** @type {any} */ s) => (s.kind === "dutch" ? "tournaments.system.dutch" : s.double ? "tournaments.system.double_round_robin" : "tournaments.system.round_robin");
  const OUTCOMES = ["white", "draw", "black", "white_forfeit", "black_forfeit", "double_forfeit"];
  const STATUS = ["here", "away", "half_bye", "withdrawn"];
  const TABS = ["players", "round", "standings", "exports", "log"];

  async function run(/** @type {() => Promise<any>} */ f) {
    try {
      const v = await f();
      error = "";
      return v;
    } catch (e) {
      error = String(e);
    }
  }
  /** Runs a change that gives the Tournament's view back. */
  const change = (/** @type {() => Promise<any>} */ f) => run(f).then((v) => v && (tv = v));

  async function open(/** @type {number} */ id) {
    const v = await run(() => api.tournamentView(id));
    if (v) {
      tv = v;
      tab = v.tournament.played.length ? "round" : "players";
      edit = order = warnings = null;
    }
  }
  async function create(/** @type {SubmitEvent} */ e) {
    e.preventDefault();
    const system = fresh.system === "dutch" ? { kind: "dutch" } : { kind: "round_robin", double: fresh.system === "double" };
    const v = await run(() => api.tournamentNew(by, fresh.name, system, Number(fresh.rounds), fresh.date, fresh.place));
    if (v) {
      onclub(v);
      fresh = { name: "", system: "dutch", rounds: 7, date: today(), place: "" };
      if (v.tournaments.length) open(v.tournaments[0].id);
    }
  }
  async function importBundle() {
    const v = await run(() => api.tournamentImport(by));
    if (v) onclub(v);
  }
  async function remove() {
    if (!confirm(t("tournaments.remove_confirm", { name: T.name }))) return;
    const v = await run(() => api.tournamentRemove(by, tv.id));
    if (v) {
      tv = null;
      onclub(v);
    }
  }

  async function addMember() {
    const m = club.members.find((/** @type {any} */ x) => x.id === member);
    if (!m) return;
    const player = { name: m.name, rating: Math.round(m.rating.rating), fide_id: m.fide_id, national_id: m.national_id, federation: m.federation, member: m.id };
    await change(() => api.tournamentRegister(by, tv.id, player, here));
    member = null;
  }
  async function addGuest(/** @type {SubmitEvent} */ e) {
    e.preventDefault();
    const player = { name: guest.name, rating: Number(guest.rating) || 0, fide_id: guest.fide_id, federation: guest.federation, title: guest.title };
    const v = await change(() => api.tournamentRegister(by, tv.id, player, here));
    if (v) guest = { name: "", rating: "", fide_id: "", federation: "", title: "" };
  }
  async function search(/** @type {Event} */ e) {
    e.preventDefault();
    found = (await run(() => api.ratingSearch(query))) ?? [];
  }
  /** A player found in a rating list fills the guest form. */
  function use(/** @type {any} */ p) {
    const fide = p.list.startsWith("FIDE");
    guest = { name: p.name, rating: p.rating ?? "", fide_id: fide ? p.id : p.fide_id || "", federation: fide ? p.federation : "", title: p.title || "" };
    found = null;
  }

  async function setResult(/** @type {number} */ r, /** @type {number} */ b, /** @type {string} */ value) {
    const g = T.played[r].games[b];
    let why = "";
    if (g.result) {
      why = prompt(t("tournaments.why_correction")) ?? "";
      if (!why.trim()) return change(() => api.tournamentView(tv.id));
    }
    await change(() => api.tournamentResult(by, tv.id, r, b, value || null, why));
  }
  async function pairAgain() {
    const why = prompt(t("tournaments.why_pair_again"));
    if (why?.trim()) await change(() => api.tournamentPairAgain(by, tv.id, why));
  }
  function startEdit() {
    const r = T.played[last];
    edit = { boards: r.games.map((/** @type {any} */ g) => [g.white, g.black]), bye: r.bye, why: "" };
    warnings = null;
  }
  const pairs = () => edit.boards.filter((/** @type {any} */ b) => b[0] !== null && b[1] !== null);
  async function check() {
    warnings = (await run(() => api.tournamentCheck(tv.id, pairs(), edit.bye))) ?? null;
  }
  async function keep(/** @type {SubmitEvent} */ e) {
    e.preventDefault();
    const v = await change(() => api.tournamentSetPairing(by, tv.id, pairs(), edit.bye, edit.why));
    if (v) edit = warnings = null;
  }
  async function exportAs(/** @type {string} */ kind) {
    const p = await run(() => api.tournamentExport(tv.id, kind));
    note = p ? t("tournaments.saved", { path: p }) : "";
  }
  async function rate() {
    const v = await run(() => api.tournamentRate(by, tv.id));
    if (v) {
      onclub(v);
      await open(tv.id);
      note = t("tournaments.rated");
    }
  }
  function move(/** @type {number} */ i, /** @type {number} */ d) {
    if (!order) return;
    const o = [...order];
    [o[i], o[i + d]] = [o[i + d], o[i]];
    order = o;
  }
  const tbName = (/** @type {string} */ code) => tv.all.find((/** @type {any} */ x) => x.code === code)?.name ?? code;
  const when = (/** @type {number} */ at) => new Date(at * 1000).toLocaleString(i18n.lang, { dateStyle: "medium", timeStyle: "short" });
</script>

{#snippet boards(/** @type {number} */ r)}
  {@const round = T.played[r]}
  <div class="scroll">
    <table>
      <thead>
        <tr><th>{t("tournaments.board")}</th><th>{t("tournaments.white")}</th><th>{t("tournaments.result")}</th><th>{t("tournaments.black")}</th></tr>
      </thead>
      <tbody>
        {#each round.games as g, b (b)}
          <tr>
            <td class="num">{b + 1}</td>
            <td>{name(g.white)}</td>
            <td>
              <select aria-label={t("tournaments.result_of", { white: name(g.white), black: name(g.black) })} value={g.result ?? ""} disabled={!official} onchange={(e) => setResult(r, b, e.currentTarget.value)}>
                <option value="">–</option>
                {#each OUTCOMES as o}<option value={o}>{t(`tournaments.outcome.${o}`)}</option>{/each}
              </select>
            </td>
            <td>{name(g.black)}</td>
          </tr>
        {/each}
        {#if round.bye !== null}<tr><td></td><td colspan="3">{t("tournaments.has_bye", { name: name(round.bye) })}</td></tr>{/if}
        {#each round.half as p}<tr><td></td><td colspan="3">{t("tournaments.has_half_bye", { name: name(p) })}</td></tr>{/each}
      </tbody>
    </table>
  </div>
{/snippet}

<div class="panel">
  <h3>{t("tournaments.title")}</h3>
  {#if !tv}
    {#if club.tournaments.length === 0}<p class="dim">{t("tournaments.none_yet")}</p>{/if}
    <ul class="known">
      {#each club.tournaments as x (x.id)}
        <li>
          <button class="link" onclick={() => open(x.id)}>{x.name}</button>
          <span class="dim">{[x.date, t(systemKey(x.system)), t("tournaments.progress", { paired: num(x.paired), rounds: num(x.rounds), players: num(x.players) })].filter(Boolean).join(" · ")}</span>
        </li>
      {/each}
    </ul>
    <form onsubmit={create}>
      <h4>{t("tournaments.new")}</h4>
      <div class="grid">
        <label>{t("tournaments.name")} <input type="text" required bind:value={fresh.name} /></label>
        <label
          >{t("tournaments.system")}
          <select bind:value={fresh.system}>
            <option value="dutch">{t("tournaments.system.dutch")}</option>
            <option value="single">{t("tournaments.system.round_robin")}</option>
            <option value="double">{t("tournaments.system.double_round_robin")}</option>
          </select>
        </label>
        {#if fresh.system === "dutch"}<label>{t("tournaments.rounds")} <input type="number" min="1" max="99" required bind:value={fresh.rounds} /></label>{/if}
        <label>{t("tournaments.date")} <input type="date" bind:value={fresh.date} /></label>
        <label>{t("tournaments.place")} <input type="text" bind:value={fresh.place} /></label>
      </div>
      <p class="dim">{t("tournaments.new_hint")}</p>
      <div class="row">
        <button class="primary" disabled={!official}>{t("tournaments.create")}</button>
        <button type="button" disabled={!official} onclick={importBundle}>{t("tournaments.import")}</button>
      </div>
    </form>
  {:else}
    <div class="head">
      <div>
        <h4>{T.name}</h4>
        <p class="dim">{[T.date, T.place, t(systemKey(T.system)), t("tournaments.progress", { paired: num(T.played.length), rounds: num(T.rounds), players: num(T.players.length) })].filter(Boolean).join(" · ")}</p>
      </div>
      <div class="row">
        <button onclick={() => (tv = null)}>{t("clubs.close")}</button>
        <button disabled={!director} onclick={remove}>{t("tournaments.remove")}</button>
      </div>
    </div>
    <div class="tabs" role="tablist">
      {#each TABS as id}
        <button role="tab" aria-selected={tab === id} class:current={tab === id} onclick={() => (tab = id)}>{t(`tournaments.tab.${id}`)}</button>
      {/each}
    </div>

    {#if tab === "players"}
      <p class="dim">{t("tournaments.players_hint")}</p>
      {#if T.players.length}
        <div class="row">
          <button disabled={!official || !T.players.some((/** @type {any} */ p) => p.status === "away")} onclick={() => change(() => api.tournamentCheckInAll(by, tv.id))}>{t("tournaments.check_in_all")}</button>
        </div>
        <div class="scroll">
          <table>
            <thead>
              <tr><th>{t("tournaments.player")}</th><th>{t("clubs.rating")}</th><th>{t("tournaments.points")}</th><th>{t("tournaments.status")}</th></tr>
            </thead>
            <tbody>
              {#each T.players as p, i (i)}
                <tr>
                  <td>{p.title ? `${p.title} ` : ""}{p.name}</td>
                  <td class="num">{p.rating ? num(p.rating) : ""}</td>
                  <td class="num">{tv.points[i]}</td>
                  <td>
                    <select aria-label={t("tournaments.status_of", { name: p.name })} value={p.status} disabled={!official} onchange={(e) => change(() => api.tournamentStatus(by, tv.id, i, e.currentTarget.value))}>
                      {#each STATUS as s}<option value={s}>{t(`tournaments.status.${s}`)}</option>{/each}
                    </select>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
      <h4>{t("tournaments.register")}</h4>
      <label class="check"><input type="checkbox" bind:checked={here} /> {t("tournaments.register_here")}</label>
      <div class="row">
        <label for="t-member">{t("tournaments.member")}</label>
        <select id="t-member" bind:value={member} disabled={!official || free.length === 0}>
          <option value={null}>–</option>
          {#each free as m (m.id)}<option value={m.id}>{m.name}</option>{/each}
        </select>
        <button disabled={!official || member === null} onclick={addMember}>{t("tournaments.add")}</button>
      </div>
      <form onsubmit={addGuest}>
        <div class="row">
          <label for="t-find">{t("tournaments.guest")}</label>
          <input id="t-find" type="search" bind:value={query} placeholder={t("clubs.find_hint")} onkeydown={(e) => e.key === "Enter" && search(e)} />
          <button type="button" onclick={search}>{t("clubs.search")}</button>
        </div>
        {#if found}
          {#if found.length === 0}<p class="dim">{t("clubs.none_found")}</p>{/if}
          <ul class="known">
            {#each found as p (p.list + p.id)}
              <li>
                <button type="button" class="link" onclick={() => use(p)}>{p.name}</button>
                <span class="dim">{p.list} {p.id}{p.federation ? ` · ${p.federation}` : ""}{p.rating ? ` · ${num(p.rating)}` : ""}</span>
              </li>
            {/each}
          </ul>
        {/if}
        <div class="grid">
          <label>{t("clubs.name")} <input type="text" required bind:value={guest.name} /></label>
          <label>{t("clubs.rating")} <input type="number" min="0" max="4000" bind:value={guest.rating} /></label>
          <label>{t("clubs.fide_id")} <input type="text" inputmode="numeric" bind:value={guest.fide_id} /></label>
          <label>{t("clubs.federation")} <input type="text" maxlength="3" bind:value={guest.federation} /></label>
        </div>
        <button class="primary" disabled={!official}>{t("tournaments.add_guest")}</button>
      </form>
    {:else if tab === "round"}
      <div class="row">
        <button class="primary" disabled={!official || !!tv.blocked} onclick={() => change(() => api.tournamentPair(by, tv.id))}>{t("tournaments.pair", { n: T.played.length + 1 })}</button>
        {#if tv.blocked}<span class="dim">{tv.blocked}</span>{/if}
      </div>
      {#if last >= 0}
        <h4>{t("tournaments.round_n", { n: last + 1 })}</h4>
        {@render boards(last)}
        <div class="row">
          <button onclick={() => run(() => api.tournamentShow(tv.id, last))}>{t("tournaments.show")}</button>
          <button disabled={!director || T.played[last].games.some((/** @type {any} */ g) => g.result)} onclick={startEdit}>{t("tournaments.change_pairing")}</button>
          <button disabled={!official || T.played[last].games.some((/** @type {any} */ g) => g.result)} onclick={pairAgain}>{t("tournaments.pair_again")}</button>
        </div>
        <p class="dim">{t("tournaments.show_hint")}</p>
        {#if edit}
          <form class="edit" onsubmit={keep}>
            <h4>{t("tournaments.change_pairing")}</h4>
            {#each edit.boards as b, i (i)}
              <div class="row">
                <span class="num">{i + 1}</span>
                <select aria-label={t("tournaments.white_on", { n: i + 1 })} bind:value={b[0]} onchange={() => (warnings = null)}>
                  <option value={null}>–</option>
                  {#each T.players as p, k (k)}{#if p.status !== "withdrawn"}<option value={k}>{p.name}</option>{/if}{/each}
                </select>
                <select aria-label={t("tournaments.black_on", { n: i + 1 })} bind:value={b[1]} onchange={() => (warnings = null)}>
                  <option value={null}>–</option>
                  {#each T.players as p, k (k)}{#if p.status !== "withdrawn"}<option value={k}>{p.name}</option>{/if}{/each}
                </select>
              </div>
            {/each}
            <div class="row">
              <button type="button" onclick={() => (edit.boards = [...edit.boards, [null, null]])}>{t("tournaments.add_board")}</button>
              <label for="t-bye">{t("tournaments.bye_label")}</label>
              <select id="t-bye" bind:value={edit.bye} onchange={() => (warnings = null)}>
                <option value={null}>–</option>
                {#each T.players as p, k (k)}{#if p.status !== "withdrawn"}<option value={k}>{p.name}</option>{/if}{/each}
              </select>
            </div>
            <button type="button" onclick={check}>{t("tournaments.check")}</button>
            {#if warnings}
              {#if warnings.length === 0}<p class="ok" role="status">{t("tournaments.no_warnings")}</p>{/if}
              <ul class="warnings" role="status">
                {#each warnings as w}<li>{w}</li>{/each}
              </ul>
            {/if}
            <label>{t("clubs.why")} <input type="text" required bind:value={edit.why} placeholder={t("tournaments.why_pairing_hint")} /></label>
            <div class="row">
              <button class="primary" disabled={warnings === null}>{t("tournaments.keep_pairing")}</button>
              <button type="button" onclick={() => (edit = warnings = null)}>{t("clubs.cancel")}</button>
            </div>
            {#if warnings === null}<p class="dim">{t("tournaments.check_first")}</p>{/if}
          </form>
        {/if}
        {#each T.played.slice(0, last) as _, r (r)}
          <details>
            <summary>{t("tournaments.round_n", { n: r + 1 })}</summary>
            {@render boards(r)}
          </details>
        {/each}
      {/if}
    {:else if tab === "standings"}
      <div class="scroll">
        <table>
          <thead>
            <tr>
              <th>{t("tournaments.rank")}</th><th>{t("tournaments.player")}</th><th>{t("tournaments.points")}</th>
              {#each tv.tiebreaks as tb}<th><abbr title={tb.name}>{tb.code}</abbr></th>{/each}
            </tr>
          </thead>
          <tbody>
            {#each tv.standings as s (s.player)}
              <tr>
                <td class="num">{s.rank}</td>
                <td>{name(s.player)}</td>
                <td class="num">{s.points}</td>
                {#each s.tiebreaks as v}<td class="num">{v}</td>{/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      <h4>{t("tournaments.tiebreaks")}</h4>
      <p class="dim">{t("tournaments.tiebreaks_hint")}</p>
      {#if order}
        <ol class="order">
          {#each order as code, i (code)}
            <li>
              <span>{tbName(code)} <span class="dim">({code})</span></span>
              <button disabled={i === 0} aria-label={t("tournaments.up", { name: tbName(code) })} onclick={() => move(i, -1)}>↑</button>
              <button disabled={i === order.length - 1} aria-label={t("tournaments.down", { name: tbName(code) })} onclick={() => move(i, 1)}>↓</button>
              <button aria-label={t("tournaments.drop", { name: tbName(code) })} onclick={() => order && (order = order.filter((c) => c !== code))}>✕</button>
            </li>
          {/each}
        </ol>
        <div class="row">
          <select aria-label={t("tournaments.add_tiebreak")} bind:value={adding}>
            <option value="">–</option>
            {#each tv.all.filter((/** @type {any} */ x) => !order?.includes(x.code)) as x (x.code)}<option value={x.code}>{x.name} ({x.code})</option>{/each}
          </select>
          <button disabled={!adding} onclick={() => order && ((order = [...order, adding]), (adding = ""))}>{t("tournaments.add")}</button>
        </div>
        <div class="row">
          <button class="primary" onclick={() => change(() => api.tournamentTiebreaks(by, tv.id, order ?? [])).then((v) => v && (order = null))}>{t("clubs.save")}</button>
          <button onclick={() => (order = null)}>{t("clubs.cancel")}</button>
        </div>
      {:else}
        <ol class="order">
          {#each tv.tiebreaks as tb}<li>{tb.name} <span class="dim">({tb.code})</span></li>{/each}
        </ol>
        <button disabled={!official} onclick={() => (order = tv.tiebreaks.map((/** @type {any} */ x) => x.code))}>{t("tournaments.change_tiebreaks")}</button>
      {/if}
      <h4>{t("tournaments.rate")}</h4>
      <p class="dim">{T.rated ? t("tournaments.rated") : t("tournaments.rate_hint")}</p>
      <button disabled={!director || T.rated || T.played.length === 0} onclick={rate}>{t("tournaments.rate_button")}</button>
    {:else if tab === "exports"}
      <p class="dim">{t("tournaments.exports_hint")}</p>
      <ul class="exports">
        {#each ["html", "trf", "pgn", "bundle"] as kind}
          <li>
            <button onclick={() => exportAs(kind)}>{t(`tournaments.export.${kind}`)}</button>
            <span class="dim">{t(`tournaments.export.${kind}.hint`)}</span>
          </li>
        {/each}
      </ul>
      <p class="dim">{t("tournaments.dsu")}</p>
    {:else}
      {#if T.log.length === 0}
        <p class="dim">{t("clubs.log_empty")}</p>
      {:else}
        <div class="scroll">
          <table>
            <thead>
              <tr><th>{t("clubs.when")}</th><th>{t("clubs.who")}</th><th>{t("tournaments.concerns")}</th><th>{t("clubs.change")}</th><th>{t("clubs.why")}</th></tr>
            </thead>
            <tbody>
              {#each [...T.log].reverse() as e, i (i)}
                <tr>
                  <td>{when(e.at)}</td>
                  <td>{e.who}</td>
                  <td>{e.member}</td>
                  <td>{t(`tournaments.log.${e.what}`)}: {e.what === "status" ? `${t(`tournaments.status.${e.old}`)} → ${t(`tournaments.status.${e.new}`)}` : `${e.old} → ${e.new}`}</td>
                  <td>{e.why}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    {/if}
  {/if}
  {#if note}<p class="dim" role="status">{note}</p>{/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</div>

<style>
  h3,
  h4 {
    font-size: 1.05rem;
    margin: 0.6rem 0 0.4rem;
  }
  p {
    margin: 0.3rem 0;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: start;
    flex-wrap: wrap;
    gap: 0.6rem;
  }
  .row {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
    align-items: center;
    margin: 0.3rem 0;
  }
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
  .row label,
  label.check {
    display: inline-flex;
    flex-direction: row;
    align-items: center;
    gap: 0.3rem;
  }
  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.4rem;
    margin: 0.4rem 0;
  }
  .tabs button.current {
    background: var(--sel-bg);
    color: var(--sel-fg);
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
    vertical-align: middle;
  }
  .num {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .known,
  .exports {
    list-style: none;
    padding: 0;
    margin: 0.3rem 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .known li,
  .exports li {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: baseline;
  }
  .order li {
    display: flex;
    gap: 0.3rem;
    align-items: center;
    margin: 0.15rem 0;
  }
  .order li > span:first-child {
    flex: 1;
  }
  .edit {
    border-top: 1px solid var(--border);
    margin-top: 0.5rem;
  }
  .warnings li {
    color: var(--red);
  }
  details {
    margin: 0.3rem 0;
  }
  /* A result or status fits a narrow window beside the names. */
  td select {
    min-width: 0;
    max-width: 9rem;
  }
  .dim {
    color: var(--dim);
    font-size: 0.9em;
    overflow-wrap: anywhere;
  }
  .error {
    color: var(--red);
  }
</style>
