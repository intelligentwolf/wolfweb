// The commands WolfWeb's quick-commands palette and command chains can run (like Vivaldi's Quick Commands).
// Each run(ctx) gets { tab, windowId } for the tab that was active when the palette opened.
// eslint-disable-next-line no-unused-vars
const WOLFWEB_COMMANDS = (() => {
  const open = (url) => chrome.tabs.create({ url });
  const page = (path) => open(chrome.runtime.getURL(path));
  /** WolfWeb's native API (src/scheme.rs); throws with a message fit to show. */
  async function api(cmd, params = {}) {
    const url = new URL(`wolfweb://app/api/${cmd}`);
    for (const [k, v] of Object.entries(params)) url.searchParams.set(k, String(v));
    let res;
    try {
      res = await fetch(url.toString(), { cache: "no-store" });
    } catch {
      throw new Error("WolfWeb didn't answer. Restart WolfWeb and try again.");
    }
    const body = await res.json().catch(() => null);
    if (!res.ok || !body || body.ok === false) throw new Error((body && body.error) || `WolfWeb refused that (${res.status}).`);
    return body;
  }
  const chromium = (name) => () => api("browser/command", { name });
  const list = [
    { id: "new-tab", title: "New tab", keys: "Ctrl+T", run: () => chrome.tabs.create({}) },
    { id: "new-window", title: "New window", keys: "Ctrl+N", run: () => chrome.windows.create({}) },
    { id: "new-private-window", title: "New private window", keys: "Ctrl+Shift+N", run: () => chrome.windows.create({ incognito: true }) },
    { id: "close-tab", title: "Close tab", keys: "Ctrl+W", run: ({ tab }) => tab && chrome.tabs.remove(tab.id) },
    { id: "reopen-tab", title: "Reopen closed tab", keys: "Ctrl+Shift+T", run: () => chrome.sessions.restore() },
    { id: "duplicate-tab", title: "Duplicate tab", run: ({ tab }) => tab && chrome.tabs.duplicate(tab.id) },
    { id: "reload", title: "Reload page", keys: "Ctrl+R", run: ({ tab }) => tab && chrome.tabs.reload(tab.id) },
    { id: "pin-tab", title: "Pin or unpin tab", run: ({ tab }) => tab && chrome.tabs.update(tab.id, { pinned: !tab.pinned }) },
    { id: "mute-tab", title: "Mute or unmute tab", run: ({ tab }) => tab && chrome.tabs.update(tab.id, { muted: !(tab.mutedInfo && tab.mutedInfo.muted) }) },
    { id: "group-tab", title: "Put tab in a new group (stack)", run: ({ tab }) => tab && chrome.tabs.group({ tabIds: [tab.id] }) },
    { id: "ungroup-tab", title: "Take tab out of its group", run: ({ tab }) => tab && chrome.tabs.ungroup([tab.id]) },
    { id: "tab-to-window", title: "Move tab to a new window", run: ({ tab }) => tab && chrome.windows.create({ tabId: tab.id }) },
    {
      id: "close-other-tabs", title: "Close other tabs",
      run: async ({ tab }) => {
        if (!tab) return;
        const others = await chrome.tabs.query({ windowId: tab.windowId, pinned: false });
        await chrome.tabs.remove(others.filter((t) => t.id !== tab.id).map((t) => t.id));
      },
    },
    {
      id: "next-tab", title: "Next tab", keys: "Ctrl+Tab",
      run: async ({ tab }) => {
        if (!tab) return;
        const tabs = await chrome.tabs.query({ windowId: tab.windowId });
        const next = tabs[(tabs.findIndex((t) => t.id === tab.id) + 1) % tabs.length];
        await chrome.tabs.update(next.id, { active: true });
      },
    },
    {
      id: "previous-tab", title: "Previous tab", keys: "Ctrl+Shift+Tab",
      run: async ({ tab }) => {
        if (!tab) return;
        const tabs = await chrome.tabs.query({ windowId: tab.windowId });
        const prev = tabs[(tabs.findIndex((t) => t.id === tab.id) - 1 + tabs.length) % tabs.length];
        await chrome.tabs.update(prev.id, { active: true });
      },
    },
    { id: "split-view", title: "Split view — tile this tab with another", run: chromium("split-view") },
    { id: "vertical-tabs", title: "Vertical tabs on or off", run: chromium("vertical-tabs") },
    { id: "vertical-tabs-collapse", title: "Collapse or expand vertical tabs", run: chromium("vertical-tabs-collapse") },
    { id: "tab-search", title: "Search open tabs", run: chromium("tab-search") },
    { id: "panel", title: "Open the side panel (notes, web panels, WolfMail)", run: ({ windowId }) => chrome.sidePanel.open({ windowId }) },
    { id: "wolfmail", title: "Open WolfMail", run: () => api("wolfmail/open") },
    { id: "bookmarks", title: "Bookmarks", keys: "Ctrl+Shift+O", run: () => open("chrome://bookmarks") },
    { id: "history", title: "History", keys: "Ctrl+H", run: () => open("chrome://history") },
    { id: "downloads", title: "Downloads", keys: "Ctrl+J", run: () => open("chrome://downloads") },
    { id: "extensions", title: "Extensions", run: () => open("chrome://extensions") },
    { id: "web-store", title: "Get extensions (Chrome Web Store)", run: () => open("https://chromewebstore.google.com/category/extensions") },
    { id: "passwords", title: "Passwords", run: () => open("chrome://password-manager/passwords") },
    { id: "settings", title: "Settings", run: () => open("chrome://settings") },
    { id: "daily", title: "Your newspaper (The WolfWeb Daily)", run: () => page("daily.html") },
    { id: "import", title: "Bring your stuff from another browser", run: () => page("import.html") },
    { id: "edit-chains", title: "Edit command chains", run: () => page("chains.html") },
  ];
  const byId = new Map(list.map((c) => [c.id, c]));

  /** Run a chain's steps in order; a step that fails stops the chain and reports which one. */
  async function runChain(chain, ctx) {
    for (const [i, id] of chain.steps.entries()) {
      const cmd = byId.get(id);
      if (!cmd) throw new Error(`Step ${i + 1} ("${id}") isn't a WolfWeb command any more.`);
      try {
        await cmd.run(ctx);
      } catch (e) {
        throw new Error(`Step ${i + 1} ("${cmd.title}") failed: ${e.message}`);
      }
      // Later steps act on whatever is active now (e.g. after "New tab").
      const [active] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
      ctx = { tab: active, windowId: active ? active.windowId : ctx.windowId };
    }
  }

  async function chains() {
    const got = await chrome.storage.local.get({ chains: [] });
    return Array.isArray(got.chains) ? got.chains : [];
  }

  return { list, byId, runChain, chains };
})();
