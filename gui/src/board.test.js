import { test } from "node:test";
import assert from "node:assert/strict";
import { place, sideways, movesOf, movesTo, plainMoves, point, gates, groups, nextCell } from "./board.js";

test("a move's number keeps the moves after it on its line", () => {
  assert.deepEqual(groups(["1.", "", "2.", ""]), [[0, 1], [2, 3]]);
  assert.deepEqual(groups(["1", "", "2", "3", ""]), [[0, 1], [2], [3, 4]]);
  assert.deepEqual(groups(["", "2."]), [[0], [1]], "a list opening with a reply");
  assert.deepEqual(groups([]), []);
});

test("a1 is bottom left, and top right when flipped", () => {
  assert.deepEqual(place(0, 0, 8, 8, false), { col: 0, row: 7 });
  assert.deepEqual(place(0, 0, 8, 8, true), { col: 7, row: 0 });
  assert.deepEqual(place(8, 9, 9, 10, false), { col: 8, row: 0 });
});

test("a board on its side runs its files down, the first Side on the left", () => {
  assert.deepEqual(place(0, 0, 12, 3, false, true), { col: 0, row: 0 });
  assert.deepEqual(place(11, 2, 12, 3, false, true), { col: 2, row: 11 });
  assert.deepEqual(place(0, 0, 12, 3, true, true), { col: 2, row: 11 });
});

test("a selection picks Rust's moves by their ends", () => {
  const state = {
    moves: [
      { wire: "e7e8q", from: "e7", to: "e8" },
      { wire: "e7e8n", from: "e7", to: "e8" },
      { wire: "P*e5", from: null, to: "e5", drop: "shogi-bP" },
      { wire: "pass", from: null, to: null, drop: null },
    ],
  };
  assert.equal(movesOf(state, { from: "e7" }).length, 2);
  assert.equal(movesTo(state, { from: "e7" }, "e8").length, 2);
  assert.deepEqual(movesTo(state, { drop: "shogi-bP" }, "e5").map((m) => m.wire), ["P*e5"]);
  assert.deepEqual(movesOf(state, null), []);
  assert.deepEqual(plainMoves(state).map((m) => m.wire), ["pass"]);
});

test("a backgammon checker moves by its steps, not by a whole turn's first move (#239)", () => {
  // After 6-5 no whole turn starts 13/8, yet 13/8 is a legal first checker.
  const state = {
    moves: [{ wire: "13/7 13/8", from: "13", to: "7" }],
    steps: [
      { wire: "13/7", from: "13", to: "7" },
      { wire: "13/8", from: "13", to: "8" },
    ],
  };
  assert.deepEqual(movesTo(state, { from: "13" }, "8").map((m) => m.wire), ["13/8"]);
  assert.equal(movesOf(state, { from: "13" }).length, 2);
});

test("backgammon points run round the board", () => {
  assert.deepEqual(point(1, false), { top: false, col: 12 });
  assert.deepEqual(point(12, false), { top: false, col: 0 });
  assert.deepEqual(point(13, false), { top: true, col: 0 });
  assert.deepEqual(point(24, false), { top: true, col: 12 });
  assert.deepEqual(point(7, false), { top: false, col: 5 });
  assert.deepEqual(point(6, false), { top: false, col: 7 });
  assert.deepEqual(point(1, true), { top: true, col: 0 });
});

test("a tray piece chosen first narrows a move to its gating ones", () => {
  const H = "seirawan-wH";
  const state = {
    moves: [
      { wire: "g1f3", from: "g1", to: "f3", drop: null },
      { wire: "g1f3h", from: "g1", to: "f3", drop: H },
      { wire: "g1f3e", from: "g1", to: "f3", drop: "seirawan-wE" },
      { wire: "e1g1h", from: "e1", to: "g1", drop: H },
      { wire: "e1g1H", from: "e1", to: "g1", drop: H },
      { wire: "P*e5", from: null, to: "e5", drop: "shogi-bP" },
    ],
  };
  assert.deepEqual(movesTo(state, { from: "g1", gate: H }, "f3").map((m) => m.wire), ["g1f3h"]);
  assert.equal(movesTo(state, { from: "e1", gate: H }, "g1").length, 2);
  assert.equal(movesTo(state, { from: "g1" }, "f3").length, 3);
  assert.deepEqual(movesOf(state, { gate: H }), []);
  assert.equal(gates(state, H), true);
  assert.equal(gates(state, "shogi-bP"), false);
});

test("only a grid race track lies on its side on a narrow window, never backgammon's board", () => {
  const aseb = { files: 12, ranks: 3 };
  assert.equal(sideways({ id: "ur", race: true }, aseb, 390), true);
  assert.equal(sideways({ id: "ur", race: true }, aseb, 1100), false);
  assert.equal(sideways({ id: "ur", race: true }, { files: 8, ranks: 3 }, 390), false);
  assert.equal(sideways({ id: "backgammon", race: true }, { files: 26, ranks: 2 }, 390), false);
  assert.equal(sideways({ id: "chess", race: false }, { files: 8, ranks: 2 }, 390), false);
});

test("the arrows move the board's focus to the next cell on the screen (#300)", () => {
  const grid = [];
  for (let row = 0; row < 3; row++) for (let col = 0; col < 3; col++) grid.push({ square: `${col}${row}`, col, row });
  assert.equal(nextCell(grid, "11", "ArrowRight"), "21");
  assert.equal(nextCell(grid, "11", "ArrowLeft"), "01");
  assert.equal(nextCell(grid, "11", "ArrowUp"), "10");
  assert.equal(nextCell(grid, "11", "ArrowDown"), "12");
  assert.equal(nextCell(grid, "21", "ArrowRight"), null, "the edge stops it");
  assert.equal(nextCell(grid, "11", "Home"), "00");
  assert.equal(nextCell(grid, "11", "End"), "22");
  assert.equal(nextCell(grid, "11", "Enter"), null);
  // Draughts names only the dark squares: Right skips the light one along the row.
  const dark = [{ square: "a", col: 0, row: 0 }, { square: "b", col: 1, row: 1 }, { square: "c", col: 2, row: 0 }];
  assert.equal(nextCell(dark, "a", "ArrowRight"), "c");
  assert.equal(nextCell(dark, "a", "ArrowDown"), "b", "no cell straight down: the nearest below");
  assert.equal(nextCell(dark, "zz", "ArrowRight"), "a", "an unknown focus starts at the first cell");
});
