import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import type { Language } from "../types";
import en from "./locales/en.json";
import zhCN from "./locales/zh-CN.json";
import zhTW from "./locales/zh-TW.json";

export const I18N_RESOURCES = {
  "zh-CN": { translation: zhCN },
  "zh-TW": { translation: zhTW },
  en: { translation: en },
} as const;

export const I18N_LANGUAGES: Language[] = ["zh-CN", "zh-TW", "en"];

i18n.use(initReactI18next).init({
  resources: I18N_RESOURCES,
  lng: "zh-CN",
  fallbackLng: "zh-CN",
  interpolation: { escapeValue: false },
  returnNull: false,
});

export function applyLanguage(language: Language): void {
  if (i18n.language !== language) {
    void i18n.changeLanguage(language);
  }
  if (typeof document !== "undefined") {
    document.documentElement.lang = language;
  }
}

export default i18n;
