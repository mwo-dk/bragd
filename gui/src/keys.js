// Play's keys: which action a key press asks for. The action itself is Play's own button, so a
// key does nothing a click could not.

/**
 * The action `e` asks for on Play: "new", "undo", "flip", "throw" or "pass"; null for any other
 * key, a key typed into a field, a held key, or one already handled.
 * @param {{key: string, ctrlKey?: boolean, metaKey?: boolean, altKey?: boolean, shiftKey?: boolean, repeat?: boolean, target?: any, defaultPrevented?: boolean}} e
 * @returns {string | null}
 */
export function playKey(e) {
  if (e.defaultPrevented || e.repeat || e.altKey) return null;
  const target = e.target;
  if (target) {
    const tag = target.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || target.isContentEditable) return null;
    // An open list takes letters to find its items.
    if (target.closest?.("[role=listbox]")) return null;
  }

  const withCtrl = e.ctrlKey || e.metaKey;
  if (e.shiftKey && withCtrl) return null;

  const key = e.key.toLowerCase();
  if (withCtrl) {
    if (key === "n") return "new";
    if (key === "z") return "undo";
    return null;
  }
  if (key === "f") return "flip";
  if (key === "t") return "throw";
  if (key === "p") return "pass";
  return null;
}
