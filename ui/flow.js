// The app's flow as a pure state machine (behaviour 13): the screens render `state.step`
// and call next(state, event). No I/O here; app.js talks to the Rust side.
//
// source → dest → details → planning → (days → planning) → preview → copying → result
//   → erasing → erased → (undoing) → done

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
    progress: null,
    outcome: null,
    eraseOutcome: null,
    canErase: false,
    canUndo: false,
    error: "",
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
        case "plan":
          return { ...s, entries: e.result.entries, step: "preview" };
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
    case "confirm":
      return s.step === "preview"
        ? { ...s, step: "copying", progress: { done: 0, total: s.entries.length } }
        : s;
    case "progress":
      return s.step === "copying" ? { ...s, progress: { done: e.done, total: e.total } } : s;
    case "copied":
      if (s.step !== "copying") return s;
      return {
        ...s,
        outcome: e.outcome,
        step: "result",
        canErase: e.outcome.failed.length === 0 && e.outcome.copied > 0,
      };
    case "erase":
      if (s.step !== "result") return s;
      if (!e.yes || !s.canErase) return { ...s, step: "done" };
      return { ...s, step: "erasing" };
    case "erased":
      return s.step === "erasing"
        ? { ...s, eraseOutcome: e.outcome, step: "erased", canUndo: e.outcome.done > 0 }
        : s;
    case "undo":
      return s.step === "erased" && s.canUndo ? { ...s, step: "undoing" } : s;
    case "undone":
      return s.step === "undoing" ? { ...s, eraseOutcome: e.outcome, step: "done" } : s;
    case "back":
      return back(s);
    case "restart":
      return { ...start({ dest: s.dest, prefix: s.prefix, preset: s.preset }) };
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
