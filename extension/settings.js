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

  chrome.storage.onChanged.addListener((changes, area) => {
    if (area === "local" && (changes.features || changes.statusBarHidden)) render();
  });
  render();
})();
