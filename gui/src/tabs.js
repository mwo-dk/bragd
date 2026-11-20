// The keys of a row of tabs (`role="tablist"`): the arrow keys, Home and End move to a tab and
// show it, as the WAI-ARIA tabs pattern asks. Only the shown tab is in the Tab order.

/** For the tablist's `onkeydown`. */
export function tabKeys(/** @type {KeyboardEvent} */ e) {
  const list = /** @type {HTMLElement} */ (e.currentTarget);
  const tabs = /** @type {HTMLElement[]} */ ([...list.querySelectorAll('[role="tab"]')]);
  const i = tabs.indexOf(/** @type {HTMLElement} */ (e.target));
  const n = tabs.length;
  const to = /** @type {Record<string, number>} */ ({ ArrowRight: i + 1, ArrowLeft: i - 1 + n, Home: 0, End: n - 1 })[e.key];
  if (i < 0 || to === undefined) return;
  e.preventDefault();
  const tab = tabs[to % n];
  tab.focus();
  tab.click();
}
