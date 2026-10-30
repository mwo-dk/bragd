<script>
  // Play against yourself or an Engine, and Analyse: a Game of one Variant of the Family. The
  // record is a list of entries; Rust replays it and hands back the board, the legal moves and
  // the result, and an Engine's move as an entry. A click picks one of those moves; nothing here
  // knows a rule.
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";
  import { movesOf, movesTo, plainMoves } from "./board.js";
  import Board from "./Board.svelte";
  import BackgammonBoard from "./BackgammonBoard.svelte";
  import Analysis from "./Analysis.svelte";
  import Known from "./Known.svelte";
  import { current } from "./current.svelte.js";

  /** @type {{ family: any, mode?: "play" | "analyse" }} */
  let { family, mode = "play" } = $props();
  const analyse = $derived(mode === "analyse");
  /** The review's mark of each entry. @type {string[]} */
  let marks = $state([]);
  /** Who plays the other Side: yourself, or an Engine. */
  let opponent = $state("self");
  /** The Engine's Side: 1 the second, 0 the first. */
  let engineSeat = $state(1);
  let thinkMs = $state(1000);
  let skill = $state(20);
  /** @type {{engines: {key: string, name: string, ready: boolean}[], sides: string[]}} */
  let choices = $state({ engines: [], sides: [] });
  let engineKey = $state("");
  let thinking = $state(false);

  const variants = $derived(family.variants.filter((/** @type {any} */ v) => v.on));
  let key = $state("");
  /** @type {string | null} */
  let initial = $state(null);
  /** @type {string[]} */
  let entries = $state([]);
  /** @type {any} */
  let st = $state(null);
  /** @type {{from?: string, drop?: string} | null} */
  let sel = $state(null);
  /** @type {any[]} */
  let choose = $state([]);
  let error = $state("");
  let flipped = $state(false);
  /** Go: the stones marked dead while scoring. @type {string[]} */
  let dead = $state([]);

  // A new Family, or its chosen Variant turned off: start that Family's first Variant.
  $effect(() => {
    if (!variants.some((/** @type {any} */ v) => v.key === key) && variants.length) newGame(variants[0].key);
  });

  const side = (/** @type {any} */ seat) => t(`side.${seat.name.toLowerCase()}`);
  const targets = $derived(st ? [...new Set(movesOf(st, sel).map((m) => m.to).filter(Boolean))] : []);
  const scoring = $derived(st?.moves.some((/** @type {any} */ m) => m.wire === "dead:"));
  const race = $derived(["backgammon", "ur", "senet"].includes(family.id));

  async function load(/** @type {Promise<any>} */ p) {
    try {
      st = await p;
      error = "";
    } catch (e) {
      error = String(/** @type {any} */ (e)?.reason ?? e);
    }
    sel = null;
    choose = [];
  }

  // The Game played here is the one Analyse can take up.
  $effect(() => {
    if (!analyse && key) current[family.id] = { variant: key, initial, entries: [...entries] };
  });
  // The Engines that can take a Side, for this Variant.
  $effect(() => {
    if (!analyse && key)
      api.engineChoices(key).then((c) => {
        choices = c;
        engineKey = c.default ?? "";
      });
  });

  // On the Engine's turn it throws (backgammon), takes a double offered, or moves; Rust gives
  // its move as an entry.
  $effect(() => {
    if (analyse || opponent !== "engine" || !st || st.outcome || thinking || error) return;
    if (st.toMove?.index !== engineSeat) return;
    thinking = true;
    const g = { variant: key, initial, entries: [...entries] };
    const done = () => (thinking = false);
    if (st.awaiting === "throw") doThrow().finally(done);
    else if (st.moves.some((/** @type {any} */ m) => m.wire === "take")) playEntry("take").finally(done);
    else
      api
        .engineMove(g, engineKey || null, thinkMs, skill)
        .then((m) => playEntry(m.entry))
        .catch((e) => (error = String(e)))
        .finally(done);
  });

  /** A list of entries from Analyse (a line clicked): the Game after them. */
  async function setEntries(/** @type {string[]} */ list) {
    await load(api.replay(key, initial, list));
    if (!error) entries = list;
  }

  /** A set-up position (or the start, with null), and the entries after it. */
  async function setUp(/** @type {string|null} */ text, /** @type {string[]} */ list = []) {
    await load(api.replay(key, text, list));
    if (!error) {
      initial = text;
      entries = list;
      marks = [];
    }
  }

  export function newGame(/** @type {string} */ variant = key) {
    key = variant;
    initial = null;
    entries = [];
    dead = [];
    marks = [];
    load(api.replay(key, null, []));
  }

  async function playEntry(/** @type {string} */ entry) {
    const before = entries;
    await load(api.play(key, initial, before, entry));
    if (!error) entries = [...before, entry];
  }

  async function doThrow() {
    try {
      const r = await api.throwDice(key, initial, entries);
      entries = [...entries, r.entry];
      st = r.state;
      error = "";
    } catch (e) {
      error = String(/** @type {any} */ (e)?.reason ?? e);
    }
  }

  function undo() {
    // Against an Engine, Undo takes back its reply too.
    const n = opponent === "engine" && !analyse && entries.length > 1 ? 2 : 1;
    entries = entries.slice(0, -n);
    dead = [];
    load(api.replay(key, initial, entries));
  }

  function pick(/** @type {any[]} */ moves) {
    if (moves.length === 1) playEntry(moves[0].wire);
    else if (moves.length > 1) choose = moves;
  }

  function cell(/** @type {string} */ square) {
    if (!st || st.awaiting !== "move") return;
    if (scoring) {
      if (st.board.pieces.some((/** @type {any} */ p) => p.square === square))
        dead = dead.includes(square) ? dead.filter((d) => d !== square) : [...dead, square];
      return;
    }
    const here = movesTo(st, sel, square);
    if (sel && here.length) return pick(here);
    // A placement (Go): a move with no start.
    const placed = st.moves.filter((/** @type {any} */ m) => !m.from && !m.drop && m.to === square);
    if (placed.length) return pick(placed);
    sel = movesOf(st, { from: square }).length ? { from: square } : null;
  }

  /** Dragging: press on a piece selects it, letting go over a target plays there. */
  function press(/** @type {string} */ square, /** @type {PointerEvent} */ e) {
    if (!st || scoring || !movesOf(st, { from: square }).length) return;
    sel = { from: square };
    const up = (/** @type {PointerEvent} */ u) => {
      window.removeEventListener("pointerup", up);
      const el = document.elementFromPoint(u.clientX, u.clientY);
      const to = el instanceof Element ? el.getAttribute("data-square") : null;
      if (to && to !== square) {
        const here = movesTo(st, sel, to);
        if (here.length) pick(here);
      }
    };
    window.addEventListener("pointerup", up);
    e.preventDefault();
  }

  function hand(/** @type {any} */ h, /** @type {any} */ piece) {
    if (!st || st.awaiting !== "move") return;
    if (st.moves.some((/** @type {any} */ m) => m.drop === piece.sprite && !m.from)) sel = { drop: piece.sprite };
    else if (h.seat === st.toMove?.index && st.moves.some((/** @type {any} */ m) => m.from === "waiting")) sel = { from: "waiting" };
  }

  /** The text for a move that has no square: Pass, Double, Take, Drop, Accept, Resume. */
  const plainLabel = (/** @type {any} */ m) => {
    const label = t(`move.${m.notation}`);
    return label.startsWith("move.") ? m.notation : label;
  };
  const hands = (/** @type {number} */ seat) => st?.board.hands.filter((/** @type {any} */ h) => h.seat === seat) ?? [];
  const offs = (/** @type {number} */ seat) => st?.board.off.filter((/** @type {any} */ h) => h.seat === seat) ?? [];
  const homeTarget = $derived(targets.find((x) => x === "home" || x === "off"));
</script>

<section class="play">
  <header class="bar">
    <select value={key} onchange={(e) => newGame(e.currentTarget.value)} aria-label={t("play.variant")}>
      {#each variants as v}
        <option value={v.key}>{v.name}</option>
      {/each}
    </select>
    <button class="primary" onclick={() => newGame()}>{t("play.new_game")}</button>
    <button onclick={undo} disabled={!entries.length}>{t("play.undo")}</button>
    <button onclick={() => (flipped = !flipped)} aria-pressed={flipped}>{t("play.flip")}</button>
    {#if !analyse && choices.engines.length}
      <select bind:value={opponent} aria-label={t("play.opponent")}>
        <option value="self">{t("play.against_self")}</option>
        <option value="engine">{t("play.against_engine")}</option>
      </select>
      {#if opponent === "engine"}
        <select bind:value={engineKey} aria-label={t("analyse.engine")}>
          {#each choices.engines as c}
            <option value={c.key}>{c.name}{c.ready ? "" : ` (${t("engines.not_ready")})`}</option>
          {/each}
        </select>
        <select bind:value={engineSeat} aria-label={t("play.engine_side")}>
          {#each choices.sides as side, seat}
            <option value={seat}>{t("play.engine_plays", { side })}</option>
          {/each}
        </select>
        <select bind:value={thinkMs} aria-label={t("play.think_time")}>
          {#each [100, 1000, 3000, 10000] as ms}
            <option value={ms}>{t("play.seconds", { n: ms / 1000 })}</option>
          {/each}
        </select>
        <label class="skill">{t("play.strength")} <input type="range" min="0" max="20" bind:value={skill} /></label>
      {/if}
    {/if}
  </header>

  {#if st}
    <div class="game">
      <div class="board-side">
        {#snippet tray(/** @type {number} */ seat)}
          {#if hands(seat).length || offs(seat).length}
            <div class="tray">
              {#each hands(seat) as h}
                {#each h.pieces as p}
                  <button
                    class="hand"
                    class:current={sel?.drop === p.sprite || (sel?.from === "waiting" && h.seat === st.toMove?.index)}
                    disabled={!p.count}
                    onclick={() => hand(h, p)}
                  >
                    <svg viewBox="0 0 1 1"><use href="#{p.sprite}" width="1" height="1" /></svg>
                    <span>{p.count}</span>
                  </button>
                {/each}
              {/each}
              {#if !race || family.id !== "backgammon"}
                {#each offs(seat) as h}
                  {#each h.pieces as p}
                    <button
                      class="hand off"
                      class:target={homeTarget && h.seat === st.toMove?.index}
                      onclick={() => homeTarget && pick(movesTo(st, sel, homeTarget))}
                      title={t("play.off")}
                    >
                      <svg viewBox="0 0 1 1"><use href="#{p.sprite}" width="1" height="1" /></svg>
                      <span>{t("play.off")}: {p.count}</span>
                    </button>
                  {/each}
                {/each}
              {/if}
            </div>
          {/if}
        {/snippet}

        {@render tray(flipped ? 0 : 1)}
        <div class="board-wrap">
          {#if family.id === "backgammon"}
            <BackgammonBoard state={st} {flipped} selected={sel?.from ?? null} {targets} oncell={cell} onpress={press} />
          {:else}
            <Board state={st} family={family.id} {flipped} selected={sel?.from ?? null} {targets} {dead} oncell={cell} onpress={press} />
          {/if}
          {#if st.outcome}
            <div class="banner" role="status">
              <strong>{st.outcome.winner ? t("play.wins", { side: side(st.outcome.winner) }) : t("play.draw")}</strong>
              <span>{st.outcome.result}</span>
              <button class="primary" onclick={() => newGame()}>{t("play.new_game")}</button>
            </div>
          {/if}
        </div>
        {@render tray(flipped ? 1 : 0)}
      </div>

      <aside class="panel info">
        {#if st.toMove && !st.outcome}
          <p class="turn">
            {st.awaiting === "throw" ? t("play.to_throw", { side: side(st.toMove) }) : t("play.to_move", { side: side(st.toMove) })}
          </p>
        {/if}
        {#if st.board.dice.length && family.id !== "backgammon"}
          <p class="dice">{t("play.thrown", { n: st.board.dice.join(" ") })}</p>
        {/if}

        <div class="actions">
          {#if st.awaiting === "throw"}
            <button class="primary" onclick={doThrow}>{t("play.throw")}</button>
          {/if}
          {#each plainMoves(st) as m}
            {#if m.wire === "dead:"}
              <button onclick={() => playEntry("dead:" + dead.join(","))}>{t("move.dead")}</button>
            {:else}
              <button onclick={() => playEntry(m.wire)}>{plainLabel(m)}</button>
            {/if}
          {/each}
        </div>
        {#if scoring}
          <p class="hint">{t("play.scoring")}</p>
        {/if}

        {#if choose.length}
          <div class="choose">
            <p>{t("play.choose")}</p>
            {#each choose as m}
              <button onclick={() => playEntry(m.wire)}>{m.notation}</button>
            {/each}
            <button onclick={() => (choose = [])}>{t("common.cancel")}</button>
          </div>
        {/if}

        {#if race && st.awaiting === "move" && !choose.length}
          <div class="choose">
            <p>{t("play.legal")}</p>
            {#each st.moves.filter((/** @type {any} */ m) => m.from || m.to) as m}
              <button onclick={() => playEntry(m.wire)}>{m.notation}</button>
            {/each}
          </div>
        {/if}

        {#if thinking}
          <p class="hint" role="status">{t("play.thinking")}</p>
        {/if}
        {#if error}
          <p class="error" role="alert">{error}</p>
        {/if}

        {#if family.id === "chess"}
          <Known game={{ variant: key, initial, entries }} onplay={playEntry} />
        {/if}
        {#if analyse}
          <Analysis {family} game={{ variant: key, initial, entries }} over={!!st.outcome} onentries={setEntries} onsetup={setUp} bind:marks />
        {/if}

        <h2>{t("play.moves")}</h2>
        <ol class="moves">
          {#each st.history as h, i}
            <li>{h}{#if marks[i]}<span class="mark" class:bad={marks[i].startsWith("?")}>{marks[i]}</span>{/if}</li>
          {/each}
        </ol>
      </aside>
    </div>
  {/if}
</section>

<style>
  .play {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    height: 100%;
    min-height: 0;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
  }
  .game {
    display: flex;
    gap: 1rem;
    align-items: flex-start;
    min-height: 0;
  }
  .board-side {
    flex: 1 1 auto;
    min-width: 0;
    max-width: min(100%, calc(100vh - 9rem) * 1.1);
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .board-wrap {
    position: relative;
  }
  .info {
    flex: 0 0 15rem;
    max-height: calc(100vh - 7rem);
    overflow: auto;
  }
  @media (max-width: 760px) {
    .game {
      flex-direction: column;
    }
    .info {
      flex: none;
      width: 100%;
      box-sizing: border-box;
      max-height: none;
    }
    .board-side {
      width: 100%;
      max-width: 100%;
    }
  }
  .tray {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    min-height: 2rem;
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
  .hand.current,
  .hand.target {
    outline: 2px solid var(--board-highlight);
  }
  .banner {
    position: absolute;
    inset: auto 8% 40% 8%;
    display: flex;
    flex-wrap: wrap;
    gap: 0.6rem;
    align-items: center;
    justify-content: center;
    padding: 0.8rem;
    background: var(--raised);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: var(--r);
    box-shadow: var(--shadow);
  }
  .turn {
    font-weight: 600;
    margin-top: 0;
  }
  .actions,
  .choose {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin: 0.4rem 0;
  }
  .choose p {
    width: 100%;
    margin: 0;
    color: var(--dim);
  }
  .hint {
    color: var(--dim);
  }
  .error {
    color: var(--red);
  }
  .mark {
    margin-inline-start: 0.15rem;
    font-weight: 700;
    color: var(--dim);
  }
  .mark.bad {
    color: var(--red);
  }
  .skill {
    display: inline-flex;
    gap: 0.3rem;
    align-items: center;
  }
  .moves {
    columns: 2;
    padding-inline-start: 1.6rem;
    margin: 0;
    font-variant-numeric: tabular-nums;
  }
  h2 {
    font-size: 1rem;
    margin: 0.8rem 0 0.3rem;
  }
</style>
