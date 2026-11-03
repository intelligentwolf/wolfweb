// Import from other browsers. WolfWeb (Rust, src/import/) reads the other browser's profile and answers on
// wolfweb://app/api/import/…; this page writes what it gets into WolfWeb with Chrome's bookmarks and history APIs.
(() => {
  const API = "wolfweb://app/api/";
  const $ = (id) => document.getElementById(id);
  const progress = $("progress");
  const errorEl = $("error");
  let sources = [];

  async function api(cmd, params = {}) {
    const url = new URL(API + cmd);
    for (const [k, v] of Object.entries(params)) url.searchParams.set(k, String(v));
    let res;
    try {
      res = await fetch(url.toString(), { cache: "no-store" });
    } catch (e) {
      throw new Error("WolfWeb didn't answer. Restart WolfWeb and try again.");
    }
    const body = await res.json().catch(() => null);
    if (!res.ok || !body || body.ok === false) throw new Error((body && body.error) || `WolfWeb refused that (${res.status}).`);
    return body;
  }

  // ── Which browser ──────────────────────────────────────────────────────────────────────────────────────────
  async function loadSources() {
    try {
      sources = (await api("import/sources")).sources;
    } catch (e) {
      errorEl.textContent = `Couldn't look for other browsers: ${e.message}`;
      sources = [];
    }
    $("sources-loading").hidden = true;
    const box = $("sources");
    box.replaceChildren(
      ...sources.map((s, i) => {
        const label = document.createElement("label");
        label.className = "source";
        const radio = document.createElement("input");
        radio.type = "radio";
        radio.name = "source";
        radio.value = String(i);
        radio.checked = i === 0;
        const text = document.createElement("span");
        const name = document.createElement("span");
        name.className = "name";
        name.textContent = s.browser;
        const sub = document.createElement("span");
        sub.className = "sub";
        sub.textContent = s.profile;
        text.append(name, sub);
        label.append(radio, text);
        return label;
      }),
    );
    const any = sources.length > 0;
    $("sources-box").hidden = !any;
    $("what").hidden = !any;
    $("go").hidden = !any;
    $("sources-empty").hidden = any;
  }

  const chosen = () => {
    const r = document.querySelector('input[name="source"]:checked');
    return r ? sources[Number(r.value)] : null;
  };

  // ── Bookmarks ──────────────────────────────────────────────────────────────────────────────────────────────
  async function existingUrls() {
    const urls = new Set();
    const walk = (n) => {
      if (n.url) urls.add(n.url);
      (n.children || []).forEach(walk);
    };
    (await chrome.bookmarks.getTree()).forEach(walk);
    return urls;
  }

  /** A folder named `title` under `parentId`, reused if it is already there (re-running the import). */
  async function folder(parentId, title) {
    const kids = await chrome.bookmarks.getChildren(parentId);
    const found = kids.find((k) => !k.url && k.title === title);
    return found || chrome.bookmarks.create({ parentId, title });
  }

  async function addTree(parentId, nodes, known, counter) {
    for (const n of nodes) {
      if (n.url) {
        if (known.has(n.url)) continue;
        await chrome.bookmarks.create({ parentId, title: n.title, url: n.url });
        known.add(n.url);
        counter.n += 1;
        if (counter.n % 50 === 0) progress.textContent = `Bookmarks: ${counter.n} added…`;
      } else {
        const f = await folder(parentId, n.title || "Folder");
        await addTree(f.id, n.children || [], known, counter);
      }
    }
  }

  async function importBookmarks(source) {
    const { bookmarks } = await api("import/bookmarks", { source: source.key });
    const [root] = await chrome.bookmarks.getTree();
    // Chrome marks its permanent folders with folderType (bookmarks-bar, other); fall back to their order.
    const bar = root.children.find((c) => c.folderType === "bookmarks-bar") || root.children[0];
    const other = root.children.find((c) => c.folderType === "other") || root.children[1];
    const label = `Imported from ${source.browser}`;
    const known = await existingUrls();
    const counter = { n: 0 };
    for (const top of bookmarks) {
      let target;
      if (top.title === "Bookmarks bar") {
        // An empty bar takes them as they were; otherwise they go in a folder on the bar (as Chrome's importer does).
        target = bar.children && bar.children.length === 0 ? bar : await folder(bar.id, label);
      } else {
        target = await folder(other.id, `${top.title} (${source.browser})`);
      }
      await addTree(target.id, top.children || [], known, counter);
    }
    return counter.n;
  }

  // ── History ────────────────────────────────────────────────────────────────────────────────────────────────
  async function importHistory(source) {
    // WolfWeb stages it with the original dates and visit counts and adds it at the next start (Chrome's extension
    // API could only add addresses "visited now" — src/import/pending.rs).
    const { staged } = await api("import/history", { source: source.key, limit: 5000 });
    return staged;
  }

  // ── Extensions ─────────────────────────────────────────────────────────────────────────────────────────────
  async function listExtensions(source) {
    const { extensions } = await api("import/extensions", { source: source.key });
    const ul = $("ext-list");
    ul.replaceChildren(
      ...extensions.map((e) => {
        const li = document.createElement("li");
        const name = document.createElement("span");
        name.textContent = e.name;
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "btn";
        btn.textContent = "Add to WolfWeb";
        btn.setAttribute("aria-label", `Add ${e.name} to WolfWeb`);
        btn.addEventListener("click", () => chrome.tabs.create({ url: e.store_url }));
        li.append(name, btn);
        return li;
      }),
    );
    $("ext-result").hidden = extensions.length === 0;
    return extensions.length;
  }

  $("go").addEventListener("click", async () => {
    const source = chosen();
    if (!source) return;
    const go = $("go");
    go.disabled = true;
    errorEl.textContent = "";
    const done = [];
    const failed = [];
    const step = async (want, label, fn) => {
      if (!$(want).checked) return;
      progress.textContent = `${label}…`;
      try {
        done.push(await fn(source));
      } catch (e) {
        failed.push(`${label}: ${e.message}`);
      }
    };
    await step("want-bookmarks", "Bookmarks", async (s) => `${await importBookmarks(s)} bookmarks`);
    await step("want-history", "History", async (s) => `${await importHistory(s)} history pages (they appear after you restart WolfWeb)`);
    await step("want-extensions", "Extensions", async (s) => `${await listExtensions(s)} extensions listed below`);
    progress.textContent = done.length ? `Done — from ${source.browser} (${source.profile}): ${done.join(", ")}.` : "";
    errorEl.textContent = failed.join(" · ");
    go.disabled = false;
  });

  document.querySelectorAll("[data-open]").forEach((el) =>
    el.addEventListener("click", () =>
      chrome.tabs.create({ url: el.dataset.open }).catch((e) => (errorEl.textContent = `Couldn't open that page: ${e.message}`)),
    ),
  );

  loadSources();
})();
