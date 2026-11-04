// The new-tab page's look comes from Chrome's "Customize Chrome" panel — one place to choose it. "Customise" opens
// that panel; the background picture chosen there (Chrome's collections, or an uploaded picture) is shown here,
// read by WolfWeb's Rust side (src/chrome_theme.rs). Light or dark follows Chrome by itself (prefers-color-scheme).
// While the page is showing it checks again every couple of seconds, so a picture chosen in the panel appears
// straight away; nothing changes on the page unless the choice did.
(() => {
  const $ = (id) => document.getElementById(id);
  const body = document.body;
  const credit = $("credit");
  const errorEl = $("error");
  const CHECK_MS = 2000;
  let shown = ""; // the choice on screen, as JSON
  let objectUrl = null;

  // WolfWeb's old background choices (before Customize Chrome) aren't used any more.
  chrome.storage.local.remove(["background", "bgDaily"]).catch(() => {});

  async function api(cmd, params = {}) {
    const u = new URL(`wolfweb://app/api/${cmd}`);
    for (const [k, v] of Object.entries(params)) u.searchParams.set(k, String(v));
    let res;
    try {
      res = await fetch(u.toString(), { cache: "no-store" });
    } catch {
      throw new Error("WolfWeb didn't answer. Restart WolfWeb and try again.");
    }
    if (!res.ok) {
      const b = await res.json().catch(() => null);
      throw new Error((b && b.error) || `WolfWeb refused that (${res.status}).`);
    }
    return res;
  }

  function show(src, creditNode) {
    body.style.backgroundImage = src ? `url("${src.replace(/["\\\n]/g, encodeURIComponent)}")` : "";
    body.classList.toggle("has-bg", !!src);
    credit.replaceChildren();
    if (creditNode) credit.appendChild(creditNode);
    credit.hidden = !creditNode;
  }

  /** "Photo by …" from Chrome's attribution lines, linked to the photo's page when Chrome gives one. */
  function attribution(bg) {
    const text = [bg.attribution1, bg.attribution2].filter(Boolean).join(" — ");
    if (!text) return null;
    if (!bg.attributionUrl) return document.createTextNode(text);
    const a = document.createElement("a");
    a.href = bg.attributionUrl;
    a.target = "_blank";
    a.rel = "noopener noreferrer";
    a.textContent = text;
    return a;
  }

  async function apply(bg) {
    if (objectUrl) {
      URL.revokeObjectURL(objectUrl);
      objectUrl = null;
    }
    if (!bg) return show(null, null);
    if (bg.kind === "uploaded") {
      const blob = await (await api("chrome-theme/image", { id: bg.id })).blob();
      objectUrl = URL.createObjectURL(blob);
      return show(objectUrl, null);
    }
    show(bg.url, attribution(bg));
  }

  async function check() {
    try {
      const { background } = await (await api("chrome-theme")).json();
      const key = JSON.stringify(background);
      if (key === shown) return;
      shown = key;
      await apply(background);
    } catch {
      // Not something the user did: the page keeps its current look (the next check tries again).
    }
  }

  $("customise").addEventListener("click", async () => {
    errorEl.textContent = "";
    try {
      await api("browser/command", { name: "customize-chrome" });
    } catch (e) {
      errorEl.textContent = `Couldn't open Customize Chrome: ${e.message}`;
    }
  });

  check();
  setInterval(() => {
    if (document.visibilityState === "visible") check();
  }, CHECK_MS);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") check();
  });
})();
