import en from './locales/en.js';
import ko from './locales/ko.js';
import ja from './locales/ja.js';
import de from './locales/de.js';
import fr from './locales/fr.js';
import zhCN from './locales/zh-CN.js';
import zhTW from './locales/zh-TW.js';
import ptBR from './locales/pt-BR.js';
import es from './locales/es.js';
import hi from './locales/hi.js';

export type Lang = 'en' | 'ko' | 'ja' | 'de' | 'fr' | 'zh-CN' | 'zh-TW' | 'pt-BR' | 'es' | 'hi';

export const LANGS: { code: Lang; label: string; flag: string }[] = [
  { code: 'en',    label: 'English',    flag: '🇺🇸' },
  { code: 'ko',    label: '한국어',      flag: '🇰🇷' },
  { code: 'ja',    label: '日本語',      flag: '🇯🇵' },
  { code: 'de',    label: 'Deutsch',    flag: '🇩🇪' },
  { code: 'fr',    label: 'Français',   flag: '🇫🇷' },
  { code: 'zh-CN', label: '简体中文',   flag: '🇨🇳' },
  { code: 'zh-TW', label: '繁體中文',   flag: '🇹🇼' },
  { code: 'pt-BR', label: 'Português',  flag: '🇧🇷' },
  { code: 'es',    label: 'Español',    flag: '🇪🇸' },
  { code: 'hi',    label: 'हिन्दी',      flag: '🇮🇳' },
];

const dictionaries = { en, ko, ja, de, fr, 'zh-CN': zhCN, 'zh-TW': zhTW, 'pt-BR': ptBR, es, hi };

type EnDict = typeof en;
export type TranslationKey = keyof EnDict;

function detectLang(): Lang {
  if (typeof navigator === 'undefined') return 'en';
  const nav = navigator.language;
  if (nav.startsWith('ko')) return 'ko';
  if (nav.startsWith('ja')) return 'ja';
  if (nav.startsWith('de')) return 'de';
  if (nav.startsWith('fr')) return 'fr';
  if (nav === 'zh-TW' || nav === 'zh-HK') return 'zh-TW';
  if (nav.startsWith('zh')) return 'zh-CN';
  if (nav === 'pt-BR' || nav.startsWith('pt')) return 'pt-BR';
  if (nav.startsWith('es')) return 'es';
  if (nav.startsWith('hi')) return 'hi';
  return 'en';
}

import { writable, derived, get } from 'svelte/store';

export const lang = writable<Lang>(detectLang());
export function setLang(l: Lang) { lang.set(l); }
export function getLang(): Lang { return get(lang); }

function translate(l: Lang, key: TranslationKey, args: number[]): string {
  const dict = dictionaries[l] ?? dictionaries['en'];
  const val = (dict as any)[key] ?? (dictionaries['en'] as any)[key];
  return typeof val === 'function' ? val(...args) : (val ?? key);
}

export const tStore = derived(lang, ($l) => (key: TranslationKey, ...args: number[]) => translate($l, key, args));

export function t(key: TranslationKey, ...args: number[]): string {
  return translate(get(lang), key, args);
}
