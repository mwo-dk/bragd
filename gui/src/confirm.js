// Asks in the page before something that cannot be undone, in the app's own dialog
// (`Confirm.svelte`, mounted once by App), never the browser's own confirm dialog.

/** @type {((text: string, yes: string) => Promise<boolean>) | null} */
let shown = null;

/** The dialog that asks; returns what takes it away again. */
export function mount(/** @type {(text: string, yes: string) => Promise<boolean>} */ f) {
  shown = f;
  return () => {
    if (shown === f) shown = null;
  };
}

/** True when the user agrees to `text` with the button `yes`; false when they cancel. */
export const ask = (/** @type {string} */ text, /** @type {string} */ yes) => (shown ? shown(text, yes) : Promise.resolve(false));
