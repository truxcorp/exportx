// Behaviour 13: the app's flow, as a pure state machine (ui/flow.js).
// source → destination → shoot details → day choice if needed → preview → copy →
// result → erase question.
import { test } from "node:test";
import assert from "node:assert/strict";
import { start, next, planRequest, effect, canUpdate } from "./flow.js";

const entries = [{ name: "a.jpg", dest: "2024/03/05/s/p/o/jpg/txo_0001.jpg" }];

function atDetails(settings = {}) {
  let s = start(settings);
  s = next(s, { type: "source", path: "/card" });
  s = next(s, { type: "dest", path: "/photos" });
  return s;
}

test("starts on the source step, with the remembered destination and prefix", () => {
  const s = start({ dest: "/photos", prefix: "md", preset: "lisible" });
  assert.equal(s.step, "source");
  assert.equal(s.dest, "/photos");
  assert.equal(s.prefix, "md");
  assert.equal(s.preset, "lisible");
});

test("without remembered settings the prefix is txo and the preset classique", () => {
  const s = start({});
  assert.equal(s.prefix, "txo");
  assert.equal(s.preset, "classique");
});

test("source, then destination, then the shoot details", () => {
  let s = start({});
  s = next(s, { type: "source", path: "/card" });
  assert.equal(s.step, "dest");
  s = next(s, { type: "dest", path: "/photos" });
  assert.equal(s.step, "details");
  assert.equal(s.source, "/card");
  assert.equal(s.dest, "/photos");
});

test("a remembered destination skips nothing: it is shown pre-filled", () => {
  let s = start({ dest: "/photos" });
  s = next(s, { type: "source", path: "/card" });
  assert.equal(s.step, "dest");
  assert.equal(s.dest, "/photos");
});

test("the details build the plan request", () => {
  let s = atDetails();
  s = next(s, { type: "details", shoot: "mariage", prefix: "dupont", date: "" });
  assert.equal(s.step, "planning");
  assert.deepEqual(planRequest(s), {
    source: "/card",
    dest: "/photos",
    shoot: "mariage",
    prefix: "dupont",
    date: { kind: "files" },
    preset: "classique",
  });
});

test("a date entered goes into the request", () => {
  let s = atDetails();
  s = next(s, { type: "details", shoot: "s", prefix: "txo", date: "2019-07-14" });
  assert.deepEqual(planRequest(s).date, { kind: "given", day: "2019-07-14" });
});

test("a plan goes to the preview", () => {
  let s = atDetails();
  s = next(s, { type: "details", shoot: "s", prefix: "txo", date: "" });
  s = next(s, { type: "planned", result: { kind: "plan", entries } });
  assert.equal(s.step, "preview");
  assert.deepEqual(s.entries, entries);
});

test("several days ask for a choice, then plan again with it", () => {
  let s = atDetails();
  s = next(s, { type: "details", shoot: "s", prefix: "txo", date: "" });
  const days = [["2024-03-05", 2], ["2024-03-07", 1]];
  s = next(s, { type: "planned", result: { kind: "days", days } });
  assert.equal(s.step, "days");
  assert.deepEqual(s.days, days);

  const one = next(s, { type: "chooseDay", choice: "one", day: "2024-03-07" });
  assert.equal(one.step, "planning");
  assert.deepEqual(planRequest(one).date, { kind: "given", day: "2024-03-07" });

  const each = next(s, { type: "chooseDay", choice: "each" });
  assert.deepEqual(planRequest(each).date, { kind: "each" });

  const none = next(s, { type: "chooseDay", choice: "none" });
  assert.deepEqual(planRequest(none).date, { kind: "none" });
});

test("a refused plan goes back to the details with the message", () => {
  let s = atDetails();
  s = next(s, { type: "details", shoot: "CON", prefix: "txo", date: "" });
  s = next(s, { type: "planned", result: { kind: "error", message: "invalid name" } });
  assert.equal(s.step, "details");
  assert.equal(s.error, "invalid name");
});

test("confirming the preview copies, then shows the result", () => {
  let s = atDetails();
  s = next(s, { type: "details", shoot: "s", prefix: "txo", date: "" });
  s = next(s, { type: "planned", result: { kind: "plan", entries } });
  s = next(s, { type: "confirm" });
  assert.equal(s.step, "copying");
  const line = { files_done: 1, files_total: 1, bytes_done: 5, bytes_total: 5, text: "1 / 1 file" };
  s = next(s, { type: "progress", line });
  assert.deepEqual(s.progress, line);
  const outcome = { copied: 1, failed: [], shoots: ["/photos/2024/03/05/s"] };
  s = next(s, { type: "copied", outcome });
  assert.equal(s.step, "result");
  assert.deepEqual(s.outcome, outcome);
  assert.equal(s.canErase, true);
});

test("after failures the erase question is not offered", () => {
  let s = atDetails();
  s = next(s, { type: "details", shoot: "s", prefix: "txo", date: "" });
  s = next(s, { type: "planned", result: { kind: "plan", entries } });
  s = next(s, { type: "confirm" });
  s = next(s, {
    type: "copied",
    outcome: { copied: 0, failed: [["a.jpg", "denied"]], shoots: ["/photos/2024/03/05/s"] },
  });
  assert.equal(s.step, "result");
  assert.equal(s.canErase, false);
});

test("the erase question: yes erases, then undo is offered; no ends", () => {
  let s = atDetails();
  s = next(s, { type: "details", shoot: "s", prefix: "txo", date: "" });
  s = next(s, { type: "planned", result: { kind: "plan", entries } });
  s = next(s, { type: "confirm" });
  s = next(s, { type: "copied", outcome: { copied: 1, failed: [], shoots: ["/x"] } });

  const no = next(s, { type: "erase", yes: false });
  assert.equal(no.step, "done");

  let yes = next(s, { type: "erase", yes: true });
  assert.equal(yes.step, "erasing");
  yes = next(yes, { type: "erased", outcome: { done: 1, kept: [] } });
  assert.equal(yes.step, "erased");
  assert.equal(yes.canUndo, true);
  yes = next(yes, { type: "undo" });
  assert.equal(yes.step, "undoing");
  yes = next(yes, { type: "undone", outcome: { done: 1, kept: [] } });
  assert.equal(yes.step, "done");
});

test("back from the preview returns to the details, keeping them", () => {
  let s = atDetails();
  s = next(s, { type: "details", shoot: "s", prefix: "zz", date: "" });
  s = next(s, { type: "planned", result: { kind: "plan", entries } });
  s = next(s, { type: "back" });
  assert.equal(s.step, "details");
  assert.equal(s.shoot, "s");
  assert.equal(s.prefix, "zz");
});

test("a new export starts over from the source with the last destination", () => {
  let s = atDetails();
  s = next(s, { type: "details", shoot: "s", prefix: "zz", date: "" });
  s = next(s, { type: "planned", result: { kind: "plan", entries } });
  s = next(s, { type: "confirm" });
  s = next(s, { type: "copied", outcome: { copied: 1, failed: [], shoots: ["/x"] } });
  s = next(s, { type: "restart" });
  assert.equal(s.step, "source");
  assert.equal(s.dest, "/photos");
  assert.equal(s.prefix, "zz");
});

test("an impossible or badly written date stays on the details with an error", () => {
  for (const date of ["2024-02-30", "2024-13-01", "05/03/2024", "2024-3-5"]) {
    let s = atDetails();
    s = next(s, { type: "details", shoot: "s", prefix: "txo", date });
    assert.equal(s.step, "details", date);
    assert.equal(s.error, "invalid_date", date);
  }
});

test("a real date passes, 29 February of a leap year included", () => {
  let s = atDetails();
  s = next(s, { type: "details", shoot: "s", prefix: "txo", date: "2024-02-29" });
  assert.equal(s.step, "planning");
});

test("the copy's progress counts only the files to copy, not those already copied", () => {
  let s = atDetails();
  s = next(s, { type: "details", shoot: "s", prefix: "txo", date: "" });
  const mixed = [...entries, { name: "old.jpg", dest: "x/txo_0000.jpg", already: true }];
  s = next(s, { type: "planned", result: { kind: "plan", entries: mixed } });
  s = next(s, { type: "confirm" });
  assert.deepEqual(s.progress, {
    files_done: 0,
    files_total: 1,
    bytes_done: 0,
    bytes_total: 0,
    text: "",
  });
});

// ADR 0006: moving instead of copying on the same disk.
function atPreview(result) {
  let s = atDetails();
  s = next(s, { type: "details", shoot: "s", prefix: "txo", date: "" });
  return next(s, { type: "planned", result: { kind: "plan", entries, ...result } });
}

test("on the same disk the preview offers moving, preselected, and can switch to copying", () => {
  let s = atPreview({ movable: true, copy_size: "12,4 Go", copy_fits: true });
  assert.equal(s.movable, true);
  assert.equal(s.copySize, "12,4 Go");
  assert.equal(s.mode, "move");
  s = next(s, { type: "mode", mode: "copy" });
  assert.equal(s.mode, "copy");
  s = next(s, { type: "mode", mode: "move" });
  assert.equal(s.mode, "move");
});

test("on another disk the preview only copies", () => {
  let s = atPreview({ movable: false, copy_size: "5 B", copy_fits: true });
  assert.equal(s.mode, "copy");
  s = next(s, { type: "mode", mode: "move" });
  assert.equal(s.mode, "copy");
});

test("when a copy doesn't fit, only moving is possible", () => {
  let s = atPreview({ movable: true, copy_size: "900 GB", copy_fits: false });
  assert.equal(s.mode, "move");
  s = next(s, { type: "mode", mode: "copy" });
  assert.equal(s.mode, "move");
});

test("after a move, no erase question: undo instead, then done", () => {
  let s = atPreview({ movable: true, copy_size: "5 B", copy_fits: true });
  s = next(s, { type: "confirm" });
  s = next(s, { type: "copied", outcome: { copied: 0, moved: 1, failed: [], shoots: ["/p/s"] } });
  assert.equal(s.step, "result");
  assert.equal(s.canErase, false);
  assert.equal(s.canUndo, true);
  s = next(s, { type: "undo" });
  assert.equal(s.step, "undoing");
  s = next(s, { type: "undone", outcome: { done: 1, kept: [] } });
  assert.equal(s.step, "done");
});

test("a copy on the same disk doesn't ask to erase the originals either", () => {
  let s = atPreview({ movable: true, copy_size: "5 B", copy_fits: true });
  s = next(s, { type: "mode", mode: "copy" });
  s = next(s, { type: "confirm" });
  s = next(s, { type: "copied", outcome: { copied: 1, moved: 0, failed: [], shoots: ["/p/s"] } });
  assert.equal(s.canErase, false);
  assert.equal(s.canUndo, false);
});

test("each step's command runs once, when the step starts, never again on a repeated key", () => {
  const step = (s, e) => {
    const after = next(s, e);
    return [after, effect(s, after)];
  };
  let s = atDetails();
  let cmd;
  [s, cmd] = step(s, { type: "details", shoot: "s", prefix: "txo", date: "" });
  assert.equal(cmd, "plan");
  [s, cmd] = step(s, { type: "planned", result: { kind: "plan", entries, movable: true, copy_size: "5 B", copy_fits: true } });
  assert.equal(cmd, null);
  [s, cmd] = step(s, { type: "confirm" });
  assert.equal(cmd, "apply");
  [s, cmd] = step(s, { type: "progress", line: { files_done: 1, files_total: 1, bytes_done: 5, bytes_total: 5, text: "" } });
  assert.equal(cmd, null);
  [s, cmd] = step(s, { type: "copied", outcome: { copied: 0, moved: 1, failed: [], shoots: ["/p/s"], run: "r1" } });
  assert.equal(cmd, null);
  [s, cmd] = step(s, { type: "undo" });
  assert.equal(cmd, "undo_move");
  // Ctrl+Z again while the undo runs: nothing more.
  [s, cmd] = step(s, { type: "undo" });
  assert.equal(cmd, null);

  let c = atPreview({ movable: false, copy_size: "5 B", copy_fits: true });
  c = next(c, { type: "confirm" });
  c = next(c, { type: "copied", outcome: { copied: 1, moved: 0, failed: [], shoots: ["/p/s"], run: "r2" } });
  [c, cmd] = step(c, { type: "erase", yes: true });
  assert.equal(cmd, "erase");
  [c, cmd] = step(c, { type: "erased", outcome: { done: 1, kept: [] } });
  [c, cmd] = step(c, { type: "undo" });
  assert.equal(cmd, "undo");
});

test("the options and information panels open over any step and close again", () => {
  let s = atDetails({ check_updates: false });
  assert.equal(s.panel, null);
  assert.equal(s.checkUpdates, false);
  s = next(s, { type: "panel", name: "info" });
  assert.equal(s.panel, "info");
  assert.equal(s.step, "details");
  s = next(s, { type: "panel", name: "options" });
  assert.equal(s.panel, "options");
  s = next(s, { type: "checkUpdates", on: true });
  assert.equal(s.checkUpdates, true);
  s = next(s, { type: "panel", name: "options" });
  assert.equal(s.panel, null, "the same button closes it");
  assert.equal(start({}).checkUpdates, true, "on unless turned off");
});

// ADR 0007: auto-update, the banner's state.
const install = { kind: "install", version: "2.0.0" };

test("U1: a found update is offered, and stays offered across steps and a restart", () => {
  let s = next(atDetails(), { type: "updateChecked", offer: install, manual: false });
  assert.deepEqual(s.update, install);
  assert.equal(canUpdate(s), true);
  s = next(s, { type: "back" });
  s = next(s, { type: "restart" });
  assert.deepEqual(s.update, install);
});

test("U4: a failed startup check shows nothing; a manual one shows why", () => {
  let s = next(atDetails(), { type: "updateFailed", message: "no network", manual: false });
  assert.equal(s.update, null);
  assert.equal(s.updateMessage, "");
  s = next(s, { type: "updateFailed", message: "no network", manual: true });
  assert.equal(s.updateMessage, "no network");
});

test("a manual check with nothing newer says the app is up to date", () => {
  let s = next(atDetails(), { type: "updateChecked", offer: null, manual: true });
  assert.equal(s.update, null);
  assert.equal(s.updateMessage, "up_to_date");
  s = next(s, { type: "updateChecked", offer: null, manual: false });
  assert.equal(s.updateMessage, "up_to_date", "a startup check doesn't clear it");
});

test("U3: a release page link is no install", () => {
  const link = { kind: "link", version: "2.0.0", page: "https://example/v2.0.0" };
  const s = next(atDetails(), { type: "updateChecked", offer: link, manual: false });
  assert.deepEqual(s.update, link);
  assert.equal(canUpdate(s), false);
  assert.equal(next(s, { type: "install" }).updating, false);
});

test("U5: no update while a copy, move or erase runs", () => {
  let s = atPreview({ movable: false, copy_size: "5 B", copy_fits: true });
  s = next(s, { type: "updateChecked", offer: install, manual: false });
  assert.equal(canUpdate(s), true);
  s = next(s, { type: "confirm" });
  assert.equal(s.step, "copying");
  assert.equal(canUpdate(s), false);
  assert.equal(next(s, { type: "install" }).updating, false, "the event is refused too");
  s = next(s, { type: "copied", outcome: { copied: 1, moved: 0, failed: [], shoots: ["/p/s"], run: "r" } });
  assert.equal(canUpdate(s), true);
  s = next(s, { type: "erase", yes: true });
  assert.equal(s.step, "erasing");
  assert.equal(canUpdate(s), false);
  s = next(s, { type: "erased", outcome: { done: 1, kept: [] } });
  s = next(s, { type: "undo" });
  assert.equal(s.step, "undoing");
  assert.equal(canUpdate(s), false);
});

test("installing: once, then a failure shows why and allows a retry", () => {
  let s = next(atDetails(), { type: "updateChecked", offer: install, manual: false });
  s = next(s, { type: "install" });
  assert.equal(s.updating, true);
  assert.equal(canUpdate(s), false, "not twice");
  s = next(s, { type: "installFailed", message: "bad signature" });
  assert.equal(s.updating, false);
  assert.equal(s.updateMessage, "bad signature");
  assert.equal(canUpdate(s), true);
});
