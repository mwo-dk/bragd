import { test } from "node:test";
import assert from "node:assert/strict";
import { playKey } from "./keys.js";

const key = (key, more = {}) => ({ key, target: { tagName: "BODY" }, ...more });

test("Play's keys name their action (#301)", () => {
  assert.equal(playKey(key("n", { ctrlKey: true })), "new");
  assert.equal(playKey(key("z", { ctrlKey: true })), "undo");
  assert.equal(playKey(key("f")), "flip");
  assert.equal(playKey(key("t")), "throw");
  assert.equal(playKey(key("p")), "pass");
  assert.equal(playKey(key("F")), "flip", "Caps Lock or Shift still flips");
  assert.equal(playKey(key("n")), null, "a plain N is no key");
  assert.equal(playKey(key("f", { ctrlKey: true })), null);
  assert.equal(playKey(key("t", { altKey: true })), null);
  assert.equal(playKey(key("z", { ctrlKey: true, shiftKey: true })), null);
});

test("Play's keys leave typing and handled keys alone", () => {
  assert.equal(playKey(key("f", { target: { tagName: "INPUT" } })), null);
  assert.equal(playKey(key("z", { ctrlKey: true, target: { tagName: "TEXTAREA" } })), null);
  assert.equal(playKey(key("p", { target: { tagName: "DIV", isContentEditable: true } })), null);
  assert.equal(playKey(key("t", { defaultPrevented: true })), null);
  assert.equal(playKey(key("f", { target: { tagName: "UL", closest: (s) => s === "[role=listbox]" } })), null, "an open list finds by letter");
  assert.equal(playKey(key("f", { repeat: true })), null, "a held key acts once");
});

test("a board keeps one tab stop: the last focus, the selection, or the first cell (#300)", async () => {
  const { stop } = await import("./rove.js");
  assert.equal(stop(["a", "b", "c"], "b", "c"), "b");
  assert.equal(stop(["a", "b", "c"], "gone", "c"), "c", "a cell that left the board gives way");
  assert.equal(stop(["a", "b", "c"], null, null), "a");
  assert.equal(stop([], null, null), null);
});
