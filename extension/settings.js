// WolfWeb settings: switches for WolfWeb's own features (stored in chrome.storage.local "features"; the background
// worker, the status bar and gestures react to the change straight away).
/* global WOLFWEB_FEATURES */
(() => {
  const statusEl = document.getElementById("status");
  const errorEl = document.getElementById("error");
  const switches = [...document.querySelectorAll(".switch[data-feature]")];

  async function render() {
    const f = await WOLFWEB_FEATURES.get();
    for (const s of switches) s.setAttribute("aria-checked", String(!!f[s.dataset.feature]));
    const { statusBarHidden } = await chrome.storage.local.get({ statusBarHidden: [] });
    const box = document.getElementById("hidden-sites");
    box.hidden = !(statusBarHidden || []).length;
    document.getElementById("hidden-list").textContent = (statusBarHidden || []).join(", ");
  }

  for (const s of switches) {
    s.addEventListener("click", async () => {
      const on = s.getAttribute("aria-checked") !== "true";
      try {
        await WOLFWEB_FEATURES.set(s.dataset.feature, on);
        await render();
        const name = document.getElementById(`l-${s.dataset.feature}`).textContent;
        statusEl.textContent = `${name} ${on ? "on" : "off"}.`;
        errorEl.textContent = "";
      } catch (e) {
        errorEl.textContent = `Couldn't change that: ${e.message}`;
      }
    });
  }

  document.getElementById("show-everywhere").addEventListener("click", async () => {
    await chrome.storage.local.set({ statusBarHidden: [] });
    await render();
    statusEl.textContent = "The status bar shows on every site again.";
  });

  document.querySelectorAll("[data-open]").forEach((el) =>
    el.addEventListener("click", () => {
      const t = el.dataset.open;
      chrome.tabs.create({ url: /^[a-z-]+:/.test(t) ? t : chrome.runtime.getURL(t) }).catch((e) => (errorEl.textContent = e.message));
    }),
  );

  // Default browser: state and the button (src/default_browser.rs does the work).
  async function defaultApi(cmd) {
    const res = await fetch(`wolfweb://app/api/${cmd}`, { cache: "no-store" });
    const body = await res.json().catch(() => null);
    if (!res.ok || !body || body.ok === false) throw new Error((body && body.error) || `WolfWeb refused that (${res.status}).`);
    return body;
  }
  async function showDefault() {
    try {
      const { isDefault } = await defaultApi("default-browser/status");
      document.getElementById("default-state").textContent = isDefault ? "WolfWeb is your default browser." : "WolfWeb isn't your default browser.";
      document.getElementById("make-default").hidden = isDefault;
    } catch (e) {
      document.getElementById("default-state").textContent = "Couldn't check the default browser.";
      errorEl.textContent = e.message;
    }
  }
  document.getElementById("make-default").addEventListener("click", async () => {
    try {
      const { message } = await defaultApi("default-browser/set");
      statusEl.textContent = message;
      await showDefault();
    } catch (e) {
      errorEl.textContent = e.message;
    }
  });
  window.addEventListener("focus", showDefault);
  showDefault();

  chrome.storage.onChanged.addListener((changes, area) => {
    if (area === "local" && (changes.features || changes.statusBarHidden)) render();
  });
  render();
})();
