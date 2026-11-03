// The keyboard on a board: one tab stop, the arrows move the focus between its cells as they lie
// on the screen, and Enter or Space picks the cell, as a click does. Which move that makes is
// Rust's: this only moves a focus and passes the cell on.
import { nextCell } from "./board.js";

/**
 * A key pressed on a board's cell. `cells` are the cells with their place on the screen.
 * @param {KeyboardEvent} e
 * @param {{square: string, col: number, row: number}[]} cells
 * @param {(square: string) => void} oncell
 */
export function boardKey(e, cells, oncell) {
  const el = /** @type {SVGElement} */ (e.target);
  const at = el.dataset?.square ?? "";
  if (e.altKey || e.ctrlKey || e.metaKey) return;
  if (e.key === "Enter" || e.key === " ") {
    if (at) e.preventDefault(), oncell(at);
    return;
  }
  const to = nextCell(cells, at, e.key);
  if (!to) return;
  e.preventDefault();
  /** @type {SVGElement | null} */ (el.closest("svg")?.querySelector(`[data-square="${to}"]`) ?? null)?.focus();
}

/** The cell that holds the board's one tab stop: where the focus last was, else the selection,
 * else the first cell. */
export function stop(/** @type {string[]} */ squares, /** @type {string | null} */ cursor, /** @type {string | null} */ selected) {
  if (cursor && squares.includes(cursor)) return cursor;
  if (selected && squares.includes(selected)) return selected;
  return squares[0] ?? null;
}
