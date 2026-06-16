import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { injectFonts } from "./fonts";
import { t, setLang, applyI18n } from "./i18n";
import "./settings.css";

injectFonts();

// Replay the unfold animation each time the window is re-shown (it's hidden,
// not destroyed, so the CSS animation only runs once otherwise).
listen("window:shown", () => {
  const el = document.querySelector<HTMLElement>(".window");
  if (!el) return;
  el.style.animation = "none";
  void el.offsetWidth;
  el.style.animation = "";
  invoke<string>("get_accent").then(setGlow);
});

// The island's live accent drives BOTH the "flashlight" glow and the window's
// own controls (toggles, dot, sliders), so the panel reads as part of the
// island — exactly like the AsBar AI window.
function setGlow(color: string) {
  if (!color) return;
  const r = document.documentElement.style;
  r.setProperty("--glow", color);
  r.setProperty("--accent", color);
}
listen<string>("accent:update", (e) => setGlow(e.payload));
invoke<string>("get_accent").then(setGlow);

interface Config {
  width: number;
  height: number;
  margin_top: number;
  anchor: "left" | "center" | "right";
  offset_x: number;
  bg_color: string;
  text_color: string;
  accent_color: string;
  opacity: number;
  corner_radius: number;
  follow_system_accent: boolean;
  always_on_top: boolean;
  dynamic_accent: boolean;
  autostart: boolean;
  language: string;
}

let currentLang = "ru";
let currentAnchor: Config["anchor"] = "center";

const ANCHOR_LABEL: Record<Config["anchor"], string> = {
  left: "settings.left",
  center: "settings.center",
  right: "settings.right",
};

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const el = {
  width: $<HTMLInputElement>("width"),
  height: $<HTMLInputElement>("height"),
  margin_top: $<HTMLInputElement>("margin_top"),
  offset_x: $<HTMLInputElement>("offset_x"),
  bg_color: $<HTMLInputElement>("bg_color"),
  text_color: $<HTMLInputElement>("text_color"),
  accent_color: $<HTMLInputElement>("accent_color"),
  dynamic_accent: $<HTMLInputElement>("dynamic_accent"),
  follow_system_accent: $<HTMLInputElement>("follow_system_accent"),
  opacity: $<HTMLInputElement>("opacity"),
  corner_radius: $<HTMLInputElement>("corner_radius"),
  always_on_top: $<HTMLInputElement>("always_on_top"),
  autostart: $<HTMLInputElement>("autostart"),
};

const valLabels: Record<string, (v: number) => string> = {
  width: (v) => `${v}px`,
  height: (v) => `${v}px`,
  margin_top: (v) => `${v}px`,
  offset_x: (v) => `${v}px`,
  opacity: (v) => `${Math.round(v * 100)}%`,
  corner_radius: (v) => `${v}px`,
};

function updateValLabel(id: string, v: number) {
  const label = document.getElementById(`${id}-val`);
  if (label && valLabels[id]) label.textContent = valLabels[id](v);
  // Fill the slider track up to the thumb in the accent color.
  const input = document.getElementById(id) as HTMLInputElement | null;
  if (input && input.type === "range") {
    const min = Number(input.min || 0);
    const max = Number(input.max || 100);
    const pct = ((v - min) / (max - min)) * 100;
    input.style.setProperty("--fill", `${pct}%`);
  }
}

function collect(): Config {
  return {
    anchor: currentAnchor,
    width: +el.width.value,
    height: +el.height.value,
    margin_top: +el.margin_top.value,
    offset_x: +el.offset_x.value,
    bg_color: el.bg_color.value,
    text_color: el.text_color.value,
    accent_color: el.accent_color.value,
    dynamic_accent: el.dynamic_accent.checked,
    follow_system_accent: el.follow_system_accent.checked,
    opacity: +el.opacity.value,
    corner_radius: +el.corner_radius.value,
    always_on_top: el.always_on_top.checked,
    autostart: el.autostart.checked,
    language: currentLang,
  };
}

/** Apply the chosen language to the UI and highlight its tile. */
function setLanguage(lang: string) {
  currentLang = lang === "en" ? "en" : "ru";
  setLang(currentLang);
  applyI18n();
  document.querySelectorAll<HTMLElement>(".lang").forEach((b) => {
    b.classList.toggle("active", b.dataset.lang === currentLang);
  });
  // Re-translate the anchor dropdown label (applyI18n only touches data-i18n,
  // but the button label reflects the *selected* option).
  $("anchor-label").textContent = t(ANCHOR_LABEL[currentAnchor]);
}

// ---- Anchor dropdown -----------------------------------------------------
const anchorDd = $("anchor-dd");
function setAnchor(value: Config["anchor"], save: boolean) {
  currentAnchor = value;
  $("anchor-label").textContent = t(ANCHOR_LABEL[value]);
  anchorDd.querySelectorAll<HTMLElement>(".dd-opt").forEach((o) => {
    o.classList.toggle("active", o.dataset.value === value);
  });
  if (save) scheduleSave();
}
$("anchor-btn").addEventListener("click", (e) => {
  e.stopPropagation();
  anchorDd.classList.toggle("open");
});
anchorDd.querySelectorAll<HTMLElement>(".dd-opt").forEach((opt) => {
  opt.addEventListener("click", () => {
    setAnchor((opt.dataset.value || "center") as Config["anchor"], true);
    anchorDd.classList.remove("open");
  });
});
document.addEventListener("click", (e) => {
  if (!anchorDd.contains(e.target as Node)) anchorDd.classList.remove("open");
});

function populate(cfg: Config) {
  setAnchor(cfg.anchor, false);
  el.width.value = String(cfg.width);
  el.height.value = String(cfg.height);
  el.margin_top.value = String(cfg.margin_top);
  el.offset_x.value = String(cfg.offset_x);
  el.bg_color.value = cfg.bg_color;
  el.text_color.value = cfg.text_color;
  el.accent_color.value = normalizeHex(cfg.accent_color);
  el.dynamic_accent.checked = cfg.dynamic_accent;
  el.follow_system_accent.checked = cfg.follow_system_accent;
  el.opacity.value = String(cfg.opacity);
  el.corner_radius.value = String(cfg.corner_radius);
  el.always_on_top.checked = cfg.always_on_top;
  el.autostart.checked = cfg.autostart;

  for (const id of ["width", "height", "margin_top", "offset_x", "opacity", "corner_radius"]) {
    updateValLabel(id, +(el as any)[id].value);
  }
  reflect();
  setLanguage(cfg.language);
}

/** color inputs need #rrggbb; coerce rgb()/short forms to a safe default. */
function normalizeHex(c: string): string {
  return /^#[0-9a-fA-F]{6}$/.test(c) ? c : "#c7c7d1";
}

function reflect() {
  // When using the Windows accent, the manual color picker is moot.
  el.accent_color.disabled = el.follow_system_accent.checked;
  el.accent_color.style.opacity = el.follow_system_accent.checked ? "0.4" : "1";
}

let saveTimer: number | undefined;
function scheduleSave() {
  const cfg = collect();
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(async () => {
    try {
      await invoke("save_config", { config: cfg });
      const fresh = await invoke<Config>("get_config");
      el.autostart.checked = fresh.autostart;
    } catch (e) {
      console.error("save_config failed", e);
    }
  }, 110);
}

for (const input of Object.values(el)) {
  const ev = input.type === "range" || input.tagName === "SELECT" ? "input" : "change";
  input.addEventListener(ev, (e) => {
    const t = e.target as HTMLInputElement;
    if (t.type === "range") updateValLabel(t.id, +t.value);
    if (t.id === "follow_system_accent") reflect();
    scheduleSave();
  });
}

// Language tiles: switch the UI language live and persist it.
document.querySelectorAll<HTMLElement>(".lang").forEach((btn) => {
  btn.addEventListener("click", () => {
    const lang = btn.dataset.lang || "ru";
    if (lang === currentLang) return;
    setLanguage(lang);
    scheduleSave();
  });
});

$("close").addEventListener("click", () => invoke("close_settings_window"));
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") invoke("close_settings_window");
});

invoke<Config>("get_config").then(populate);
