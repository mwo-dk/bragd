<script>
  // Play online on Lichess: sign in, seek a Game or play a challenge, then the Game on Bragd's
  // board with its clocks, offers and chat. Rust keeps the account, the streams and the Game,
  // checks each move by Bragd's rules before it is sent, and hands this page a view a few times a
  // second; nothing here knows a rule.
  import { onDestroy } from "svelte";
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";
  import Picker from "./Picker.svelte";
  import Board from "./Board.svelte";
  import { movesOf, movesTo } from "./board.js";
  import { guide } from "./guide.svelte.js";

  /** @type {{ family: any, save: (name: string, value: any) => Promise<any> }} */
  let { family, save } = $props();

  /** @type {any} */
  let view = $state(null);
  let error = $state("");
  let busy = $state(false);
  // The seek's and the challenge's settings.
  let variant = $state("chess");
  let time = $state("10+5");
  let rated = $state(false);
  let user = $state("");
  let color = $state("random");
  let line = $state("");
  /** @type {{from?: string} | null} */
  let sel = $state(null);
  /** @type {any[]} */
  let choose = $state([]);

  // Rapid and classical: what the Board API plays in real time.
  const TIMES = ["10+0", "10+5", "15+10", "30+0", "30+20"];

  const refresh = () =>
    api.onlineView().then(
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
  // A Variant turned off is not offered.
  $effect(() => {
    if (view && !view.variants.some((/** @type {any} */ v) => v.key === variant) && view.variants.length) variant = view.variants[0].key;
  });

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

  const [minutes, increment] = $derived(time.split("+").map(Number));
  const signIn = () => run(api.onlineSignIn);
  const seek = () => run(() => api.onlineSeek(variant, minutes, increment, rated));
  const challenge = () => run(() => api.onlineChallenge(user, variant, minutes, increment, rated, color).then((r) => ((user = ""), r)));
  const answer = (/** @type {string} */ id, /** @type {string} */ what) => run(() => api.onlineAnswer(id, what));
  const act = (/** @type {string} */ what) => run(() => api.onlineAction(what));
  const say = () => run(() => api.onlineSay(line).then(() => (line = "")));

  function playEntry(/** @type {string} */ entry) {
    sel = null;
    choose = [];
    run(() => api.onlineMove(entry));
  }
  function pick(/** @type {any[]} */ moves) {
    if (moves.length === 1) playEntry(moves[0].wire);
    else if (moves.length > 1) choose = moves;
  }
  function cell(/** @type {string} */ square) {
    if (!mine) return;
    const here = movesTo(st, sel, square);
    if (sel && here.length) return pick(here);
    sel = movesOf(st, { from: square }).length ? { from: square } : null;
  }
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

<section class="online">
  <header class="bar">
    <h1>{t("online.title")}</h1>
    {#if view?.signedIn}
      <span class="who">{t("online.signed_in_as", { name: view.signedIn })}</span>
      <button onclick={() => run(api.onlineSignOut)} disabled={busy}>{t("online.sign_out")}</button>
    {/if}
  </header>

  {#if error || view?.error}
    <p class="error" role="alert">
      {error || view.error}
      {#if view?.signedIn && !view.connected}
        <button onclick={() => run(api.onlineReconnect)} disabled={busy}>{t("online.reconnect")}</button>
      {/if}
    </p>
  {/if}

  {#if view && !view.signedIn}
    <div class="panel">
      <p>{t("online.intro")}</p>
      {#if view.signingIn}
        <p class="hint" role="status">{t("online.signing_in")}</p>
        <button onclick={api.onlineCancelSignIn}>{t("common.cancel")}</button>
      {:else}
        <button class="primary" onclick={signIn}>{t("online.sign_in")}</button>
      {/if}
      <p class="hint">{t("online.sign_in_hint")}</p>
    </div>
  {:else if view && !game}
    <div class="lobby">
      <div class="panel">
        <h2>{t("online.new_game")}</h2>
        <div class="form">
          <Picker value={variant} choices={view.variants.map((/** @type {any} */ v) => ({ value: v.key, label: v.name }))} label={t("play.variant")} onchange={(v) => (variant = v)} />
          <Picker value={time} choices={TIMES.map((x) => ({ value: x, label: t("online.minutes_plus", { m: x.split("+")[0], s: x.split("+")[1] }) }))} label={t("online.time")} onchange={(v) => (time = v)} />
          <label class="check"><input type="checkbox" bind:checked={rated} /> {t("online.rated")}</label>
        </div>
        {#if rated}<p class="hint">{t("online.rated_hint")}</p>{/if}
        {#if view.seeking}
          <p class="hint" role="status">{t("online.seeking")}</p>
          <button onclick={api.onlineSeekCancel}>{t("common.cancel")}</button>
        {:else}
          <button class="primary" onclick={seek} disabled={busy || !view.connected}>{t("online.seek")}</button>
        {/if}
        <h3>{t("online.challenge_someone")}</h3>
        <div class="form">
          <label class="field">{t("online.username")} <input bind:value={user} autocomplete="off" spellcheck="false" /></label>
          <Picker value={color} choices={["random", "white", "black"].map((c) => ({ value: c, label: t(`online.color.${c}`) }))} label={t("online.your_colour")} onchange={(v) => (color = v)} />
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
                  <span class="hint">{t("online.error.variant")}</span>
                {/if}
                <button onclick={() => answer(c.id, "decline")} disabled={busy}>{t("online.decline")}</button>
              {:else}
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
          <div class="player"><span>{game.names[seats[0]]}</span><span class="clock" class:running={game.running === seats[0]}>{clock(game.clocks[seats[0]])}</span></div>
          <div class="board-wrap">
            <Board state={st} family={family.id} flipped={game.seat === 1} selected={sel?.from ?? null} {targets} oncell={cell} onpress={press} />
          </div>
          <div class="player"><span>{game.names[seats[1]]}</span><span class="clock" class:running={game.running === seats[1]}>{clock(game.clocks[seats[1]])}</span></div>
        {:else}
          <p>{t("online.no_rules", { variant: game.variantName })}</p>
          <button class="link" onclick={() => api.openUrl(game.url)}>{t("online.open_on_lichess")}</button>
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
          <p class="hint" role="status">{t("online.opponent_gone")}</p>
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
            {#if game.entries.length < 2}
              <button onclick={() => act("abort")} disabled={busy}>{t("online.abort")}</button>
            {/if}
            <button onclick={() => act("draw")} disabled={busy || game.offering[0]}>{game.offering[0] ? t("online.draw_sent") : t("online.offer_draw")}</button>
            <button onclick={() => act("takeback")} disabled={busy || game.offering[1] || !game.entries.length}>{game.offering[1] ? t("online.takeback_sent") : t("online.ask_takeback")}</button>
            <button onclick={() => act("resign")} disabled={busy}>{t("online.resign")}</button>
          {/if}
          <button class="link" onclick={() => api.openUrl(game.url)}>{t("online.open_on_lichess")}</button>
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
            {t("online.chat_rules")}
            <button class="link" onclick={() => api.openUrl("https://lichess.org/terms-of-service")}>{t("online.lichess_terms")}</button>
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
  .clock {
    font-variant-numeric: tabular-nums;
    padding: 0.1rem 0.5rem;
    border-radius: var(--r-sm);
    background: var(--alt);
    border: 1px solid var(--border);
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
