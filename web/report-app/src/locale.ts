export type Locale = "en" | "zh-CN";

export type LocaleInputs = {
  search: string;
  stored?: string | null;
  browserLanguages?: readonly string[];
};

export function normalizeLocale(value?: string | null): Locale | undefined {
  if (!value) return undefined;
  const normalized = value.trim().toLowerCase();
  if (normalized === "zh" || normalized === "zh-cn") return "zh-CN";
  if (normalized === "en" || normalized.startsWith("en-")) return "en";
  return undefined;
}

export function resolveLocale({ search, stored, browserLanguages = [] }: LocaleInputs): Locale {
  const query = normalizeLocale(new URLSearchParams(search).get("lang"));
  if (query) return query;
  const persisted = normalizeLocale(stored);
  if (persisted) return persisted;
  for (const language of browserLanguages) {
    const browser = normalizeLocale(language);
    if (browser) return browser;
  }
  return "en";
}

export function initialLocale(): Locale {
  let stored: string | null = null;
  try {
    stored = window.localStorage.getItem("reforge.locale");
  } catch {
    // Reports opened from restricted file URLs can deny storage access.
  }
  return resolveLocale({
    search: window.location.search,
    stored,
    browserLanguages: navigator.languages?.length ? navigator.languages : [navigator.language],
  });
}

export function persistLocale(locale: Locale): void {
  document.documentElement.lang = locale;
  const url = new URL(window.location.href);
  url.searchParams.set("lang", locale);
  window.history.replaceState(null, "", url);
  try {
    window.localStorage.setItem("reforge.locale", locale);
  } catch {
    // The visible locale still changes when storage is unavailable.
  }
}
