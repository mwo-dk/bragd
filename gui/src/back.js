// Which input means Back: Alt+←, the mouse's back button, and Escape on F1. Bragd's history is
// in Rust (bragd_core::history); this only reads the key or button.

/**
 * Whether `e` asks to go back. `page` is the page's kind; `overlay` is true while the wizard,
 * tour cards or a dialog are open, which keep their own keys.
 * @param {{type: string, key?: string, altKey?: boolean, button?: number, target?: any, defaultPrevented?: boolean}} e
 * @param {string} page
 * @param {boolean} overlay
 */
export function wantsBack(e, page, overlay) {
  if (overlay || e.defaultPrevented) return false;
  if (e.type === "mouseup") return e.button === 3;
  if (e.type !== "keydown") return false;
  const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(e.target?.tagName ?? "") || e.target?.isContentEditable;
  if (e.key === "ArrowLeft" && e.altKey) return !typing;
  // Escape in a field clears or leaves it first.
  if (e.key === "Escape") return page === "help" && !(typing && e.target.value);
  return false;
}
