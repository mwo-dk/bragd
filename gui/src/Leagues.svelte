<script>
  // A Club's team Leagues: set one up, add its Teams, pair each Round, line the Teams up, enter
  // board results and follow the standings. Rust decides the pairing, the points and the
  // standings (bragd_clubs::league); this page draws them.
  import * as api from "./api.js";
  import { t, num, i18n } from "./i18n.svelte.js";
  import Why from "./Why.svelte";

  /** @type {{ club: any, by: number|null, role: string|null, onclub: (v: any) => void, onopen: (open: boolean) => void }} */
  let { club, by, role, onclub, onopen } = $props();

  /** The open League's view. @type {any} */
  let lv = $state(null);
  let tab = $state("teams");
  let error = $state("");
  const today = () => new Date().toISOString().slice(0, 10);
  const blank = () => ({ name: "", boards: 4, double: false, date: today() });
  let fresh = $state(blank());
  /** The Team being made: its name and its players in board order. @type {{ name: string, players: any[] }} */
  let team = $state({ name: "", players: [] });
  let member = $state(/** @type {number|null} */ (null));
  let guest = $state("");
  /** @type {any} */
  let whyBox;
  // An open League takes the page (Clubs folds the rest away).
  $effect(() => onopen(lv !== null));

  const official = $derived(by !== null && (role === "director" || role === "arbiter") && !club.held_by);
  const director = $derived(by !== null && role === "director" && !club.held_by);
  const L = $derived(lv?.league);
  const OUTCOMES = ["white", "draw", "black", "white_forfeit", "black_forfeit", "double_forfeit"];
  const TABS = ["teams", "rounds", "standings", "log"];
  const homeWhite = (/** @type {number} */ b) => b % 2 === 0;
  const player = (/** @type {number} */ team, /** @type {number|null} */ p) => (p === null ? "–" : (L.teams[team].players[p]?.name ?? ""));

  async function run(/** @type {() => Promise<any>} */ f) {
    try {
      const v = await f();
      error = "";
      return v;
    } catch (e) {
      error = String(e);
    }
  }
  const change = (/** @type {() => Promise<any>} */ f) => run(f).then((v) => v && (lv = v));

  async function open(/** @type {number} */ id) {
    const v = await run(() => api.leagueView(id));
    if (v) {
      lv = v;
      tab = v.league.played.length ? "rounds" : "teams";
    }
  }
  async function create(/** @type {SubmitEvent} */ e) {
    e.preventDefault();
    const v = await run(() => api.leagueNew(by, fresh.name, Number(fresh.boards), fresh.double, fresh.date));
    if (v) {
      onclub(v);
      fresh = blank();
      if (v.leagues.length) open(v.leagues[0].id);
    }
  }
  async function remove() {
    if (!confirm(t("leagues.remove_confirm", { name: L.name }))) return;
    const v = await run(() => api.leagueRemove(by, lv.id));
    if (v) {
      lv = null;
      onclub(v);
    }
  }
  function addMember() {
    const m = club.members.find((/** @type {any} */ x) => x.id === member);
    if (m) team.players = [...team.players, { name: m.name, rating: Math.round(m.rating.rating), fide_id: m.fide_id, national_id: m.national_id, federation: m.federation, member: m.id }];
    member = null;
  }
  function addGuest() {
    if (guest.trim()) team.players = [...team.players, { name: guest.trim() }];
    guest = "";
  }
  function move(/** @type {number} */ i, /** @type {number} */ d) {
    const p = [...team.players];
    [p[i], p[i + d]] = [p[i + d], p[i]];
    team.players = p;
  }
  async function addTeam(/** @type {SubmitEvent} */ e) {
    e.preventDefault();
    const v = await change(() => api.leagueAddTeam(by, lv.id, team.name, team.players));
    if (v) team = { name: "", players: [] };
  }
  /** Changes one board of a Team's line-up, keeping the rest as it is. */
  async function lineUp(/** @type {number} */ r, /** @type {number} */ m, /** @type {boolean} */ away, /** @type {number} */ b, /** @type {string} */ value) {
    const players = L.played[r].matches[m].boards.map((/** @type {any} */ x, /** @type {number} */ k) => (k === b ? (value === "" ? null : Number(value)) : away ? x.away : x.home));
    const v = await change(() => api.leagueLineUp(by, lv.id, r, m, away, players));
    if (!v) await change(() => api.leagueView(lv.id));
  }
  async function setResult(/** @type {number} */ r, /** @type {number} */ m, /** @type {number} */ b, /** @type {string} */ value) {
    let why = "";
    if (L.played[r].matches[m].boards[b].result) {
      why = (await whyBox.ask(t("tournaments.why_correction"))) ?? "";
      if (!why) return change(() => api.leagueView(lv.id));
    }
    const v = await change(() => api.leagueResult(by, lv.id, r, m, b, value || null, why));
    if (!v) await change(() => api.leagueView(lv.id));
  }
  const when = (/** @type {number} */ at) => new Date(at * 1000).toLocaleString(i18n.lang, { dateStyle: "medium", timeStyle: "short" });
</script>

{#snippet side(/** @type {number} */ r, /** @type {number} */ m, /** @type {any} */ match, /** @type {any} */ x, /** @type {number} */ b, /** @type {boolean} */ away)}
  {@const tm = away ? match.away : match.home}
  {@const p = away ? x.away : x.home}
  <select
    aria-label={t("leagues.line_up_of", { team: L.teams[tm].name, board: num(b + 1) })}
    value={p ?? ""}
    disabled={!official || match.boards.some((/** @type {any} */ y) => y.result)}
    onchange={(e) => lineUp(r, m, away, b, e.currentTarget.value)}
  >
    <option value="">–</option>
    {#each L.teams[tm].players as pl, k (k)}<option value={k}>{pl.name}</option>{/each}
  </select>
{/snippet}

<div class="panel">
  <Why bind:this={whyBox} />
  <h3>{t("leagues.title")}</h3>
  {#if !lv}
    {#if club.leagues.length === 0}<p class="dim">{t("leagues.none_yet")}</p>{/if}
    <ul class="known">
      {#each club.leagues as x (x.id)}
        <li>
          <button class="link" onclick={() => open(x.id)}>{x.name}</button>
          <span class="dim">{[x.date, t("leagues.progress", { paired: num(x.paired), rounds: num(x.rounds), teams: num(x.teams) })].filter(Boolean).join(" · ")}</span>
        </li>
      {/each}
    </ul>
    <form onsubmit={create}>
      <h4>{t("leagues.new")}</h4>
      <div class="grid">
        <label>{t("leagues.name")} <input type="text" required bind:value={fresh.name} /></label>
        <label>{t("leagues.boards")} <input type="number" min="1" max="20" required bind:value={fresh.boards} /></label>
        <label>{t("tournaments.date")} <input type="date" bind:value={fresh.date} /></label>
      </div>
      <label class="check"><input type="checkbox" bind:checked={fresh.double} /> {t("leagues.double")}</label>
      <div class="row"><button class="primary" disabled={!official}>{t("leagues.create")}</button></div>
    </form>
  {:else}
    <div class="head">
      <div>
        <h4>{L.name}</h4>
        <p class="dim">{[L.date, t("leagues.boards_of", { boards: num(L.boards) }), t("leagues.progress", { paired: num(L.played.length), rounds: num(lv.rounds), teams: num(L.teams.length) })].filter(Boolean).join(" · ")}</p>
      </div>
      <div class="row">
        <button onclick={() => (lv = null)}>{t("clubs.back", { name: club.name })}</button>
        <button disabled={!director} onclick={remove}>{t("leagues.remove")}</button>
      </div>
    </div>
    <div class="tabs" role="tablist">
      {#each TABS as id}
        <button role="tab" aria-selected={tab === id} class:current={tab === id} onclick={() => ((tab = id), (error = ""))}>{t(`leagues.tab.${id}`)}</button>
      {/each}
    </div>

    {#if tab === "teams"}
      {#each L.teams as tm, i (i)}
        <details open={L.teams.length < 5}>
          <summary>{tm.name} <span class="dim">{t("leagues.players", { n: num(tm.players.length) })}</span></summary>
          <ol>{#each tm.players as p, k (k)}<li>{p.name}</li>{/each}</ol>
        </details>
      {/each}
      {#if L.played.length === 0}
        <form onsubmit={addTeam} class="edit">
          <h4>{t("leagues.new_team")}</h4>
          <label>{t("leagues.team_name")} <input type="text" required bind:value={team.name} /></label>
          <p class="dim">{t("leagues.order_hint")}</p>
          <ol class="order">
            {#each team.players as p, i (i)}
              <li>
                <span>{p.name}</span>
                <button type="button" aria-label={t("leagues.up", { name: p.name })} disabled={i === 0} onclick={() => move(i, -1)}>↑</button>
                <button type="button" aria-label={t("leagues.down", { name: p.name })} disabled={i === team.players.length - 1} onclick={() => move(i, 1)}>↓</button>
                <button type="button" aria-label={t("leagues.drop", { name: p.name })} onclick={() => (team.players = team.players.filter((_, k) => k !== i))}>✕</button>
              </li>
            {/each}
          </ol>
          <div class="row">
            <select aria-label={t("leagues.add_member")} bind:value={member}>
              <option value={null}>{t("leagues.add_member")}</option>
              {#each club.members as m (m.id)}<option value={m.id}>{m.name}</option>{/each}
            </select>
            <button type="button" disabled={member === null} onclick={addMember}>{t("leagues.add")}</button>
            <input type="text" aria-label={t("leagues.guest")} placeholder={t("leagues.guest")} bind:value={guest} />
            <button type="button" disabled={!guest.trim()} onclick={addGuest}>{t("leagues.add")}</button>
          </div>
          <div class="row"><button class="primary" disabled={!official}>{t("leagues.add_team")}</button></div>
        </form>
      {:else}
        <p class="dim">{t("leagues.teams_fixed")}</p>
      {/if}
    {:else if tab === "rounds"}
      <div class="row">
        <button class="primary" disabled={!official || !!lv.blocked} onclick={() => change(() => api.leaguePair(by, lv.id))}>{t("tournaments.pair", { n: num(L.played.length + 1) })}</button>
        {#if lv.blocked}<span class="dim">{lv.blocked}</span>{/if}
      </div>
      {#each L.played as round, r (r)}
        <h4>{t("leagues.round", { n: num(r + 1) })}</h4>
        {#each round.matches as match, m (m)}
          <p class="match">
            <strong>{L.teams[match.home].name} – {L.teams[match.away].name}</strong>
            <span class="num">{lv.scores[r][m][0]} – {lv.scores[r][m][1]}</span>
          </p>
          <div class="scroll">
            <table class="stack">
              <thead>
                <tr><th>{t("tournaments.board")}</th><th>{L.teams[match.home].name}</th><th>{t("tournaments.result")}</th><th>{L.teams[match.away].name}</th></tr>
              </thead>
              <tbody>
                {#each match.boards as x, b (b)}
                  <tr>
                    <td class="num">{b + 1} <span class="dim">{homeWhite(b) ? t("leagues.home_white") : t("leagues.home_black")}</span></td>
                    <td data-label={L.teams[match.home].name}>{@render side(r, m, match, x, b, false)}</td>
                    <td data-label={t("tournaments.result")}>
                      <select
                        aria-label={t("tournaments.result_of", { white: player(homeWhite(b) ? match.home : match.away, homeWhite(b) ? x.home : x.away), black: player(homeWhite(b) ? match.away : match.home, homeWhite(b) ? x.away : x.home) })}
                        value={x.result ?? ""}
                        disabled={!official}
                        onchange={(e) => setResult(r, m, b, e.currentTarget.value)}
                      >
                        <option value="">–</option>
                        {#each OUTCOMES as o}<option value={o}>{t(`tournaments.outcome.${o}`)}</option>{/each}
                      </select>
                    </td>
                    <td data-label={L.teams[match.away].name}>{@render side(r, m, match, x, b, true)}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/each}
        {#if round.bye !== null}<p class="dim">{t("leagues.sits_out", { team: L.teams[round.bye].name })}</p>{/if}
      {/each}
    {:else if tab === "standings"}
      <div class="scroll">
        <table class="stack">
          <thead>
            <tr>
              <th>{t("leagues.th.rank")}</th><th>{t("leagues.th.team")}</th><th>{t("leagues.th.played")}</th><th>{t("leagues.th.wdl")}</th><th>{t("leagues.th.match_points")}</th><th>{t("leagues.th.board_points")}</th>
            </tr>
          </thead>
          <tbody>
            {#each lv.standings as s (s.team)}
              <tr>
                <td class="num">{s.rank}</td>
                <td>{L.teams[s.team].name}</td>
                <td class="num" data-label={t("leagues.th.played")}>{num(s.played)}</td>
                <td class="num" data-label={t("leagues.th.wdl")}>{num(s.won)}–{num(s.drawn)}–{num(s.lost)}</td>
                <td class="num" data-label={t("leagues.th.match_points")}>{s.match_points}</td>
                <td class="num" data-label={t("leagues.th.board_points")}>{s.board_points}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      <p class="dim">{t("leagues.standings_hint")}</p>
    {:else}
      {#if L.log.length === 0}<p class="dim">{t("leagues.log_empty")}</p>{/if}
      <ul class="known">
        {#each [...L.log].reverse() as e, i (i)}
          <li><span class="dim">{when(e.at)} · {e.who}</span> {e.member}: {e.old} → {e.new} <span class="dim">({e.why})</span></li>
        {/each}
      </ul>
    {/if}
  {/if}
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
  .match {
    display: flex;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 0.5rem;
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
  .known {
    list-style: none;
    padding: 0;
    margin: 0.3rem 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .known li {
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
  /* A line-up or a result fits a narrow window beside the names. */
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
