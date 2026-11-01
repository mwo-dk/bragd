<script>
  // A Slice: made or changed, its Games shown live as the fields change, and kept as a separate
  // copy on request. "Use current board position" takes the position of the Game in replay.
  import * as k from "./knowledge.js";
  import { t } from "./i18n.svelte.js";
  import Picker from "./Picker.svelte";
  import GameList from "./GameList.svelte";
  import Replay from "./Replay.svelte";
  import SliceAnalyse from "./SliceAnalyse.svelte";

  /** @type {{ library: string, slice: any, family: string, variants: {key: string, name: string}[], position: {family: string, fen: string} | null, onstep: (family: string, step: any) => void, onsaved: () => void, onplayer: (name: string) => void }} */
  let { library, slice, family, variants, position, onstep, onsaved, onplayer } = $props();

  const blank = () => ({ players: [], eco_from: "", eco_to: "", date_from: "", date_to: "", rating_min: null, result: "", family, variant: "", position: null });
  // svelte-ignore state_referenced_locally
  let name = $state(slice?.name ?? "");
  // svelte-ignore state_referenced_locally
  /** @type {any} */
  let q = $state({ ...blank(), ...(slice?.query ?? {}) });
  let players = $state(q.players.join(", "));
  let message = $state("");
  let error = $state("");
  /** @type {any} */
  let game = $state(null);
  let analysing = $state(false);

  const query = $derived({ ...q, players: players.split(",").map((/** @type {string} */ p) => p.trim()).filter(Boolean), rating_min: q.rating_min || null, family: q.family || null });
  const fetch = (/** @type {number} */ from, /** @type {number} */ count) => k.libraryRows(library, query, from, count);

  async function usePosition() {
    if (!position) return;
    try {
      q.position = await k.positionKey(position.family, position.fen);
    } catch (e) {
      error = k.why(e);
    }
  }

  async function save() {
    error = message = "";
    try {
      await k.sliceSave(library, name, query);
      message = t("slice.saved", { name });
      onsaved();
    } catch (e) {
      error = k.why(e);
    }
  }

  async function keep() {
    error = message = "";
    try {
      await k.sliceSave(library, name, query);
      const path = await k.sliceMaterialise(library, name.trim());
      message = t("slice.kept", { path });
      onsaved();
    } catch (e) {
      error = k.why(e);
    }
  }
</script>

<div class="slice">
  <div class="fields">
    <label>{t("slice.name")} <input type="text" bind:value={name} /></label>
    <label>{t("slice.players")} <input type="text" bind:value={players} placeholder={t("slice.players_hint")} /></label>
    <label>{t("slice.eco_from")} <input type="text" bind:value={q.eco_from} placeholder="D70" size="5" /></label>
    <label>{t("slice.eco_to")} <input type="text" bind:value={q.eco_to} placeholder="D99" size="5" /></label>
    <label>{t("slice.date_from")} <input type="text" bind:value={q.date_from} placeholder="1990" size="10" /></label>
    <label>{t("slice.date_to")} <input type="text" bind:value={q.date_to} placeholder="1999.12" size="10" /></label>
    <label>{t("slice.rating_min")} <input type="number" min="0" step="50" bind:value={q.rating_min} /></label>
    <label
      >{t("slice.result")}
      <Picker value={q.result} choices={[{ value: "", label: t("slice.any") }, ...["1-0", "0-1", "1/2-1/2", "*"].map((r) => ({ value: r, label: r }))]} label={t("slice.result")} onchange={(v) => (q.result = v)} />
    </label>
    <label
      >{t("slice.variant")}
      <Picker value={q.variant} choices={[{ value: "", label: t("slice.any") }, ...variants.map((v) => ({ value: v.key, label: v.name }))]} label={t("slice.variant")} onchange={(v) => (q.variant = v)} />
    </label>
    <div class="position">
      <span>{t("slice.position")}: {q.position ? t("slice.position_set") : t("slice.any")}</span>
      <button onclick={usePosition} disabled={!position} title={position ? "" : t("slice.position_hint")}>{t("slice.use_position")}</button>
      {#if q.position}<button onclick={() => (q.position = null)}>{t("slice.clear")}</button>{/if}
    </div>
  </div>
  <div class="bar">
    <button class="primary" onclick={save} disabled={!name.trim()}>{t("slice.save")}</button>
    <button onclick={keep} disabled={!name.trim()} title={t("slice.keep_hint")}>{t("slice.keep")}</button>
    {#if family === "chess"}
      <button onclick={() => (analysing = !analysing)} disabled={!slice?.name} title={slice?.name ? t("background.action_hint") : t("background.save_first")} aria-expanded={analysing}>{t("background.action")}</button>
    {/if}
  </div>
  {#if analysing && slice?.name}<SliceAnalyse {library} name={slice.name} onclose={() => (analysing = false)} />{/if}
  {#if !position}<p class="hint">{t("slice.position_hint")}</p>{/if}
  {#if message}<p role="status">{message}</p>{/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}

  {#if game}
    <Replay replay={game} onclose={() => (game = null)} onstep={(s) => onstep(game.family, s)} />
  {:else}
    <h3>{t("slice.live")}</h3>
    <GameList {fetch} version={JSON.stringify(query)} {onplayer} onopen={async (r) => (game = await k.libraryGame(library, r.id))} />
  {/if}
</div>

<style>
  .slice {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .fields {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem 1rem;
    align-items: flex-end;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    font-size: 0.9em;
  }
  input[type="number"] {
    width: 6rem;
    font: inherit;
  }
  .position,
  .bar {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
  }
  h3 {
    font-size: 1rem;
    margin: 0.4rem 0 0;
  }
  .hint {
    color: var(--dim);
    font-size: 0.85em;
    margin: 0;
  }
  .error {
    color: var(--red);
  }
</style>
