// Pinned tabs, remembered between runs: WolfWeb keeps the list of pinned tabs (chrome.storage.local "pinnedTabs")
// and pins them again in the first window each time WolfWeb starts, whatever the startup setting — the home page
// or the last session (WolfWeb's Rust side doesn't know which tabs are pinned; only the tabs API does).
//
// The list only loses a page when the user takes it off: unpins it, closes the pinned tab, or the pinned tab moves to
// another address (the new address replaces the old one). Everything else — a start that's slow to bring tabs back,
// a window that never showed, quitting (chrome.tabs.onRemoved's removeInfo.isWindowClosing) — can't shorten it:
// pages on the list that aren't open now are kept, after the pinned tabs that are, in their old order.
//
// - Restoring runs once per browser start (chrome.storage.session is cleared when the browser restarts; the service
//   worker itself can stop and start again in the middle of a session). Until it has, nothing is saved.
// - Which pinned tab shows which page is kept in chrome.storage.session too ("pinnedKnown", tab id → address), so a
//   pinned tab closed after the worker restarted is still taken off the list.
// - A page already open in that window (e.g. brought back by "Continue where you left off") is pinned rather than
//   opened twice. Private (incognito) tabs are never remembered.
// eslint-disable-next-line no-unused-vars
const WOLFWEB_PINNED = (() => {
  const SAVE_DELAY_MS = 500;
  let saveTimer = null;
  let removedSoon = []; // addresses the user took off since the last save
  let queue = Promise.resolve(); // storage updates, one at a time

  const serially = (job) => (queue = queue.then(job, job));
  const urlOf = (t) => t.pendingUrl || t.url || "";

  async function restored() {
    const { pinnedRestored } = await chrome.storage.session.get({ pinnedRestored: false });
    return pinnedRestored;
  }

  async function getKnown() {
    const { pinnedKnown } = await chrome.storage.session.get({ pinnedKnown: {} });
    return pinnedKnown || {};
  }

  /** The pinned tabs now (non-private), in window then tab order. */
  async function currentTabs() {
    const windows = await chrome.windows.getAll({ populate: true, windowTypes: ["normal"] });
    const tabs = [];
    for (const w of windows) {
      if (w.incognito) continue;
      for (const t of w.tabs) if (t.pinned && urlOf(t)) tabs.push(t);
    }
    return tabs;
  }

  /** Remove one occurrence of each of `urls` from `list`. */
  function without(list, urls) {
    const out = list.slice();
    for (const u of urls) {
      const i = out.indexOf(u);
      if (i !== -1) out.splice(i, 1);
    }
    return out;
  }

  /** The open pinned tabs in order, then the saved pages that aren't open, in their saved order. */
  function merge(open, saved) {
    return open.concat(without(saved, open));
  }

  function saveSoon(removed = []) {
    removedSoon.push(...removed);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      const gone = removedSoon;
      removedSoon = [];
      serially(async () => {
        try {
          if (!(await restored())) return; // the list from last time hasn't been put back yet
          const tabs = await currentTabs();
          const { pinnedTabs } = await chrome.storage.local.get({ pinnedTabs: [] });
          const saved = without(Array.isArray(pinnedTabs) ? pinnedTabs : [], gone);
          const known = {};
          for (const t of tabs) known[t.id] = urlOf(t);
          await chrome.storage.local.set({ pinnedTabs: merge(tabs.map(urlOf), saved) });
          await chrome.storage.session.set({ pinnedKnown: known });
        } catch (e) {
          console.error("WolfWeb pinned tabs:", e);
        }
      });
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
  function restore() {
    return serially(async () => {
      if (await restored()) return;
      const { pinnedTabs } = await chrome.storage.local.get({ pinnedTabs: [] });
      const saved = (Array.isArray(pinnedTabs) ? pinnedTabs : []).filter((u) => typeof u === "string" && u);
      if (saved.length) {
        const w = await firstWindow();
        const taken = new Set();
        for (let i = 0; i < saved.length; i++) {
          const open = w.tabs.find((t) => !taken.has(t.id) && urlOf(t) === saved[i]);
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
    }).then(() => saveSoon());
  }

  function listen() {
    chrome.tabs.onUpdated.addListener((id, change, tab) => {
      if (tab.incognito) return;
      if (change.pinned === false) {
        // Unpinned: off the list (the address it had while pinned, if known).
        getKnown().then((known) => saveSoon([known[id] || urlOf(tab)]));
      } else if (change.pinned === true) {
        // Known straight away, so closing it before the next save still takes it off the list.
        serially(async () => {
          const known = await getKnown();
          known[id] = urlOf(tab);
          await chrome.storage.session.set({ pinnedKnown: known });
        }).then(() => saveSoon());
      } else if (tab.pinned && change.url) {
        // A pinned tab went to another address: that one replaces the old.
        getKnown().then((known) => saveSoon(known[id] ? [known[id]] : []));
      }
    });
    chrome.tabs.onRemoved.addListener((id, info) => {
      getKnown().then((known) => {
        if (!(id in known)) return;
        // Closed by the user: off the list. Closed by its window closing (as when WolfWeb quits): kept.
        saveSoon(info.isWindowClosing ? [] : [known[id]]);
      });
    });
    chrome.tabs.onMoved.addListener(() => saveSoon());
    chrome.tabs.onAttached.addListener(() => saveSoon());
    chrome.tabs.onDetached.addListener(() => saveSoon());
  }

  return { restore, listen };
})();
