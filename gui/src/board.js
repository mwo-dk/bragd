// Where things go on the screen, and which of Rust's legal moves a click picks. No rule is
// worked out here: the moves, their ends and the cells all come from bragd-games.

/** The column and row (from the top left) of cell (x, y), counted from the first Side's bottom left. */
export function place(/** @type {number} */ x, /** @type {number} */ y, /** @type {number} */ files, /** @type {number} */ ranks, /** @type {boolean} */ flipped) {
  return flipped ? { col: files - 1 - x, row: y } : { col: x, row: ranks - 1 - y };
}

/**
 * The moves a selection can make: from a square, or a piece dropped from hand.
 * @param {{moves: any[]}} state
 * @param {{from?: string, drop?: string} | null} sel
 */
export function movesOf(state, sel) {
  if (!sel) return [];
  if (sel.drop) return state.moves.filter((m) => m.drop === sel.drop && !m.from);
  return state.moves.filter((m) => m.from === sel.from);
}

/** The moves of a selection that end on `to`: one to play, or several to choose from. */
export function movesTo(/** @type {{moves: any[]}} */ state, /** @type {{from?: string, drop?: string} | null} */ sel, /** @type {string} */ to) {
  return movesOf(state, sel).filter((m) => m.to === to);
}

/** The moves with no square at all: pass, double, take, drop, accept, resume. */
export const plainMoves = (/** @type {{moves: any[]}} */ state) => state.moves.filter((m) => !m.from && !m.to && !m.drop);

/**
 * A backgammon point's place, from White's side: points 13–24 along the top from the left,
 * 12–1 along the bottom, a bar in the middle. `col` counts 0–12 with the bar at 6.
 */
export function point(/** @type {number} */ n, /** @type {boolean} */ flipped) {
  let top = n >= 13;
  let i = top ? n - 13 : 12 - n; // 0–11 from the left
  if (flipped) (top = !top), (i = 11 - i);
  return { top, col: i < 6 ? i : i + 1 };
}
