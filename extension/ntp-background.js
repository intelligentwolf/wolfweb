// New-tab backgrounds: none, Wikimedia Commons' picture of the day, a photo from the user's Pictures folder (a new
// one each day, read by WolfWeb's Rust side — src/backgrounds.rs), or a picture the user chooses.
(() => {
  const $ = (id) => document.getElementById(id);
  const body = document.body;
  const credit = $("credit");
  const bgStatus = $("bg-status");
  const bgError = $("bg-error");
  let objectUrl = null;

  const localDate = (d = new Date()) =>
    `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  const dayNumber = () => Math.floor(Date.parse(`${localDate()}T00:00:00Z`) / 86400000);

  async function getSettings() {
    const { background } = await chrome.storage.local.get({ background: { mode: "none", offset: 0 } });
    return { mode: "none", offset: 0, ...background };
  }
  const setSettings = (s) => chrome.storage.local.set({ background: s });

  function show(src, creditNode) {
    body.style.backgroundImage = src ? `url("${src.replace(/"/g, "%22")}")` : "";
    body.classList.toggle("has-bg", !!src);
    credit.replaceChildren();
    if (creditNode) credit.appendChild(creditNode);
    credit.hidden = !creditNode;
  }

  // ── Picture of the day (Wikimedia Commons featured feed; checked 2026-11-03: image.{title, thumbnail.source,
  //    image.source, artist.text, license.type, file_page}) ──────────────────────────────────────────────────────
  async function pictureOfTheDay() {
    const { bgDaily } = await chrome.storage.local.get({ bgDaily: null });
    if (bgDaily && bgDaily.date === localDate()) return bgDaily;
    for (let back = 0; back < 5; back++) {
      const d = new Date();
      d.setDate(d.getDate() - back);
      const url = `https://api.wikimedia.org/feed/v1/wikipedia/en/featured/${d.getFullYear()}/${String(d.getMonth() + 1).padStart(2, "0")}/${String(d.getDate()).padStart(2, "0")}`;
      const res = await fetch(url, { cache: "no-store", credentials: "omit" });
      if (!res.ok) continue;
      const feed = await res.json();
      const img = feed && feed.image;
      // Photos only (some days it's a video or a diagram).
      if (!img || !img.thumbnail || !/\.(jpe?g|png|webp)$/i.test(img.title || "")) continue;
      // The 960 px thumbnail address works at 1920 px too (checked: 200 image/jpeg).
      const src = img.thumbnail.source.replace(/\/\d+px-/, "/1920px-");
      const picked = {
        date: localDate(),
        src,
        fallback: img.thumbnail.source,
        artist: (img.artist && img.artist.text ? img.artist.text : "").replace(/\s+/g, " ").trim(),
        license: img.license ? img.license.type : "",
        page: img.file_page || "https://commons.wikimedia.org/",
        about: (img.description && img.description.text ? img.description.text : "").replace(/\s+/g, " ").trim(),
      };
      await chrome.storage.local.set({ bgDaily: picked });
      return picked;
    }
    throw new Error("Wikimedia didn't have a photo for the last few days.");
  }

  function dailyCredit(p) {
    const span = document.createElement("span");
    const a = document.createElement("a");
    a.href = p.page;
    a.target = "_blank";
    a.rel = "noopener noreferrer";
    a.textContent = "Wikimedia Commons";
    span.append(`${p.about ? `${p.about.slice(0, 120)}${p.about.length > 120 ? "…" : ""} — ` : ""}Photo: ${p.artist || "unknown"}${p.license ? `, ${p.license}` : ""}, `, a);
    return span;
  }

  // ── Pictures folder ────────────────────────────────────────────────────────────────────────────────────────
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
      const body = await res.json().catch(() => null);
      throw new Error((body && body.error) || `WolfWeb refused that (${res.status}).`);
    }
    return res;
  }

  async function folderPicture(offset) {
    const list = await (await api("backgrounds/pictures")).json();
    if (!list.count) throw new Error(`There are no photos in your Pictures folder (${list.folder}).`);
    const index = (((dayNumber() + offset) % list.count) + list.count) % list.count;
    const blob = await (await api("backgrounds/picture", { index })).blob();
    if (objectUrl) URL.revokeObjectURL(objectUrl);
    objectUrl = URL.createObjectURL(blob);
    return { src: objectUrl, count: list.count };
  }

  // ── A picture the user picks (kept, shrunk to at most 2560 px, in extension storage) ───────────────────────────
  async function shrink(file) {
    const bitmap = await createImageBitmap(file);
    const scale = Math.min(1, 2560 / Math.max(bitmap.width, bitmap.height));
    const canvas = document.createElement("canvas");
    canvas.width = Math.round(bitmap.width * scale);
    canvas.height = Math.round(bitmap.height * scale);
    canvas.getContext("2d").drawImage(bitmap, 0, 0, canvas.width, canvas.height);
    return canvas.toDataURL("image/jpeg", 0.88);
  }

  // ── Apply ──────────────────────────────────────────────────────────────────────────────────────────────────
  async function apply() {
    const s = await getSettings();
    bgError.textContent = "";
    try {
      if (s.mode === "daily") {
        const p = await pictureOfTheDay();
        const probe = new Image();
        probe.onload = () => show(p.src, dailyCredit(p));
        probe.onerror = () => show(p.fallback, dailyCredit(p));
        probe.src = p.src;
      } else if (s.mode === "folder") {
        const f = await folderPicture(s.offset || 0);
        show(f.src, null);
      } else if (s.mode === "custom") {
        const { bgCustom } = await chrome.storage.local.get({ bgCustom: null });
        show(bgCustom, null);
      } else {
        show(null, null);
      }
    } catch (e) {
      show(null, null);
      bgError.textContent = e.message;
      // Outside the dialog too: a background that can't load is said so on the page.
      const pageError = $("error");
      if (pageError && $("dialog-backdrop").hidden) pageError.textContent = `Background: ${e.message}`;
    }
  }

  // ── Customise dialog (role=dialog, focus moves in and back, Tab stays inside, Esc closes) ──────────────────────
  const backdrop = $("dialog-backdrop");
  const dialog = $("dialog");
  const opener = $("customise");

  function syncButtons(mode) {
    $("bg-choose").hidden = mode !== "custom";
    $("bg-next").hidden = mode !== "folder";
  }

  async function openDialog() {
    const s = await getSettings();
    for (const r of document.querySelectorAll('input[name="bg"]')) r.checked = r.value === s.mode;
    syncButtons(s.mode);
    bgStatus.textContent = "";
    bgError.textContent = "";
    backdrop.hidden = false;
    (document.querySelector('input[name="bg"]:checked') || document.querySelector('input[name="bg"]')).focus();
  }
  function closeDialog() {
    backdrop.hidden = true;
    opener.focus();
  }

  opener.addEventListener("click", openDialog);
  $("dialog-close").addEventListener("click", closeDialog);
  backdrop.addEventListener("click", (e) => {
    if (e.target === backdrop) closeDialog();
  });
  dialog.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      e.preventDefault();
      closeDialog();
    } else if (e.key === "Tab") {
      const items = [...dialog.querySelectorAll("button:not([hidden]), input:not([hidden])")].filter((x) => !x.disabled && x.offsetParent !== null);
      if (!items.length) return;
      const first = items[0];
      const last = items[items.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  });

  for (const r of document.querySelectorAll('input[name="bg"]')) {
    r.addEventListener("change", async () => {
      const s = await getSettings();
      s.mode = r.value;
      await setSettings(s);
      syncButtons(s.mode);
      if (s.mode === "custom") {
        const { bgCustom } = await chrome.storage.local.get({ bgCustom: null });
        if (!bgCustom) {
          $("bg-file").click();
          return;
        }
      }
      bgStatus.textContent = s.mode === "daily" ? "Fetching today's picture…" : "";
      await apply();
      if (!bgError.textContent) bgStatus.textContent = "Background changed.";
    });
  }

  $("bg-choose").addEventListener("click", () => $("bg-file").click());
  $("bg-file").addEventListener("change", async () => {
    const file = $("bg-file").files[0];
    if (!file) return;
    try {
      const data = await shrink(file);
      await chrome.storage.local.set({ bgCustom: data });
      await apply();
      bgStatus.textContent = "Background changed.";
    } catch (e) {
      bgError.textContent = `Couldn't use that picture: ${e.message}`;
    }
  });
  $("bg-next").addEventListener("click", async () => {
    const s = await getSettings();
    s.offset = (s.offset || 0) + 1;
    await setSettings(s);
    await apply();
    if (!bgError.textContent) bgStatus.textContent = "Here's another.";
  });

  apply();
})();
