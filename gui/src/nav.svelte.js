// Where the window is, and Back. The place (page, tab, open item) is `here.view`; pages read
// their tab and open item from it and change them through `set`, so Back can restore them.
// Rust keeps the way back (bragd_core::history); this reports each move and asks it.
import * as api from "./api.js";
import { t } from "./i18n.svelte.js";

export const here = $state({
  /** @type {Record<string, any>} */
  view: { kind: "settings" },
  /** Back has somewhere to go. */
  back: false,
  /** Something was typed on this page since it opened. */
  dirty: false,
});

const main = () => document.querySelector("main");
const place = (/** @type {Record<string, any>} */ view, scroll = 0) => ({ view: $state.snapshot(view), scroll });

/** Go to `view`, a new page. */
export function go(/** @type {Record<string, any>} */ view) {
  const from = place(here.view, main()?.scrollTop ?? 0);
  here.view = view;
  here.dirty = false;
  api.navGo(from, place(view)).then((b) => (here.back = b));
}

/** Change part of the place: a tab, an open item. A null value closes it. */
export function set(/** @type {Record<string, any>} */ changes) {
  const view = { ...here.view, ...changes };
  for (const k in changes) if (changes[k] == null) delete view[k];
  go(view);
}

/** Show `view` without a step back (the place Bragd opens on, or one that went away). */
export const replace = (/** @type {Record<string, any>} */ view) => (here.view = view);

/** Back to where the window was, with its scroll; asks first when typing would be lost. */
export async function back() {
  if (here.dirty && !confirm(t("nav.discard"))) return;
  const r = await api.navBack();
  if (!r) return (here.back = false);
  const [p, more] = r;
  here.view = p.view;
  here.back = more;
  here.dirty = false;
  // shortcut: a page that loads its rows later may need a moment before it is tall enough;
  // tries for about a second, then leaves the scroll where it got to.
  let tries = 60;
  const scroll = () => {
    const m = main();
    if (!m) return;
    m.scrollTop = p.scroll;
    if (m.scrollTop < p.scroll - 1 && --tries) requestAnimationFrame(scroll);
  };
  requestAnimationFrame(scroll);
}
