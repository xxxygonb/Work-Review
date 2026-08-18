const BASIC_ASSISTANT_MODEL_ID = '__basic__';

type ProviderLabelLocale = 'zh-CN';
type ProviderLabels = Record<ProviderLabelLocale, string>;

type KnownProviderId =
  | 'ollama'
  | 'openai'
  | 'siliconflow'
  | 'deepseek'
  | 'qwen'
  | 'zhipu'
  | 'moonshot'
  | 'doubao'
  | 'minimax'
  | 'gemini'
  | 'claude'
  | 'openrouter'
  | 'groq'
  | 'xai'
  | 'mistral'
  | 'lmstudio'
  | 'custom';

export const MODEL_PROVIDER_DISPLAY_NAMES = {
  ollama: { 'zh-CN': 'Ollama (本地)' },
  openai: { 'zh-CN': 'OpenAI 兼容' },
  siliconflow: { 'zh-CN': '硅基流动' },
  deepseek: { 'zh-CN': 'DeepSeek' },
  qwen: { 'zh-CN': '通义千问' },
  zhipu: { 'zh-CN': '智谱清言' },
  moonshot: { 'zh-CN': 'Kimi' },
  doubao: { 'zh-CN': '豆包' },
  minimax: { 'zh-CN': 'MiniMax' },
  gemini: { 'zh-CN': 'Google Gemini' },
  claude: { 'zh-CN': 'Anthropic Claude' },
  openrouter: { 'zh-CN': 'OpenRouter' },
  groq: { 'zh-CN': 'Groq' },
  xai: { 'zh-CN': 'xAI Grok' },
  mistral: { 'zh-CN': 'Mistral' },
  lmstudio: { 'zh-CN': 'LM Studio (本地)' },
  custom: { 'zh-CN': '自定义接口' },
} as const satisfies Record<KnownProviderId, ProviderLabels>;

interface ModelProfileLike {
  id?: unknown;
  name?: unknown;
  model_config?: unknown;
}

interface ModelConfigLike {
  provider?: unknown;
  model?: unknown;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function hasOwnKey<ObjectType extends object>(
  value: ObjectType,
  key: PropertyKey,
): key is keyof ObjectType {
  return Object.prototype.hasOwnProperty.call(value, key);
}

function isProviderLabelLocale(value: string): value is ProviderLabelLocale {
  return value === 'zh-CN';
}

function translatedLabel(translate: unknown, key: string): string {
  const label = typeof translate === 'function' ? translate(key) : '';
  return typeof label === 'string' && label.trim() ? label : key;
}

function localizedProviderName(providerId: unknown, locale: unknown): string {
  if (typeof providerId !== 'string') return '';
  if (!hasOwnKey(MODEL_PROVIDER_DISPLAY_NAMES, providerId)) return providerId;

  const providerLabels = MODEL_PROVIDER_DISPLAY_NAMES[providerId];
  const localizedLabel =
    typeof locale === 'string' && isProviderLabelLocale(locale)
      ? providerLabels[locale]
      : '';
  return localizedLabel || providerLabels['zh-CN'] || providerId;
}

export function resolveModelOptionLabel(
  selectedModelId: unknown,
  modelProfiles: unknown,
  locale: unknown,
  translate: unknown,
): string {
  const basicLabel = translatedLabel(translate, 'ask.basicTemplate');
  if (selectedModelId === BASIC_ASSISTANT_MODEL_ID) return basicLabel;

  const profiles = Array.isArray(modelProfiles) ? modelProfiles : [];
  const profileValue = profiles.find(
    (item) => isRecord(item) && item.id === selectedModelId,
  );
  if (!isRecord(profileValue)) return basicLabel;
  const profile: ModelProfileLike = profileValue;

  const profileName = typeof profile.name === 'string' ? profile.name.trim() : '';
  if (profileName) return profileName;

  const modelConfig: ModelConfigLike = isRecord(profile.model_config)
    ? profile.model_config
    : {};
  const providerName = localizedProviderName(modelConfig.provider, locale);
  const modelName = typeof modelConfig.model === 'string' ? modelConfig.model.trim() : '';
  if (providerName && modelName) return `${providerName} · ${modelName}`;
  if (modelName) return modelName;

  return translatedLabel(translate, 'ask.aiEnhanced');
}