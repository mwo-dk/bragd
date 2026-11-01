<script>
  // A player's summary over a Library (bragd_library::player), or over the open Foreign database
  // when no Library is named, for each Game family Settings
  // show that they play in. A part a Family's Games do not give (ratings, ECO codes, dates) is
  // left out. A Game named here opens in replay.
  import * as k from "./knowledge.js";
  import { t, tn, num, i18n } from "./i18n.svelte.js";

  /** @type {{ library?: string, name: string, onopen: (id: number) => void, onclose: () => void }} */
  let { library = "", name, onopen, onclose } = $props();
  /** @type {any} */
  let s = $state(null);
  let error = $state("");

  $effect(() => {
    s = null;
    error = "";
    (library ? k.libraryPlayer(library, name) : k.dbPlayer(name)).then((r) => (s = r), (e) => (error = k.why(e)));
  });

  const pct = (/** @type {number} */ p) => p.toLocaleString(i18n.lang, { maximumFractionDigits: 1 });
  const record = (/** @type {any} */ r) => t("player.record", { wins: num(r.wins), draws: num(r.draws), losses: num(r.losses) });

  /** The rating line: points across 0..100 by their order, up and down between the lowest and highest. */
  function line(/** @type {[string, number][]} */ rs) {
    const elos = rs.map((r) => r[1]);
    const lo = Math.min(...elos);
    const span = Math.max(...elos) - lo || 1;
    return rs.map((r, i) => `${rs.length > 1 ? (i * 100) / (rs.length - 1) : 50},${30 - ((r[1] - lo) * 30) / span}`).join(" ");
  }
</script>

{#snippet game(/** @type {any} */ g)}
  <button class="link" onclick={() => onopen(g.id)}>
    {g.opponent}{g.opponentElo ? ` (${g.opponentElo})` : ""}, {g.date || "?"}, {g.result}
  </button>
{/snippet}

<section class="player" aria-labelledby="player-name">
  <div class="bar">
    <h3 id="player-name">{name}</h3>
    <button onclick={onclose}>{t("player.close")}</button>
  </div>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if s && !s.families.length}<p class="dim">{t("player.none")}</p>{/if}
  {#each s?.families ?? [] as f (f.family)}
    <h4>{f.title}</h4>
    <p>
      {tn("player.games", f.games)}{#if f.score !== null}{" · "}{t("player.score", { p: pct(f.score) })}{/if}
    </p>
    <table class="record">
      <tbody>
        {#each [0, 1] as i}
          {@const r = i ? f.secondSide : f.firstSide}
          {#if r.wins + r.draws + r.losses + r.unfinished}
            <tr>
              <th scope="row">{t("player.as", { side: f.sides[i] })}</th>
              <td>{record(r)}{#if r.unfinished}{" · "}{t("player.unfinished", { n: num(r.unfinished) })}{/if}</td>
            </tr>
          {/if}
        {/each}
      </tbody>
    </table>

    {#if f.ratings.length}
      {@const elos = f.ratings.map((/** @type {any} */ r) => r[1])}
      <h5>{t("player.rating")}</h5>
      <svg class="rating" viewBox="-2 -2 104 34" preserveAspectRatio="none" role="img" aria-label={t("player.rating_from", { from: f.ratings[0][1], to: f.ratings.at(-1)[1], low: Math.min(...elos), high: Math.max(...elos) })}>
        <polyline points={line(f.ratings)} fill="none" stroke="currentColor" stroke-width="1" vector-effect="non-scaling-stroke" />
      </svg>
      <p class="dim">
        {t("player.rating_from", { from: `${f.ratings[0][1]} (${f.ratings[0][0]})`, to: `${f.ratings.at(-1)[1]} (${f.ratings.at(-1)[0]})`, low: Math.min(...elos), high: Math.max(...elos) })}
      </p>
    {/if}

    {#if f.openingsFirst.length || f.openingsSecond.length}
      <div class="cols">
        {#each [[f.openingsFirst, f.sides[0]], [f.openingsSecond, f.sides[1]]] as [list, side]}
          {#if list.length}
            <div>
              <h5>{t("player.openings", { side })}</h5>
              <ul>
                {#each list as o (o.eco)}
                  <li><b>{o.eco}</b> {o.name} · {record(o.record)}</li>
                {/each}
              </ul>
            </div>
          {/if}
        {/each}
      </div>
    {/if}

    <div class="cols">
      {#if f.bestWins.length}
        <div>
          <h5>{t("player.best_wins")}</h5>
          <ul>{#each f.bestWins as g (g.id)}<li>{@render game(g)}</li>{/each}</ul>
        </div>
      {/if}
      {#if f.worstLosses.length}
        <div>
          <h5>{t("player.worst_losses")}</h5>
          <ul>{#each f.worstLosses as g (g.id)}<li>{@render game(g)}</li>{/each}</ul>
        </div>
      {/if}
    </div>

    {#if f.first}
      <dl>
        <dt>{t("player.first")}</dt>
        <dd>{@render game(f.first)}</dd>
        <dt>{t("player.last")}</dt>
        <dd>{@render game(f.last)}</dd>
      </dl>
    {/if}
  {/each}
</section>

<style>
  .player {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    max-width: 48rem;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
    justify-content: space-between;
  }
  h3,
  h4,
  h5,
  p {
    margin: 0;
  }
  h3 {
    font-size: 1.1rem;
  }
  h4 {
    font-size: 1rem;
    margin-top: 0.8rem;
    border-bottom: 1px solid var(--border);
  }
  h5 {
    font-size: 0.9rem;
    margin-top: 0.4rem;
  }
  .record th {
    text-align: start;
    font-weight: normal;
    padding-inline-end: 1rem;
  }
  .rating {
    width: 100%;
    height: 4rem;
    color: var(--accent, currentColor);
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr));
    gap: 0 1rem;
  }
  ul {
    margin: 0.2rem 0;
    padding-inline-start: 1.2rem;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.1rem 0.8rem;
    margin: 0.4rem 0 0;
  }
  dd {
    margin: 0;
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    color: inherit;
    text-decoration: underline;
    text-align: start;
  }
  .dim {
    color: var(--dim);
    font-size: 0.9em;
  }
  .error {
    color: var(--red);
  }
</style>
