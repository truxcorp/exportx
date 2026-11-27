// The screens: render the flow's state (flow.js) and turn clicks into events. The only
// file that talks to Tauri.
import { start, next, planRequest, effect, canUpdate, DEFAULT_PREFIX } from "./flow.js";
import { t } from "./i18n.js";

const tauri = window.__TAURI__;
const invoke = tauri.core.invoke;

let lang = "en";
let state = start({});
// An error to show above the screen (language switch, open folder), cleared on the next event.
let notice = "";
// What was typed in the details form but not submitted yet: survives a re-render.
let draft = {};
// The version and formats the Information panel lists (from the core).
let info = { version: "", video_exts: [], dated_exts: [] };
const root = document.getElementById("app");

async function init() {
  lang = await invoke("lang");
  state = start(await invoke("settings"));
  info = await invoke("info");
  // Drag and drop a folder onto the window on the source and destination steps.
  await tauri.event.listen("tauri://drag-drop", (e) => {
    const path = e.payload.paths && e.payload.paths[0];
    if (path && (state.step === "source" || state.step === "dest")) {
      chooseFolder(state.step, path);
    }
  });
  // Ctrl+Z (Cmd+Z on a Mac) undoes the last move or erase, like the Undo button.
  document.addEventListener("keydown", (e) => {
    const typing = e.target instanceof HTMLInputElement;
    const undoKey = (e.ctrlKey || e.metaKey) && !e.shiftKey && !e.repeat && e.key.toLowerCase() === "z";
    if (undoKey && !typing && state.canUndo) {
      e.preventDefault();
      send({ type: "undo" });
    } else if (e.key === "Escape" && state.panel && !e.defaultPrevented && !(e.target instanceof HTMLSelectElement)) {
      send({ type: "panel", name: state.panel });
    }
  });
  render();
  if (state.checkUpdates) checkForUpdate(false);
}

/** Ask whether a newer release is out (ADR 0007); `manual`: Options → "Check now". */
async function checkForUpdate(manual) {
  try {
    send({ type: "updateChecked", offer: await invoke("check_update", { lang }), manual });
  } catch (err) {
    send({ type: "updateFailed", message: String(err), manual });
  }
}

/** Download, verify and install the newer release; the app restarts when it is done. */
async function installUpdate() {
  send({ type: "install" });
  if (!state.updating) return;
  try {
    await invoke("install_update");
  } catch (err) {
    send({ type: "installFailed", message: String(err) });
  }
}

/** Apply an event, run what the new step needs, re-render. */
async function send(event) {
  // Opening a panel leaves a notice on screen: it may be what the user wants to check.
  const keepsNotice = ["panel", "checkUpdates", "updateChecked", "updateFailed", "install", "installFailed"];
  if (!keepsNotice.includes(event.type)) notice = "";
  if (event.type === "details" || event.type === "restart") draft = {};
  const before = state;
  state = next(state, event);
  render();
  try {
    const cmd = effect(before, state);
    if (cmd === "plan") {
      const result = await invoke("plan", { req: planRequest(state), lang });
      send({ type: "planned", result });
    } else if (cmd === "apply") {
      const progress = new tauri.core.Channel();
      progress.onmessage = (line) => send({ type: "progress", line });
      const outcome = await invoke("apply", {
        req: planRequest(state),
        expected: state.entries,
        mode: state.mode,
        lang,
        progress,
      });
      send({ type: "copied", outcome });
    } else if (cmd === "erase") {
      send({ type: "erased", outcome: await invoke("erase", { shoots: state.outcome.shoots, lang }) });
    } else if (cmd === "undo_move") {
      const { shoots, run } = state.outcome;
      send({ type: "undone", outcome: await invoke("undo_move", { shoots, run, lang }) });
    } else if (cmd === "undo") {
      send({ type: "undone", outcome: await invoke("undo", { shoots: state.outcome.shoots, lang }) });
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
  if (path) chooseFolder(step, path);
}

/** Check a folder (picked, dropped or pasted) before moving on: errors stay on this screen. */
async function chooseFolder(step, path) {
  try {
    const source = step === "dest" ? state.source : null;
    const folder = await invoke("check_folder", { path, source, lang });
    delete draft[`${step}Path`];
    send({ type: step, path: folder });
  } catch (err) {
    notice = String(err);
    render();
  }
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

const button = (key, onclick, primary = false, vars = {}, action = key) =>
  el("button", { class: primary ? "primary" : "", "data-action": action, onclick }, t(lang, key, vars));

/** `b`, disabled while an update installs (the restart would cut its command short). */
function whenIdle(b) {
  b.disabled = state.updating;
  return b;
}

/** Run a Tauri command whose failure the user should see. */
async function tryInvoke(cmd, args) {
  try {
    await invoke(cmd, args);
    return true;
  } catch (err) {
    notice = String(err);
    render();
    return false;
  }
}

/** Switch the screens to `other` ("fr" or "en"), remembered for the next start. */
async function switchLang(other) {
  if (await tryInvoke("set_lang", { lang: other })) {
    lang = other;
    notice = "";
    render();
  }
}

function render() {
  const panelButton = (name, symbol) =>
    el(
      "button",
      {
        class: state.panel === name ? "icon open" : "icon",
        "data-action": name,
        title: t(lang, `${name}_title`),
        "aria-label": t(lang, `${name}_title`),
        "aria-expanded": state.panel === name ? "true" : "false",
        onclick: () => send({ type: "panel", name }),
      },
      symbol,
    );
  const header = el(
    "header",
    {},
    // The logo drawn for each theme; CSS shows the one matching the system's.
    el(
      "h1",
      { class: "brand" },
      el("img", { class: "logo-light", src: "brand/exportx-logo-light-theme.svg", alt: t(lang, "title") }),
      el("img", { class: "logo-dark", src: "brand/exportx-logo-dark-theme.svg", alt: t(lang, "title") }),
    ),
    el(
      "nav",
      {},
      el(
        "button",
        { class: "link", "data-action": "lang", onclick: () => switchLang(lang === "fr" ? "en" : "fr") },
        t(lang, "lang_switch"),
      ),
      panelButton("info", "i"),
      panelButton("options", "⚙"),
    ),
  );
  const shown = notice ? el("p", { id: "notice", class: "error", role: "alert" }, notice) : "";
  const panel = state.panel === "info" ? infoPanel() : state.panel === "options" ? optionsPanel() : "";
  root.replaceChildren(header, updateBanner(), shown, panel, screen());
}

/** The newer release found, and what the last manual check or install said (ADR 0007). */
function updateBanner() {
  const message = state.updateMessage
    ? el(
        "p",
        { id: "update-message", class: state.updateMessage === "up_to_date" ? "" : "error", role: "status" },
        state.updateMessage === "up_to_date" ? t(lang, "up_to_date", { version: info.version }) : state.updateMessage,
      )
    : "";
  if (!state.update) return message;
  const { kind, version } = state.update;
  const action =
    kind === "install"
      ? el(
          "button",
          { class: "primary", "data-action": "update", disabled: !canUpdate(state), onclick: installUpdate },
          t(lang, state.updating ? "updating" : "update_install"),
        )
      : button("update_page", () => tryInvoke("open_release", { version }), true, {}, "release-page");
  return el(
    "div",
    { id: "update", class: "banner" },
    el("p", {}, t(lang, "update_available", { version })),
    action,
    message,
  );
}

function closeButton() {
  return button("close", () => send({ type: "panel", name: state.panel }), false, {}, "close-panel");
}

function infoPanel() {
  return el(
    "section",
    { class: "panel", id: "info-panel" },
    el("h2", {}, t(lang, "info_title")),
    el("p", {}, `ExportX ${info.version}`),
    el("h3", {}, t(lang, "info_videos")),
    el("p", { class: "exts" }, info.video_exts.join(", ")),
    el("p", {}, t(lang, "info_photos")),
    el("h3", {}, t(lang, "info_dates")),
    el("p", {}, t(lang, "info_dates_text", { exts: info.dated_exts.join(", ") })),
    el("h3", {}, t(lang, "info_log")),
    el("p", {}, t(lang, "info_log_text")),
    el("div", { class: "actions" }, closeButton()),
  );
}

function optionsPanel() {
  const language = el(
    "select",
    { id: "language", onchange: (e) => switchLang(e.target.value) },
    el("option", { value: "fr", selected: lang === "fr" }, "Français"),
    el("option", { value: "en", selected: lang === "en" }, "English"),
  );
  const updates = el("input", {
    type: "checkbox",
    id: "check-updates",
    checked: state.checkUpdates,
    onchange: async (e) => {
      const on = e.target.checked;
      if (await tryInvoke("set_check_updates", { on })) send({ type: "checkUpdates", on });
    },
  });
  return el(
    "section",
    { class: "panel", id: "options-panel" },
    el("h2", {}, t(lang, "options_title")),
    el("label", { for: "language" }, t(lang, "options_language")),
    language,
    el("label", { class: "check", for: "check-updates" }, updates, t(lang, "options_updates")),
    el("small", {}, t(lang, "options_updates_help")),
    el(
      "div",
      { class: "actions" },
      el("button", { id: "check-now", onclick: () => checkForUpdate(true) }, t(lang, "options_check_now")),
    ),
    el("p", {}, `ExportX ${info.version}`),
    el("div", { class: "actions" }, closeButton()),
  );
}

function folderStep(step, titleKey, helpKey, value) {
  // A pasted path, besides the picker and drag and drop (and the only way tests enter one).
  const key = `${step}Path`;
  const path = el("input", {
    id: `${step}-path`,
    value: draft[key] ?? value,
    placeholder: t(lang, "paste_placeholder"),
    oninput: (e) => {
      draft[key] = e.target.value;
    },
  });
  const submit = (e) => {
    e.preventDefault();
    if (path.value.trim()) chooseFolder(step, path.value.trim());
  };
  return el(
    "section",
    { class: "drop" },
    el("h2", {}, t(lang, titleKey)),
    el("p", {}, t(lang, helpKey)),
    el(
      "div",
      { class: "actions" },
      step === "dest" ? button("back", () => send({ type: "back" })) : "",
      button("choose_folder", () => pickFolder(step), !value, {}, `${step}-pick`),
    ),
    el(
      "form",
      { class: "paste", onsubmit: submit },
      el("label", { for: `${step}-path` }, t(lang, "paste_label")),
      path,
      el("button", { id: `${step}-ok`, type: "submit", class: value ? "primary" : "" }, t(lang, "next")),
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
      const p = state.progress || { files_done: 0, files_total: 0, bytes_done: 0, bytes_total: 0 };
      // By bytes; by files when every file is empty.
      const [value, max] =
        p.bytes_total > 0 ? [p.bytes_done, p.bytes_total] : [p.files_done, p.files_total];
      return el(
        "section",
        {},
        el("p", { class: "busy" }, t(lang, state.mode === "move" ? "moving" : "copying")),
        el("progress", { value, max: max || 1 }),
        el("p", { class: "progress-line" }, p.text || ""),
      );
    }
    case "result":
      return result();
    case "erasing":
      return el("section", {}, el("p", { class: "busy" }, t(lang, "erasing")));
    case "erased":
      return erased();
    case "undoing":
      return el(
        "section",
        {},
        el("p", { class: "busy" }, t(lang, state.outcome && state.outcome.moved ? "moving_back" : "undoing")),
      );
    default:
      return done();
  }
}

function details() {
  const keep = (key) => (e) => {
    draft[key] = e.target.value;
  };
  const shoot = el("input", {
    id: "shoot",
    value: draft.shoot ?? state.shoot,
    required: true,
    autofocus: true,
    oninput: keep("shoot"),
  });
  const example = el(
    "small",
    { id: "prefix-example" },
    t(lang, "prefix_help", { prefix: (draft.prefix ?? state.prefix) || DEFAULT_PREFIX }),
  );
  const prefix = el("input", {
    id: "prefix",
    value: draft.prefix ?? state.prefix,
    oninput: (e) => {
      keep("prefix")(e);
      example.textContent = t(lang, "prefix_help", { prefix: e.target.value || DEFAULT_PREFIX });
    },
  });
  // A text field, not type=date: WebKitGTK draws an empty date input as today's date.
  const date = el("input", {
    id: "date",
    value: draft.date ?? (state.date.day || ""),
    placeholder: t(lang, "date_placeholder"),
    pattern: "\\d{4}-\\d{2}-\\d{2}",
    inputmode: "numeric",
    oninput: keep("date"),
  });
  const preset = el(
    "select",
    { id: "preset" },
    ["classique", "lisible"].map((p) =>
      el("option", { value: p, selected: p === (draft.preset ?? state.preset) }, t(lang, `preset_${p}`)),
    ),
  );
  preset.addEventListener("change", keep("preset"));
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
    // A key of i18n.js (checks made on this side) or a message the Rust side translated.
    state.error ? el("p", { class: "error" }, t(lang, state.error)) : "",
    el("label", { for: "shoot" }, t(lang, "shoot_label")),
    shoot,
    el("label", { for: "prefix" }, t(lang, "prefix_label")),
    prefix,
    example,
    el("label", { for: "date" }, t(lang, "date_label")),
    date,
    el("small", {}, t(lang, "date_help")),
    el("label", { for: "preset" }, t(lang, "preset_label")),
    preset,
    el(
      "div",
      { class: "actions" },
      button("back", () => send({ type: "back" })),
      el("button", { id: "details-ok", class: "primary", type: "submit" }, t(lang, "next")),
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
        button("day_one", () => send({ type: "chooseDay", choice: "one", day }), false, { day }, "day-one"),
      ),
      button("day_each", () => send({ type: "chooseDay", choice: "each" }), true, {}, "day-each"),
      button("day_none", () => send({ type: "chooseDay", choice: "none" }), false, {}, "day-none"),
    ),
    el("div", { class: "actions" }, button("back", () => send({ type: "back" }))),
  );
}

/** The undo shortcut as this computer writes it. */
const undoKeys = () => (/Mac/.test(navigator.platform) ? "⌘Z" : "Ctrl+Z");

/** On the same disk: move (instant, undoable) or copy (twice the space). */
function modeChoice() {
  const option = (mode, key, help, enabled) =>
    el(
      "label",
      { class: enabled ? "option" : "option disabled" },
      el("input", {
        type: "radio",
        name: "mode",
        value: mode,
        id: `mode-${mode}`,
        checked: state.mode === mode,
        disabled: !enabled,
        onchange: () => send({ type: "mode", mode }),
      }),
      el("span", {}, el("strong", {}, t(lang, key)), " — ", help),
    );
  const copyHelp = state.copyFits
    ? t(lang, "mode_copy_help", { size: state.copySize })
    : t(lang, "mode_copy_no_room", { size: state.copySize });
  return el(
    "fieldset",
    { class: "mode" },
    el("legend", {}, t(lang, "mode_same_disk")),
    option("move", "mode_move", t(lang, "mode_move_help"), true),
    option("copy", "mode_copy", copyHelp, state.copyFits),
  );
}

function preview() {
  const moving = state.mode === "move";
  const n = state.entries.filter((e) => !e.already).length;
  return el(
    "section",
    {},
    el("h2", {}, t(lang, moving ? "preview_title_move" : "preview_title")),
    el("p", {}, t(lang, moving ? "preview_count_move" : "preview_count", { n, dest: state.dest })),
    state.movable ? modeChoice() : el("p", { class: "safe" }, t(lang, "preview_safe")),
    el(
      "table",
      { class: "preview" },
      el(
        "tbody",
        {},
        state.entries.map((e) =>
          el(
            "tr",
            {},
            el("td", {}, e.name),
            el("td", {}, "→"),
            el("td", {}, e.dest),
            el("td", { class: "note" }, e.note || ""),
          ),
        ),
      ),
    ),
    el(
      "div",
      { class: "actions" },
      button("back", () => send({ type: "back" })),
      whenIdle(button(moving ? "move" : "copy", () => send({ type: "confirm" }), true, {}, "copy")),
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
    el(
      "button",
      { "data-action": "open-folder", onclick: () => tryInvoke("open_folder", { path }) },
      `${t(lang, "open_folder")} — ${path}`,
    ),
  );
}

function result() {
  const o = state.outcome;
  return el(
    "section",
    {},
    el("h2", {}, t(lang, "result_title")),
    o.moved ? el("p", {}, t(lang, "moved", { n: o.moved })) : "",
    o.copied || !o.moved ? el("p", {}, t(lang, "copied", { n: o.copied })) : "",
    issues("failed", o.failed),
    el("div", { class: "actions" }, openButtons()),
    state.canUndo
      ? el(
          "div",
          { class: "actions" },
          whenIdle(button("undo_move", () => send({ type: "undo" }), false, {}, "undo")),
          button("restart", () => send({ type: "restart" }), true),
          el("small", { class: "hint" }, t(lang, "undo_hint", { keys: undoKeys() })),
        )
      : state.canErase
      ? el(
          "div",
          { class: "question" },
          el("p", {}, t(lang, "erase_question")),
          whenIdle(button("erase_no", () => send({ type: "erase", yes: false }), false, {}, "erase-no")),
          whenIdle(button("erase_yes", () => send({ type: "erase", yes: true }), true, {}, "erase-yes")),
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
      state.canUndo ? whenIdle(button("undo", () => send({ type: "undo" }))) : "",
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
    state.outcome && (state.outcome.copied || !state.outcome.moved)
      ? el("p", {}, t(lang, "copied", { n: state.outcome.copied }))
      : "",
    o ? el("p", {}, t(lang, state.outcome && state.outcome.moved ? "moved_back" : "restored", { n: o.done })) : "",
    o ? issues("kept", o.kept) : "",
    el("div", { class: "actions" }, state.outcome ? openButtons() : "", button("restart", () => send({ type: "restart" }), true)),
  );
}

init();
