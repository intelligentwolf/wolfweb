// Quick commands (Ctrl+Shift+Space): one box to find open tabs, bookmarks, history, commands and command chains.
/* global WOLFWEB_COMMANDS */
(() => {
  const input = document.getElementById("q");
  const resultsEl = document.getElementById("results");
  const emptyEl = document.getElementById("empty");
  const errorEl = document.getElementById("error");
  let ctx = { tab: null, windowId: undefined };
  let items = [];
  let selected = 0;
  let seq = 0;

  const favicon = (url) => {
    const u = new URL(chrome.runtime.getURL("/_favicon/"));
    u.searchParams.set("pageUrl", url);
    u.searchParams.set("size", "16");
    return u.toString();
  };
  const matches = (q, ...fields) => fields.some((f) => f && f.toLowerCase().includes(q));

  async function search(text) {
    const mine = ++seq;
    const q = text.trim().toLowerCase();
    const out = [];
    const chains = await WOLFWEB_COMMANDS.chains().catch(() => []);
    for (const c of chains) {
      if (!q || matches(q, c.name)) {
        out.push({ kind: "Chain", title: c.name, sub: `${c.steps.length} steps`, run: () => WOLFWEB_COMMANDS.runChain(c, ctx) });
      }
    }
    for (const c of WOLFWEB_COMMANDS.list) {
      if (!q || matches(q, c.title, c.id)) out.push({ kind: "Command", title: c.title, keys: c.keys, run: () => c.run(ctx) });
    }
    if (q) {
      const [tabs, marks, hist] = await Promise.all([
        chrome.tabs.query({}),
        chrome.bookmarks.search(q).catch(() => []),
        chrome.history.search({ text: q, maxResults: 20, startTime: 0 }).catch(() => []),
      ]);
      for (const t of tabs) {
        if (matches(q, t.title, t.url)) {
          out.unshift({
            kind: "Tab", title: t.title || t.url, sub: t.url, icon: t.url,
            run: async () => {
              await chrome.tabs.update(t.id, { active: true });
              await chrome.windows.update(t.windowId, { focused: true });
            },
          });
        }
      }
      for (const b of marks.filter((b) => b.url).slice(0, 15)) {
        out.push({ kind: "Bookmark", title: b.title || b.url, sub: b.url, icon: b.url, run: () => chrome.tabs.create({ url: b.url }) });
      }
      for (const h of hist) {
        out.push({ kind: "History", title: h.title || h.url, sub: h.url, icon: h.url, run: () => chrome.tabs.create({ url: h.url }) });
      }
    }
    if (mine !== seq) return; // a newer keystroke won
    items = out.slice(0, 60);
    selected = 0;
    render();
  }

  function render() {
    resultsEl.replaceChildren(
      ...items.map((it, i) => {
        const li = document.createElement("li");
        li.id = `r${i}`;
        li.setAttribute("role", "option");
        li.setAttribute("aria-selected", String(i === selected));
        const kind = document.createElement("span");
        kind.className = "kind";
        kind.textContent = it.kind;
        li.appendChild(kind);
        if (it.icon) {
          const img = document.createElement("img");
          img.alt = "";
          img.src = favicon(it.icon);
          li.appendChild(img);
        }
        const text = document.createElement("span");
        text.className = "text";
        const title = document.createElement("span");
        title.className = "title";
        title.textContent = it.title;
        text.appendChild(title);
        if (it.sub) {
          const sub = document.createElement("span");
          sub.className = "sub";
          sub.textContent = it.sub;
          text.appendChild(sub);
        }
        li.appendChild(text);
        if (it.keys) {
          const keys = document.createElement("span");
          keys.className = "keys";
          keys.textContent = it.keys;
          li.appendChild(keys);
        }
        li.addEventListener("click", () => choose(i));
        return li;
      }),
    );
    emptyEl.hidden = items.length > 0;
    input.setAttribute("aria-activedescendant", items.length ? `r${selected}` : "");
    const sel = resultsEl.children[selected];
    if (sel) sel.scrollIntoView({ block: "nearest" });
  }

  async function choose(i) {
    const it = items[i];
    try {
      if (it) {
        await it.run();
      } else if (input.value.trim()) {
        const u = new URL("https://www.google.com/search");
        u.searchParams.set("q", input.value.trim());
        await chrome.tabs.create({ url: u.toString() });
      }
      window.close();
    } catch (e) {
      errorEl.textContent = `That didn't work: ${e.message}`;
    }
  }

  input.addEventListener("input", () => search(input.value));
  input.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown" && items.length) {
      e.preventDefault();
      selected = (selected + 1) % items.length;
      render();
    } else if (e.key === "ArrowUp" && items.length) {
      e.preventDefault();
      selected = (selected - 1 + items.length) % items.length;
      render();
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(items.length ? selected : -1);
    }
  });

  chrome.tabs
    .query({ active: true, lastFocusedWindow: true })
    .then(([tab]) => {
      ctx = { tab: tab || null, windowId: tab ? tab.windowId : undefined };
    })
    .finally(() => search(""));
  input.focus();
})();
