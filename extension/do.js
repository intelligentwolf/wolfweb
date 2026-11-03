// Fallback for a WolfWeb menu address that reached a tab (normally WolfWeb's Rust side runs it before any page
// loads — src/menu.rs). Does the same thing, then closes this tab.
(async () => {
  const msg = document.getElementById("msg");
  const [key, raw] = location.hash.slice(1).split("=");
  const value = decodeURIComponent(raw || "");
  try {
    if (key === "cmd") {
      const res = await fetch(`wolfweb://app/api/browser/command?name=${encodeURIComponent(value)}`, { cache: "no-store" });
      const body = await res.json().catch(() => null);
      if (!res.ok || !body || body.ok === false) throw new Error((body && body.error) || "WolfWeb refused that.");
    } else if (key === "page" && /^[\w.#-]+$/.test(value)) {
      await chrome.tabs.create({ url: chrome.runtime.getURL(value) });
    } else if (key === "open" && /^(chrome|https):\/\//.test(value)) {
      await chrome.tabs.create({ url: value });
    } else {
      throw new Error("That isn't a WolfWeb menu item.");
    }
    const tab = await chrome.tabs.getCurrent();
    if (tab) await chrome.tabs.remove(tab.id);
  } catch (e) {
    msg.className = "msg msg-err";
    msg.setAttribute("role", "alert");
    msg.textContent = e.message;
  }
})();
