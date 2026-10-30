// The Tauri commands of Knowledge (gui/src-tauri/src/knowledge.rs): the Helper and its jobs,
// Foreign databases, Libraries, Slices, Sources and Disk use. Every text and value comes from
// Rust; the pages only draw them and send back what the user chose.
import { invoke } from "@tauri-apps/api/core";

/** @typedef {Record<string, any>} Obj */

/** @returns {Promise<any>} */
export const helperStatus = () => invoke("helper_status");
/** @returns {Promise<any>} */
export const helperRetry = () => invoke("helper_retry");
export const jobCancel = (/** @type {number} */ id) => invoke("job_cancel", { id });

/** @returns {Promise<any>} */
export const dbPick = () => invoke("db_pick");
/** @returns {Promise<any>} */
/** @returns {Promise<any>} */
export const dbCurrent = () => invoke("db_current");
export const dbClose = () => invoke("db_close");
/** @returns {Promise<any>} */
export const dbRows = (/** @type {Obj} */ query, /** @type {number} */ from, /** @type {number} */ count) => invoke("db_rows", { query, from, count });
/** @returns {Promise<any>} */
export const dbGame = (/** @type {number} */ id) => invoke("db_game", { id });
/** @returns {Promise<number>} */
export const dbImport = () => invoke("db_import");
/** @returns {Promise<any>} */
export const dbSaveAs = () => invoke("db_save_as");
/** @returns {Promise<any>} */
export const dbSaveTo = () => invoke("db_save_to");

/** @returns {Promise<any[]>} */
export const libraries = () => invoke("libraries_list");
/** @returns {Promise<any>} */
export const libraryRows = (/** @type {string} */ path, /** @type {Obj} */ query, /** @type {number} */ from, /** @type {number} */ count) =>
  invoke("library_rows", { path, query, from, count });
/** @returns {Promise<any>} */
export const libraryGame = (/** @type {string} */ path, /** @type {number} */ id) => invoke("library_game", { path, id });
/** @returns {Promise<any>} */
export const libraryReview = (/** @type {string} */ path) => invoke("library_review", { path });
/** @returns {Promise<number>} */
export const libraryDedupe = (/** @type {string} */ path) => invoke("library_dedupe", { path });
/** @returns {Promise<number>} */
export const libraryPrune = (/** @type {boolean} */ dryRun) => invoke("library_prune", { dryRun });
/** @returns {Promise<string[]>} */
export const priorityGet = () => invoke("priority_get");
/** @returns {Promise<string[]>} */
export const prioritySave = (/** @type {string[]} */ list) => invoke("priority_save", { list });

/** @returns {Promise<Record<string, any[]>>} */
export const slices = () => invoke("slices_list");
export const sliceSave = (/** @type {string} */ library, /** @type {string} */ name, /** @type {Obj} */ query) => invoke("slice_save", { library, name, query });
/** @returns {Promise<string>} */
export const sliceMaterialise = (/** @type {string} */ library, /** @type {string} */ name) => invoke("slice_materialise", { library, name });
/** @returns {Promise<string>} */
export const positionKey = (/** @type {string} */ family, /** @type {string} */ fen) => invoke("position_key", { family, fen });

/** @returns {Promise<any>} */
export const sources = () => invoke("sources_list");
export const sourceSet = (/** @type {string} */ id, /** @type {Obj} */ options, /** @type {string | null} */ diskCap = null) =>
  invoke("source_set", { id, options, diskCap });
export const sourceAdd = (/** @type {string} */ name, /** @type {string} */ url) => invoke("source_add", { name, url });
/** @returns {Promise<number>} */
export const sourceFetch = (/** @type {string} */ id) => invoke("source_fetch", { id });
/** @returns {Promise<number>} */
export const sourceStrip = (/** @type {string} */ id, /** @type {boolean} */ dryRun) => invoke("source_strip", { id, dryRun });

/** @returns {Promise<any>} */
export const diskReport = () => invoke("disk_report");
export const diskLimitsSet = (/** @type {string} */ total, /** @type {string} */ freeBelow, /** @type {Record<string, string>} */ items) =>
  invoke("disk_limits_set", { total, freeBelow, items });

/** An error from a command, as text. */
export const why = (/** @type {any} */ e) => String(e?.reason ?? e);
