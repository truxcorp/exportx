// A small W3C WebDriver client over fetch, and an isolated app per test (ADR 0005):
// its own Xvfb display, tauri-driver, HOME, config folder, Trash and language.
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const APP = process.env.EXPORTPRO_APP || "target/debug/exportpro-app";
const SHOTS = process.env.EXPORTPRO_SHOTS || join(process.env.CARGO_TARGET_DIR || "target", "e2e-screenshots");
const ELEMENT = "element-6066-11e4-a52e-4f735466cecf";
let nextPort = 4444;
let nextDisplay = 90;

/** Start the app for one test. `lang`: the system language ("fr_FR.UTF-8"…). */
export async function launch({ lang = "fr_FR.UTF-8", settings = null, configDir = null } = {}) {
  const home = mkdtempSync(join(tmpdir(), "exportpro-e2e-"));
  const config = configDir ?? join(home, ".config");
  if (settings) {
    mkdirSync(join(config, "exportx"), { recursive: true });
    writeFileSync(join(config, "exportx", "settings.toml"), settings);
  }
  const display = `:${nextDisplay++}`;
  const port = nextPort++;
  const env = {
    ...process.env,
    DISPLAY: display,
    HOME: home,
    XDG_CONFIG_HOME: config,
    XDG_DATA_HOME: join(home, ".local/share"),
    LANG: lang,
    LC_ALL: "",
    TZ: "UTC",
    WEBKIT_DISABLE_DMABUF_RENDERER: "1",
    LIBGL_ALWAYS_SOFTWARE: "1",
  };
  const xvfb = spawn("Xvfb", [display, "-screen", "0", "1024x768x24", "-nolisten", "tcp"], {
    stdio: "ignore",
  });
  const driver = spawn("tauri-driver", ["--port", String(port), "--native-port", String(port + 1000)], {
    env,
    stdio: ["ignore", "ignore", "inherit"],
  });
  const stop = () => {
    driver.kill();
    xvfb.kill();
    rmSync(home, { recursive: true, force: true });
  };
  try {
    const socket = `/tmp/.X11-unix/X${display.slice(1)}`;
    await waitFor(async () => existsSync(socket), 10000, `Xvfb ${display}`);
    const base = `http://127.0.0.1:${port}`;
    await waitFor(async () => (await fetch(`${base}/status`)).ok, 15000, "tauri-driver");
    const res = await call(base, "POST", "/session", {
      capabilities: { alwaysMatch: { browserName: "wry", "tauri:options": { application: APP } } },
    });
    const session = new Session(base, res.sessionId, home, stop);
    await session.find("#app > header");
    return session;
  } catch (e) {
    // Never leave Xvfb or tauri-driver behind: node --test would wait for them forever.
    stop();
    throw e;
  }
}

class Session {
  constructor(base, id, home, stop) {
    this.url = `${base}/session/${id}`;
    this.home = home;
    this.stop = stop;
  }

  async close() {
    try {
      await call(this.url, "DELETE", "");
    } finally {
      this.stop();
    }
  }

  /** The first element matching `css`, waiting up to `ms` for it to appear. */
  async find(css, ms = 10000) {
    let found;
    await waitFor(
      async () => {
        const r = await fetch(`${this.url}/element`, post({ using: "css selector", value: css }));
        if (!r.ok) return false;
        found = (await r.json()).value[ELEMENT];
        return Boolean(found);
      },
      ms,
      css,
    );
    return new Element(this.url, found);
  }

  /** Whether an element matching `css` is on the screen now. */
  async has(css) {
    const r = await call(this.url, "POST", "/elements", { using: "css selector", value: css });
    return r.length > 0;
  }

  /** Wait until the screen's text contains `text`. */
  async waitText(text, ms = 15000) {
    await waitFor(async () => (await this.text()).includes(text), ms, `text "${text}"`);
  }

  async text() {
    return (await this.find("#app")).text();
  }

  /** The attribute `name` of the first element matching `css`. */
  async attribute(css, name) {
    const el = await this.find(css);
    return call(this.url, "GET", `/element/${el.id}/attribute/${name}`);
  }

  async click(css) {
    await (await this.find(css)).click();
  }

  /** Clear the field `css` and type `text` into it. */
  async type(css, text) {
    const el = await this.find(css);
    await call(this.url, "POST", `/element/${el.id}/clear`, {});
    await el.sendKeys(text);
  }

  /** Save a PNG of the window as `<name>.png` in the screenshots folder. */
  async screenshot(name) {
    const png = await call(this.url, "GET", "/screenshot");
    mkdirSync(SHOTS, { recursive: true });
    writeFileSync(join(SHOTS, `${name}.png`), Buffer.from(png, "base64"));
  }
}

class Element {
  constructor(url, id) {
    this.url = url;
    this.id = id;
  }

  click() {
    return call(this.url, "POST", `/element/${this.id}/click`, {});
  }

  sendKeys(text) {
    return call(this.url, "POST", `/element/${this.id}/value`, { text });
  }

  text() {
    return call(this.url, "GET", `/element/${this.id}/text`);
  }

  value() {
    return call(this.url, "GET", `/element/${this.id}/property/value`);
  }
}

function post(body) {
  return { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) };
}

async function call(base, method, path, body) {
  const init = body === undefined ? { method } : { ...post(body), method };
  const res = await fetch(`${base}${path}`, init);
  const json = await res.json();
  if (!res.ok) throw new Error(`${method} ${path}: ${JSON.stringify(json.value)}`);
  return json.value;
}

async function waitFor(check, ms, what) {
  const end = Date.now() + ms;
  let last;
  while (Date.now() < end) {
    try {
      if (await check()) return;
    } catch (e) {
      last = e;
    }
    await sleep(200);
  }
  throw new Error(`timed out waiting for ${what}${last ? `: ${last.message}` : ""}`);
}
