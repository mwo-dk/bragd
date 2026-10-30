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
/** @returns {Promise<{wizard: boolean, cards: any[]}>} */
export const guideDue = () => invoke("guide_due");
/** @returns {Promise<any>} */
export const wizardOpen = () => invoke("wizard_open");
/** @returns {Promise<{totals: any, summary: string[]}>} */
export const wizardChosen = (/** @type {string[]} */ chosen) => invoke("wizard_chosen", { chosen });
/** @returns {Promise<string[]>} */
export const wizardDone = (/** @type {string[]} */ chosen) => invoke("wizard_done", { chosen });
export const wizardSeen = () => invoke("wizard_seen");
export const tourSeen = () => invoke("tour_seen");
/** @returns {Promise<[string, any[]][]>} */
export const whatsNew = () => invoke("whats_new");
/** @returns {Promise<any[]>} */
export const noticeList = (/** @type {boolean} */ start) => invoke("notice_list", { start });
export const noticeDismiss = (/** @type {string} */ id) => invoke("notice_dismiss", { id });
/** @returns {Promise<any[]>} */
export const updateCheck = () => invoke("update_check");
export const openConfig = () => invoke("open_config");
export const suggestTranslation = (/** @type {string} */ shown) => invoke("suggest_translation", { shown });
export const openUrl = (/** @type {string} */ url) => invoke("open_url", { url });
