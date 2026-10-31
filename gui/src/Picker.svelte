<script>
  // A choice from a list, in place of a <select>: the chosen item's whole name on the button,
  // the list in a popover below it with every name in full (wrapped, never cut), a search field
  // when the list is long, a hint under an item and as its tooltip, and the keys a list box has
  // (arrows, Home, End, Enter, Escape, and typing to search).
  import { t } from "./i18n.svelte.js";

  /** @typedef {{ value: any, label: string, hint?: string, group?: string, disabled?: boolean }} Choice */
  /** @type {{ value: any, choices: Choice[], label: string, onchange: (value: any) => void, disabled?: boolean, id?: string }} */
  let { value, choices, label, onchange, disabled = false, id = "" } = $props();

  const own = `picker-${Math.random().toString(36).slice(2, 9)}`;
  const uid = $derived(id || own);
  let open = $state(false);
  let query = $state("");
  let active = $state(0);
  /** @type {HTMLElement | undefined} */
  let root = $state();
  /** @type {HTMLButtonElement | undefined} */
  let button = $state();
  /** @type {HTMLInputElement | undefined} */
  let search = $state();
  /** @type {HTMLUListElement | undefined} */
  let list = $state();
  /** The popover opens upwards when there is more room above. */
  let up = $state(false);

  // A search field once the list is longer than a glance takes in.
  const searchable = $derived(choices.length > 8);
  const shown = $derived(query.trim() ? choices.filter((c) => (c.label + " " + (c.hint ?? "") + " " + (c.group ?? "")).toLowerCase().includes(query.trim().toLowerCase())) : choices);
  const current = $derived(choices.find((c) => c.value === value));

  function show() {
    if (disabled) return;
    const r = button?.getBoundingClientRect();
    up = !!r && r.top > window.innerHeight - r.bottom && window.innerHeight - r.bottom < 260;
    query = "";
    open = true;
    active = Math.max(0, shown.findIndex((c) => c.value === value));
    queueMicrotask(() => {
      (searchable ? search : list)?.focus();
      scrollTo(active);
    });
  }

  function hide(/** @type {boolean} */ refocus = true) {
    open = false;
    if (refocus) button?.focus();
  }

  /** `key`: chosen with the keyboard, which goes back to the button; a click leaves no ring. */
  function pick(/** @type {Choice | undefined} */ c, key = false) {
    if (!c || c.disabled) return;
    hide(key);
    if (c.value !== value) onchange(c.value);
  }

  function scrollTo(/** @type {number} */ i) {
    list?.querySelector(`#${uid}-o${i}`)?.scrollIntoView({ block: "nearest" });
  }

  function move(/** @type {number} */ to) {
    if (!shown.length) return;
    active = Math.max(0, Math.min(shown.length - 1, to));
    scrollTo(active);
  }

  function key(/** @type {KeyboardEvent} */ e) {
    const step = { ArrowDown: 1, ArrowUp: -1, PageDown: 8, PageUp: -8 }[e.key];
    if (step) move(active + step);
    else if (e.key === "Home" && !searchable) move(0);
    else if (e.key === "End" && !searchable) move(shown.length - 1);
    else if (e.key === "Enter") pick(shown[active], true);
    else if (e.key === "Escape") hide();
    else if (e.key === "Tab") return hide(false);
    else if (!searchable && e.key.length === 1) {
      // Typing in a short list goes to the first item starting with that letter.
      const i = shown.findIndex((c, j) => j > active && c.label.toLowerCase().startsWith(e.key.toLowerCase()));
      const k = i >= 0 ? i : shown.findIndex((c) => c.label.toLowerCase().startsWith(e.key.toLowerCase()));
      if (k >= 0) move(k);
      else return;
    } else return;
    e.preventDefault();
    e.stopPropagation();
  }

  function buttonKey(/** @type {KeyboardEvent} */ e) {
    if (["ArrowDown", "ArrowUp", "Enter", " "].includes(e.key)) {
      e.preventDefault();
      show();
    }
  }

  $effect(() => {
    if (!open) return;
    const away = (/** @type {Event} */ e) => root && !root.contains(/** @type {Node} */ (e.target)) && hide(false);
    document.addEventListener("pointerdown", away, true);
    return () => document.removeEventListener("pointerdown", away, true);
  });
</script>

<div class="picker" bind:this={root}>
  <button
    bind:this={button}
    type="button"
    class="current"
    id={uid}
    {disabled}
    aria-label={label}
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-controls="{uid}-list"
    title={current ? [current.label, current.hint].filter(Boolean).join(" — ") : label}
    onclick={() => (open ? hide() : show())}
    onkeydown={buttonKey}
  >
    <span class="text">{current?.label ?? "—"}</span>
    <span class="caret" aria-hidden="true">▾</span>
  </button>
  {#if open}
    <div class="pop panel" class:up>
      {#if searchable}
        <input bind:this={search} type="search" bind:value={query} oninput={() => (active = 0)} placeholder={t("common.search")} aria-label={t("common.search")} aria-controls="{uid}-list" aria-activedescendant={shown.length ? `${uid}-o${active}` : undefined} onkeydown={key} />
      {/if}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <ul bind:this={list} id="{uid}-list" role="listbox" aria-label={label} tabindex={searchable ? -1 : 0} aria-activedescendant={shown.length ? `${uid}-o${active}` : undefined} onkeydown={key}>
        {#each shown as c, i}
          {#if c.group && c.group !== shown[i - 1]?.group}
            <li class="group" role="presentation">{c.group}</li>
          {/if}
          <li
            id="{uid}-o{i}"
            role="option"
            aria-selected={c.value === value}
            aria-disabled={c.disabled || undefined}
            class:active={i === active}
            class:chosen={c.value === value}
            class:off={c.disabled}
            title={c.hint || c.label}
            onpointerenter={() => (active = i)}
            onclick={() => pick(c)}
            onkeydown={() => {}}
          >
            <span>{c.label}</span>
            {#if c.hint}<small>{c.hint}</small>{/if}
          </li>
        {:else}
          <li class="none" role="presentation">{t("common.no_match")}</li>
        {/each}
      </ul>
    </div>
  {/if}
</div>

<style>
  .picker {
    position: relative;
    display: inline-block;
    max-width: 100%;
    vertical-align: middle;
  }
  .current {
    width: 100%;
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    max-width: 100%;
    min-width: 6rem;
    text-align: start;
    justify-content: space-between;
  }
  .text {
    white-space: normal;
    overflow-wrap: anywhere;
  }
  .caret {
    flex: none;
    opacity: 0.7;
  }
  .pop {
    position: absolute;
    z-index: 30;
    inset-inline-start: 0;
    top: calc(100% + 2px);
    min-width: 100%;
    width: max-content;
    max-width: min(32rem, 92vw);
    padding: 0.3rem;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.25);
  }
  .pop.up {
    top: auto;
    bottom: calc(100% + 2px);
  }
  .pop input {
    width: 100%;
    box-sizing: border-box;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: min(20rem, 60vh);
    overflow: auto;
    outline: none;
  }
  li {
    padding: 0.35rem 0.5rem;
    border-radius: 3px;
    cursor: pointer;
    white-space: normal;
    overflow-wrap: anywhere;
    display: flex;
    flex-direction: column;
  }
  li small {
    color: var(--dim);
  }
  li.active {
    background: var(--alt);
    outline: 2px solid var(--accent, currentColor);
    outline-offset: -2px;
  }
  li.chosen {
    font-weight: 600;
  }
  li.off {
    opacity: 0.55;
    cursor: default;
  }
  li.group,
  li.none {
    cursor: default;
    font-size: 0.85em;
    color: var(--dim);
    padding-top: 0.5rem;
  }
</style>
