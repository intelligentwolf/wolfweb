// WolfWeb's menu bar: File, Edit, View, Bookmarks, Tools, Window, Help folders (and 📰 Daily) at the start of the
// bookmarks bar. Items are chrome-extension://<WolfWeb>/do.html#… addresses that WolfWeb's Rust side runs instead of
// navigating (src/menu.rs; commands: src/actions.rs chrome_command).
/* global WOLFWEB_FEATURES */
// eslint-disable-next-line no-unused-vars
const WOLFWEB_MENUBAR = (() => {
  const base = chrome.runtime.getURL("do.html");
  const cmd = (title, name) => ({ title, url: `${base}#cmd=${name}` });
  const page = (title, path) => ({ title, url: `${base}#page=${encodeURIComponent(path)}` });
  const open = (title, url) => ({ title, url: `${base}#open=${encodeURIComponent(url)}` });

  const MENUS = [
    { title: "File", items: [
      cmd("New tab  (Ctrl+T)", "new-tab"), cmd("New window  (Ctrl+N)", "new-window"),
      cmd("New private window  (Ctrl+Shift+N)", "new-private-window"), cmd("Open file…  (Ctrl+O)", "open-file"),
      cmd("Save page as…  (Ctrl+S)", "save-page"), cmd("Print…  (Ctrl+P)", "print"),
      cmd("Close tab  (Ctrl+W)", "close-tab"), cmd("Close window  (Ctrl+Shift+W)", "close-window"), cmd("Quit WolfWeb", "exit"),
    ] },
    { title: "Edit", items: [
      cmd("Cut  (Ctrl+X)", "cut"), cmd("Copy  (Ctrl+C)", "copy"), cmd("Paste  (Ctrl+V)", "paste"), cmd("Find…  (Ctrl+F)", "find"),
    ] },
    { title: "View", items: [
      cmd("Zoom in  (Ctrl++)", "zoom-in"), cmd("Zoom out  (Ctrl+−)", "zoom-out"), cmd("Actual size  (Ctrl+0)", "zoom-reset"),
      cmd("Full screen  (F11)", "fullscreen"), cmd("Vertical tabs on/off", "vertical-tabs"), cmd("Split view (tile tabs)", "split-view"),
      cmd("Reload  (Ctrl+R)", "reload"), cmd("View page source  (Ctrl+U)", "view-source"),
    ] },
    { title: "Bookmarks", items: [
      cmd("Bookmark this tab  (Ctrl+D)", "bookmark-this-tab"), cmd("Bookmark manager  (Ctrl+Shift+O)", "bookmark-manager"),
      page("Bring your bookmarks from another browser", "import.html"),
    ] },
    { title: "Tools", items: [
      page("📰 Your newspaper", "daily.html"), cmd("Downloads  (Ctrl+J)", "downloads"), cmd("History  (Ctrl+H)", "history"),
      cmd("Passwords", "passwords"), cmd("Extensions", "extensions"), open("Get extensions (Chrome Web Store)", "https://chromewebstore.google.com/category/extensions"),
      page("Command chains", "chains.html"), cmd("Task manager  (Shift+Esc)", "task-manager"), cmd("Developer tools  (Ctrl+Shift+I)", "dev-tools"),
      cmd("Clear browsing data…  (Ctrl+Shift+Del)", "clear-browsing-data"), page("WolfWeb settings", "settings.html"), cmd("All settings", "settings"),
    ] },
    { title: "Window", items: [
      cmd("Next tab  (Ctrl+Tab)", "next-tab"), cmd("Previous tab  (Ctrl+Shift+Tab)", "previous-tab"), cmd("Duplicate tab", "duplicate-tab"),
      cmd("Pin or unpin tab", "pin-tab"), cmd("Reopen closed tab  (Ctrl+Shift+T)", "reopen-tab"), cmd("Search open tabs", "tab-search"),
    ] },
    { title: "Help", items: [
      open("WolfWeb on GitHub", "https://github.com/intelligentwolf/wolfweb"), open("Get WolfMail", "https://github.com/intelligentwolf/wolfmail/releases"),
      open("♥ Sponsor WolfWeb", "https://github.com/sponsors/intelligentwolf"), cmd("About WolfWeb", "about"),
    ] },
  ];
  // Straight to the page (no menu address), so the button shows the newspaper icon once the paper has been opened.
  const DAILY = { title: "📰 Daily", url: chrome.runtime.getURL("daily.html") };

  async function barId() {
    const [root] = await chrome.bookmarks.getTree();
    const bar = root.children.find((c) => c.folderType === "bookmarks-bar") || root.children[0];
    return bar.id;
  }

  /** Remove what WolfWeb made earlier (ids kept in storage). */
  async function removeOurs() {
    const { menuBarIds } = await chrome.storage.local.get({ menuBarIds: [] });
    for (const id of menuBarIds) await chrome.bookmarks.removeTree(id).catch(() => {});
    await chrome.storage.local.set({ menuBarIds: [] });
  }

  async function ourItemsStillThere() {
    const { menuBarIds } = await chrome.storage.local.get({ menuBarIds: [] });
    if (!menuBarIds.length) return false;
    const found = await chrome.bookmarks.get(menuBarIds).catch(() => []);
    return found.length === menuBarIds.length;
  }

  /** Make the menus and/or the Daily button match the switches. Rebuilds both so their order stays right. */
  async function sync() {
    const f = await WOLFWEB_FEATURES.get();
    const want = (f.menuBar ? MENUS.length : 0) + (f.dailyButton ? 1 : 0);
    const { menuBarIds, menuBarShape } = await chrome.storage.local.get({ menuBarIds: [], menuBarShape: "" });
    const shape = `${f.menuBar}-${f.dailyButton}-v2`;
    if (menuBarIds.length === want && menuBarShape === shape && (want === 0 || (await ourItemsStillThere()))) return;
    await removeOurs();
    const parentId = await barId();
    const ids = [];
    let index = 0;
    if (f.menuBar) {
      for (const m of MENUS) {
        const folder = await chrome.bookmarks.create({ parentId, index: index++, title: m.title });
        ids.push(folder.id);
        for (const item of m.items) await chrome.bookmarks.create({ parentId: folder.id, ...item });
      }
    }
    if (f.dailyButton) {
      const d = await chrome.bookmarks.create({ parentId, index: index++, ...DAILY });
      ids.push(d.id);
    }
    await chrome.storage.local.set({ menuBarIds: ids, menuBarShape: shape });
  }

  return { sync };
})();
