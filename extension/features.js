// WolfWeb's switchable features — all on to start with; WolfWeb settings (settings.html) turns them off.
// eslint-disable-next-line no-unused-vars
const WOLFWEB_FEATURES = {
  defaults: { menuBar: true, dailyButton: true, statusBar: true, gestures: true, askDefault: true },
  async get() {
    const { features } = await chrome.storage.local.get({ features: {} });
    return { ...WOLFWEB_FEATURES.defaults, ...(features || {}) };
  },
  async set(name, on) {
    const current = await WOLFWEB_FEATURES.get();
    current[name] = on;
    await chrome.storage.local.set({ features: current });
  },
};
