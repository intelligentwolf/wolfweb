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
  // Ready on first open (BBC News top stories and technology); "Choose what's in it" changes it.
  const DEFAULTS = {
    masthead: "The WolfWeb Daily",
    topics: [],
    feeds: ["https://feeds.bbci.co.uk/news/rss.xml", "https://feeds.bbci.co.uk/news/technology/rss.xml"],
  };
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
  // Interests come from Bing News' RSS search: unlike Google News it links straight to the publisher (in the
  // link's url= parameter), with a real summary and a picture per story (checked 2026-11-03).
  function topicFeed(topic) {
    const region = ((navigator.language || "en-GB").split("-")[1] || "GB").toUpperCase();
    const u = new URL("https://www.bing.com/news/search");
    u.searchParams.set("q", topic);
    u.searchParams.set("format", "rss");
    u.searchParams.set("cc", region);
    return u.toString();
  }

  /** Bing's tracking link → the publisher's own address. */
  function directLink(link) {
    try {
      const u = new URL(link);
      if (/(^|\.)bing\.com$/.test(u.hostname) && u.searchParams.get("url")) return httpUrl(u.searchParams.get("url"));
    } catch {
      /* not a URL */
    }
    return link;
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
        const link = directLink(httpUrl((linkEl && (linkEl.getAttribute("href") || linkEl.textContent)) || "") || "");
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
        // Bing: <News:Image>https://www.bing.com/th?id=…</News:Image> — its image service takes a width.
        const bingImage = (n.getElementsByTagName("News:Image")[0] || {}).textContent;
        const image = media.length
          ? media[0].url
          : enclosure
            ? httpUrl(enclosure.getAttribute("url") || "")
            : bingImage
              ? httpUrl(`${bingImage.trim().replace(/^http:/, "https:")}&w=960`)
              : null;
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

  // ── The stories themselves ─────────────────────────────────────────────────────────────────────────────────
  // Each story gets its article's opening paragraphs, so the paper can be read without clicking through. A small
  // readability-style pass: the <article> (or the block with the most paragraph text), its real paragraphs, minus
  // page furniture. Pages that don't yield text keep the feed's summary.
  const BOILERPLATE = /cookie|subscribe|sign up|newsletter|all rights reserved|©|javascript|advertisement|log in|register to|follow us|share this/i;
  const STORY_WORDS = 200;
  const LEAD_WORDS = 450;

  async function fetchText(url, signal) {
    const res = await fetch(url, { credentials: "omit", signal, redirect: "follow" });
    if (!res.ok || !/text\/html/i.test(res.headers.get("content-type") || "")) return null;
    return res.text();
  }

  function extract(html, words) {
    const doc = new DOMParser().parseFromString(html, "text/html");
    doc.querySelectorAll("script, style, noscript, nav, header, footer, aside, form, figure, iframe, [role='navigation'], [aria-hidden='true']").forEach((e) => e.remove());
    const candidates = [...doc.querySelectorAll("article, main, [itemprop='articleBody'], div, section")];
    let best = null;
    let bestScore = 0;
    for (const c of candidates) {
      const score = [...c.querySelectorAll(":scope > p, :scope > div > p")].reduce((n, p) => n + (p.textContent || "").trim().length, 0);
      const bonus = c.matches("article, [itemprop='articleBody']") ? 1.5 : 1;
      if (score * bonus > bestScore) {
        best = c;
        bestScore = score * bonus;
      }
    }
    if (!best) return { paragraphs: [], image: null };
    const paragraphs = [];
    let count = 0;
    for (const p of best.querySelectorAll("p")) {
      const t = (p.textContent || "").replace(/\s+/g, " ").trim();
      if (t.length < 60 || BOILERPLATE.test(t)) continue;
      paragraphs.push(t);
      count += t.split(" ").length;
      if (count >= words) break;
    }
    const og = doc.querySelector('meta[property="og:image"], meta[name="twitter:image"]');
    return { paragraphs, image: og ? httpUrl(og.getAttribute("content") || "") : null };
  }

  /** Fill stories with their text, a few at a time. */
  async function addText(stories, leadLink) {
    let next = 0;
    let done = 0;
    const worker = async () => {
      while (next < stories.length) {
        const s = stories[next++];
        const ctrl = new AbortController();
        const timer = setTimeout(() => ctrl.abort(), 12000);
        try {
          const html = await fetchText(s.link, ctrl.signal);
          if (html) {
            const got = extract(html, s.link === leadLink ? LEAD_WORDS : STORY_WORDS);
            if (got.paragraphs.join(" ").length > (s.summary || "").length) s.paragraphs = got.paragraphs;
            if (!s.image && got.image) s.image = got.image;
          }
        } catch {
          /* unreachable or too slow: the summary stays */
        } finally {
          clearTimeout(timer);
          done += 1;
          statusEl.textContent = `Reading the stories… ${done} of ${stories.length}`;
        }
      }
    };
    await Promise.all(Array.from({ length: 6 }, worker));
  }

  /** The newest story with a picture, else the newest. */
  function pickLead(all) {
    const byDate = [...all].sort((a, b) => b.date - a.date);
    return byDate.find((x) => x.image) || byDate[0];
  }

  async function buildEdition() {
    const jobs = [
      ...settings.topics.map((t) => fetchFeed(topicFeed(t), t).then((f) => ({ name: t, stories: f.stories }))),
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
    const all = sections.flatMap((x) => x.stories);
    const lead = pickLead(all);
    await addText(all, lead && lead.link);
    return { date: today(), sections, failed, leadLink: lead ? lead.link : null };
  }

  // ── The broadsheet ─────────────────────────────────────────────────────────────────────────────────────────
  function el(tag, cls, content) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (content !== undefined) e.textContent = content;
    return e;
  }

  /** The story's paragraphs (or its summary), then a link to the rest. */
  function appendBody(node, s) {
    const paras = s.paragraphs && s.paragraphs.length ? s.paragraphs : s.summary ? [s.summary] : [];
    paras.forEach((t) => node.appendChild(el("p", "body", t)));
    const more = el("a", "more", `Continue at ${s.source || new URL(s.link).hostname.replace(/^www\./, "")} →`);
    more.href = s.link;
    more.target = "_blank";
    more.rel = "noopener noreferrer";
    node.appendChild(more);
  }

  function storyNode(s, cls) {
    const art = el("article", cls);
    const a = el("a");
    a.href = s.link;
    a.target = "_blank";
    a.rel = "noopener noreferrer";
    if (s.image) {
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
    appendBody(art, s);
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
    const lead = (edition.leadLink && all.find((s) => s.link === edition.leadLink)) || pickLead(all);
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
      appendBody(words, lead);
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
    buildPrintEdition(edition, lead);
    errorEl.textContent = edition.failed.length ? `Couldn't reach: ${edition.failed.join(", ")}` : "";
    if (!edition.sections.length) errorEl.textContent = "No stories came back — check your connection, then press “Fetch today's news”.";
  }

  // ── The printed edition ────────────────────────────────────────────────────────────────────────────────────
  // A broadsheet in print: page 1 is the front page (masthead, the lead story in full, six teasers taken from the
  // sections in turn, and an index); the inside pages run every section in four columns. daily.css lays it out on
  // A4 with the page number at the foot of each page.
  const TEASER_WORDS = 45;
  const TEASERS = 6;

  function words(text, n) {
    const w = text.split(/\s+/);
    return w.length <= n ? text : `${w.slice(0, n).join(" ")}…`;
  }

  function printStory(s, { teaser = false, image = true, lead = false } = {}) {
    const art = el("article", teaser ? "p-teaser" : "p-story");
    if (image && s.image) {
      const img = el("img");
      img.alt = "";
      img.src = s.image;
      img.addEventListener("error", () => img.remove());
      art.appendChild(img);
    }
    // The headline, byline and first paragraph stay together (daily.css .p-head never breaks), so a headline is
    // never left at the foot of a column; Chromium doesn't keep a heading with what follows on its own.
    const head = el("div", "p-head");
    head.appendChild(el("h3", "", s.headline));
    const by = [s.source, s.date ? new Date(s.date).toLocaleDateString(undefined, { day: "numeric", month: "short" }) : ""].filter(Boolean).join(" · ");
    if (by) head.appendChild(el("p", "p-byline", by));
    art.appendChild(head);
    const paras = s.paragraphs && s.paragraphs.length ? s.paragraphs : s.summary ? [s.summary] : [];
    if (teaser) {
      if (paras.length) head.appendChild(el("p", "", words(paras.join(" "), TEASER_WORDS)));
      art.appendChild(el("p", "p-turn", `Continued inside: ${s.section}`));
    } else {
      // The lead's headline spans its columns on its own (daily.css), so its paragraphs all flow in the columns.
      paras.forEach((t, i) => (i === 0 && !lead ? head : art).appendChild(el("p", "", t)));
      art.appendChild(el("p", "p-source", `Full story: ${s.link.replace(/^https?:\/\/(www\.)?/, "").slice(0, 70)}`));
    }
    return art;
  }

  // The front page must be one sheet: A4's 297 mm less the 10 mm top and 12 mm bottom margins (daily.css @page),
  // with 2 mm to spare for rounding. Measured off-screen at the printed width. Things give way gently, in order:
  // pictures on the second row of teasers, a shorter lead photo, the last row of teasers (whole, so the grid stays
  // even), then the lead's closing paragraphs.
  const FRONT_HEIGHT_MM = 297 - 10 - 12 - 2;
  const LEAD_PHOTO_MIN_MM = 45;

  function fitFront(front) {
    const root = $("print-edition");
    root.classList.add("measuring");
    try {
      const max = (FRONT_HEIGHT_MM * 96) / 25.4; // CSS px per mm
      const fits = () => front.getBoundingClientRect().height <= max;
      const teasers = () => [...front.querySelectorAll(".p-teaser")];
      if (!fits()) teasers().slice(3).forEach((t) => t.querySelector("img")?.remove());
      const photo = front.querySelector(".p-lead img");
      if (photo) {
        const mm = (px) => (px * 25.4) / 96;
        for (let h = mm(photo.getBoundingClientRect().height) - 5; !fits() && h >= LEAD_PHOTO_MIN_MM; h -= 5) photo.style.height = `${h}mm`;
      }
      while (!fits() && teasers().length > 3) {
        const t = teasers();
        t.slice(t.length - (t.length % 3 || 3)).forEach((n) => n.remove());
      }
      const leadParas = () => [...front.querySelectorAll(".p-lead .p-story > p:not(.p-byline):not(.p-source)")];
      while (!fits() && leadParas().length > 1) leadParas().pop().remove();
    } finally {
      root.classList.remove("measuring");
    }
  }

  function buildPrintEdition(edition, lead) {
    const root = $("print-edition");
    root.replaceChildren();
    const name = settings.masthead || DEFAULTS.masthead;
    const dateText = new Date(`${edition.date}T12:00:00`).toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "long", year: "numeric" });
    // The running footer: an @page margin box can't read the page's data, so the rule is written with the text in it.
    let foot = $("print-foot");
    if (!foot) {
      foot = el("style");
      foot.id = "print-foot";
      document.head.appendChild(foot);
    }
    foot.textContent = `@page { @bottom-center { content: ${JSON.stringify(`${name} · ${dateText} · Page `)} counter(page) " of " counter(pages); font: 7pt sans-serif; color: #555; } }`;

    // Front page.
    const front = el("section", "p-front");
    const mast = el("header", "p-mast");
    mast.appendChild(el("div", "p-ears", settings.topics.length ? `About: ${settings.topics.join(", ")}` : "Made by WolfWeb"));
    mast.appendChild(el("h1", "p-name", name));
    mast.appendChild(el("div", "p-dateline", dateText));
    front.appendChild(mast);
    if (lead) {
      const leadBox = el("div", "p-lead");
      leadBox.appendChild(el("div", "p-kicker", lead.section));
      leadBox.appendChild(printStory(lead, { lead: true }));
      front.appendChild(leadBox);
    }
    // Six teasers (two rows of three) for a full front page: each section's top story, then each one's next, and so on.
    const queues = edition.sections.map((sec) => sec.stories.filter((st) => st !== lead));
    const tops = [];
    for (let i = 0; tops.length < TEASERS && queues.some((q) => i < q.length); i++) {
      queues.forEach((q) => q[i] && tops.length < TEASERS && tops.push(q[i]));
    }
    if (tops.length) {
      const box = el("div", "p-tops");
      box.appendChild(el("h2", "p-tops-title", "Today's top stories"));
      tops.forEach((st) => box.appendChild(printStory(st, { teaser: true })));
      front.appendChild(box);
    }
    const index = el("div", "p-index");
    index.appendChild(el("h2", "", "Inside"));
    const ul = el("ul");
    edition.sections.forEach((sec) => ul.appendChild(el("li", "", `${sec.name} — ${sec.stories.filter((st) => st !== lead).length} stories`)));
    index.appendChild(ul);
    front.appendChild(index);
    root.appendChild(front);
    fitFront(front);

    // Inside pages.
    const inside = el("section", "p-inside");
    edition.sections.forEach((sec) => {
      const stories = sec.stories.filter((st) => st !== lead);
      if (!stories.length) return;
      // Each section is its own block of columns, with its heading kept on the same page as the first story.
      const block = el("section", "p-sec");
      const cols = el("div", "p-cols");
      stories.forEach((st, i) => cols.appendChild(printStory(st, { image: i === 0 })));
      const top = el("div", "p-sec-top");
      top.appendChild(el("h2", "p-section", sec.name));
      block.append(top, cols);
      inside.appendChild(block);
    });
    root.appendChild(inside);
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
