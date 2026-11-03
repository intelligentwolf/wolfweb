// Notes side panel (like Vivaldi's Notes): kept in chrome.storage.local on this computer, saved as you type.
(() => {
  const listEl = document.getElementById("list");
  const emptyEl = document.getElementById("empty");
  const editor = document.getElementById("editor");
  const text = document.getElementById("text");
  const statusEl = document.getElementById("status");
  const errorEl = document.getElementById("error");
  let notes = [];
  let current = null;
  let saveTimer = null;

  const fail = (what, e) => (errorEl.textContent = `${what}: ${e && e.message ? e.message : e}`);

  async function load() {
    try {
      const got = await chrome.storage.local.get({ notes: [] });
      notes = Array.isArray(got.notes) ? got.notes : [];
    } catch (e) {
      fail("Couldn't read your notes", e);
    }
  }

  async function save() {
    try {
      await chrome.storage.local.set({ notes });
    } catch (e) {
      fail("Couldn't save your notes", e);
    }
  }

  const titleOf = (n) => (n.text.split("\n").find((l) => l.trim()) || "Empty note").trim().slice(0, 80);

  function render() {
    notes.sort((a, b) => b.updated - a.updated);
    listEl.replaceChildren(
      ...notes.map((n) => {
        const li = document.createElement("li");
        li.setAttribute("role", "option");
        li.tabIndex = 0;
        li.setAttribute("aria-selected", String(n.id === current));
        li.dataset.id = n.id;
        li.textContent = titleOf(n);
        const when = document.createElement("span");
        when.className = "when";
        when.textContent = new Date(n.updated).toLocaleString();
        li.appendChild(when);
        return li;
      }),
    );
    emptyEl.hidden = notes.length > 0;
    const note = notes.find((n) => n.id === current);
    editor.hidden = !note;
    if (note && document.activeElement !== text) text.value = note.text;
  }

  function select(id) {
    current = id;
    render();
    text.focus();
  }

  document.getElementById("new").addEventListener("click", async () => {
    const note = { id: crypto.randomUUID(), text: "", updated: Date.now() };
    notes.push(note);
    await save();
    select(note.id);
  });

  listEl.addEventListener("click", (e) => {
    const li = e.target.closest("li");
    if (li) select(li.dataset.id);
  });
  listEl.addEventListener("keydown", (e) => {
    const li = e.target.closest("li");
    if (li && (e.key === "Enter" || e.key === " ")) {
      e.preventDefault();
      select(li.dataset.id);
    }
  });

  text.addEventListener("input", () => {
    const note = notes.find((n) => n.id === current);
    if (!note) return;
    note.text = text.value;
    note.updated = Date.now();
    clearTimeout(saveTimer);
    saveTimer = setTimeout(async () => {
      await save();
      render();
    }, 400);
  });

  document.getElementById("add-page").addEventListener("click", async () => {
    try {
      const [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
      if (!tab || !tab.url) return;
      const note = notes.find((n) => n.id === current);
      if (!note) return;
      note.text = `${note.text}${note.text && !note.text.endsWith("\n") ? "\n" : ""}${tab.title ? tab.title + " — " : ""}${tab.url}\n`;
      note.updated = Date.now();
      text.value = note.text;
      await save();
      render();
    } catch (e) {
      fail("Couldn't add the page", e);
    }
  });

  // Delete with Undo instead of an "are you sure?" question.
  document.getElementById("delete").addEventListener("click", async () => {
    const idx = notes.findIndex((n) => n.id === current);
    if (idx < 0) return;
    const [removed] = notes.splice(idx, 1);
    current = null;
    await save();
    render();
    statusEl.textContent = "Note deleted.";
    const undo = document.createElement("button");
    undo.type = "button";
    undo.className = "btn";
    undo.textContent = "Undo";
    undo.addEventListener("click", async () => {
      notes.push(removed);
      await save();
      statusEl.textContent = "";
      select(removed.id);
    });
    statusEl.appendChild(undo);
  });

  chrome.storage.onChanged.addListener((changes, area) => {
    if (area === "local" && changes.notes && document.activeElement !== text) {
      notes = changes.notes.newValue || [];
      render();
    }
  });

  load().then(render);
})();
