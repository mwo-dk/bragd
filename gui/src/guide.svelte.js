// What the wizard, the tour cards, the notice area and Settings share: whether the wizard or
// cards are open, the notices to show, and the Settings area a link asks for.
export const guide = $state({
  wizard: false,
  /** The wizard opened by itself, on a first start: no choice of Families is made yet. */
  first: false,
  /** Tour cards still to show (bragd_core::tour::View). @type {any[]} */
  cards: [],
  /** @type {any[]} */
  notices: [],
  /** The Settings area to open next, or null. @type {string|null} */
  area: null,
  /** The Tip on screen (bragd_core::tips::Tip), or null. @type {any} */
  tip: null,
  /** A Game is under way on screen: no Tip shows. */
  live: false,
});
