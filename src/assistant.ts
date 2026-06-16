import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { injectFonts } from "./fonts";
import { t, setLang, applyI18n } from "./i18n";
import "./assistant.css";

injectFonts();

// Pull the UI language from config, then keep it in sync with live changes.
function syncLang(cfg: any) {
  setLang(cfg?.language);
  applyI18n();
}
invoke<any>("get_config").then(syncLang).catch(() => {});
listen<any>("config:update", (e) => syncLang(e.payload));

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const panel = $("panel");
const threadEl = $("thread");
const inputEl = $<HTMLTextAreaElement>("input");
const sendEl = $<HTMLButtonElement>("send");
const formEl = $<HTMLFormElement>("composer");
const modelWrap = $("model");
const modelBtn = $("model-btn");
const modelLabel = $("model-label");
const modelDot = $("model-dot");
const modelMenu = $("model-menu");

interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

const MODEL_LABELS: Record<string, string> = {
  "gemini-2.5-flash": "Gemini 2.5 Flash",
  "gemini-2.5-flash-lite": "Gemini 2.5 Flash-Lite",
  "qwen/qwen3-32b": "Qwen 3 32B",
};

function providerOf(id: string): "gemini" | "qwen" | "other" {
  if (id.startsWith("gemini")) return "gemini";
  if (id.startsWith("qwen")) return "qwen";
  return "other";
}

// ---- Accent / glow (mirrored from the island) ----------------------------
function setGlow(color: string) {
  if (!color) return;
  const r = document.documentElement.style;
  r.setProperty("--glow", color);
  r.setProperty("--accent", color);
}
listen<string>("accent:update", (e) => setGlow(e.payload));
invoke<string>("get_accent").then(setGlow).catch(() => {});

// ---- Model dropdown -------------------------------------------------------
const history: ChatMessage[] = [];
let busy = false;
let currentModel = "gemini-2.5-flash";

function selectModel(id: string) {
  currentModel = id;
  modelLabel.textContent = MODEL_LABELS[id] ?? id;
  modelDot.className = "pdot " + providerOf(id);
  localStorage.setItem("asbar.model", id);
  modelMenu.querySelectorAll<HTMLElement>(".model-opt").forEach((el) => {
    el.classList.toggle("active", el.dataset.id === id);
  });
}

function openMenu(open: boolean) {
  modelWrap.classList.toggle("open", open);
}

invoke<string[]>("ai_models")
  .then((models) => {
    modelMenu.innerHTML = "";
    for (const id of models) {
      const opt = document.createElement("button");
      opt.type = "button";
      opt.className = "model-opt";
      opt.dataset.id = id;
      opt.innerHTML =
        `<span class="pdot ${providerOf(id)}"></span>` +
        `<span>${MODEL_LABELS[id] ?? id}</span>` +
        `<svg class="tick" viewBox="0 0 24 24" width="14" height="14"><path fill="currentColor" d="M9 16.2 4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4z"/></svg>`;
      // pointerdown (not click): fires immediately and isn't affected by any
      // titlebar drag handling that could otherwise cancel the click.
      opt.addEventListener("pointerdown", (e) => {
        e.preventDefault();
        selectModel(id);
        openMenu(false);
      });
      modelMenu.appendChild(opt);
    }
    const saved = localStorage.getItem("asbar.model");
    selectModel(saved && models.includes(saved) ? saved : models[0] ?? currentModel);
  })
  .catch(() => selectModel(currentModel));

// Keep the header drag-region from intercepting dropdown interactions.
modelWrap.addEventListener("mousedown", (e) => e.stopPropagation());
modelWrap.addEventListener("pointerdown", (e) => e.stopPropagation());

modelBtn.addEventListener("click", (e) => {
  e.stopPropagation();
  openMenu(!modelWrap.classList.contains("open"));
});
// Close the menu when clicking anywhere else.
document.addEventListener("pointerdown", (e) => {
  if (!modelWrap.contains(e.target as Node)) openMenu(false);
});

// ---- Chat rendering -------------------------------------------------------
function clearEmpty() {
  document.getElementById("empty")?.remove();
}

function addMessage(role: "user" | "ai" | "error", text: string): HTMLElement {
  clearEmpty();
  const wrap = document.createElement("div");
  wrap.className = `msg ${role}`;
  const who = document.createElement("div");
  who.className = "who";
  who.textContent =
    role === "user" ? t("assistant.you") : role === "error" ? t("assistant.error") : t("assistant.ai");
  const bubble = document.createElement("div");
  bubble.className = "bubble";
  bubble.textContent = text;
  wrap.append(who, bubble);
  threadEl.appendChild(wrap);
  threadEl.scrollTop = threadEl.scrollHeight;
  return bubble;
}

function addTyping(): HTMLElement {
  clearEmpty();
  const wrap = document.createElement("div");
  wrap.className = "msg ai";
  wrap.innerHTML =
    `<div class="who">AI</div>` +
    `<div class="bubble"><span class="typing"><span></span><span></span><span></span></span></div>`;
  threadEl.appendChild(wrap);
  threadEl.scrollTop = threadEl.scrollHeight;
  return wrap;
}

function setBusy(state: boolean) {
  busy = state;
  sendEl.disabled = state;
  inputEl.disabled = state;
}

async function send() {
  const text = inputEl.value.trim();
  if (!text || busy) return;

  addMessage("user", text);
  history.push({ role: "user", content: text });
  inputEl.value = "";
  autoGrow();
  setBusy(true);
  const typing = addTyping();

  try {
    const reply = await invoke<string>("ai_chat", {
      model: currentModel,
      system: t("assistant.system"),
      messages: history,
    });
    typing.remove();
    addMessage("ai", reply);
    history.push({ role: "assistant", content: reply });
  } catch (e) {
    typing.remove();
    addMessage("error", String(e));
  } finally {
    setBusy(false);
    inputEl.focus();
  }
}

// ---- New chat -------------------------------------------------------------
function newChat() {
  history.length = 0;
  threadEl.innerHTML =
    `<div class="empty" id="empty"><div class="empty-glyph">✦</div>` +
    `<p>${t("assistant.ask")}</p><span>${t("assistant.hint")}</span></div>`;
  inputEl.value = "";
  autoGrow();
  inputEl.focus();
}
$("new-chat").addEventListener("click", newChat);

// ---- Composer -------------------------------------------------------------
formEl.addEventListener("submit", (e) => {
  e.preventDefault();
  send();
});
inputEl.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    send();
  }
});
function autoGrow() {
  inputEl.style.height = "auto";
  inputEl.style.height = Math.min(inputEl.scrollHeight, 120) + "px";
}
inputEl.addEventListener("input", autoGrow);

// ---- Window behaviour -----------------------------------------------------
$("close").addEventListener("click", () => invoke("close_assistant_window"));
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") invoke("close_assistant_window");
});

function focusInput() {
  // A short delay lets the OS finish activating the freshly-shown window.
  setTimeout(() => inputEl.focus(), 60);
}
// Replay the open animation and grab focus each time the window is revealed.
listen("window:shown", () => {
  panel.style.animation = "none";
  void panel.offsetWidth;
  panel.style.animation = "";
  invoke<string>("get_accent").then(setGlow).catch(() => {});
  focusInput();
});
window.addEventListener("focus", focusInput);
focusInput();
