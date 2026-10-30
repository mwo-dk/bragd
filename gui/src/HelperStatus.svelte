<script>
  // The Helper in the sidebar's footer: idle, how many jobs, paused, or out of reach, with the
  // running job's progress. A click opens the Jobs page.
  import * as k from "./knowledge.js";
  import { t } from "./i18n.svelte.js";

  /** @type {{ collapsed: boolean, current: boolean, open: () => void }} */
  let { collapsed, current, open } = $props();
  /** @type {any} */
  let s = $state(null);

  $effect(() => {
    let stop = false;
    const tick = async () => {
      try {
        s = await k.helperStatus();
      } catch (e) {
        s = { on: false, line: k.why(e), active: 0 };
      }
      // Quicker while work runs.
      if (!stop) setTimeout(tick, s?.active ? 1500 : 5000);
    };
    tick();
    return () => (stop = true);
  });
</script>

<button class="helper" class:current onclick={open} title={s ? `${t("helper.title")}: ${s.line}` : t("helper.title")} aria-label={t("helper.title")}>
  <span class="dot" class:on={s?.on} class:busy={s?.active > 0} class:warn={s?.notices?.length}></span>
  {#if !collapsed}
    <span class="text">{s ? s.line : t("helper.connecting")}</span>
  {/if}
  {#if s?.progress != null && !collapsed}
    <progress max="1" value={s.progress}></progress>
  {/if}
</button>

<style>
  .helper {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
    text-align: start;
    background: none;
    border: none;
    color: var(--dim);
    font: inherit;
    font-size: 0.85em;
    padding: 0.4rem 0.6rem;
    margin: 0 0.4rem;
    border-radius: var(--r-sm);
    cursor: pointer;
  }
  .helper:hover {
    background: color-mix(in srgb, var(--sel-bg) 35%, transparent);
  }
  .helper.current {
    background: var(--sel-bg);
    color: var(--sel-fg);
  }
  .dot {
    width: 0.6rem;
    height: 0.6rem;
    border-radius: 50%;
    background: var(--dim);
    flex: none;
  }
  .dot.on {
    background: var(--brand-light);
  }
  .dot.busy {
    background: var(--board-highlight);
  }
  .dot.warn {
    background: var(--red);
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  progress {
    width: 100%;
    height: 0.4rem;
  }
</style>
