// Websites are told the reader's language as used in their country (Accept-Language): once the reader has told
// WolfWeb their country (the Daily's "Where are you reading from?", stored as daily.country), WolfWeb puts that
// country's form of their first language first (src/languages.rs) — someone in Britain on a computer set to American
// English is then asked for British English. Done once per country chosen, so a reader who rearranges their
// languages in Chrome's settings afterwards keeps their own order. Runs on WolfWeb's pages (the new tab, the Daily):
// WolfWeb's API answers its pages, not its background worker.
(() => {
  const isCountry = (c) => typeof c === "string" && /^[A-Z]{2}$/.test(c);

  async function apply() {
    const { daily, websiteRegion } = await chrome.storage.local.get({ daily: null, websiteRegion: null });
    const country = daily && daily.country;
    if (!isCountry(country) || country === websiteRegion) return;
    const u = new URL("wolfweb://app/api/languages/region");
    u.searchParams.set("region", country);
    const res = await fetch(u.toString(), { cache: "no-store" });
    if (res.ok) await chrome.storage.local.set({ websiteRegion: country });
    // Not answered: nothing is stored, so the next WolfWeb page tries again.
  }

  apply().catch(() => {});
  chrome.storage.onChanged.addListener((changes, area) => {
    if (area === "local" && changes.daily) apply().catch(() => {});
  });
})();
