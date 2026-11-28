// The app's flow as a pure state machine (behaviour 13): the screens render `state.step`
// and call next(state, event). No I/O here; app.js talks to the Rust side.
//
// source → dest → details → planning → (days → planning) → preview → copying → result
//   → erasing → erased → (undoing) → done
// After a move (same disk, ADR 0006): result → (undoing) → done, no erase question.

export const DEFAULT_PREFIX = "txo";
const DEFAULT_PRESET = "classique";

/** The first screen, pre-filled with what was remembered from the last copy. */
export function start(settings = {}) {
  return {
    step: "source",
    source: "",
    dest: settings.dest || "",
    shoot: "",
    prefix: settings.prefix || DEFAULT_PREFIX,
    preset: settings.preset || DEFAULT_PRESET,
    date: { kind: "files" },
    days: [],
    entries: [],
    // The preview's choice: "copy", or "move" when the destination is on the same disk.
    movable: false,
    copySize: "",
    copyFits: true,
    mode: "copy",
    progress: null,
    outcome: null,
    eraseOutcome: null,
    canErase: false,
    canUndo: false,
    error: "",
    // "options" or "info" when that panel is open, over whatever step.
    panel: null,
    checkUpdates: settings.check_updates !== false,
    // ADR 0007: the newer release found ({ kind: "install" | "link", version, page }),
    // what the last manual check or install said, and an install under way.
    update: null,
    updateMessage: "",
    updating: false,
  };
}

/** The state after `event`; an event that doesn't fit the current step changes nothing. */
export function next(s, e) {
  switch (e.type) {
    case "source":
      return s.step === "source" ? { ...s, source: e.path, step: "dest", error: "" } : s;
    case "dest":
      return s.step === "dest" ? { ...s, dest: e.path, step: "details", error: "" } : s;
    case "details":
      if (s.step !== "details") return s;
      if (e.date && !isRealDate(e.date)) return { ...s, error: "invalid_date" };
      return {
        ...s,
        shoot: e.shoot,
        prefix: e.prefix || DEFAULT_PREFIX,
        preset: e.preset || s.preset,
        date: e.date ? { kind: "given", day: e.date } : { kind: "files" },
        step: "planning",
        error: "",
      };
    case "planned":
      if (s.step !== "planning") return s;
      switch (e.result.kind) {
        case "plan": {
          const { entries, movable = false, copy_size = "", copy_fits = true } = e.result;
          const mode = movable ? "move" : "copy";
          return { ...s, entries, movable, copySize: copy_size, copyFits: copy_fits, mode, step: "preview" };
        }
        case "days":
          return { ...s, days: e.result.days, step: "days" };
        default:
          return { ...s, error: e.result.message, step: "details" };
      }
    case "chooseDay": {
      if (s.step !== "days") return s;
      const date =
        e.choice === "one"
          ? { kind: "given", day: e.day }
          : e.choice === "each"
            ? { kind: "each" }
            : { kind: "none" };
      return { ...s, date, step: "planning" };
    }
    case "mode": {
      if (s.step !== "preview") return s;
      const allowed = e.mode === "move" ? s.movable : s.copyFits;
      return allowed ? { ...s, mode: e.mode } : s;
    }
    // An update that installs restarts the app: nothing starts that it would cut short.
    case "confirm":
      return s.step === "preview" && !s.updating
        ? {
            ...s,
            step: "copying",
            progress: {
              files_done: 0,
              files_total: s.entries.filter((e) => !e.already).length,
              bytes_done: 0,
              bytes_total: 0,
              text: "",
            },
          }
        : s;
    case "progress":
      return s.step === "copying" ? { ...s, progress: e.line } : s;
    case "copied": {
      if (s.step !== "copying") return s;
      const moved = e.outcome.moved || 0;
      return {
        ...s,
        outcome: e.outcome,
        step: "result",
        // On the same disk erasing is refused (one exemplar left): never asked.
        canErase: !s.movable && e.outcome.failed.length === 0 && e.outcome.copied > 0,
        canUndo: moved > 0,
      };
    }
    case "erase":
      if (s.step !== "result" || s.updating) return s;
      if (!e.yes || !s.canErase) return { ...s, step: "done" };
      return { ...s, step: "erasing" };
    case "erased":
      return s.step === "erasing"
        ? { ...s, eraseOutcome: e.outcome, step: "erased", canUndo: e.outcome.done > 0 }
        : s;
    case "undo":
      return (s.step === "erased" || s.step === "result") && s.canUndo && !s.updating
        ? { ...s, step: "undoing" }
        : s;
    case "undone":
      return s.step === "undoing" ? { ...s, eraseOutcome: e.outcome, step: "done" } : s;
    case "panel":
      return { ...s, panel: s.panel === e.name ? null : e.name };
    case "checkUpdates":
      return { ...s, checkUpdates: e.on };
    case "updateChecked":
      if (e.offer) return { ...s, update: e.offer, updateMessage: "" };
      return e.manual ? { ...s, update: null, updateMessage: "up_to_date" } : s;
    case "updateFailed":
      // A startup check that fails shows nothing (no network is no news).
      return e.manual ? { ...s, updateMessage: e.message } : s;
    case "install":
      return canUpdate(s) ? { ...s, updating: true, updateMessage: "" } : s;
    case "installFailed":
      return { ...s, updating: false, updateMessage: e.message };
    case "back":
      return back(s);
    case "restart":
      return {
        ...start({ dest: s.dest, prefix: s.prefix, preset: s.preset, check_updates: s.checkUpdates }),
        update: s.update,
        updating: s.updating,
      };
    default:
      return s;
  }
}

/** `YYYY-MM-DD` naming a day that exists (2024-02-29 yes, 2023-02-29 no). */
function isRealDate(text) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(text)) return false;
  const d = new Date(`${text}T00:00:00Z`);
  return !Number.isNaN(d.getTime()) && d.toISOString().slice(0, 10) === text;
}

function back(s) {
  const previous = { dest: "source", details: "dest", days: "details", preview: "details" };
  const step = previous[s.step];
  return step ? { ...s, step, error: "" } : s;
}

/** What the Rust side's `plan` and `apply` commands take. */
export function planRequest(s) {
  return {
    source: s.source,
    dest: s.dest,
    shoot: s.shoot,
    prefix: s.prefix,
    date: s.date,
    preset: s.preset,
  };
}

/**
 * The Rust command a transition starts: once, when its step begins. An event that leaves
 * the step as it was (a second Ctrl+Z during an undo) starts nothing.
 */
export function effect(before, after) {
  if (before.step === after.step) return null;
  switch (after.step) {
    case "planning":
      return "plan";
    case "copying":
      return "apply";
    case "erasing":
      return "erase";
    case "undoing":
      return after.outcome && after.outcome.moved ? "undo_move" : "undo";
    default:
      return null;
  }
}

/** Whether the banner's "update and restart" button works now (ADR 0007). */
export function canUpdate(s) {
  const busy = ["copying", "erasing", "undoing"].includes(s.step);
  return Boolean(s.update) && s.update.kind === "install" && !busy && !s.updating;
}
