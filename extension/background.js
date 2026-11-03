// WolfWeb extension service worker.
/* global WOLFWEB_FEATURES, WOLFWEB_MENUBAR */
importScripts("features.js", "engine.js", "menubar.js");

// Menu bar and 📰 Daily button on the bookmarks bar, kept in step with WolfWeb settings.
const syncMenuBar = () => WOLFWEB_MENUBAR.sync().catch((e) => console.error("WolfWeb menu bar:", e));
chrome.runtime.onStartup.addListener(syncMenuBar);
chrome.runtime.onInstalled.addListener(syncMenuBar);
// Also whenever the worker starts: for an extension loaded from the command line Chromium doesn't reliably send
// onStartup/onInstalled to an existing profile (observed — the menus never appeared). sync() is cheap when the
// menus are already right.
syncMenuBar();
chrome.storage.onChanged.addListener((changes, area) => {
  if (area === "local" && (changes.features || changes.engine)) syncMenuBar();
});

// First run: offer to bring bookmarks, history, passwords and extensions over from other browsers.
// WolfWeb loads this extension with --load-extension, which Chromium reports as a fresh "install" at every start
// (observed), so a stored flag — not the install reason alone — decides whether this is really the first run.
chrome.runtime.onInstalled.addListener(async (details) => {
  if (details.reason !== "install") return;
  const { welcomed } = await chrome.storage.local.get({ welcomed: false });
  if (welcomed) return;
  await chrome.storage.local.set({ welcomed: true });
  await chrome.tabs.create({ url: chrome.runtime.getURL("import.html#welcome") });
});

// The toolbar button opens quick commands (its popup); the notes panel opens from there or the side-panel menu.
chrome.sidePanel.setPanelBehavior({ openPanelOnActionClick: false }).catch(() => {});

// Mouse gestures (gestures.js) and the status bar (statusbar.js) ask for what a page can't do itself.
chrome.runtime.onMessage.addListener((msg, sender, reply) => {
  if (!msg || !sender.tab) return;
  const tabId = sender.tab.id;
  if (msg.gesture === "close-tab") chrome.tabs.remove(tabId);
  else if (msg.gesture === "new-tab") chrome.tabs.create({ windowId: sender.tab.windowId, index: sender.tab.index + 1 });
  else if (msg.statusBar === "zoom-get") {
    chrome.tabs.getZoom(tabId).then((zoom) => reply({ zoom }), () => reply({}));
    return true; // answer asynchronously
  } else if (msg.statusBar === "zoom-in" || msg.statusBar === "zoom-out" || msg.statusBar === "zoom-reset") {
    chrome.tabs.getZoom(tabId).then((z) => {
      const next = msg.statusBar === "zoom-reset" ? 0 : Math.min(5, Math.max(0.25, z + (msg.statusBar === "zoom-in" ? 0.1 : -0.1)));
      return chrome.tabs.setZoom(tabId, next);
    });
  } else if (msg.statusBar === "settings") {
    chrome.tabs.create({ url: chrome.runtime.getURL("settings.html"), index: sender.tab.index + 1 });
  }
});

// Tell a page's status bar when its zoom changes (from any source: keyboard, menu, status bar).
chrome.tabs.onZoomChange.addListener(({ tabId, newZoomFactor }) => {
  chrome.tabs.sendMessage(tabId, { zoomChanged: newZoomFactor }).catch(() => {});
});
