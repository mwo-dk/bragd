// What the wizard, the tour cards and the notice area share: whether the wizard or
// cards are open, and the notices to show.
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
