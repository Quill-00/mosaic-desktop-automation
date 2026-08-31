import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";

export type Locale = "en" | "zh-CN";

const STORAGE_KEY = "mosaic.locale";

export function localeForTimezoneOffset(offsetMinutes: number): Locale {
  return offsetMinutes === -480 ? "zh-CN" : "en";
}

export function systemDefaultLocale(): Locale {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    if (saved === "en" || saved === "zh-CN") return saved;
  } catch {
    // The timezone fallback still gives first-run users a useful default.
  }
  return localeForTimezoneOffset(new Date().getTimezoneOffset());
}

type I18nValue = {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (english: string, chinese: string) => string;
};

const I18nContext = createContext<I18nValue | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(systemDefaultLocale);

  const setLocale = useCallback((next: Locale) => {
    window.localStorage.setItem(STORAGE_KEY, next);
    setLocaleState(next);
  }, []);

  useEffect(() => {
    document.documentElement.lang = locale;
    document.documentElement.dataset.locale = locale;
    void invoke("set_locale", { locale }).catch(() => {
      // The browser-only documentation harness does not provide a Rust backend.
    });
  }, [locale]);

  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key === STORAGE_KEY && (event.newValue === "en" || event.newValue === "zh-CN")) {
        setLocaleState(event.newValue);
      }
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  const t = useCallback((english: string, chinese: string) => (locale === "zh-CN" ? chinese : english), [locale]);
  const value = useMemo(() => ({ locale, setLocale, t }), [locale, setLocale, t]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nValue {
  const value = useContext(I18nContext);
  if (!value) throw new Error("useI18n must be used inside I18nProvider");
  return value;
}
