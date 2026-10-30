// The Tauri commands of bragd-gui (gui/src-tauri/src/main.rs). Every rule, text, setting and
// colour comes through here; the page only draws and passes input back.
import { invoke } from "@tauri-apps/api/core";

/** @returns {Promise<any>} */
export const snapshot = () => invoke("get_snapshot");
/** @returns {Promise<any>} */
export const saveSetting = (/** @type {string} */ name, /** @type {any} */ value) => invoke("save_setting", { name, value });
/** @returns {Promise<any>} */
export const replay = (/** @type {string} */ variant, /** @type {string|null} */ initial, /** @type {string[]} */ entries) =>
  invoke("game_replay", { variant, initial, entries });
/** @returns {Promise<any>} */
export const play = (/** @type {string} */ variant, /** @type {string|null} */ initial, /** @type {string[]} */ entries, /** @type {string} */ entry) =>
  invoke("game_play", { variant, initial, entries, entry });
/** @returns {Promise<{entry: string, state: any}>} */
export const throwDice = (/** @type {string} */ variant, /** @type {string|null} */ initial, /** @type {string[]} */ entries) =>
  invoke("game_throw", { variant, initial, entries });
/** @returns {Promise<{html: string, positions: any[]}>} */
export const rulesPage = (/** @type {string} */ family) => invoke("rules_page", { family });
/** @returns {Promise<any[]>} */
export const featureList = (/** @type {string} */ query) => invoke("feature_list", { query });
/** @returns {Promise<[string, string, string][]>} */
export const whatsNew = () => invoke("whats_new");
