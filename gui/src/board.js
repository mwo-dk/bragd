// Where things go on the screen, and which of Rust's legal moves a click picks. No rule is
// worked out here: the moves, their ends and the cells all come from bragd-games.

/** The column and row (from the top left) of cell (x, y), counted from the first Side's bottom left. */
export function place(/** @type {number} */ x, /** @type {number} */ y, /** @type {number} */ files, /** @type {number} */ ranks, /** @type {boolean} */ flipped, sideways = false) {
  // On its side (a long race track on a narrow window): files run down, the first Side on the left.
  if (sideways) return flipped ? { col: ranks - 1 - y, row: files - 1 - x } : { col: y, row: x };
  return flipped ? { col: files - 1 - x, row: y } : { col: x, row: ranks - 1 - y };
}

/** Whether a race Family's long track is drawn on its side: on a narrow window, and only on the
 * grid board. Backgammon draws its own board, which keeps its width. */
export function sideways(/** @type {{id: string, race: boolean}} */ family, /** @type {{files: number, ranks: number}} */ board, /** @type {number} */ width) {
  return width <= 760 && family.race && family.id !== "backgammon" && board.files >= 4 * board.ranks;
}

/**
 * The moves a selection can make: from a square, or a piece dropped from hand. With a gate
 * (a Seirawan piece chosen in the tray), only the moves from its square that bring it in.
 * Backgammon's board moves one checker at a time: its `steps`, not its whole turns.
 * @param {{moves: any[], steps?: any[]}} state
 * @param {{from?: string, drop?: string, gate?: string} | null} sel
 */
export function movesOf(state, sel) {
  if (!sel) return [];
  if (state.steps?.length) return state.steps.filter((m) => m.from === sel.from);
  if (sel.drop) return state.moves.filter((m) => m.drop === sel.drop && !m.from);
  if (sel.gate && sel.from) return state.moves.filter((m) => m.from === sel.from && m.drop === sel.gate);
  if (sel.gate) return [];
  return state.moves.filter((m) => m.from === sel.from);
}

/** The moves of a selection that end on `to`: one to play, or several to choose from. */
export function movesTo(/** @type {{moves: any[], steps?: any[]}} */ state, /** @type {{from?: string, drop?: string} | null} */ sel, /** @type {string} */ to) {
  return movesOf(state, sel).filter((m) => m.to === to);
}

/** The moves with no square at all: pass, double, take, drop, accept, resume. */
/**
 * The move list's entries in groups that wrap as one: a numbered entry and the unnumbered ones
 * after it, as `1. e4 e5` or Ur's `3 2: 5-7`. Each group holds indexes into `numbers`.
 * @param {string[]} numbers
 * @returns {number[][]}
 */
export const groups = (numbers) =>
  numbers.reduce((/** @type {number[][]} */ g, n, i) => (n || !g.length ? g.push([i]) : g[g.length - 1].push(i), g), []);

export const plainMoves = (/** @type {{moves: any[]}} */ state) => state.moves.filter((m) => !m.from && !m.to && !m.drop);

/**
 * Whether a piece in hand can gate now (Seirawan): some move from a square brings it in.
 * @param {{moves: any[]}} state
 * @param {string} sprite
 * @returns {boolean}
 */
export function gates(state, sprite) {
  return state.moves.some((m) => m.drop === sprite && m.from);
}

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

/**
 * Where the board's keyboard focus goes from the cell `at` on `key`: the nearest cell that way
 * on the screen (a straight neighbour before a diagonal one), the first or last cell for Home
 * and End, null at the edge or for any other key. An unknown `at` starts at the first cell.
 * @param {{square: string, col: number, row: number}[]} cells
 * @param {string} at
 * @param {string} key
 * @returns {string | null}
 */
export function nextCell(cells, at, key) {
  if (!cells.length) return null;
  if (key === "Home") return cells[0].square;
  if (key === "End") return cells[cells.length - 1].square;

  /** The way an arrow moves on the screen, as [columns, rows]. */
  const way = /** @type {Record<string, number[]>} */ ({ ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] })[key];
  if (!way) return null;
  const from = cells.find((c) => c.square === at);
  if (!from) return cells[0].square;
  const [dx, dy] = way;
  let best = null;
  let bestScore = Infinity;
  let bestSide = Infinity;
  for (const cell of cells) {
    const [x, y] = [cell.col - from.col, cell.row - from.row];
    const primary = x * dx + y * dy;
    if (primary <= 0) continue;
    const side = Math.abs(x * dy + y * dx);
    const score = primary + 2 * side;
    if (score < bestScore || (score === bestScore && side < bestSide)) [best, bestScore, bestSide] = [cell, score, side];
  }
  return best ? best.square : null;
}
