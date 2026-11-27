// Behaviour 13 end to end: the real app, driven through WebDriver (ADR 0005).
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { launch } from "./driver.js";

/** Files whose content is their name, modified at `stamp` (UTC). */
function mk(dir, stamp, names) {
  mkdirSync(dir, { recursive: true });
  const t = new Date(`${stamp}Z`);
  for (const n of names) {
    writeFileSync(join(dir, n), n);
    utimesSync(join(dir, n), t, t);
  }
}

/** Paste the source and destination folders, as a user without the pickers would. */
async function pasteFolders(app, src, dest) {
  await app.type("#source-path", src);
  await app.click("#source-ok");
  await app.type("#dest-path", dest);
  await app.click("#dest-ok");
  await app.find("#shoot");
}

test("copy, erase and undo from the source to the last screen", async () => {
  const app = await launch({ lang: "fr_FR.UTF-8" });
  // Another device than the card (erase refuses copies on the same one).
  // Fails loudly without /dev/shm (run the tests with make box).
  const dest = mkdtempSync("/dev/shm/exportpro-e2e-");
  try {
    const src = join(app.home, "carte");
    mk(src, "2024-03-05T09:00", ["A.CR2", "A.JPG"]);
    mk(src, "2024-03-07T10:00", ["B.JPG"]);
    mkdirSync(dest, { recursive: true });

    await app.waitText("1. Les photos à ranger");
    await app.screenshot("1-source");
    await pasteFolders(app, src, dest);
    await app.type("#shoot", "mariage");
    await app.screenshot("3-details");
    await app.click("#details-ok");

    await app.waitText("Les fichiers couvrent plusieurs jours");
    await app.screenshot("4-days");
    await app.click("[data-action=day-each]");

    await app.waitText("4. Vérifiez avant de copier");
    const preview = await app.text();
    assert.match(preview, /A\.CR2\s*→\s*2024\/03\/05\/mariage\/p\/o\/cr2\/txo_0001\.cr2/);
    assert.match(preview, /B\.JPG\s*→\s*2024\/03\/07\/mariage\/p\/o\/jpg\/txo_0001\.jpg/);
    await app.screenshot("5-preview");
    await app.click("[data-action=copy]");

    await app.waitText("3 fichier(s) copié(s)");
    await app.screenshot("6-result");
    assert.equal(readFileSync(join(dest, "2024/03/05/mariage/p/o/cr2/txo_0001.cr2"), "utf8"), "A.CR2");
    assert.ok(existsSync(join(src, "A.CR2")), "the originals stay after the copy");

    await app.click("[data-action=erase-yes]");
    await app.waitText("3 original(aux) mis à la corbeille");
    await app.screenshot("7-erased");
    assert.ok(!existsSync(join(src, "A.CR2")), "erased to the Trash");

    await app.click("[data-action=undo]");
    await app.waitText("3 original(aux) restauré(s)");
    await app.screenshot("8-undone");
    assert.equal(readFileSync(join(src, "A.CR2"), "utf8"), "A.CR2");
  } finally {
    await app.close();
    if (dest?.startsWith("/dev/shm/")) rmSync(dest, { recursive: true, force: true });
  }
});

test("the language switch changes every text and keeps what was typed", async () => {
  const app = await launch({ lang: "fr_FR.UTF-8" });
  try {
    const src = join(app.home, "carte");
    mk(src, "2024-03-05T09:00", ["a.jpg"]);
    await pasteFolders(app, src, app.home);
    await app.type("#shoot", "mariage");
    await app.type("#prefix", "md");

    await app.click("[data-action=lang]");

    await app.waitText("3. The shoot");
    await app.screenshot("3-details-en");
    assert.ok(!(await app.text()).includes("La séance"), "no French left");
    assert.equal(await (await app.find("#shoot")).value(), "mariage");
    assert.equal(await (await app.find("#prefix")).value(), "md");
  } finally {
    await app.close();
  }
});

test("the remembered destination is pre-filled", async () => {
  const app = await launch({
    lang: "en_US.UTF-8",
    settings: 'dest = "/srv/photos"\nprefix = "md"\n',
  });
  try {
    const src = join(app.home, "card");
    mk(src, "2024-03-05T09:00", ["a.jpg"]);
    await app.type("#source-path", src);
    await app.click("#source-ok");

    assert.equal(await (await app.find("#dest-path")).value(), "/srv/photos");
  } finally {
    await app.close();
  }
});

test("an error from the language switch is shown, not lost", async () => {
  // The config folder is a file: remembering the language fails.
  const home = join(process.env.TMPDIR || "/tmp", `exportpro-e2e-badcfg-${process.pid}`);
  mkdirSync(home, { recursive: true });
  const notADir = join(home, "config-is-a-file");
  writeFileSync(notADir, "");
  const app = await launch({ lang: "en_US.UTF-8", configDir: notADir });
  try {
    await app.click("[data-action=lang]");

    const notice = await app.find("#notice");
    assert.notEqual((await notice.text()).trim(), "");
  } finally {
    await app.close();
    rmSync(home, { recursive: true, force: true });
  }
});

test("the details form shows an empty date as empty, and the example follows the prefix", async () => {
  const app = await launch({ lang: "fr_FR.UTF-8" });
  try {
    const src = join(app.home, "carte");
    mk(src, "2024-03-05T09:00", ["a.jpg"]);
    await pasteFolders(app, src, app.home);

    const date = await app.find("#date");
    assert.equal(await date.value(), "");
    assert.equal(await app.attribute("#date", "placeholder"), "AAAA-MM-JJ");

    await app.type("#prefix", "md");
    await app.waitText("Exemple : md_0001.jpg");
  } finally {
    await app.close();
  }
});

test("a pasted path survives the language switch; an emptied prefix shows the default example", async () => {
  const app = await launch({ lang: "fr_FR.UTF-8" });
  try {
    const src = join(app.home, "carte");
    mk(src, "2024-03-05T09:00", ["a.jpg"]);
    await app.type("#source-path", src);
    await app.click("[data-action=lang]");
    await app.waitText("1. The photos to sort");
    assert.equal(await (await app.find("#source-path")).value(), src);

    await app.click("#source-ok");
    await app.type("#dest-path", app.home);
    await app.click("#dest-ok");
    // Empty the prefix as a user does (Ctrl+A, Backspace): typing "" isn't allowed.
    await (await app.find("#prefix")).sendKeys("\uE009a\uE000\uE003");
    await app.click("[data-action=lang]");
    await app.waitText("Exemple : txo_0001.jpg");
  } finally {
    await app.close();
  }
});

test("a pasted folder is checked right away, on its own screen", async () => {
  const app = await launch({ lang: "fr_FR.UTF-8" });
  try {
    mk(join(app.home, "carte"), "2024-03-05T09:00", ["a.jpg"]);

    await app.type("#source-path", join(app.home, "nulle-part"));
    await app.click("#source-ok");
    await app.waitText("Ce dossier n'existe pas");
    assert.ok(await app.has("#source-path"), "still on the source screen");
    await app.screenshot("2-source-missing");

    await app.type("#source-path", "~/carte");
    await app.click("#source-ok");
    await app.type("#dest-path", "~/carte");
    await app.click("#dest-ok");
    await app.waitText("La destination ne peut pas être le dossier source");
    assert.ok(await app.has("#dest-path"), "still on the destination screen");

    await app.type("#dest-path", "~");
    await app.click("#dest-ok");
    await app.type("#shoot", "s");
    await app.click("#details-ok");
    await app.waitText("a.jpg");
  } finally {
    await app.close();
  }
});

test("on the same disk: move, then Ctrl+Z puts the files back (ADR 0006)", async () => {
  const app = await launch({ lang: "fr_FR.UTF-8" });
  try {
    const src = join(app.home, "carte");
    mk(src, "2024-03-05T09:00", ["A.JPG", "B.JPG"]);
    const dest = join(app.home, "photos");
    mkdirSync(dest, { recursive: true });
    await pasteFolders(app, src, dest);
    await app.type("#shoot", "rangement");
    await app.click("#details-ok");

    await app.waitText("La destination est sur le même disque que les fichiers");
    assert.match(await app.text(), /2 fichier\(s\) seront déplacés/);
    await app.screenshot("5b-preview-same-disk");
    await app.click("[data-action=copy]");

    await app.waitText("2 fichier(s) déplacé(s)");
    await app.screenshot("6b-moved");
    assert.ok(!existsSync(join(src, "A.JPG")), "moved, not copied");
    assert.equal(readFileSync(join(dest, "2024/03/05/rangement/p/o/jpg/txo_0001.jpg"), "utf8"), "A.JPG");
    assert.ok(!(await app.text()).includes("Effacer les originaux"), "no erase question after a move");

    // Ctrl+Shift+Z (redo elsewhere) does nothing here.
    await (await app.find("body")).sendKeys("\uE009\uE008z\uE000");
    await app.waitText("2 fichier(s) déplacé(s)");
    assert.ok(!existsSync(join(src, "A.JPG")), "Ctrl+Shift+Z doesn't undo");

    await (await app.find("body")).sendKeys("\uE009z\uE000");
    await app.waitText("2 fichier(s) remis à leur place");
    await app.screenshot("8b-moved-back");
    assert.equal(readFileSync(join(src, "A.JPG"), "utf8"), "A.JPG");
  } finally {
    await app.close();
  }
});
