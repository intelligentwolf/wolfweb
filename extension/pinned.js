// Pinned tabs, remembered between runs: WolfWeb keeps the list of pinned tabs (chrome.storage.local "pinnedTabs")
// and pins them again in the first window each time WolfWeb starts, whatever the startup setting — the home page
// or the last session (WolfWeb's Rust side doesn't know which tabs are pinned; only the tabs API does).
//
// - The list follows every pin, unpin, move and close, except the closes that come from a window closing (as when
//   WolfWeb quits: chrome.tabs.onRemoved's removeInfo.isWindowClosing), so quitting doesn't empty it.
// - Restoring runs once per browser start (chrome.storage.session is cleared when the browser restarts; the service
//   worker itself can stop and start again in the middle of a session). Until it has, the list isn't saved over.
// - A page already open in that window (e.g. brought back by "Continue where you left off") is pinned rather than
//   opened twice. Private (incognito) tabs are never remembered.
// eslint-disable-next-line no-unused-vars
const WOLFWEB_PINNED = (() => {
  const SAVE_DELAY_MS = 500;
  let saveTimer = null;

  async function restored() {
    const { pinnedRestored } = await chrome.storage.session.get({ pinnedRestored: false });
    return pinnedRestored;
  }

  /** The pinned tabs now, in window then tab order. */
  async function current() {
    const windows = await chrome.windows.getAll({ populate: true, windowTypes: ["normal"] });
    const urls = [];
    for (const w of windows) {
      if (w.incognito) continue;
      for (const t of w.tabs) {
        const url = t.pendingUrl || t.url;
        if (t.pinned && url) urls.push(url);
      }
    }
    return urls;
  }

  function saveSoon() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(async () => {
      try {
        if (!(await restored())) return; // the list from last time hasn't been put back yet
        await chrome.storage.local.set({ pinnedTabs: await current() });
      } catch (e) {
        console.error("WolfWeb pinned tabs:", e);
      }
    }, SAVE_DELAY_MS);
  }

  /** The first normal, non-private window, waiting for one to open if there's none yet. */
  async function firstWindow() {
    const pick = async () => {
      const windows = await chrome.windows.getAll({ populate: true, windowTypes: ["normal"] });
      return windows.find((w) => !w.incognito) || null;
    };
    const now = await pick();
    if (now) return now;
    return new Promise((resolve) => {
      const onCreated = async (w) => {
        if (w.type !== "normal" || w.incognito) return;
        chrome.windows.onCreated.removeListener(onCreated);
        resolve(await chrome.windows.get(w.id, { populate: true }));
      };
      chrome.windows.onCreated.addListener(onCreated);
    });
  }

  /** Put last run's pinned tabs back (once per browser start). */
  async function restore() {
    if (await restored()) return;
    const { pinnedTabs } = await chrome.storage.local.get({ pinnedTabs: [] });
    const saved = (pinnedTabs || []).filter((u) => typeof u === "string" && u);
    if (saved.length) {
      const w = await firstWindow();
      const taken = new Set();
      for (let i = 0; i < saved.length; i++) {
        const open = w.tabs.find((t) => !taken.has(t.id) && (t.pendingUrl || t.url) === saved[i]);
        if (open) {
          taken.add(open.id);
          await chrome.tabs.update(open.id, { pinned: true });
          await chrome.tabs.move(open.id, { index: i });
        } else {
          const t = await chrome.tabs.create({ windowId: w.id, url: saved[i], pinned: true, index: i, active: false });
          taken.add(t.id);
        }
      }
    }
    await chrome.storage.session.set({ pinnedRestored: true });
  }

  function listen() {
    chrome.tabs.onUpdated.addListener((_id, change, tab) => {
      if ("pinned" in change || (tab.pinned && change.url)) saveSoon();
    });
    chrome.tabs.onRemoved.addListener((_id, info) => {
      if (!info.isWindowClosing) saveSoon();
    });
    chrome.tabs.onMoved.addListener(saveSoon);
    chrome.tabs.onAttached.addListener(saveSoon);
    chrome.tabs.onDetached.addListener(saveSoon);
  }

  return { restore, listen };
})();
