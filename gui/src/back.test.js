import { test } from "node:test";
import assert from "node:assert/strict";
import { wantsBack } from "./back.js";

const key = (key, more = {}) => ({ type: "keydown", key, target: { tagName: "BODY" }, ...more });

test("Alt+Left and the mouse's back button go back", () => {
  assert.equal(wantsBack(key("ArrowLeft", { altKey: true }), "settings", false), true);
  assert.equal(wantsBack(key("ArrowLeft"), "settings", false), false, "a plain arrow moves on a board");
  assert.equal(wantsBack({ type: "mouseup", button: 3 }, "item", false), true);
  assert.equal(wantsBack({ type: "mouseup", button: 0 }, "item", false), false);
});

test("Escape leaves F1 only", () => {
  assert.equal(wantsBack(key("Escape"), "help", false), true);
  assert.equal(wantsBack(key("Escape"), "settings", false), false);
  assert.equal(wantsBack(key("Escape", { target: { tagName: "INPUT", value: "eng" } }), "help", false), false, "Escape clears the search first");
  assert.equal(wantsBack(key("Escape", { target: { tagName: "INPUT", value: "" } }), "help", false), true);
});

test("an open wizard or dialog keeps its keys, and a field keeps Alt+Left", () => {
  assert.equal(wantsBack(key("Escape"), "help", true), false);
  assert.equal(wantsBack({ type: "mouseup", button: 3 }, "item", true), false);
  assert.equal(wantsBack(key("ArrowLeft", { altKey: true, target: { tagName: "TEXTAREA" } }), "item", false), false);
  assert.equal(wantsBack(key("Escape", { defaultPrevented: true }), "help", false), false);
});
