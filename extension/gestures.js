// Mouse gestures: hold Alt, press the left button and draw. (Right-button gestures can't work from an extension
// on Linux and macOS: Chromium opens the context menu as the right button goes down.)
//   ←  back      →  forward      ↑↓  reload      ↓→  close tab      ↓  new tab      ↑  scroll to top
(() => {
  const STEP = 30; // pixels before a stroke counts
  // On unless switched off in WolfWeb settings.
  let enabled = true;
  chrome.storage.local.get({ features: {} }).then(({ features }) => (enabled = (features || {}).gestures !== false));
  chrome.storage.onChanged.addListener((c, area) => {
    if (area === "local" && c.features) enabled = ((c.features.newValue || {}).gestures) !== false;
  });
  let start = null;
  let last = null;
  let moves = "";
  let hint = null;

  const ACTIONS = {
    L: ["Back", () => history.back()],
    R: ["Forward", () => history.forward()],
    UD: ["Reload", () => location.reload()],
    DR: ["Close tab", () => chrome.runtime.sendMessage({ gesture: "close-tab" })],
    D: ["New tab", () => chrome.runtime.sendMessage({ gesture: "new-tab" })],
    U: ["Top of page", () => window.scrollTo({ top: 0, behavior: "smooth" })],
  };

  function show(text) {
    if (!hint) {
      hint = document.createElement("div");
      hint.setAttribute("role", "status");
      hint.style.cssText =
        "position:fixed;left:50%;top:40%;transform:translate(-50%,-50%);z-index:2147483647;padding:10px 18px;" +
        "border-radius:12px;background:rgba(45,27,45,.92);color:#fff;font:600 16px system-ui,sans-serif;pointer-events:none";
      document.documentElement.appendChild(hint);
    }
    hint.textContent = text;
  }
  function hide() {
    if (hint) hint.remove();
    hint = null;
  }

  window.addEventListener(
    "mousedown",
    (e) => {
      if (!enabled || e.button !== 0 || !e.altKey) return;
      start = last = { x: e.clientX, y: e.clientY };
      moves = "";
      e.preventDefault(); // no text selection or link drag while drawing
    },
    true,
  );
  window.addEventListener(
    "mousemove",
    (e) => {
      if (!start) return;
      const dx = e.clientX - last.x;
      const dy = e.clientY - last.y;
      if (Math.abs(dx) < STEP && Math.abs(dy) < STEP) return;
      const dir = Math.abs(dx) > Math.abs(dy) ? (dx > 0 ? "R" : "L") : dy > 0 ? "D" : "U";
      if (!moves.endsWith(dir)) moves += dir;
      last = { x: e.clientX, y: e.clientY };
      const action = ACTIONS[moves];
      show(action ? action[0] : "…");
    },
    true,
  );
  window.addEventListener(
    "mouseup",
    (e) => {
      if (!start) return;
      const action = ACTIONS[moves];
      const drew = moves.length > 0;
      start = null;
      hide();
      if (drew) {
        e.preventDefault();
        e.stopPropagation();
        // The click that ends a gesture must not also follow a link.
        window.addEventListener("click", (c) => { c.preventDefault(); c.stopPropagation(); }, { capture: true, once: true });
      }
      if (action) action[1]();
    },
    true,
  );
})();
