// Command chains editor: name + ordered steps from WOLFWEB_COMMANDS, stored in chrome.storage.local.
/* global WOLFWEB_COMMANDS */
(() => {
  const form = document.getElementById("form");
  const nameEl = document.getElementById("name");
  const stepSel = document.getElementById("step");
  const stepsEl = document.getElementById("steps");
  const savedEl = document.getElementById("saved");
  const statusEl = document.getElementById("status");
  const errorEl = document.getElementById("error");
  let steps = [];

  // Commands that only make sense alone in a chain list.
  for (const c of WOLFWEB_COMMANDS.list.filter((c) => c.id !== "edit-chains")) {
    const o = document.createElement("option");
    o.value = c.id;
    o.textContent = c.title;
    stepSel.appendChild(o);
  }

  function renderSteps() {
    stepsEl.replaceChildren(
      ...steps.map((id, i) => {
        const li = document.createElement("li");
        const label = document.createElement("span");
        label.textContent = WOLFWEB_COMMANDS.byId.get(id).title;
        const remove = document.createElement("button");
        remove.type = "button";
        remove.className = "btn";
        remove.textContent = "Remove";
        remove.setAttribute("aria-label", `Remove step ${i + 1}, ${label.textContent}`);
        remove.addEventListener("click", () => {
          steps.splice(i, 1);
          renderSteps();
        });
        li.append(label, remove);
        return li;
      }),
    );
  }

  async function renderSaved() {
    const chains = await WOLFWEB_COMMANDS.chains();
    document.getElementById("none").hidden = chains.length > 0;
    savedEl.replaceChildren(
      ...chains.map((c) => {
        const li = document.createElement("li");
        li.className = "card";
        const what = document.createElement("div");
        what.className = "what";
        const name = document.createElement("strong");
        name.textContent = c.name;
        const st = document.createElement("div");
        st.className = "steps-text";
        st.textContent = c.steps.map((id) => (WOLFWEB_COMMANDS.byId.get(id) || { title: id }).title).join(" → ");
        what.append(name, st);
        const del = document.createElement("button");
        del.type = "button";
        del.className = "btn";
        del.textContent = "Delete";
        del.setAttribute("aria-label", `Delete chain ${c.name}`);
        del.addEventListener("click", async () => {
          const all = await WOLFWEB_COMMANDS.chains();
          // Match the chain itself, not its position: the list may have changed in another tab.
          const at = all.findIndex((x) => JSON.stringify(x) === JSON.stringify(c));
          if (at < 0) {
            renderSaved();
            return;
          }
          const [removed] = all.splice(at, 1);
          await chrome.storage.local.set({ chains: all });
          renderSaved();
          statusEl.textContent = `Deleted “${removed.name}”.`;
          const undo = document.createElement("button");
          undo.type = "button";
          undo.className = "btn";
          undo.textContent = "Undo";
          undo.addEventListener("click", async () => {
            const now = await WOLFWEB_COMMANDS.chains();
            now.splice(Math.min(at, now.length), 0, removed);
            await chrome.storage.local.set({ chains: now });
            statusEl.textContent = "";
            renderSaved();
          });
          statusEl.appendChild(undo);
        });
        li.append(what, del);
        return li;
      }),
    );
  }

  document.getElementById("add-step").addEventListener("click", () => {
    steps.push(stepSel.value);
    document.getElementById("steps-error").hidden = true;
    renderSteps();
  });

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const name = nameEl.value.trim();
    const nameErr = document.getElementById("name-error");
    const stepsErr = document.getElementById("steps-error");
    nameErr.hidden = !!name;
    nameEl.setAttribute("aria-invalid", String(!name));
    stepsErr.hidden = steps.length > 0;
    if (!name) {
      nameEl.focus();
      return;
    }
    if (!steps.length) {
      stepSel.focus();
      return;
    }
    try {
      const all = await WOLFWEB_COMMANDS.chains();
      all.push({ name, steps: [...steps] });
      await chrome.storage.local.set({ chains: all });
      statusEl.textContent = `Saved “${name}”.`;
      nameEl.value = "";
      steps = [];
      renderSteps();
      renderSaved();
    } catch (err) {
      errorEl.textContent = `Couldn't save the chain: ${err.message}`;
    }
  });

  renderSaved();
})();
