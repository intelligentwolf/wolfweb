// The WolfWeb side panel: Notes (notes.js), Web panels and WolfMail.
(() => {
  const $ = (id) => document.getElementById(id);

  // ── Switcher (WAI-ARIA tabs: arrows move, the panel follows the selected tab) ────────────────────────────────
  const tabs = [...document.querySelectorAll('.switcher [role="tab"]')];
  function select(tab) {
    for (const t of tabs) {
      const on = t === tab;
      t.setAttribute("aria-selected", String(on));
      t.tabIndex = on ? 0 : -1;
      $(t.getAttribute("aria-controls")).hidden = !on;
    }
    chrome.storage.local.set({ panelSection: tab.id }).catch(() => {});
  }
  tabs.forEach((t, i) => {
    t.addEventListener("click", () => select(t));
    t.addEventListener("keydown", (e) => {
      const next = e.key === "ArrowRight" ? tabs[(i + 1) % tabs.length] : e.key === "ArrowLeft" ? tabs[(i - 1 + tabs.length) % tabs.length] : null;
      if (next) {
        e.preventDefault();
        select(next);
        next.focus();
      }
    });
  });
  chrome.storage.local.get({ panelSection: "tab-notes" }).then(({ panelSection }) => {
    const t = tabs.find((x) => x.id === panelSection);
    if (t) select(t);
  });

  // ── Web panels ───────────────────────────────────────────────────────────────────────────────────────────────
  // Sites refuse to be framed by default; extension/rules.json drops those headers for frames this page loads.
  const chips = $("web-chips");
  const frame = $("web-frame");
  const webStatus = $("web-status");
  let panels = [];
  let current = null;

  const favicon = (url) => {
    const u = new URL(chrome.runtime.getURL("/_favicon/"));
    u.searchParams.set("pageUrl", url);
    u.searchParams.set("size", "16");
    return u.toString();
  };

  function normalise(text) {
    const t = text.trim();
    if (!t) return null;
    try {
      const u = new URL(/^[a-z][a-z0-9+.-]*:\/\//i.test(t) ? t : `https://${t}`);
      return /^https?:$/.test(u.protocol) && u.hostname.includes(".") ? u.toString() : null;
    } catch {
      return null;
    }
  }

  async function savePanels() {
    await chrome.storage.local.set({ webPanels: panels });
  }

  function renderPanels() {
    chips.replaceChildren(
      ...panels.map((p) => {
        const b = document.createElement("button");
        b.type = "button";
        b.setAttribute("role", "tab");
        b.setAttribute("aria-selected", String(p.url === current));
        const img = document.createElement("img");
        img.alt = "";
        img.src = favicon(p.url);
        const label = document.createElement("span");
        label.textContent = p.title;
        b.append(img, label);
        b.addEventListener("click", () => show(p.url));
        return b;
      }),
    );
    $("web-empty").hidden = panels.length > 0;
    const p = panels.find((x) => x.url === current);
    $("web-view").hidden = !p;
    if (p) {
      $("web-title").textContent = p.title;
      if (frame.dataset.url !== p.url) {
        frame.dataset.url = p.url;
        frame.src = p.url;
      }
    }
  }

  function show(url) {
    current = url;
    renderPanels();
  }

  async function add(url, title) {
    if (!panels.some((p) => p.url === url)) {
      panels.push({ url, title: title || new URL(url).hostname.replace(/^www\./, "") });
      await savePanels();
    }
    show(url);
  }

  $("web-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const input = $("web-url");
    const url = normalise(input.value);
    $("web-url-error").hidden = !!url;
    input.setAttribute("aria-invalid", String(!url));
    if (!url) {
      input.focus();
      return;
    }
    input.value = "";
    await add(url);
  });

  $("web-add-current").addEventListener("click", async () => {
    const [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
    const url = tab && normalise(tab.url || "");
    if (url) await add(url, tab.title);
    else webStatus.textContent = "This page can't be a web panel — open a website first.";
  });

  $("web-open-tab").addEventListener("click", () => current && chrome.tabs.create({ url: current }));

  // Remove with Undo, not an "are you sure?".
  $("web-remove").addEventListener("click", async () => {
    const at = panels.findIndex((p) => p.url === current);
    if (at < 0) return;
    const [removed] = panels.splice(at, 1);
    current = null;
    frame.removeAttribute("src");
    delete frame.dataset.url;
    await savePanels();
    renderPanels();
    webStatus.textContent = `Removed ${removed.title}.`;
    const undo = document.createElement("button");
    undo.type = "button";
    undo.className = "btn";
    undo.textContent = "Undo";
    undo.addEventListener("click", async () => {
      panels.splice(Math.min(at, panels.length), 0, removed);
      await savePanels();
      webStatus.textContent = "";
      show(removed.url);
    });
    webStatus.appendChild(undo);
  });

  chrome.storage.local.get({ webPanels: [] }).then(({ webPanels }) => {
    panels = Array.isArray(webPanels) ? webPanels : [];
    current = panels.length ? panels[0].url : null;
    renderPanels();
  });

  // ── WolfMail ─────────────────────────────────────────────────────────────────────────────────────────────────
  $("mail-open").addEventListener("click", async () => {
    $("mail-error").textContent = "";
    $("mail-status").textContent = "Opening WolfMail…";
    try {
      const res = await fetch("wolfweb://app/api/wolfmail/open", { cache: "no-store" });
      const body = await res.json().catch(() => null);
      if (!res.ok || !body || body.ok === false) throw new Error((body && body.error) || `WolfWeb refused that (${res.status}).`);
      $("mail-status").textContent = "WolfMail is opening.";
    } catch (e) {
      $("mail-status").textContent = "";
      $("mail-error").textContent = e.message === "Failed to fetch" ? "WolfWeb didn't answer. Restart WolfWeb and try again." : e.message;
    }
  });
  $("mail-get").addEventListener("click", () => chrome.tabs.create({ url: "https://github.com/intelligentwolf/wolfmail/releases" }));
})();
