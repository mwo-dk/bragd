<script>
  // The app's one confirmation: a modal dialog in the Theme, so the keyboard stays in it,
  // Escape cancels and the focus goes back where it was. Cancel has the focus, so Enter is safe.
  import { onMount } from "svelte";
  import { mount } from "./confirm.js";
  import { t } from "./i18n.svelte.js";

  const uid = $props.id();
  /** @type {HTMLDialogElement} */
  let dlg;
  /** @type {HTMLButtonElement} */
  let cancel;
  let text = $state("");
  let yes = $state("");
  /** @type {((v: boolean) => void) | null} */
  let done = null;

  function close(/** @type {boolean} */ v) {
    if (dlg.open) dlg.close();
    done?.(v);
    done = null;
  }
  onMount(() =>
    mount((q, y) => {
      close(false);
      text = q;
      yes = y;
      dlg.showModal();
      cancel.focus();
      return new Promise((r) => (done = r));
    }),
  );
</script>

<dialog bind:this={dlg} class="panel" role="alertdialog" aria-labelledby="{uid}-q" oncancel={(e) => (e.preventDefault(), close(false))}>
  <p id="{uid}-q">{text}</p>
  <div class="row">
    <button class="primary" onclick={() => close(true)}>{yes}</button>
    <button bind:this={cancel} onclick={() => close(false)}>{t("common.cancel")}</button>
  </div>
</dialog>

<style>
  dialog {
    color: var(--fg);
    max-width: min(30rem, 90vw);
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.3);
  }
  p {
    margin: 0 0 0.6rem;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }
</style>
