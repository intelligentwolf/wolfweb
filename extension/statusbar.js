// WolfWeb's status bar, like Vivaldi's: how long the page took, how much there is to read, zoom (− / % / +) and the
// time. With WolfWeb's own engine it's a strip of the window below the page (statusstrip.html), and this script only
// tells it about the page (statusInfo). With the standard engine this script draws it over the bottom of web pages,
// in a closed shadow root so pages can't restyle it or read it. Switch it off in WolfWeb settings, or hide this one
// for a site with ×.
(() => {
  if (window.top !== window || document.contentType !== "text/html") return;
  const HEIGHT = 26;
  let host = null;
  let parts = null;
  let clockTimer = null;

  const site = location.hostname;

  /** What the strip shows about this page. */
  function pageInfo() {
    const nav = performance.getEntriesByType("navigation")[0];
    const words = (document.body ? document.body.innerText : "").split(/\s+/).filter(Boolean).length;
    return { loadedMs: nav && nav.loadEventEnd > 0 ? nav.loadEventEnd : null, words };
  }
  chrome.runtime.onMessage.addListener((msg, _sender, reply) => {
    if (msg && msg.statusInfo) reply(pageInfo());
  });

  async function wanted() {
    const { features, statusBarHidden, engine } = await chrome.storage.local.get({ features: {}, statusBarHidden: [], engine: {} });
    if (engine && engine.nativeStatusBar) return false; // the engine's own strip shows it
    const on = (features || {}).statusBar !== false;
    return on && !(statusBarHidden || []).includes(site);
  }

  function build() {
    host = document.createElement("wolfweb-status-bar");
    const root = host.attachShadow({ mode: "closed" });
    root.innerHTML = `
      <style>
        :host { all: initial; position: fixed; left: 0; right: 0; bottom: 0; height: ${HEIGHT}px; z-index: 2147483646; }
        .bar { box-sizing: border-box; height: 100%; display: flex; align-items: center; gap: 12px; padding: 0 8px 0 12px;
          font: 12px/1 system-ui, -apple-system, "Segoe UI", sans-serif; color: #17141a; background: rgba(246, 241, 231, 0.96);
          border-top: 1px solid rgba(45, 27, 45, 0.18); }
        @media (prefers-color-scheme: dark) { .bar { color: #eceff1; background: rgba(46, 47, 55, 0.96); border-top-color: rgba(224, 230, 230, 0.18); } }
        .grow { flex: 1 1 auto; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
        button { all: unset; cursor: pointer; padding: 3px 7px; border-radius: 6px; font: inherit; }
        button:hover { background: rgba(196, 30, 30, 0.14); }
        button:focus-visible { outline: 2px solid #c41e1e; }
        .zoom { display: inline-flex; align-items: center; gap: 2px; }
        .sep { opacity: 0.35; }
        @media print { :host { display: none; } }
      </style>
      <div class="bar" role="status" aria-label="WolfWeb status bar">
        <span class="grow" data-part="page"></span>
        <span class="zoom">
          <button data-act="zoom-out" aria-label="Zoom out">−</button>
          <button data-act="zoom-reset" data-part="zoom" aria-label="Reset zoom">100%</button>
          <button data-act="zoom-in" aria-label="Zoom in">+</button>
        </span>
        <span class="sep">|</span>
        <span data-part="clock"></span>
        <button data-act="settings" aria-label="WolfWeb settings" title="WolfWeb settings">⚙</button>
        <button data-act="hide" aria-label="Hide the status bar on this site" title="Hide on ${site}">×</button>
      </div>`;
    parts = {
      page: root.querySelector('[data-part="page"]'),
      zoom: root.querySelector('[data-part="zoom"]'),
      clock: root.querySelector('[data-part="clock"]'),
    };
    root.addEventListener("click", async (e) => {
      const act = e.target.closest("[data-act]")?.dataset.act;
      if (!act) return;
      if (act === "hide") {
        const { statusBarHidden } = await chrome.storage.local.get({ statusBarHidden: [] });
        await chrome.storage.local.set({ statusBarHidden: [...new Set([...(statusBarHidden || []), site])] });
      } else {
        chrome.runtime.sendMessage({ statusBar: act }).catch(() => {});
      }
    });
    (document.body || document.documentElement).appendChild(host);
    // Room for the bar at the end of the page, so the last line isn't hidden under it.
    document.documentElement.style.setProperty("padding-bottom", `${HEIGHT}px`, "important");
    fillPage();
    tick();
    clockTimer = setInterval(tick, 15000);
    chrome.runtime.sendMessage({ statusBar: "zoom-get" }).then(showZoom).catch(() => {});
  }

  function remove() {
    if (!host) return;
    host.remove();
    host = null;
    clearInterval(clockTimer);
    document.documentElement.style.removeProperty("padding-bottom");
  }

  function fillPage() {
    if (!parts) return;
    const nav = performance.getEntriesByType("navigation")[0];
    const loaded = nav && nav.loadEventEnd > 0 ? `Loaded in ${(nav.loadEventEnd / 1000).toFixed(1)} s` : "Loading…";
    const words = (document.body ? document.body.innerText : "").split(/\s+/).filter(Boolean).length;
    const minutes = Math.max(1, Math.round(words / 230));
    parts.page.textContent = `${loaded}  ·  ${words.toLocaleString()} words  ·  ${minutes} min read`;
  }

  function tick() {
    if (parts) parts.clock.textContent = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }

  function showZoom(reply) {
    if (parts && reply && typeof reply.zoom === "number") parts.zoom.textContent = `${Math.round(reply.zoom * 100)}%`;
  }

  chrome.runtime.onMessage.addListener((msg) => {
    if (msg && typeof msg.zoomChanged === "number") showZoom({ zoom: msg.zoomChanged });
  });

  async function apply() {
    const on = await wanted();
    if (on && !host) build();
    else if (!on) remove();
  }

  chrome.storage.onChanged.addListener((changes, area) => {
    if (area === "local" && (changes.features || changes.statusBarHidden || changes.engine)) apply();
  });

  if (document.readyState === "complete") apply();
  else window.addEventListener("load", () => apply().then(fillPage), { once: true });
  // The engine's strip asks again once the page has finished loading.
  if (document.readyState !== "complete") {
    window.addEventListener("load", () => chrome.runtime.sendMessage({ statusInfoChanged: true }).catch(() => {}), { once: true });
  }
})();
