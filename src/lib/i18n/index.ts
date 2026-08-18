import { get, writable } from 'svelte/store';
import zhCN from './locales/zh-CN.ts';

const LOCALE_STORAGE_KEY = 'work-review.locale';
const DEFAULT_LOCALE: Locale = 'zh-CN';

export const SUPPORTED_LOCALES = ['zh-CN'] as const;
export type Locale = typeof SUPPORTED_LOCALES[number];

export type TranslationValue =
  | string
  | string[]
  | TranslationDictionary;

export interface TranslationDictionary {
  [key: string]: TranslationValue;
}

export type InterpolationParams = Readonly<Record<string, unknown>>;

export interface DurationFormatOptions {
  compact?: boolean;
}

interface LocaleMeta {
  short: string;
  label: string;
}

const LOCALE_CYCLE: readonly Locale[] = ['zh-CN'];

const LOCALE_META = {
  'zh-CN': {
    short: 'ZH',
    label: '简体中文',
  },
} satisfies Record<Locale, LocaleMeta>;

const CATEGORY_LABELS: Record<string, Record<Locale, string>> = {
  development: { 'zh-CN': '开发工具' },
  browser: { 'zh-CN': '浏览器' },
  communication: { 'zh-CN': '通讯协作' },
  office: { 'zh-CN': '办公软件' },
  design: { 'zh-CN': '设计工具' },
  entertainment: { 'zh-CN': '娱乐摸鱼' },
  other: { 'zh-CN': '其他' },
};

const SEMANTIC_LABELS: Record<string, Record<Locale, string>> = {
  '编码开发': { 'zh-CN': '编码开发' },
  '内容撰写': { 'zh-CN': '内容撰写' },
  '资料阅读': { 'zh-CN': '资料阅读' },
  '资料调研': { 'zh-CN': '资料调研' },
  '任务规划': { 'zh-CN': '任务规划' },
  '设计创作': { 'zh-CN': '设计创作' },
  'AI 协作': { 'zh-CN': 'AI 协作' },
  '即时聊天': { 'zh-CN': '即时聊天' },
  '会议沟通': { 'zh-CN': '会议沟通' },
  '视频内容': { 'zh-CN': '视频内容' },
  '音乐音频': { 'zh-CN': '音乐音频' },
  '休息娱乐': { 'zh-CN': '休息娱乐' },
  '未知活动': { 'zh-CN': '未知活动' },
};

const MESSAGES = {
  'zh-CN': zhCN,
} satisfies Record<Locale, TranslationDictionary>;

export const locale = writable<Locale>(DEFAULT_LOCALE);

function isSupportedLocale(value: string): value is Locale {
  return SUPPORTED_LOCALES.some((localeCode) => localeCode === value);
}

function normalizeLocale(_value?: string | null): Locale {
  return DEFAULT_LOCALE;
}

function getStoredLocale(): string | null {
  if (typeof window === 'undefined') {
    return null;
  }

  try {
    return window.localStorage.getItem(LOCALE_STORAGE_KEY);
  } catch {
    return null;
  }
}

function persistLocale(nextLocale: Locale): void {
  if (typeof window === 'undefined') {
    return;
  }

  try {
    window.localStorage.setItem(LOCALE_STORAGE_KEY, nextLocale);
  } catch {
  }
}

export function initializeLocale(_preferredLocale?: string | null): Locale {
  locale.set(DEFAULT_LOCALE);
  persistLocale(DEFAULT_LOCALE);
  return DEFAULT_LOCALE;
}

export function setLocale(_nextLocale?: string | null): Locale {
  locale.set(DEFAULT_LOCALE);
  persistLocale(DEFAULT_LOCALE);
  return DEFAULT_LOCALE;
}

export function cycleLocale(): Locale {
  return DEFAULT_LOCALE;
}

function resolveKey(
  object: TranslationDictionary,
  key: string,
): TranslationValue | undefined {
  let current: TranslationValue | undefined = object;

  for (const segment of key.split('.')) {
    if (Array.isArray(current)) {
      if (!/^(?:0|[1-9]\d*)$/.test(segment)) {
        return undefined;
      }
      current = current[Number(segment)];
      continue;
    }

    if (!current || typeof current !== 'object') {
      return undefined;
    }
    current = current[segment];
  }

  return current;
}

function resolveMessageValue(key: string): TranslationValue | undefined {
  const currentLocale = get(locale);
  return (
    resolveKey(MESSAGES[currentLocale], key) ??
    resolveKey(MESSAGES[DEFAULT_LOCALE], key)
  );
}

function interpolate(template: string, params: InterpolationParams): string {
  return Object.entries(params).reduce(
    (output, [paramKey, paramValue]) => output.replaceAll(`{${paramKey}}`, String(paramValue)),
    template,
  );
}

export function t(key: string, params: InterpolationParams = {}): string {
  const rawValue = resolveMessageValue(key) ?? key;

  if (typeof rawValue !== 'string') {
    return key;
  }

  return interpolate(rawValue, params);
}

export function tm(key: string): TranslationValue | undefined {
  return resolveMessageValue(key);
}

export function getLocaleShortLabel(
  _localeCode: string | null = get(locale),
): string {
  return LOCALE_META[DEFAULT_LOCALE].short;
}

export function getLocaleLabel(_localeCode: string | null = get(locale)): string {
  return LOCALE_META[DEFAULT_LOCALE].label;
}

export function applyLocaleToDocument(
  _nextLocale: string | null = get(locale),
): void {
  if (typeof document === 'undefined') {
    return;
  }

  document.documentElement.lang = DEFAULT_LOCALE;
  document.documentElement.dir = 'ltr';
}

export function formatLocalizedDate(
  date: Date | number,
  options?: Intl.DateTimeFormatOptions,
): string {
  return new Intl.DateTimeFormat(DEFAULT_LOCALE, options).format(date);
}

export function formatLocalizedTime(
  date: Date | number,
  options?: Intl.DateTimeFormatOptions,
): string {
  return new Intl.DateTimeFormat(DEFAULT_LOCALE, options).format(date);
}

export function formatDurationLocalized(
  seconds: number | null | undefined,
  { compact = false }: DurationFormatOptions = {},
): string {
  const hourUnit = compact ? '时' : '小时';
  const minuteUnit = compact ? '分' : '分钟';
  const secondUnit = '秒';

  if (!seconds || seconds <= 0) {
    return `0${minuteUnit}`;
  }

  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  if (hours > 0) {
    return minutes > 0 ? `${hours}${hourUnit}${minutes}${minuteUnit}` : `${hours}${hourUnit}`;
  }

  if (minutes > 0) {
    return `${minutes}${minuteUnit}`;
  }

  return `${secs}${secondUnit}`;
}

export function translateCategoryLabel(categoryKey: string): string {
  const currentLocale = get(locale);
  return CATEGORY_LABELS[categoryKey]?.[currentLocale] || CATEGORY_LABELS[categoryKey]?.[DEFAULT_LOCALE] || categoryKey;
}

export function translateSemanticCategoryLabel(label: string): string {
  const currentLocale = get(locale);
  return SEMANTIC_LABELS[label]?.[currentLocale] || SEMANTIC_LABELS[label]?.[DEFAULT_LOCALE] || label;
}