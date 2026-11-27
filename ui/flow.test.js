// Behaviour 13: the app's flow, as a pure state machine (ui/flow.js).
// source → destination → shoot details → day choice if needed → preview → copy →
// result → erase question.
import { test } from "node:test";
import assert from "node:assert/strict";
import { start, next, planRequest } from "./flow.js";

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
  s = next(s, { type: "progress", done: 1, total: 1 });
  assert.deepEqual(s.progress, { done: 1, total: 1 });
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
