//! Tiny i18n layer shared by every window.
//!
//! Static markup carries `data-i18n` (textContent), `data-i18n-ph`
//! (placeholder), `data-i18n-title` (title) or `data-i18n-aria` (aria-label)
//! attributes — `applyI18n()` fills them in. Dynamic strings use `t(key)`.

export type Lang = "ru" | "en";

type Dict = Record<string, string>;

const RU: Dict = {
  // island
  "island.noPlayer": "Нет активного проигрывателя",
  "island.waiting": "Ожидание плеера…",
  "island.untitled": "Без названия",
  "island.source": "Источник",
  "island.tip": "Двойной клик — настройки · ПКМ — AI",
  // settings
  "settings.page": "Настройки",
  "settings.close": "Закрыть",
  "settings.geometry": "Геометрия",
  "settings.position": "Положение",
  "settings.left": "Слева",
  "settings.center": "По центру",
  "settings.right": "Справа",
  "settings.width": "Ширина",
  "settings.height": "Высота",
  "settings.marginTop": "Отступ сверху",
  "settings.offsetX": "Смещение по X",
  "settings.appearance": "Внешний вид",
  "settings.bg": "Фон",
  "settings.text": "Текст",
  "settings.accent": "Акцент",
  "settings.dynamicAccent": "Цвет с обложки",
  "settings.dynamicAccentSub": "акцент берётся из арта трека",
  "settings.systemAccent": "Акцент Windows",
  "settings.systemAccentSub": "использовать системный цвет",
  "settings.opacity": "Прозрачность",
  "settings.cornerRadius": "Скругление",
  "settings.alwaysOnTop": "Поверх приложений",
  "settings.autostart": "Запуск с Windows",
  "settings.language": "Язык",
  // assistant
  "assistant.ask": "Спросите что угодно",
  "assistant.hint": "AsBar AI · Ctrl + Space",
  "assistant.placeholder": "Сообщение…",
  "assistant.you": "Вы",
  "assistant.ai": "AI",
  "assistant.error": "Ошибка",
  "assistant.newChat": "Новый чат",
  "assistant.close": "Закрыть",
  "assistant.system":
    "Ты — AI-ассистент внутри приложения AsBar (Dynamic Island для Windows). " +
    "Отвечай кратко и по делу, на русском языке.",
};

const EN: Dict = {
  // island
  "island.noPlayer": "No active player",
  "island.waiting": "Waiting for a player…",
  "island.untitled": "Untitled",
  "island.source": "Source",
  "island.tip": "Double-click — settings · Right-click — AI",
  // settings
  "settings.page": "Settings",
  "settings.close": "Close",
  "settings.geometry": "Geometry",
  "settings.position": "Position",
  "settings.left": "Left",
  "settings.center": "Center",
  "settings.right": "Right",
  "settings.width": "Width",
  "settings.height": "Height",
  "settings.marginTop": "Top offset",
  "settings.offsetX": "Horizontal offset",
  "settings.appearance": "Appearance",
  "settings.bg": "Background",
  "settings.text": "Text",
  "settings.accent": "Accent",
  "settings.dynamicAccent": "Color from artwork",
  "settings.dynamicAccentSub": "accent taken from the track art",
  "settings.systemAccent": "Windows accent",
  "settings.systemAccentSub": "use the system color",
  "settings.opacity": "Opacity",
  "settings.cornerRadius": "Corner radius",
  "settings.alwaysOnTop": "Always on top",
  "settings.autostart": "Launch on startup",
  "settings.language": "Language",
  // assistant
  "assistant.ask": "Ask anything",
  "assistant.hint": "AsBar AI · Ctrl + Space",
  "assistant.placeholder": "Message…",
  "assistant.you": "You",
  "assistant.ai": "AI",
  "assistant.error": "Error",
  "assistant.newChat": "New chat",
  "assistant.close": "Close",
  "assistant.system":
    "You are the AI assistant inside AsBar (a Dynamic Island for Windows). " +
    "Answer concisely and to the point, in English.",
};

const DICT: Record<Lang, Dict> = { ru: RU, en: EN };

let current: Lang = "ru";

export function getLang(): Lang {
  return current;
}

export function setLang(lang: string | undefined) {
  current = lang === "en" ? "en" : "ru";
}

export function t(key: string): string {
  return DICT[current][key] ?? RU[key] ?? key;
}

/** Fill every translatable node under `root` with the current language. */
export function applyI18n(root: ParentNode = document) {
  root.querySelectorAll<HTMLElement>("[data-i18n]").forEach((el) => {
    el.textContent = t(el.dataset.i18n!);
  });
  root.querySelectorAll<HTMLElement>("[data-i18n-ph]").forEach((el) => {
    (el as HTMLInputElement | HTMLTextAreaElement).placeholder = t(el.dataset.i18nPh!);
  });
  root.querySelectorAll<HTMLElement>("[data-i18n-title]").forEach((el) => {
    el.title = t(el.dataset.i18nTitle!);
  });
  root.querySelectorAll<HTMLElement>("[data-i18n-aria]").forEach((el) => {
    el.setAttribute("aria-label", t(el.dataset.i18nAria!));
  });
}
