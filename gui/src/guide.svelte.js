// What the wizard, the tour cards and the notice area share: whether the wizard or
// cards are open, and the notices to show.
import * as api from "./api.js";

export const guide = $state({
  wizard: false,
  /** The wizard opened by itself, on a first start: no choice of Families is made yet. */
  first: false,
  /** Tour cards still to show (bragd_core::tour::View). @type {any[]} */
  cards: [],
  /** @type {any[]} */
  notices: [],
  /** The Tip on screen (bragd_core::tips::Tip), or null. @type {any} */
  tip: null,
  /** A Game is under way on screen: no Tip shows. */
  live: false,
});

/**
 * An action of Play ran, by its key or from its button. Rust counts it and, once the button alone
 * ran it three times, hands back a hint naming the key; it shows where no notice waits.
 * @param {string} action "new", "undo", "flip", "throw" or "pass"
 * @param {boolean} byKey
 */
export function actionRan(action, byKey) {
  const free = !guide.notices.length && !guide.wizard && !guide.cards.length;
  api.actionRan(action, byKey, free).then(
    (/** @type {any} */ n) => n && !guide.notices.some((x) => x.id === n.id) && (guide.notices = [...guide.notices, n]),
    () => {},
  );
}
