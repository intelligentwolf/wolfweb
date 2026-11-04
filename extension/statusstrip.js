// WolfWeb's status bar as a strip of the window, below the page (WolfWeb's own engine shows this page there:
// build/cef/patches/wolfweb-status-bar.patch, WolfWebStatusBar). One strip per window, told its window's id
// (?window=); it follows that window's active tab: how long the page took and how much there is to read (asked of
// the page's statusbar.js), its zoom (− / % / +), and the time.
(() => {
  const $ = (id) => document.getElementById(id);
  const windowId = Number(new URLSearchParams(location.search).get("window"));
  const ZOOM_STEP = 0.1;
  const ZOOM_MIN = 0.25;
  const ZOOM_MAX = 5;
  let tabId = null;

  function tick() {
    $("clock").textContent = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }

  function describe(info, tab) {
    if (!info) return tab && tab.status === "loading" ? "Loading…" : "";
    const loaded = info.loadedMs != null ? `Loaded in ${(info.loadedMs / 1000).toFixed(1)} s` : "Loading…";
    const minutes = Math.max(1, Math.round(info.words / 230));
    return `${loaded}  ·  ${info.words.toLocaleString()} words  ·  ${minutes} min read`;
  }

  async function showPage(tab) {
    let info = null;
    try {
      // Only web pages have statusbar.js; others (WolfWeb's and Chrome's own pages) have nothing to say.
      if (tab && /^(https?|file):/.test(tab.url || "")) info = await chrome.tabs.sendMessage(tab.id, { statusInfo: true });
    } catch {
      info = null; // the page isn't ready, or isn't one statusbar.js runs in
    }
    if (tab && tab.id === tabId) $("page").textContent = describe(info, tab);
  }

  async function showZoom() {
    if (tabId == null) return;
    try {
      const zoom = await chrome.tabs.getZoom(tabId);
      $("zoom-reset").textContent = `${Math.round(zoom * 100)}%`;
    } catch {
      $("zoom-reset").textContent = "";
    }
  }

  /** Follow the window's active tab. */
  async function follow() {
    if (!Number.isInteger(windowId)) return;
    let tab;
    try {
      [tab] = await chrome.tabs.query({ active: true, windowId });
    } catch {
      return;
    }
    tabId = tab ? tab.id : null;
    const zoomable = !!tab;
    for (const id of ["zoom-out", "zoom-reset", "zoom-in"]) $(id).disabled = !zoomable;
    await Promise.all([showPage(tab), showZoom()]);
  }

  async function zoom(change) {
    if (tabId == null) return;
    const now = await chrome.tabs.getZoom(tabId);
    const next = change === 0 ? 0 : Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, now + change));
    await chrome.tabs.setZoom(tabId, next); // 0: back to the default
  }

  $("zoom-out").addEventListener("click", () => zoom(-ZOOM_STEP).catch(() => {}));
  $("zoom-in").addEventListener("click", () => zoom(ZOOM_STEP).catch(() => {}));
  $("zoom-reset").addEventListener("click", () => zoom(0).catch(() => {}));
  $("settings").addEventListener("click", async () => {
    const at = tabId != null ? await chrome.tabs.get(tabId).catch(() => null) : null;
    chrome.tabs.create({ windowId, url: chrome.runtime.getURL("settings.html"), ...(at ? { index: at.index + 1 } : {}) });
  });

  chrome.tabs.onActivated.addListener((info) => {
    if (info.windowId === windowId) follow();
  });
  chrome.tabs.onUpdated.addListener((id, change, tab) => {
    if (id === tabId && (change.status || change.url)) showPage(tab);
  });
  chrome.tabs.onZoomChange.addListener((z) => {
    if (z.tabId === tabId) $("zoom-reset").textContent = `${Math.round(z.newZoomFactor * 100)}%`;
  });
  chrome.windows.onFocusChanged.addListener(() => follow());
  chrome.runtime.onMessage.addListener((msg, sender) => {
    if (msg && msg.statusInfoChanged && sender.tab && sender.tab.id === tabId) showPage(sender.tab);
  });

  // This page only shows in the engine's own strip, so the engine has one: statusbar.js then stops drawing its bar
  // over pages (engine.js stores the same answer when a WolfWeb page asks the engine; this covers the time before).
  chrome.storage.local.get({ engine: {} }).then(({ engine }) => {
    if (!engine || engine.nativeStatusBar !== true) chrome.storage.local.set({ engine: { ...(engine || {}), nativeStatusBar: true } });
  });

  tick();
  setInterval(tick, 15000);
  follow();
})();
