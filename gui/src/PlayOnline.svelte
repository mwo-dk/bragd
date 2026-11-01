<script>
  // Play online on Lichess, Lishogi or OGS: sign in, seek a Game or play a challenge, then the
  // Game on Bragd's board with its clocks, offers and chat. Rust keeps the account, the streams and the Game,
  // checks each move by Bragd's rules before it is sent, and hands this page a view a few times a
  // second; nothing here knows a rule.
  import { onDestroy } from "svelte";
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";
  import Picker from "./Picker.svelte";
  import Board from "./Board.svelte";
  import { movesOf, movesTo, plainMoves } from "./board.js";
  import { guide } from "./guide.svelte.js";

  /** @type {{ server: string, family: any, save: (name: string, value: any) => Promise<any> }} */
  let { server, family, save } = $props();

  /** @type {any} */
  let view = $state(null);
  let error = $state("");
  let busy = $state(false);
  // The seek's and the challenge's settings.
  let variant = $state("");
  let time = $state("");
  let rated = $state(false);
  let user = $state("");
  let color = $state("random");
  let line = $state("");
  /** @type {{from?: string, drop?: string} | null} */
  let sel = $state(null);
  /** @type {any[]} */
  let choose = $state([]);

  const refresh = () =>
    api.onlineView(server).then(
      (v) => (view = v),
      (e) => (error = String(e)),
    );
  refresh();
  const timer = setInterval(refresh, 300);
  onDestroy(() => clearInterval(timer));

  const game = $derived(view?.game ?? null);
  const st = $derived(game?.state ?? null);
  const mine = $derived(!!st && game.seat !== null && !game.over && st.toMove?.index === game.seat);
  const targets = $derived(st && sel ? [...new Set(movesOf(st, sel).map((/** @type {any} */ m) => m.to).filter(Boolean))] : []);
  // A Game on keeps Tips away.
  $effect(() => {
    guide.live = !!game && !game.over;
    return () => (guide.live = false);
  });
  // A Variant turned off is not offered; the times are the server's own.
  $effect(() => {
    if (view && !view.variants.some((/** @type {any} */ v) => v.value === variant) && view.variants.length) variant = view.variants[0].value;
    if (view && !view.times.some((/** @type {any} */ x) => x.value === time) && view.times.length) time = view.times[Math.min(1, view.times.length - 1)].value;
  });
  const name = $derived(view?.server.name ?? "");

  /** Runs a command, showing its refusal. */
  async function run(/** @type {() => Promise<any>} */ f) {
    busy = true;
    try {
      const r = await f();
      if (r && r.variants) view = r;
      error = "";
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }

  const signIn = () => run(() => api.onlineSignIn(server));
  const seek = () => run(() => api.onlineSeek(server, variant, time, rated));
  const challenge = () => run(() => api.onlineChallenge(server, user, variant, time, rated, color).then((r) => ((user = ""), r)));
  const answer = (/** @type {string} */ id, /** @type {string} */ what) => run(() => api.onlineAnswer(server, id, what));
  const act = (/** @type {string} */ what) => run(() => api.onlineAction(server, what));
  const say = () => run(() => api.onlineSay(server, line).then(() => (line = "")));

  function playEntry(/** @type {string} */ entry) {
    sel = null;
    choose = [];
    run(() => api.onlineMove(server, entry));
  }
  function pick(/** @type {any[]} */ moves) {
    if (moves.length === 1) playEntry(moves[0].wire);
    else if (moves.length > 1) choose = moves;
  }
  function cell(/** @type {string} */ square) {
    if (!mine) return;
    const here = movesTo(st, sel, square);
    if (sel && here.length) return pick(here);
    // A placement (go): a move with no start.
    const placed = st.moves.filter((/** @type {any} */ m) => !m.from && !m.drop && m.to === square);
    if (placed.length) return pick(placed);
    sel = movesOf(st, { from: square }).length ? { from: square } : null;
  }
  /** A piece in hand (shogi): chosen to drop. */
  function hand(/** @type {any} */ piece) {
    if (mine && st.moves.some((/** @type {any} */ m) => m.drop === piece.sprite && !m.from)) sel = { drop: piece.sprite };
  }
  const hands = (/** @type {number} */ seat) => st?.board.hands.filter((/** @type {any} */ h) => h.seat === seat) ?? [];
  /** The text for a move that has no square: Pass. */
  const plainLabel = (/** @type {any} */ m) => {
    const label = t(`move.${m.notation}`);
    return label.startsWith("move.") ? m.notation : label;
  };
  function press(/** @type {string} */ square, /** @type {PointerEvent} */ e) {
    if (!mine || !movesOf(st, { from: square }).length) return;
    sel = { from: square };
    const up = (/** @type {PointerEvent} */ u) => {
      window.removeEventListener("pointerup", up);
      const el = document.elementFromPoint(u.clientX, u.clientY);
      const to = el instanceof Element ? el.getAttribute("data-square") : null;
      if (to && to !== square) pick(movesTo(st, sel, to));
    };
    window.addEventListener("pointerup", up);
    e.preventDefault();
  }

  /** `m:ss`, or `h:mm:ss` from an hour. */
  function clock(/** @type {number} */ ms) {
    const s = Math.ceil(ms / 1000);
    const [h, m, x] = [Math.floor(s / 3600), Math.floor((s % 3600) / 60), s % 60];
    const two = (/** @type {number} */ n) => String(n).padStart(2, "0");
    return h ? `${h}:${two(m)}:${two(x)}` : `${m}:${two(x)}`;
  }
  const variantName = (/** @type {any} */ c) => (c.variant ? t(`variant.${c.variant}`) : c.variantName);
  /** The seats from the top of the board down: the opponent's first. */
  const seats = $derived(game ? (game.seat === 1 ? [0, 1] : [1, 0]) : []);
</script>

{#snippet tray(/** @type {number} */ seat)}
  {#if hands(seat).length}
    <div class="tray">
      {#each hands(seat) as h}
        {#each h.pieces as p}
          <button class="hand" class:current={sel?.drop === p.sprite} disabled={!p.count || !mine || seat !== game.seat} onclick={() => hand(p)}>
            <svg viewBox="0 0 1 1"><use href="#{p.sprite}" width="1" height="1" /></svg>
            <span>{p.count}</span>
          </button>
        {/each}
      {/each}
    </div>
  {/if}
{/snippet}

<section class="online">
  <header class="bar">
    <h1>{t("online.title", { server: name })}</h1>
    {#if view?.signedIn}
      <span class="who">{t("online.signed_in_as", { name: view.signedIn })}</span>
      <button onclick={() => run(() => api.onlineSignOut(server))} disabled={busy}>{t("online.sign_out")}</button>
    {/if}
  </header>

  {#if error || view?.error}
    <p class="error" role="alert">
      {error || view.error}
      {#if view?.signedIn && !view.connected}
        <button onclick={() => run(() => api.onlineReconnect(server))} disabled={busy}>{t("online.reconnect")}</button>
      {/if}
    </p>
  {/if}

  {#if view && !view.signedIn}
    <div class="panel">
      <p>{t(`online.intro.${server}`)}</p>
      {#if !view.ready}
        <p class="hint" role="status">{t("online.error.not_registered", { server: name })}</p>
        <button class="link" onclick={() => api.openUrl(`https://${view.server.site}/`)}>{t("online.open_on", { site: view.server.site })}</button>
      {:else if view.signingIn}
        <p class="hint" role="status">{t("online.signing_in")}</p>
        <button onclick={() => api.onlineCancelSignIn(server)}>{t("common.cancel")}</button>
      {:else}
        <button class="primary" onclick={signIn}>{t("online.sign_in", { server: name })}</button>
      {/if}
      <p class="hint">{t("online.sign_in_hint", { server: name })}</p>
    </div>
  {:else if view && !game}
    <div class="lobby">
      <div class="panel">
        <h2>{t("online.new_game")}</h2>
        <div class="form">
          <Picker value={variant} choices={view.variants} label={t("play.variant")} onchange={(v) => (variant = v)} />
          <Picker value={time} choices={view.times} label={t("online.time")} onchange={(v) => (time = v)} />
          <label class="check"><input type="checkbox" bind:checked={rated} /> {t("online.rated")}</label>
        </div>
        {#if rated}<p class="hint">{t("online.rated_hint", { server: name })}</p>{/if}
        {#if view.seeking}
          <p class="hint" role="status">{t("online.seeking")}</p>
          <button onclick={() => api.onlineSeekCancel(server)}>{t("common.cancel")}</button>
        {:else}
          <button class="primary" onclick={seek} disabled={busy || !view.connected}>{t("online.seek")}</button>
        {/if}
        <h3>{t("online.challenge_someone")}</h3>
        <div class="form">
          <label class="field">{t("online.username", { server: name })} <input bind:value={user} autocomplete="off" spellcheck="false" /></label>
          <Picker value={color} choices={view.colours} label={t("online.your_colour")} onchange={(v) => (color = v)} />
          <button onclick={challenge} disabled={busy || !user.trim() || !view.connected}>{t("online.challenge")}</button>
        </div>
      </div>

      <div class="panel">
        <h2>{t("online.challenges")}</h2>
        {#if !view.challenges.length}
          <p class="hint">{t("online.no_challenges")}</p>
        {/if}
        <ul class="challenges">
          {#each view.challenges as c (c.id)}
            <li>
              <span>
                <strong>{c.incoming ? t("online.from", { name: c.who }) : t("online.to", { name: c.who })}</strong>
                · {variantName(c)} · {c.time} · {c.rated ? t("online.rated") : t("online.casual")}
              </span>
              {#if c.incoming}
                {#if c.variant}
                  <button class="primary" onclick={() => answer(c.id, "accept")} disabled={busy}>{t("online.accept")}</button>
                {:else}
                  <span class="hint">{t("online.error.variant", { site: view.server.site })}</span>
                {/if}
                <button onclick={() => answer(c.id, "decline")} disabled={busy}>{t("online.decline")}</button>
              {:else if view.can.cancel}
                <button onclick={() => answer(c.id, "cancel")} disabled={busy}>{t("common.cancel")}</button>
              {/if}
            </li>
          {/each}
        </ul>
      </div>
    </div>
  {:else if game}
    <div class="game">
      <div class="board-side">
        {#if st}
          <div class="player"><span>{game.names[seats[0]]}</span><span class="clock" class:running={game.running === seats[0]}>{clock(game.clocks[seats[0]])}{#if game.byoyomi}<small class="byo">+ {game.periods[seats[0]] > 1 ? `${game.periods[seats[0]]} × ` : ""}{game.byoyomi} s</small>{/if}</span></div>
          {@render tray(seats[0])}
          <div class="board-wrap">
            <Board state={st} family={family.id} flipped={game.seat === 1} selected={sel?.from ?? null} {targets} oncell={cell} onpress={press} />
          </div>
          {@render tray(seats[1])}
          <div class="player"><span>{game.names[seats[1]]}</span><span class="clock" class:running={game.running === seats[1]}>{clock(game.clocks[seats[1]])}{#if game.byoyomi}<small class="byo">+ {game.periods[seats[1]] > 1 ? `${game.periods[seats[1]]} × ` : ""}{game.byoyomi} s</small>{/if}</span></div>
        {:else}
          <p>{t("online.no_rules", { variant: game.variantName, site: view.server.site })}</p>
          <button class="link" onclick={() => api.openUrl(game.url)}>{t("online.open_on", { site: view.server.site })}</button>
        {/if}
      </div>

      <aside class="panel info">
        <p class="turn">
          {#if game.over}{game.result}{:else if mine}{t("online.your_turn")}{:else}{t("online.their_turn")}{/if}
        </p>
        <p class="hint">{variantName(game)} · {game.rated ? t("online.rated") : t("online.casual")}</p>
        {#if game.rated && game.againstPerson && !game.over}
          <p class="lock" role="status">{t("online.locked")}</p>
        {/if}
        {#if game.opponentGone && !game.over}
          <p class="hint" role="status">{t("online.opponent_gone", { server: name })}</p>
        {/if}
        {#if game.dead && !game.over}
          <div class="offer" role="status">
            <span>{game.dead.length ? t("online.dead_stones", { server: name, stones: game.dead.join(", ") }) : t("online.no_dead_stones", { server: name })}</span>
            <button class="primary" onclick={() => act("score")}>{t("online.accept_score")}</button>
            <button class="link" onclick={() => api.openUrl(game.url)}>{t("online.open_on", { site: view.server.site })}</button>
          </div>
        {/if}
        {#if game.offered[0] && !game.over}
          <div class="offer" role="status">
            <span>{t("online.draw_offered")}</span>
            <button class="primary" onclick={() => act("draw")}>{t("online.accept")}</button>
            <button onclick={() => act("no_draw")}>{t("online.decline")}</button>
          </div>
        {/if}
        {#if game.offered[1] && !game.over}
          <div class="offer" role="status">
            <span>{t("online.takeback_offered")}</span>
            <button class="primary" onclick={() => act("takeback")}>{t("online.accept")}</button>
            <button onclick={() => act("no_takeback")}>{t("online.decline")}</button>
          </div>
        {/if}
        {#if choose.length}
          <div class="actions">
            <p>{t("play.choose")}</p>
            {#each choose as m}
              <button onclick={() => playEntry(m.wire)}>{m.notation}</button>
            {/each}
            <button onclick={() => (choose = [])}>{t("common.cancel")}</button>
          </div>
        {/if}
        <div class="actions">
          {#if game.over}
            <button class="primary" onclick={() => act("leave")}>{t("online.back")}</button>
          {:else}
            {#if game.entries.length < 2 && view.can.abort}
              <button onclick={() => act("abort")} disabled={busy}>{t("online.abort")}</button>
            {/if}
            {#if view.can.draw}
              <button onclick={() => act("draw")} disabled={busy || game.offering[0]}>{game.offering[0] ? t("online.draw_sent") : t("online.offer_draw")}</button>
            {/if}
            {#if view.can.takeback}
              <button onclick={() => act("takeback")} disabled={busy || game.offering[1] || !game.entries.length}>{game.offering[1] ? t("online.takeback_sent") : t("online.ask_takeback")}</button>
            {/if}
            {#if mine && !game.dead}
              {#each plainMoves(st).filter((/** @type {any} */ m) => !m.wire.startsWith("dead:")) as m}
                <button onclick={() => playEntry(m.wire)} disabled={busy}>{plainLabel(m)}</button>
              {/each}
            {/if}
            <button onclick={() => act("resign")} disabled={busy}>{t("online.resign")}</button>
          {/if}
          <button class="link" onclick={() => api.openUrl(game.url)}>{t("online.open_on", { site: view.server.site })}</button>
        </div>

        {#if st}
          <h2>{t("play.moves")}</h2>
          <p class="moves">
            {#each st.history as h, i}
              <span class="entry">{#if st.numbers[i]}<span class="no">{st.numbers[i]}</span>{/if}{h}</span>
            {/each}
          </p>
        {/if}

        <h2>{t("online.chat")}</h2>
        {#if view.chatOn}
          <ul class="chat" aria-live="polite">
            {#each view.chat as c}
              <li><strong>{c.user}</strong> {c.text}</li>
            {/each}
          </ul>
          <form class="say" onsubmit={(e) => (e.preventDefault(), say())}>
            <input bind:value={line} maxlength="140" aria-label={t("online.chat_line")} placeholder={t("online.chat_line")} />
            <button type="submit" disabled={!line.trim()}>{t("online.send")}</button>
          </form>
          <p class="hint">
            {t("online.chat_rules", { server: name })}
            <button class="link" onclick={() => api.openUrl(view.terms)}>{t("online.terms", { server: name })}</button>
          </p>
          <button class="link" onclick={() => save("online_chat", false)}>{t("online.chat_off")}</button>
        {:else}
          <p class="hint">{t("online.chat_is_off")}</p>
          <button onclick={() => save("online_chat", true)}>{t("online.chat_on")}</button>
        {/if}
      </aside>
    </div>
  {/if}
</section>

<style>
  .online {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    gap: 0.6rem;
    align-items: baseline;
  }
  h1 {
    margin: 0;
    font-size: 1.4rem;
  }
  h2 {
    font-size: 1rem;
    margin: 0.6rem 0 0.3rem;
  }
  h3 {
    font-size: 0.95rem;
    margin: 0.8rem 0 0.3rem;
  }
  .who,
  .hint {
    color: var(--dim);
  }
  .error {
    color: var(--red);
  }
  .lock {
    font-weight: 600;
  }
  .lobby {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(18rem, 1fr));
    gap: 1rem;
    align-items: start;
  }
  .form,
  .actions,
  .offer {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
    margin: 0.4rem 0;
  }
  .actions p {
    width: 100%;
    margin: 0;
  }
  .check,
  .field {
    display: inline-flex;
    gap: 0.3rem;
    align-items: center;
  }
  .field input {
    width: 10rem;
  }
  .challenges {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .challenges li {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
    padding: 0.3rem 0;
    border-bottom: 1px solid var(--border);
  }
  .challenges li > span:first-child {
    flex: 1 1 12rem;
  }
  .game {
    display: flex;
    gap: 1rem;
    align-items: flex-start;
  }
  .board-side {
    flex: 1 1 auto;
    min-width: 0;
    max-width: min(100%, calc(100vh - 9rem));
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .player {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-weight: 600;
  }
  .tray {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }
  .hand {
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
    padding: 0.1rem 0.35rem;
  }
  .hand svg {
    width: 1.7rem;
    height: 1.7rem;
  }
  .hand.current {
    outline: 2px solid var(--board-highlight);
  }
  .clock {
    font-variant-numeric: tabular-nums;
    padding: 0.1rem 0.5rem;
    border-radius: var(--r-sm);
    background: var(--alt);
    border: 1px solid var(--border);
  }
  .byo {
    margin-inline-start: 0.3em;
  }
  .clock.running {
    background: var(--sel-bg);
    color: var(--sel-fg);
  }
  .info {
    flex: 0 0 17rem;
    max-height: calc(100vh - 6rem);
    overflow: auto;
  }
  .turn {
    font-weight: 600;
    margin-top: 0;
  }
  .moves {
    display: flex;
    flex-wrap: wrap;
    gap: 0 0.5rem;
    margin: 0;
    line-height: 1.6;
    font-variant-numeric: tabular-nums;
  }
  .moves .no {
    color: var(--dim);
    margin-inline-end: 0.2rem;
  }
  .moves .entry {
    white-space: nowrap;
  }
  .chat {
    list-style: none;
    padding: 0;
    margin: 0;
    max-height: 12rem;
    overflow: auto;
    overflow-wrap: anywhere;
  }
  .say {
    display: flex;
    gap: 0.3rem;
    margin: 0.4rem 0;
  }
  .say input {
    flex: 1;
    min-width: 0;
  }
  @media (max-width: 760px) {
    .game {
      flex-direction: column;
    }
    .info,
    .board-side {
      flex: none;
      width: 100%;
      max-width: 100%;
      box-sizing: border-box;
      max-height: none;
    }
  }
</style>
