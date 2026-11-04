// What WolfWeb's engine can do itself. WolfWeb's own build of the engine has a built-in menu bar along the top of
// each window (build/cef/patches/wolfweb-menu-bar.patch) and a status bar strip along the bottom, below the page
// (wolfweb-status-bar.patch, showing statusstrip.html); the standard engine has neither, and then the menus live on
// the bookmarks bar (menubar.js) and the status bar is drawn over the bottom of each page (statusbar.js). Only WolfWeb's pages can ask (src/actions.rs engine/features), so they check
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

  async function nativeStatusBar() {
    const { engine } = await chrome.storage.local.get({ engine: {} });
    return !!(engine && engine.nativeStatusBar);
  }

  /** Ask the engine, store the answer, and make its built-in bars match the "Menu bar" and "Status bar" switches. */
  async function check() {
    const features = await api("engine/features");
    const found = { nativeMenuBar: !!features.nativeMenuBar, nativeStatusBar: !!features.nativeStatusBar };
    const { engine } = await chrome.storage.local.get({ engine: {} });
    if (!engine || engine.nativeMenuBar !== found.nativeMenuBar || engine.nativeStatusBar !== found.nativeStatusBar) {
      await chrome.storage.local.set({ engine: found });
    }
    const wanted = await WOLFWEB_FEATURES.get();
    if (found.nativeMenuBar) await setMenuBar(wanted.menuBar);
    if (found.nativeStatusBar) await setStatusBar(wanted.statusBar);
    return found.nativeMenuBar;
  }

  /** Show or hide the built-in menu bar (takes effect in every window at once). */
  async function setMenuBar(on) {
    await api(`menu-bar/set?on=${on ? 1 : 0}`);
  }

  /** Show or hide the built-in status bar strip (every window at once). */
  async function setStatusBar(on) {
    await api(`status-bar/set?on=${on ? 1 : 0}`);
  }

  return { nativeMenuBar, nativeStatusBar, check, setMenuBar, setStatusBar };
})();
