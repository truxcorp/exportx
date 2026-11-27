// The screens: render the flow's state (flow.js) and turn clicks into events. The only
// file that talks to Tauri.
import { start, next, planRequest } from "./flow.js";
import { t } from "./i18n.js";

const tauri = window.__TAURI__;
const invoke = tauri.core.invoke;

let lang = "en";
let state = start({});
const root = document.getElementById("app");

async function init() {
  lang = await invoke("lang");
  state = start(await invoke("settings"));
  // Drag and drop a folder onto the window on the source and destination steps.
  await tauri.event.listen("tauri://drag-drop", (e) => {
    const path = e.payload.paths && e.payload.paths[0];
    if (path && (state.step === "source" || state.step === "dest")) {
      send({ type: state.step, path });
    }
  });
  render();
}

/** Apply an event, run what the new step needs, re-render. */
async function send(event) {
  state = next(state, event);
  render();
  try {
    if (state.step === "planning") {
      const result = await invoke("plan", { req: planRequest(state), lang });
      send({ type: "planned", result });
    } else if (state.step === "copying" && event.type === "confirm") {
      const progress = new tauri.core.Channel();
      progress.onmessage = ([done, total]) => send({ type: "progress", done, total });
      const outcome = await invoke("apply", {
        req: planRequest(state),
        expected: state.entries,
        lang,
        progress,
      });
      send({ type: "copied", outcome });
    } else if (state.step === "erasing") {
      send({ type: "erased", outcome: await invoke("erase", { shoots: state.outcome.shoots }) });
    } else if (state.step === "undoing") {
      send({ type: "undone", outcome: await invoke("undo", { shoots: state.outcome.shoots }) });
    }
  } catch (err) {
    const message = String(err);
    if (state.step === "erasing" || state.step === "undoing") {
      // Keep the result: its folders and copies are still there.
      const outcome = { done: 0, kept: [["", message]] };
      send({ type: state.step === "erasing" ? "erased" : "undone", outcome });
    } else {
      // A plan or copy that failed outright (e.g. the card changed): back to the details.
      state = { ...state, step: "details", error: message };
      render();
    }
  }
}

async function pickFolder(step) {
  const path = await tauri.dialog.open({ directory: true, multiple: false });
  if (path) send({ type: step, path });
}

// --- rendering --------------------------------------------------------------------

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k.startsWith("on")) node.addEventListener(k.slice(2), v);
    else if (v !== false && v != null) node.setAttribute(k, v === true ? "" : v);
  }
  for (const c of children.flat()) node.append(c instanceof Node ? c : String(c));
  return node;
}

const button = (key, onclick, primary = false, vars = {}) =>
  el("button", { class: primary ? "primary" : "", onclick }, t(lang, key, vars));

function render() {
  const header = el(
    "header",
    {},
    el("h1", {}, t(lang, "title")),
    el(
      "button",
      {
        class: "link",
        onclick: async () => {
          lang = lang === "fr" ? "en" : "fr";
          await invoke("set_lang", { lang });
          render();
        },
      },
      t(lang, "lang_switch"),
    ),
  );
  root.replaceChildren(header, screen());
}

function folderStep(step, titleKey, helpKey, value) {
  return el(
    "section",
    { class: "drop" },
    el("h2", {}, t(lang, titleKey)),
    el("p", {}, t(lang, helpKey)),
    value ? el("p", { class: "path" }, value) : "",
    el(
      "div",
      { class: "actions" },
      step === "dest" ? button("back", () => send({ type: "back" })) : "",
      button("choose_folder", () => pickFolder(step), !value),
      value && step === "dest" ? button("next", () => send({ type: "dest", path: value }), true) : "",
    ),
  );
}

function screen() {
  switch (state.step) {
    case "source":
      return folderStep("source", "source_title", "source_help", "");
    case "dest":
      return folderStep("dest", "dest_title", "dest_help", state.dest);
    case "details":
      return details();
    case "planning":
      return el("section", {}, el("p", { class: "busy" }, t(lang, "planning")));
    case "days":
      return days();
    case "preview":
      return preview();
    case "copying": {
      const p = state.progress || { done: 0, total: 0 };
      return el(
        "section",
        {},
        el("p", { class: "busy" }, t(lang, "copying", p)),
        el("progress", { value: p.done, max: p.total || 1 }),
      );
    }
    case "result":
      return result();
    case "erasing":
      return el("section", {}, el("p", { class: "busy" }, t(lang, "erasing")));
    case "erased":
      return erased();
    case "undoing":
      return el("section", {}, el("p", { class: "busy" }, t(lang, "undoing")));
    default:
      return done();
  }
}

function details() {
  const shoot = el("input", { id: "shoot", value: state.shoot, required: true, autofocus: true });
  const prefix = el("input", { id: "prefix", value: state.prefix });
  const date = el("input", { id: "date", type: "date", value: state.date.day || "" });
  const preset = el(
    "select",
    { id: "preset" },
    ["classique", "lisible"].map((p) =>
      el("option", { value: p, selected: p === state.preset }, t(lang, `preset_${p}`)),
    ),
  );
  const submit = (e) => {
    e.preventDefault();
    send({
      type: "details",
      shoot: shoot.value,
      prefix: prefix.value,
      date: date.value,
      preset: preset.value,
    });
  };
  return el(
    "form",
    { onsubmit: submit },
    el("h2", {}, t(lang, "details_title")),
    state.error ? el("p", { class: "error" }, state.error) : "",
    el("label", { for: "shoot" }, t(lang, "shoot_label")),
    shoot,
    el("label", { for: "prefix" }, t(lang, "prefix_label")),
    prefix,
    el("small", {}, t(lang, "prefix_help", { prefix: state.prefix })),
    el("label", { for: "date" }, t(lang, "date_label")),
    date,
    el("small", {}, t(lang, "date_help")),
    el("label", { for: "preset" }, t(lang, "preset_label")),
    preset,
    el(
      "div",
      { class: "actions" },
      button("back", () => send({ type: "back" })),
      el("button", { class: "primary", type: "submit" }, t(lang, "next")),
    ),
  );
}

function days() {
  return el(
    "section",
    {},
    el("h2", {}, t(lang, "days_title")),
    el("ul", {}, state.days.map(([day, n]) => el("li", {}, t(lang, "day_count", { day, n })))),
    el(
      "div",
      { class: "choices" },
      state.days.map(([day]) =>
        button("day_one", () => send({ type: "chooseDay", choice: "one", day }), false, { day }),
      ),
      button("day_each", () => send({ type: "chooseDay", choice: "each" }), true),
      button("day_none", () => send({ type: "chooseDay", choice: "none" })),
    ),
    el("div", { class: "actions" }, button("back", () => send({ type: "back" }))),
  );
}

function preview() {
  return el(
    "section",
    {},
    el("h2", {}, t(lang, "preview_title")),
    el("p", {}, t(lang, "preview_count", { n: state.entries.length, dest: state.dest })),
    el("p", { class: "safe" }, t(lang, "preview_safe")),
    el(
      "table",
      { class: "preview" },
      el(
        "tbody",
        {},
        state.entries.map((e) => el("tr", {}, el("td", {}, e.name), el("td", {}, "→"), el("td", {}, e.dest))),
      ),
    ),
    el(
      "div",
      { class: "actions" },
      button("back", () => send({ type: "back" })),
      button("copy", () => send({ type: "confirm" }), true),
    ),
  );
}

function issues(key, list) {
  if (!list.length) return "";
  return el(
    "div",
    { class: "error" },
    el("p", {}, t(lang, key, { n: list.length })),
    el("ul", {}, list.map(([what, why]) => el("li", {}, lang === "fr" ? `${what} : ${why}` : `${what}: ${why}`))),
  );
}

function openButtons() {
  return state.outcome.shoots.map((path) =>
    el("button", { onclick: () => invoke("open_folder", { path }) }, `${t(lang, "open_folder")} — ${path}`),
  );
}

function result() {
  const o = state.outcome;
  return el(
    "section",
    {},
    el("h2", {}, t(lang, "result_title")),
    el("p", {}, t(lang, "copied", { n: o.copied })),
    issues("failed", o.failed),
    el("div", { class: "actions" }, openButtons()),
    state.canErase
      ? el(
          "div",
          { class: "question" },
          el("p", {}, t(lang, "erase_question")),
          button("erase_no", () => send({ type: "erase", yes: false })),
          button("erase_yes", () => send({ type: "erase", yes: true }), true),
        )
      : el("div", { class: "actions" }, button("restart", () => send({ type: "restart" }), true)),
  );
}

function erased() {
  const o = state.eraseOutcome;
  return el(
    "section",
    {},
    el("h2", {}, t(lang, "result_title")),
    el("p", {}, t(lang, "erased", { n: o.done })),
    issues("kept", o.kept),
    el(
      "div",
      { class: "actions" },
      state.canUndo ? button("undo", () => send({ type: "undo" })) : "",
      button("restart", () => send({ type: "restart" }), true),
    ),
  );
}

function done() {
  const o = state.eraseOutcome;
  return el(
    "section",
    {},
    el("h2", {}, t(lang, "result_title")),
    state.outcome ? el("p", {}, t(lang, "copied", { n: state.outcome.copied })) : "",
    o ? el("p", {}, t(lang, "restored", { n: o.done })) : "",
    o ? issues("kept", o.kept) : "",
    el("div", { class: "actions" }, state.outcome ? openButtons() : "", button("restart", () => send({ type: "restart" }), true)),
  );
}

init();
