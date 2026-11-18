import { createContext, useContext } from 'react';
import { en, type TranslationDict } from './en.js';
import { ru } from './ru.js';

export type Locale = 'en' | 'ru';

export const DICTIONARIES: Record<Locale, TranslationDict> = {
  en,
  ru,
};

export const I18nContext = createContext<{
  locale: Locale;
  t: TranslationDict;
  setLocale: (locale: Locale) => void;
}>({
  locale: 'en',
  t: en,
  setLocale: () => {},
});

export function useI18n() {
  return useContext(I18nContext);
}
