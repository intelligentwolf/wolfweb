// WolfWeb's new-tab page: Google search (or a typed address), most-visited sites, shortcuts.
(() => {
  const form = document.querySelector(".search");
  const input = document.getElementById("q");
  const errorEl = document.getElementById("error");

  const looksLikeAddress = (text) =>
    /^[a-z][a-z0-9+.-]*:\/\//i.test(text) || (!/\s/.test(text) && /^[^.\s/]+(\.[^.\s/]+)+(:\d+)?(\/.*)?$/.test(text)) ||
    /^localhost(:\d+)?(\/.*)?$/i.test(text);

  // A typed web address goes straight there; anything else is searched on Google (the form's own action).
  form.addEventListener("submit", (e) => {
    const text = input.value.trim();
    if (!text) {
      e.preventDefault();
      input.focus();
      return;
    }
    if (looksLikeAddress(text)) {
      e.preventDefault();
      const url = /^[a-z][a-z0-9+.-]*:\/\//i.test(text) ? text : `https://${text}`;
      chrome.tabs.update({ url }).catch((err) => (errorEl.textContent = `Couldn't open ${url}: ${err.message}`));
    }
  });

  // Extension pages may open chrome:// pages and other extension pages.
  document.querySelectorAll("[data-open]").forEach((el) =>
    el.addEventListener("click", () => {
      const target = el.dataset.open;
      const url = /^[a-z-]+:/.test(target) ? target : chrome.runtime.getURL(target);
      chrome.tabs.create({ url }).catch((err) => (errorEl.textContent = `Couldn't open that page: ${err.message}`));
    }),
  );

  // Most visited (chrome.topSites) — like Vivaldi's Speed Dial, filled from your own browsing.
  const dial = document.getElementById("dial");
  chrome.topSites
    .get()
    .then((sites) => {
      const list = sites.filter((s) => /^https?:/i.test(s.url)).slice(0, 8);
      document.getElementById("dial-empty").hidden = list.length > 0;
      for (const s of list) {
        const li = document.createElement("li");
        const a = document.createElement("a");
        a.href = s.url;
        const img = document.createElement("img");
        img.alt = "";
        // Chrome's local favicon cache ("favicon" permission) — no request goes to the site.
        const fav = new URL(chrome.runtime.getURL("/_favicon/"));
        fav.searchParams.set("pageUrl", s.url);
        fav.searchParams.set("size", "32");
        img.src = fav.toString();
        img.addEventListener("error", () => (img.src = "icons/wolfweb-32.png"));
        const name = document.createElement("span");
        name.className = "name";
        name.textContent = s.title || new URL(s.url).hostname;
        a.append(img, name);
        a.title = s.url;
        li.appendChild(a);
        dial.appendChild(li);
      }
    })
    .catch(() => (document.getElementById("dial-empty").hidden = false));

  // Keep WolfWeb's menu bar and 📰 Daily button on the bookmarks bar (cheap when they're already there).
  /* global WOLFWEB_MENUBAR */
  WOLFWEB_MENUBAR.sync().catch(() => {});

  input.focus();
})();
