import { test } from "node:test";
import assert from "node:assert/strict";
import { place, movesOf, movesTo, plainMoves, point, gates } from "./board.js";

test("a1 is bottom left, and top right when flipped", () => {
  assert.deepEqual(place(0, 0, 8, 8, false), { col: 0, row: 7 });
  assert.deepEqual(place(0, 0, 8, 8, true), { col: 7, row: 0 });
  assert.deepEqual(place(8, 9, 9, 10, false), { col: 8, row: 0 });
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
