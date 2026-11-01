<script>
  // Asks for the reason a change is made, in the page: the log keeps it. A modal dialog, so the
  // keyboard stays in it, Escape cancels, and the focus goes back where it was.
  import { t } from "./i18n.svelte.js";

  const uid = $props.id();
  /** @type {HTMLDialogElement} */
  let dlg;
  let question = $state("");
  let why = $state("");
  /** @type {((v: string|null) => void)|null} */
  let done = null;

  /** The reason typed, or null when the user cancels. */
  export function ask(/** @type {string} */ q) {
    question = q;
    why = "";
    dlg.showModal();
    return new Promise((/** @type {(v: string|null) => void} */ r) => (done = r));
  }
  function close(/** @type {string|null} */ v) {
    dlg.close();
    done?.(v);
    done = null;
  }
</script>

<dialog bind:this={dlg} class="panel" aria-labelledby="{uid}-q" oncancel={(e) => (e.preventDefault(), close(null))}>
  <form onsubmit={(e) => (e.preventDefault(), close(why.trim()))}>
    <label>
      <span id="{uid}-q">{question}</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input type="text" required autofocus bind:value={why} placeholder={t("clubs.why_hint")} />
    </label>
    <div class="row">
      <button class="primary" disabled={!why.trim()}>{t("clubs.save")}</button>
      <button type="button" onclick={() => close(null)}>{t("clubs.cancel")}</button>
    </div>
  </form>
</dialog>

<style>
  dialog {
    color: var(--fg);
    max-width: min(30rem, 90vw);
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.3);
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .row {
    display: flex;
    gap: 0.4rem;
    margin-top: 0.6rem;
  }
</style>
