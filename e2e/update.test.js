// ADR 0007 end to end: the startup update check, Options → "Check now", the banner.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { setTimeout as sleep } from "node:timers/promises";
import { launch } from "./driver.js";

/** A local update endpoint serving `latest.json` for `version`; counts its requests. */
async function endpoint(version = "99.0.0") {
  const server = createServer((req, res) => {
    server.requests++;
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ version, notes: "", platforms: {} }));
  });
  server.requests = 0;
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  server.url = `http://127.0.0.1:${server.address().port}/latest.json`;
  return server;
}

test("U1: an AppImage is offered the newer version, update and restart", async () => {
  const server = await endpoint();
  const app = await launch({ env: { EXPORTX_UPDATE_URL: server.url, EXPORTX_BUNDLE: "appimage" } });
  try {
    await app.waitText("Version 99.0.0 disponible");
    assert.equal(await app.attribute("[data-action=update]", "disabled"), null, "the button works");
    assert.ok(!(await app.has("[data-action=release-page]")));
    await app.screenshot("11-update-banner");
  } finally {
    await app.close();
    server.close();
  }
});

test("U3: a .deb install gets the release page instead of an install", async () => {
  const server = await endpoint();
  const app = await launch({ lang: "en_US.UTF-8", env: { EXPORTX_UPDATE_URL: server.url } });
  try {
    await app.waitText("Version 99.0.0 available");
    assert.ok(await app.has("[data-action=release-page]"));
    assert.ok(!(await app.has("[data-action=update]")));
  } finally {
    await app.close();
    server.close();
  }
});

test("U2: with the check off nothing is asked at startup; Check now still works", async () => {
  const server = await endpoint();
  const app = await launch({
    settings: "check_updates = false\n",
    env: { EXPORTX_UPDATE_URL: server.url, EXPORTX_BUNDLE: "appimage" },
  });
  try {
    await app.waitText("1. Les photos à ranger");
    await sleep(2000);
    assert.equal(server.requests, 0, "no network request");
    assert.ok(!(await app.text()).includes("99.0.0"));
    await app.click("[data-action=options]");
    await app.click("#check-now");
    await app.waitText("Version 99.0.0 disponible");
    assert.equal(server.requests, 1);
  } finally {
    await app.close();
    server.close();
  }
});

test("U4: no network shows nothing at startup; a manual check shows the error", async () => {
  const server = await endpoint();
  const closed = server.url;
  await new Promise((resolve) => server.close(resolve));
  const app = await launch({ env: { EXPORTX_UPDATE_URL: closed } });
  try {
    await app.waitText("1. Les photos à ranger");
    await sleep(2000);
    assert.ok(!(await app.has("#update")), "no banner");
    assert.ok(!(await app.has("#update-message")), "no error");
    await app.click("[data-action=options]");
    await app.click("#check-now");
    await app.find("#update-message");
    await app.waitText("impossible de rechercher une nouvelle version");
  } finally {
    await app.close();
  }
});

test("a manual check with nothing newer says the app is up to date", async () => {
  const server = await endpoint("0.0.1");
  const app = await launch({ lang: "en_US.UTF-8", env: { EXPORTX_UPDATE_URL: server.url } });
  try {
    await app.click("[data-action=options]");
    await app.click("#check-now");
    await app.waitText("is up to date");
  } finally {
    await app.close();
    server.close();
  }
});
