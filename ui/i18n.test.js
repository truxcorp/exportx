// Behaviour 14 in the app: every screen text exists in French and English.
import { test } from "node:test";
import assert from "node:assert/strict";
import { STRINGS, t } from "./i18n.js";

test("every text has a French and an English version", () => {
  const missing = Object.entries(STRINGS)
    .filter(([, v]) => !v.fr || !v.en)
    .map(([k]) => k);
  assert.deepEqual(missing, []);
  assert.ok(Object.keys(STRINGS).length > 20, "the screens' texts are all here");
});

test("placeholders are filled in", () => {
  assert.equal(t("fr", "copied", { n: 3 }), "3 fichier(s) copié(s)");
  assert.equal(t("en", "copied", { n: 3 }), "3 file(s) copied");
});

test("an unknown language falls back to English", () => {
  assert.equal(t("de", "copied", { n: 1 }), "1 file(s) copied");
});

test("the prefix field is called a prefix in both languages", () => {
  assert.equal(t("fr", "prefix_label"), "Préfixe des noms de fichier");
  assert.equal(t("en", "prefix_label"), "File name prefix");
});
