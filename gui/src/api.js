// The Tauri commands of bragd-gui (gui/src-tauri/src/main.rs). Every rule, text, setting and
// colour comes through here; the page only draws and passes input back.
import { invoke } from "@tauri-apps/api/core";

/** @returns {Promise<any>} */
export const snapshot = () => invoke("get_snapshot");
/** @returns {Promise<any>} */
export const saveSetting = (/** @type {string} */ name, /** @type {any} */ value) => invoke("save_setting", { name, value });
/** Turns a Family that is off back on; the new snapshot. @returns {Promise<any>} */
export const familyTurnOn = (/** @type {string} */ family) => invoke("family_turn_on", { family });
/** A new Game: its start (a Chess960 array, picked by Rust) and its state. @returns {Promise<{initial: string|null, array: number|null, state: any}>} */
export const newGame = (/** @type {string} */ variant) => invoke("game_new", { variant });
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
/** Only the Family `family` shows: the wizard's *Only chess*. @returns {Promise<any>} */
export const familiesOnly = (/** @type {string} */ family) => invoke("families_only", { family });
/** The Tip due now, if any, counted as shown; `busy` when a live Game or the wizard is on screen. @returns {Promise<any|null>} */
export const tipNext = (/** @type {boolean} */ busy) => invoke("tip_next", { busy });
/** *Don't suggest this again* for the Tip `id`. */
export const tipSilence = (/** @type {string} */ id) => invoke("tip_silence", { id });
/** @returns {Promise<[string, any[]][]>} */
export const whatsNew = () => invoke("whats_new");
/** @returns {Promise<any[]>} */
export const noticeList = (/** @type {boolean} */ start) => invoke("notice_list", { start });
/** Closes a notice; for a hint, the next hint due comes back. @returns {Promise<any>} */
export const noticeDismiss = (/** @type {string} */ id) => invoke("notice_dismiss", { id });
/** @returns {Promise<any[]>} */
export const updateCheck = () => invoke("update_check");
export const openConfig = () => invoke("open_config");
export const suggestTranslation = (/** @type {string} */ shown) => invoke("suggest_translation", { shown });
/** A path chosen with the file picker for a field: "engine", "book" or "tablebases"; null when cancelled. @returns {Promise<string|null>} */
export const pickPath = (/** @type {string} */ what) => invoke("pick_path", { what });
export const openUrl = (/** @type {string} */ url) => invoke("open_url", { url });

// Engines, Analyse, Tablebases and Opening books (gui/src-tauri/src/engine_commands.rs).
/** @typedef {{variant: string, initial: string|null, entries: string[]}} Game */
/** @returns {Promise<any>} */
export const enginesPage = (/** @type {string} */ familyKey) => invoke("engines_page", { familyKey });
/** @returns {Promise<void>} */
export const engineDefault = (/** @type {string} */ variant, /** @type {string} */ engine) => invoke("engine_default", { variant, engine });
/** @returns {Promise<void>} */
export const engineAdd = (/** @type {string} */ familyKey, /** @type {string} */ path) => invoke("engine_add", { familyKey, path });
/** @returns {Promise<void>} */
export const engineRemove = (/** @type {string} */ key) => invoke("engine_remove", { key });
/** @returns {Promise<any>} */
export const engineOptions = (/** @type {string} */ key, /** @type {string} */ variant) => invoke("engine_options", { key, variant });
/** @returns {Promise<any>} */
export const engineSetOption = (/** @type {string} */ key, /** @type {string} */ name, /** @type {any} */ value) =>
  invoke("engine_set_option", { key, name, value });
/** @returns {Promise<any>} */
export const engineResetOptions = (/** @type {string} */ key) => invoke("engine_reset_options", { key });
/** @returns {Promise<void>} */
export const enginePull = (/** @type {string} */ key) => invoke("engine_pull", { key });
export const engineNetworkDownload = (/** @type {string} */ key) => invoke("engine_network_download", { key });
/** Engine matches of a Family: what a match can be set up with, and every match so far (gui/src-tauri/src/matches.rs). @returns {Promise<any>} */
export const matchesPage = (/** @type {string} */ familyKey) => invoke("matches_page", { familyKey });
/** Starts a match in the Helper: its job id. @returns {Promise<number>} */
export const matchStart = (/** @type {any} */ setup) => invoke("match_start", { setup });
/** Asks for an opening book or a file of opening lines. @returns {Promise<string|null>} */
export const matchBookPick = () => invoke("match_book_pick");
/** The engines there to run for a Variant, and whether more could be got. @returns {Promise<{engines: {key: string, name: string}[], default: string|null, more: boolean, sides: string[]}>} */
export const engineChoices = (/** @type {string} */ variant) => invoke("engine_choices", { variant });
/** @returns {Promise<{entry: string, notation: string}>} */
export const engineMove = (/** @type {Game} */ g, /** @type {string|null} */ engine, /** @type {number} */ ms, /** @type {number|null} */ skill) =>
  invoke("engine_move", { ...g, engine, ms, skill });
/** @returns {Promise<void>} */
export const analysisStart = (/** @type {Game} */ g, /** @type {string|null} */ engine, /** @type {number} */ lines) =>
  invoke("analysis_start", { ...g, engine, lines });
/** @returns {Promise<any>} */
export const analysisNow = () => invoke("analysis_now");
/** @returns {Promise<void>} */
export const analysisStop = () => invoke("analysis_stop");
/** @returns {Promise<void>} */
export const reviewStart = (/** @type {Game} */ g, /** @type {string|null} */ engine) => invoke("review_start", { ...g, engine });
/** @returns {Promise<any>} */
export const reviewNow = () => invoke("review_now");
/** @returns {Promise<void>} */
export const reviewStop = () => invoke("review_stop");
/** @returns {Promise<any>} */
export const positionKnown = (/** @type {Game} */ g) => invoke("position_known", { ...g });
/** @returns {Promise<any>} */
export const tablebaseProbe = (/** @type {Game} */ g) => invoke("tablebase_probe", { ...g });
/** @returns {Promise<any>} */
export const positionSetup = (/** @type {string} */ variant, /** @type {string} */ text) => invoke("position_setup", { variant, text });
/** @returns {Promise<any>} */
export const tablebasesPage = () => invoke("tablebases_page");
/** @returns {Promise<number>} */
export const tablebasePlan = (/** @type {string} */ set) => invoke("tablebase_plan", { set });
/** @returns {Promise<void>} */
export const tablebaseDownload = (/** @type {string} */ set) => invoke("tablebase_download", { set });
/** @returns {Promise<void>} */
export const tablebaseStop = () => invoke("tablebase_stop");
/** @returns {Promise<any[]>} */
export const booksPage = () => invoke("books_page");
/** @returns {Promise<void>} */
export const bookAdd = (/** @type {string} */ path) => invoke("book_add", { path });
/** @returns {Promise<void>} */
export const bookRemove = (/** @type {string} */ path) => invoke("book_remove", { path });

// Clubs (gui/src-tauri/src/clubs.rs).
/** The Club files known here, the Club open and the rating lists. @returns {Promise<any>} */
export const clubsPage = () => invoke("clubs_page");
/** @returns {Promise<any>} */
export const clubOpen = (/** @type {string} */ path) => invoke("club_open", { path });
/** A Club file chosen with the file dialog; null when none was. @returns {Promise<any>} */
export const clubPick = () => invoke("club_pick");
/** @returns {Promise<any>} */
export const clubCreate = (/** @type {string} */ name, /** @type {string} */ system, /** @type {string} */ director) => invoke("club_create", { name, system, director });
export const clubClose = () => invoke("club_close");
/** Takes a Club off the list; its file stays. @returns {Promise<any>} */
export const clubForget = (/** @type {string} */ path) => invoke("club_forget", { path });
/** @returns {Promise<any>} */
export const clubAdd = (/** @type {number|null} */ by, /** @type {any} */ details, /** @type {number|null} */ rating) => invoke("club_add", { by, details, rating });
/** @returns {Promise<any>} */
export const clubUpdate = (/** @type {number|null} */ by, /** @type {number} */ id, /** @type {any} */ details) => invoke("club_update", { by, id, details });
/** @returns {Promise<any>} */
export const clubRemove = (/** @type {number|null} */ by, /** @type {number} */ id) => invoke("club_remove", { by, id });
/** @returns {Promise<any>} */
export const clubRating = (/** @type {number|null} */ by, /** @type {number} */ id, /** @type {number} */ rating, /** @type {string} */ why) => invoke("club_rating", { by, id, rating, why });
/** @returns {Promise<any>} */
export const clubSystem = (/** @type {number|null} */ by, /** @type {string} */ system, /** @type {string} */ why) => invoke("club_system", { by, system, why });
/** @returns {Promise<any>} */
export const clubTakeOver = (/** @type {number|null} */ by) => invoke("club_take_over", { by });
/** Players in the rating lists by name or ID. @returns {Promise<any[]>} */
export const ratingSearch = (/** @type {string} */ query) => invoke("rating_search", { query });

// Tournaments of the open Club (gui/src-tauri/src/tournaments.rs). Each change gives the
// Tournament's view back; creating, removing and importing give the Club's.
/** @typedef {number|null} By */
/** @returns {Promise<any>} */
export const tournamentNew = (
  /** @type {By} */ by,
  /** @type {string} */ name,
  /** @type {any} */ system,
  /** @type {number} */ rounds,
  /** @type {string} */ date,
  /** @type {string} */ place,
  /** @type {string} */ variant,
  /** @type {any} */ handicap,
  /** @type {number|null} */ matchTo,
) => invoke("tournament_new", { by, name, system, rounds, date, place, variant, handicap, matchTo });
/** The Variants a Tournament can be played in. @returns {Promise<any[]>} */
export const tournamentVariants = () => invoke("tournament_variants");
/** @returns {Promise<any>} */
export const tournamentView = (/** @type {number} */ id) => invoke("tournament_view", { id });
/** @returns {Promise<any>} */
export const tournamentRemove = (/** @type {By} */ by, /** @type {number} */ id) => invoke("tournament_remove", { by, id });
/** @returns {Promise<any>} */
export const tournamentRegister = (/** @type {By} */ by, /** @type {number} */ id, /** @type {any} */ player, /** @type {boolean} */ here) => invoke("tournament_register", { by, id, player, here });
/** @returns {Promise<any>} */
export const tournamentStatus = (/** @type {By} */ by, /** @type {number} */ id, /** @type {number} */ player, /** @type {string} */ status) => invoke("tournament_status", { by, id, player, status });
/** @returns {Promise<any>} */
export const tournamentCheckInAll = (/** @type {By} */ by, /** @type {number} */ id) => invoke("tournament_check_in_all", { by, id });
/** @returns {Promise<any>} */
export const tournamentPair = (/** @type {By} */ by, /** @type {number} */ id) => invoke("tournament_pair", { by, id });
/** @returns {Promise<any>} */
export const tournamentPairAgain = (/** @type {By} */ by, /** @type {number} */ id, /** @type {string} */ why) => invoke("tournament_pair_again", { by, id, why });
/** The warnings a pairing made by hand raises, in words. @returns {Promise<string[]>} */
export const tournamentCheck = (/** @type {number} */ id, /** @type {any[]} */ pairs, /** @type {number|null} */ bye) => invoke("tournament_check", { id, pairs, bye });
/** @returns {Promise<any>} */
export const tournamentSetPairing = (/** @type {By} */ by, /** @type {number} */ id, /** @type {any[]} */ pairs, /** @type {number|null} */ bye, /** @type {string} */ why) =>
  invoke("tournament_set_pairing", { by, id, pairs, bye, why });
/** @returns {Promise<any>} */
export const tournamentResult = (/** @type {By} */ by, /** @type {number} */ id, /** @type {number} */ round, /** @type {number} */ board, /** @type {string|null} */ result, /** @type {string} */ why) =>
  invoke("tournament_result", { by, id, round, board, result, why });
/** @returns {Promise<any>} */
export const tournamentTiebreaks = (/** @type {By} */ by, /** @type {number} */ id, /** @type {string[]} */ codes) => invoke("tournament_tiebreaks", { by, id, codes });
/** Rates the Club's members from the Tournament's games; the Club's view. @returns {Promise<any>} */
export const tournamentRate = (/** @type {By} */ by, /** @type {number} */ id) => invoke("tournament_rate", { by, id });
/** Saves `trf`, `pgn`, `html` or `bundle`; the path, or null. @returns {Promise<string|null>} */
export const tournamentExport = (/** @type {number} */ id, /** @type {string} */ kind) => invoke("tournament_export", { id, kind });
/** Adds a bundle's Tournament to the Club; the Club's view, or null. @returns {Promise<any>} */
export const tournamentImport = (/** @type {By} */ by) => invoke("tournament_import", { by });
/** Shows a Round's pairings in a window of their own. */
export const tournamentShow = (/** @type {number} */ id, /** @type {number} */ round) => invoke("tournament_show", { id, round });

// Team Leagues of the open Club (gui/src-tauri/src/leagues.rs). Each change gives the League's
// view back; creating and removing give the Club's.
/** @returns {Promise<any>} */
export const leagueNew = (/** @type {By} */ by, /** @type {string} */ name, /** @type {number} */ boards, /** @type {boolean} */ double, /** @type {string} */ date) =>
  invoke("league_new", { by, name, boards, double, date });
/** @returns {Promise<any>} */
export const leagueView = (/** @type {number} */ id) => invoke("league_view", { id });
/** @returns {Promise<any>} */
export const leagueRemove = (/** @type {By} */ by, /** @type {number} */ id) => invoke("league_remove", { by, id });
/** @returns {Promise<any>} */
export const leagueAddTeam = (/** @type {By} */ by, /** @type {number} */ id, /** @type {string} */ name, /** @type {any[]} */ players) => invoke("league_add_team", { by, id, name, players });
/** @returns {Promise<any>} */
export const leaguePair = (/** @type {By} */ by, /** @type {number} */ id) => invoke("league_pair", { by, id });
/** @returns {Promise<any>} */
export const leagueLineUp = (/** @type {By} */ by, /** @type {number} */ id, /** @type {number} */ round, /** @type {number} */ m, /** @type {boolean} */ away, /** @type {(number|null)[]} */ players) =>
  invoke("league_line_up", { by, id, round, m, away, players });
/** @returns {Promise<any>} */
export const leagueResult = (
  /** @type {By} */ by,
  /** @type {number} */ id,
  /** @type {number} */ round,
  /** @type {number} */ m,
  /** @type {number} */ board,
  /** @type {string|null} */ result,
  /** @type {string} */ why,
) => invoke("league_result", { by, id, round, m, board, result, why });
