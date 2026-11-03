// What WolfWeb's engine can do itself. WolfWeb's own build of the engine has a built-in menu bar along the top of
// each window (build/cef/patches/wolfweb-menu-bar.patch); the standard engine doesn't, and then the menus live on the
// bookmarks bar instead (menubar.js). Only WolfWeb's pages can ask (src/actions.rs engine/features), so they check
// and store the answer ("engine" in chrome.storage.local) for the background worker.
/* global WOLFWEB_FEATURES */
// eslint-disable-next-line no-unused-vars
const WOLFWEB_ENGINE = (() => {
  async function api(cmd) {
    const res = await fetch(`wolfweb://app/api/${cmd}`, { cache: "no-store" });
    const body = await res.json().catch(() => null);
    if (!res.ok || !body || body.ok === false) throw new Error((body && body.error) || `WolfWeb refused that (${res.status}).`);
    return body;
  }

  /** The stored answer (background worker); false until a page has asked. */
  async function nativeMenuBar() {
    const { engine } = await chrome.storage.local.get({ engine: {} });
    return !!(engine && engine.nativeMenuBar);
  }

  /** Ask the engine, store the answer, and make the built-in menu bar match the "Menu bar" switch. */
  async function check() {
    const { nativeMenuBar: native } = await api("engine/features");
    const { engine } = await chrome.storage.local.get({ engine: {} });
    if (!engine || engine.nativeMenuBar !== native) await chrome.storage.local.set({ engine: { nativeMenuBar: native } });
    if (native) await setMenuBar((await WOLFWEB_FEATURES.get()).menuBar);
    return native;
  }

  /** Show or hide the built-in menu bar (takes effect in every window at once). */
  async function setMenuBar(on) {
    await api(`menu-bar/set?on=${on ? 1 : 0}`);
  }

  return { nativeMenuBar, check, setMenuBar };
})();
