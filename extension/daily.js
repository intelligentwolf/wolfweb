// The WolfWeb Daily: a broadsheet front page made from the user's interests (Google News searches) and chosen
// news feeds (RSS/Atom). Extension pages may fetch any http(s) feed (manifest host_permissions); everything from a
// feed is put on the page as text or as http(s) link/image addresses only.
(() => {
  const $ = (id) => document.getElementById(id);
  const statusEl = $("status");
  const errorEl = $("error");

  // Feeds checked 2026-11-03 (each answered 200 with items).
  const PRESETS = [
    { name: "BBC News — top stories", url: "https://feeds.bbci.co.uk/news/rss.xml" },
    { name: "BBC Technology", url: "https://feeds.bbci.co.uk/news/technology/rss.xml" },
    { name: "BBC Science & Environment", url: "https://feeds.bbci.co.uk/news/science_and_environment/rss.xml" },
    { name: "The Guardian — World", url: "https://www.theguardian.com/world/rss" },
    { name: "The Guardian — UK", url: "https://www.theguardian.com/uk-news/rss" },
    { name: "Hacker News", url: "https://hnrss.org/frontpage" },
  ];
  const DEFAULTS = { masthead: "The WolfWeb Daily", topics: [], feeds: [] };
  const PER_SECTION = 7;
  let settings = { ...DEFAULTS };

  // Local date (not UTC), so the paper changes at the user's midnight.
  const today = () => {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  };
  const longDate = () => new Date().toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "long", year: "numeric" });
  const httpUrl = (u) => {
    try {
      const url = new URL(u);
      return /^https?:$/.test(url.protocol) ? url.toString() : null;
    } catch {
      return null;
    }
  };
  const text = (html) => {
    const d = new DOMParser().parseFromString(`<body>${html || ""}</body>`, "text/html");
    return (d.body.textContent || "").replace(/\s+/g, " ").trim();
  };

  // ── Settings ───────────────────────────────────────────────────────────────────────────────────────────────
  async function loadSettings() {
    const { daily } = await chrome.storage.local.get({ daily: null });
    settings = { ...DEFAULTS, ...(daily || {}) };
  }
  const saveSettings = () => chrome.storage.local.set({ daily: settings });

  // ── Fetching and reading feeds ─────────────────────────────────────────────────────────────────────────────
  function googleNews(topic) {
    const lang = (navigator.language || "en-GB").split("-");
    const region = (lang[1] || "GB").toUpperCase();
    const u = new URL("https://news.google.com/rss/search");
    u.searchParams.set("q", topic);
    u.searchParams.set("hl", navigator.language || "en-GB");
    u.searchParams.set("gl", region);
    u.searchParams.set("ceid", `${region}:${lang[0]}`);
    return u.toString();
  }

  /** Parse RSS 2.0 or Atom into stories. */
  function parseFeed(xmlText, section) {
    const doc = new DOMParser().parseFromString(xmlText, "application/xml");
    if (doc.querySelector("parsererror")) throw new Error("not a news feed");
    const title = (doc.querySelector("channel > title, feed > title") || {}).textContent || section;
    const nodes = [...doc.querySelectorAll("item, entry")];
    const MEDIA = "http://search.yahoo.com/mrss/";
    return {
      title: title.trim(),
      stories: nodes.map((n) => {
        const get = (sel) => (n.querySelector(sel) || {}).textContent || "";
        let headline = text(get("title"));
        let source = text(get("source"));
        // Google News puts the publisher at the end of the headline: "Headline - Publisher".
        if (source && headline.endsWith(` - ${source}`)) headline = headline.slice(0, -(source.length + 3));
        const linkEl = n.querySelector("link");
        const link = httpUrl((linkEl && (linkEl.getAttribute("href") || linkEl.textContent)) || "");
        let summary = text(get("description") || get("summary") || get("content"));
        if (summary.startsWith(headline)) summary = summary.slice(headline.length).trim();
        // Google News' description is the headline plus the publisher — nothing more to say.
        if (source && summary.replace(/^[\s\-–—|]+/, "") === source) summary = "";
        if (summary.length > 420) summary = `${summary.slice(0, 400).replace(/\s+\S*$/, "")}…`;
        // Biggest picture offered: media:content / media:thumbnail (width attribute), else an image enclosure.
        const media = [...n.getElementsByTagNameNS(MEDIA, "content"), ...n.getElementsByTagNameNS(MEDIA, "thumbnail")]
          .map((m) => ({ url: httpUrl(m.getAttribute("url") || ""), w: Number(m.getAttribute("width")) || 0 }))
          .filter((m) => m.url)
          .sort((a, b) => b.w - a.w);
        const enclosure = [...n.querySelectorAll("enclosure")].find((e) => /^image\//.test(e.getAttribute("type") || ""));
        const image = media.length ? media[0].url : enclosure ? httpUrl(enclosure.getAttribute("url") || "") : null;
        const date = Date.parse(get("pubDate") || get("updated") || get("published")) || 0;
        return { headline, link, source, summary, image, date, section };
      }).filter((s) => s.headline && s.link),
    };
  }

  async function fetchFeed(url, section) {
    const res = await fetch(url, { cache: "no-store", credentials: "omit" });
    if (!res.ok) throw new Error(`${res.status}`);
    return parseFeed(await res.text(), section);
  }

  async function buildEdition() {
    const jobs = [
      ...settings.topics.map((t) => fetchFeed(googleNews(t), t).then((f) => ({ name: t, stories: f.stories }))),
      // A feed's section (and each story's kicker) is the feed's own title, not its address.
      ...settings.feeds.map((u) =>
        fetchFeed(u, u).then((f) => ({ name: f.title, stories: f.stories.map((s) => ({ ...s, section: f.title })) })),
      ),
    ];
    const names = [...settings.topics, ...settings.feeds];
    const results = await Promise.allSettled(jobs);
    const failed = results.map((r, i) => (r.status === "rejected" ? names[i] : null)).filter(Boolean);
    const seen = new Set();
    const sections = results
      .filter((r) => r.status === "fulfilled")
      .map((r) => ({
        name: r.value.name,
        stories: r.value.stories
          .sort((a, b) => b.date - a.date)
          .filter((s) => {
            const key = s.headline.toLowerCase();
            if (seen.has(key) || seen.has(s.link)) return false;
            seen.add(key);
            seen.add(s.link);
            return true;
          })
          .slice(0, PER_SECTION),
      }))
      .filter((s) => s.stories.length);
    return { date: today(), sections, failed };
  }

  // ── The broadsheet ─────────────────────────────────────────────────────────────────────────────────────────
  function el(tag, cls, content) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (content !== undefined) e.textContent = content;
    return e;
  }

  function storyNode(s, cls) {
    const art = el("article", cls);
    const a = el("a");
    a.href = s.link;
    a.target = "_blank";
    a.rel = "noopener noreferrer";
    if (s.image && cls !== "story") {
      const img = el("img");
      img.alt = "";
      img.loading = "lazy";
      img.src = s.image;
      img.addEventListener("error", () => img.remove());
      a.appendChild(img);
    }
    a.appendChild(el("h3", "", s.headline));
    art.appendChild(a);
    const by = [s.source, s.date ? new Date(s.date).toLocaleString(undefined, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }) : ""]
      .filter(Boolean)
      .join(" · ");
    if (by) art.appendChild(el("p", "byline", by));
    if (s.summary) art.appendChild(el("p", "", s.summary));
    return art;
  }

  function render(edition) {
    $("setup").hidden = true;
    $("paper").hidden = false;
    $("paper-name").textContent = settings.masthead || DEFAULTS.masthead;
    document.title = settings.masthead || DEFAULTS.masthead;
    $("paper-date").textContent = new Date(`${edition.date}T12:00:00`).toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "long", year: "numeric" });
    $("edition-for").textContent = settings.topics.length ? `About: ${settings.topics.join(", ")}` : "";
    const total = edition.sections.reduce((n, s) => n + s.stories.length, 0);
    $("edition-count").textContent = `${total} stories`;

    // Lead: the newest story that has a picture (else the newest), taken out of its section.
    const all = edition.sections.flatMap((s) => s.stories);
    const lead = [...all].filter((s) => s.image).sort((a, b) => b.date - a.date)[0] || [...all].sort((a, b) => b.date - a.date)[0];
    const leadEl = $("lead");
    leadEl.replaceChildren();
    leadEl.classList.toggle("no-image", !(lead && lead.image));
    if (lead) {
      const words = el("div");
      words.appendChild(el("div", "kicker", lead.section));
      const a = el("a");
      a.href = lead.link;
      a.target = "_blank";
      a.rel = "noopener noreferrer";
      a.appendChild(el("h2", "", lead.headline));
      words.appendChild(a);
      if (lead.source) words.appendChild(el("p", "byline", lead.source));
      if (lead.summary) words.appendChild(el("p", "summary", lead.summary));
      leadEl.appendChild(words);
      if (lead.image) {
        const img = el("img");
        img.alt = "";
        img.src = lead.image;
        img.addEventListener("error", () => {
          img.remove();
          leadEl.classList.add("no-image");
        });
        leadEl.appendChild(img);
      }
    }

    $("sections").replaceChildren(
      ...edition.sections.map((sec) => {
        const s = el("section", "section");
        s.appendChild(el("h2", "", sec.name));
        const cols = el("div", "cols");
        sec.stories.filter((st) => st !== lead).forEach((st, i) => cols.appendChild(storyNode(st, i === 0 ? "story top" : "story")));
        s.appendChild(cols);
        return s;
      }),
    );
    errorEl.textContent = edition.failed.length ? `Couldn't reach: ${edition.failed.join(", ")}` : "";
    if (!edition.sections.length) errorEl.textContent = "No stories came back — check your connection, then press “Fetch today's news”.";
  }

  async function refresh() {
    statusEl.textContent = "Fetching today's news…";
    errorEl.textContent = "";
    try {
      const edition = await buildEdition();
      await chrome.storage.local.set({ dailyEdition: edition });
      render(edition);
      statusEl.textContent = "";
    } catch (e) {
      statusEl.textContent = "";
      errorEl.textContent = `Couldn't make the paper: ${e.message}`;
    }
  }

  // ── Setup form ─────────────────────────────────────────────────────────────────────────────────────────────
  let draft = { ...DEFAULTS };

  function chip(list, value, onRemove) {
    const li = el("li", "", value);
    const x = el("button", "", "×");
    x.type = "button";
    x.setAttribute("aria-label", `Remove ${value}`);
    x.addEventListener("click", onRemove);
    li.appendChild(x);
    list.appendChild(li);
  }

  function renderSetup() {
    $("masthead").value = draft.masthead;
    const topics = $("topics");
    topics.replaceChildren();
    draft.topics.forEach((t, i) => chip(topics, t, () => { draft.topics.splice(i, 1); renderSetup(); }));
    const presetUrls = new Set(PRESETS.map((p) => p.url));
    $("presets").replaceChildren(
      ...PRESETS.map((p) => {
        const label = el("label");
        const box = el("input");
        box.type = "checkbox";
        box.checked = draft.feeds.includes(p.url);
        box.addEventListener("change", () => {
          draft.feeds = box.checked ? [...draft.feeds, p.url] : draft.feeds.filter((f) => f !== p.url);
        });
        label.append(box, document.createTextNode(p.name));
        return label;
      }),
    );
    const feeds = $("feeds");
    feeds.replaceChildren();
    draft.feeds.filter((f) => !presetUrls.has(f)).forEach((f) => chip(feeds, f, () => { draft.feeds = draft.feeds.filter((x) => x !== f); renderSetup(); }));
  }

  function openSetup() {
    draft = { masthead: settings.masthead, topics: [...settings.topics], feeds: [...settings.feeds] };
    renderSetup();
    $("paper").hidden = true;
    $("setup").hidden = false;
    $("setup-cancel").hidden = !(settings.topics.length || settings.feeds.length);
    $("topic-input").focus();
  }

  function addTopic() {
    const input = $("topic-input");
    const values = input.value.split(",").map((v) => v.trim()).filter(Boolean);
    for (const v of values) if (!draft.topics.some((t) => t.toLowerCase() === v.toLowerCase())) draft.topics.push(v.slice(0, 80));
    input.value = "";
    $("topic-error").hidden = true;
    renderSetup();
    input.focus();
  }
  $("topic-add").addEventListener("click", addTopic);
  $("topic-input").addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addTopic();
    }
  });
  $("feed-add").addEventListener("click", () => {
    const input = $("feed-input");
    const url = httpUrl(input.value.trim());
    $("feed-error").hidden = !!url;
    input.setAttribute("aria-invalid", String(!url));
    if (!url) {
      input.focus();
      return;
    }
    if (!draft.feeds.includes(url)) draft.feeds.push(url);
    input.value = "";
    renderSetup();
  });
  $("setup-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    if ($("topic-input").value.trim()) addTopic();
    draft.masthead = $("masthead").value.trim() || DEFAULTS.masthead;
    if (!draft.topics.length && !draft.feeds.length) {
      $("topic-error").hidden = false;
      $("topic-input").setAttribute("aria-invalid", "true");
      $("topic-input").focus();
      return;
    }
    settings = draft;
    await saveSettings();
    await refresh();
  });
  $("setup-cancel").addEventListener("click", () => {
    $("setup").hidden = true;
    $("paper").hidden = false;
  });

  // ── Toolbar ────────────────────────────────────────────────────────────────────────────────────────────────
  $("refresh").addEventListener("click", refresh);
  $("edit").addEventListener("click", openSetup);
  $("print").addEventListener("click", () => window.print());
  $("pdf").addEventListener("click", async () => {
    const btn = $("pdf");
    btn.disabled = true;
    errorEl.textContent = "";
    statusEl.textContent = "Saving the PDF…";
    try {
      const u = new URL("wolfweb://app/api/pdf/save");
      u.searchParams.set("title", `${settings.masthead || DEFAULTS.masthead} ${longDate()}`);
      const res = await fetch(u.toString(), { cache: "no-store" });
      const body = await res.json().catch(() => null);
      if (!res.ok || !body || body.ok === false) throw new Error((body && body.error) || `WolfWeb refused that (${res.status}).`);
      statusEl.textContent = `Saved: ${body.path}`;
    } catch (e) {
      statusEl.textContent = "";
      errorEl.textContent = e.message === "Failed to fetch" ? "WolfWeb didn't answer. Restart WolfWeb and try again." : e.message;
    } finally {
      btn.disabled = false;
    }
  });

  // ── Start: today's paper if it's already made, otherwise make it (or ask what to put in it) ───────────────────
  (async () => {
    await loadSettings();
    if (!settings.topics.length && !settings.feeds.length) {
      openSetup();
      return;
    }
    const { dailyEdition } = await chrome.storage.local.get({ dailyEdition: null });
    if (dailyEdition && dailyEdition.date === today()) render(dailyEdition);
    else await refresh();
  })();
})();
