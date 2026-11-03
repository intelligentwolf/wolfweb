// "WolfWeb isn't your default browser" — asked on the new-tab page until the user says yes, "not now" (asked again
// in a week) or "don't ask again" (WolfWeb settings can turn it back on). WolfWeb's Rust side checks and changes the
// default (src/default_browser.rs).
/* global WOLFWEB_FEATURES */
(() => {
  const WEEK = 7 * 24 * 60 * 60 * 1000;
  const box = document.getElementById("default-ask");
  if (!box) return;
  const statusEl = document.getElementById("default-status");
  let watchingFocus = false;
  const errorEl = document.getElementById("default-error");

  async function api(cmd) {
    let res;
    try {
      res = await fetch(`wolfweb://app/api/${cmd}`, { cache: "no-store" });
    } catch {
      throw new Error("WolfWeb didn't answer. Restart WolfWeb and try again.");
    }
    const body = await res.json().catch(() => null);
    if (!res.ok || !body || body.ok === false) throw new Error((body && body.error) || `WolfWeb refused that (${res.status}).`);
    return body;
  }

  async function check() {
    const f = await WOLFWEB_FEATURES.get();
    const { askDefaultAfter } = await chrome.storage.local.get({ askDefaultAfter: 0 });
    if (!f.askDefault || Date.now() < askDefaultAfter) return;
    const { isDefault } = await api("default-browser/status");
    box.hidden = isDefault;
  }

  document.getElementById("default-yes").addEventListener("click", async () => {
    errorEl.textContent = "";
    try {
      const { message } = await api("default-browser/set");
      statusEl.textContent = message;
      // Linux changes straight away; macOS and Windows wait for the user's confirmation — check again on return.
      const recheck = async () => {
        const { isDefault } = await api("default-browser/status");
        if (isDefault) {
          statusEl.textContent = "WolfWeb is now your default browser.";
          setTimeout(() => (box.hidden = true), 2500);
        }
      };
      await recheck();
      if (!watchingFocus) {
        watchingFocus = true;
        window.addEventListener("focus", recheck);
      }
    } catch (e) {
      errorEl.textContent = e.message;
    }
  });
  document.getElementById("default-later").addEventListener("click", async () => {
    await chrome.storage.local.set({ askDefaultAfter: Date.now() + WEEK });
    box.hidden = true;
  });
  document.getElementById("default-never").addEventListener("click", async () => {
    await WOLFWEB_FEATURES.set("askDefault", false);
    box.hidden = true;
  });

  check().catch(() => {});
})();
