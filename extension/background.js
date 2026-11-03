// WolfWeb extension service worker.

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

// Mouse gestures (gestures.js) ask for the tab actions a page can't do itself.
chrome.runtime.onMessage.addListener((msg, sender) => {
  if (!msg || !sender.tab) return;
  if (msg.gesture === "close-tab") chrome.tabs.remove(sender.tab.id);
  else if (msg.gesture === "new-tab") chrome.tabs.create({ windowId: sender.tab.windowId, index: sender.tab.index + 1 });
});
