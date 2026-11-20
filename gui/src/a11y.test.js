import { test } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { ask, mount } from "./confirm.js";
import { tabKeys } from "./tabs.js";

const src = (/** @type {string} */ f) => readFileSync(new URL(f, import.meta.url), "utf8");
const pages = readdirSync(new URL(".", import.meta.url)).filter((f) => /\.(svelte|js)$/.test(f) && !f.endsWith(".test.js"));

test("no page uses the browser's own confirm(): it is outside the Theme and the UI check", () => {
  for (const f of pages) assert.ok(!/(^|[^.\w])confirm\(/m.test(src(f)), f);
});

test("every action that deletes for good asks first", () => {
  for (const [f, call] of [
    ["Peers.svelte", "api.peerUnpair("],
    ["StorePanel.svelte", "storePrune"],
    ["UrTables.svelte", "api.urTableDelete("],
    ["Sources.svelte", "sourceRemove"],
  ]) {
    const s = src(f);
    assert.ok(s.includes(call), `${f} calls ${call}`);
    assert.ok(/await ask\(/.test(s), `${f} asks before ${call}`);
  }
});

test("ask() says no until a dialog is mounted, then answers what the user chose", async () => {
  assert.equal(await ask("Delete?", "Delete"), false);
  /** @type {any[]} */
  const seen = [];
  const unmount = mount((text, yes) => (seen.push([text, yes]), Promise.resolve(true)));
  assert.equal(await ask("Delete?", "Delete"), true);
  assert.deepEqual(seen, [["Delete?", "Delete"]]);
  unmount();
  assert.equal(await ask("Delete?", "Delete"), false);
});

/** A tablist of three fake tabs. */
function tablist() {
  /** @type {string[]} */
  const log = [];
  const tabs = ["a", "b", "c"].map((id) => ({ id, focus: () => log.push(`focus ${id}`), click: () => log.push(`click ${id}`) }));
  return { log, tabs, list: { querySelectorAll: () => tabs } };
}
const key = (/** @type {any} */ t, /** @type {string} */ k) => {
  const e = { key: k, target: t.tabs[0], currentTarget: t.list, prevented: false, preventDefault() { this.prevented = true; } };
  tabKeys(/** @type {any} */ (e));
  return e.prevented;
};

test("arrow keys, Home and End move between tabs and show the one reached", () => {
  let t = tablist();
  assert.equal(key(t, "ArrowRight"), true);
  assert.deepEqual(t.log, ["focus b", "click b"]);
  t = tablist();
  key(t, "ArrowLeft");
  assert.deepEqual(t.log, ["focus c", "click c"], "Left from the first wraps to the last");
  t = tablist();
  key(t, "End");
  assert.deepEqual(t.log, ["focus c", "click c"]);
  t = tablist();
  assert.equal(key(t, "Enter"), false);
  assert.deepEqual(t.log, [], "other keys are left alone");
});

test("every tablist has its keys, its panel and one tab in the Tab order", () => {
  for (const f of pages.filter((f) => f.endsWith(".svelte") && src(f).includes('role="tablist"'))) {
    const s = src(f);
    assert.ok(/role="tablist"[^>]*onkeydown=\{tabKeys\}/.test(s), `${f}: arrow keys`);
    assert.ok(/role="tabpanel"/.test(s), `${f}: a tabpanel`);
    assert.ok(/aria-controls=/.test(s), `${f}: aria-controls`);
    assert.ok(/role="tab"[^>]*tabindex=/.test(s), `${f}: roving tabindex`);
  }
});
