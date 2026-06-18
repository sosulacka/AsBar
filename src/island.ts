import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { injectFonts } from "./fonts";
import { t, setLang, applyI18n } from "./i18n";
import "./island.css";

injectFonts();

interface Snapshot {
  has_session: boolean;
  title: string;
  artist: string;
  album: string;
  status: string;
  source: string;
  source_id: string;
  position: number;
  duration: number;
  track_id: string;
}

interface MediaEvent {
  media: Snapshot;
  thumb: string | null;
  thumbChanged: boolean;
}

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const island = $("island");
const artImg = $<HTMLImageElement>("art-img");
const glowImg = $<HTMLImageElement>("glow-img");
const titleEl = $("title");
const titleSeg = titleEl.querySelector(".seg") as HTMLElement;
const titleWrap = titleEl.parentElement as HTMLElement;
const subtitleEl = $("subtitle");
const sourceName = $("source-name");
const sourceIc = $("source-ic");
const fill = $("progress-fill");
const knob = $("progress-knob");
const seekEl = $("seek");
// ---- Equalizer (canvas) --------------------------------------------------
// One canvas, one compositor layer. The backend streams band levels as targets;
// a rAF loop eases the bars toward them and STOPS as soon as they settle, so a
// steady passage animates nothing and the island idles at its paused cost.
const eqCanvas = $<HTMLCanvasElement>("eq");
const eqCtx = eqCanvas.getContext("2d", { alpha: true });
const EQ_BANDS = 5;
const EQ_W = 34, EQ_H = 16, EQ_BW = 4, EQ_GAP = 3.5;
let eqDpr = 1;
function setupEqCanvas() {
  eqDpr = Math.max(1, Math.min(2, window.devicePixelRatio || 1));
  eqCanvas.width = Math.round(EQ_W * eqDpr);
  eqCanvas.height = Math.round(EQ_H * eqDpr);
  eqCanvas.style.width = EQ_W + "px";
  eqCanvas.style.height = EQ_H + "px";
}
setupEqCanvas();

const eqTarget = new Array(EQ_BANDS).fill(0);
const eqCur = new Array(EQ_BANDS).fill(0);
let eqRaf = 0;
let eqAccent = "#e0e0ec"; // kept in sync by setAccent()

function eqRoundRect(c: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  c.beginPath();
  c.moveTo(x + r, y);
  c.arcTo(x + w, y, x + w, y + h, r);
  c.arcTo(x + w, y + h, x, y + h, r);
  c.arcTo(x, y + h, x, y, r);
  c.arcTo(x, y, x + w, y, r);
  c.closePath();
}

function drawEq() {
  if (!eqCtx) { eqRaf = 0; return; }
  let moving = false;
  for (let i = 0; i < EQ_BANDS; i++) {
    const d = eqTarget[i] - eqCur[i];
    eqCur[i] += d * 0.35;
    if (Math.abs(d) > 0.003) moving = true;
  }
  const dpr = eqDpr;
  eqCtx.clearRect(0, 0, eqCanvas.width, eqCanvas.height);
  eqCtx.fillStyle = eqAccent;
  eqCtx.shadowColor = eqAccent;
  eqCtx.shadowBlur = 4 * dpr;
  for (let i = 0; i < EQ_BANDS; i++) {
    const v = Math.max(0, Math.min(1, eqCur[i]));
    const h = (3 + v * (EQ_H - 3)) * dpr;
    const w = EQ_BW * dpr;
    const x = i * (EQ_BW + EQ_GAP) * dpr;
    const y = EQ_H * dpr - h;
    eqCtx.globalAlpha = 0.45 + v * 0.55;
    eqRoundRect(eqCtx, x, y, w, h, Math.min(w / 2, h / 2, 3 * dpr));
    eqCtx.fill();
  }
  eqCtx.globalAlpha = 1;
  // Keep animating only while the bars are actually in motion and visible.
  eqRaf = moving && playing && !document.hidden ? requestAnimationFrame(drawEq) : 0;
}
/** Wake the eq loop after new targets arrive (no-op if already running/idle). */
function kickEq() {
  if (eqRaf === 0 && playing && !document.hidden) eqRaf = requestAnimationFrame(drawEq);
}

// ---- Tight window sizing -------------------------------------------------
// Measure the real pill (plus any open popup) and ask Rust to size the window
// to match, so transparent margins never block desktop clicks.
const islandBg = island.querySelector(".island-bg") as HTMLElement;
let lastSent = "";
function syncSize() {
  // Pill box.
  const r = islandBg.getBoundingClientRect();
  let w = r.width;
  let h = r.height;
  // If the tooltip is visible, extend the window to cover it.
  if (island.classList.contains("show-tip")) {
    const pop = $("tip").getBoundingClientRect();
    h = Math.max(h, pop.bottom - r.top);
    w = Math.max(w, pop.right - r.left, r.right - pop.left);
  }
  const key = `${Math.ceil(w)}x${Math.ceil(h)}`;
  if (key === lastSent) return;
  lastSent = key;
  invoke("resize_island", { w: Math.ceil(w), h: Math.ceil(h) });
}
const ro = new ResizeObserver(() => syncSize());
ro.observe(islandBg);

let dynamicAccent = true;
let configAccent = "#e0e0ec";

let lastPos = 0;
let lastDur = 0;
let lastStamp = performance.now();
let playing = false;
let seeking = false;

// ---- Source identity (color + glyph) -------------------------------------
const SPOTIFY = `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M12 2a10 10 0 1 0 .001 20.001A10 10 0 0 0 12 2zm4.6 14.5a.9.9 0 0 1-1.24.3c-2.9-1.77-6.55-2.17-10.86-1.2a.9.9 0 1 1-.4-1.76c4.76-1.07 8.86-.6 12.19 1.44.42.26.55.82.31 1.22zm1.23-2.74a1.12 1.12 0 0 1-1.54.37c-3.32-2.04-8.38-2.63-12.3-1.44a1.12 1.12 0 1 1-.65-2.15c4.48-1.36 10.06-.7 13.87 1.64.53.32.7 1.02.62 1.58zm.1-2.85C14.2 8.6 7.97 8.38 4.5 9.43a1.35 1.35 0 1 1-.78-2.58c3.98-1.2 10.86-.97 15.14 1.57a1.35 1.35 0 0 1-1.4 2.31z"/></svg>`;
const YOUTUBE = `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M23.5 6.2c-.3-1-1-1.8-2-2C19.8 3.7 12 3.7 12 3.7s-7.8 0-9.5.5c-1 .3-1.8 1-2 2C0 8 0 12 0 12s0 4 .5 5.8c.3 1 1 1.8 2 2 1.7.5 9.5.5 9.5.5s7.8 0 9.5-.5c1-.3 1.8-1 2-2 .5-1.8.5-5.8.5-5.8s0-4-.5-5.8zM9.6 15.6V8.4l6.4 3.6-6.4 3.6z"/></svg>`;
const MUSIC = `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6z"/></svg>`;
const GLOBE = `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm-1 17.9A8 8 0 0 1 4 12c0-.6.07-1.2.2-1.8L9 15v1a2 2 0 0 0 2 2v1.9zm6.9-2.5a2 2 0 0 0-1.9-1.4h-1v-3a1 1 0 0 0-1-1H8v-2h2a1 1 0 0 0 1-1V7h2a2 2 0 0 0 2-2v-.4a8 8 0 0 1 2.9 12.8z"/></svg>`;

function sourceColor(source: string): string | null {
  const s = source.toLowerCase();
  if (s.includes("spotify")) return "#1DB954";
  if (s.includes("youtube")) return "#FF3B30";
  if (s.includes("yandex")) return "#FFCC00";
  if (s.includes("apple") || s.includes("itunes")) return "#FA243C";
  if (s.includes("edge")) return "#33B1E1";
  if (s.includes("chrome")) return "#4285F4";
  if (s.includes("firefox") || s.includes("zen")) return "#FF7139";
  if (s.includes("vlc")) return "#FF8800";
  return null;
}
function sourceGlyph(source: string): string {
  const s = source.toLowerCase();
  if (s.includes("spotify")) return SPOTIFY;
  if (s.includes("youtube")) return YOUTUBE;
  if (s.includes("chrome") || s.includes("edge") || s.includes("firefox") || s.includes("zen") || s.includes("opera") || s.includes("brave"))
    return GLOBE;
  return MUSIC;
}

function currentPos(): number {
  let pos = lastPos;
  if (playing && !seeking) pos += (performance.now() - lastStamp) / 1000;
  return Math.min(pos, lastDur || pos);
}
let progTimer = 0;
let lastPct = -1;
function paintProgress() {
  let pct = 0;
  if (lastDur > 0) pct = Math.min(100, Math.max(0, (currentPos() / lastDur) * 100));
  // The fill/knob carry CSS transitions, so sub-pixel updates are wasted work.
  // Only touch the DOM when the bar actually moves a visible amount.
  if (Math.abs(pct - lastPct) < 0.05) return;
  lastPct = pct;
  fill.style.width = pct + "%";
  knob.style.left = pct + "%";
}
function stopTicking() {
  if (progTimer) { clearInterval(progTimer); progTimer = 0; }
}
/** Drive the progress bar on a slow timer while playing; CSS transitions smooth
 *  the steps. No per-frame rAF — at rest the island does essentially no work.
 *  Stops itself when paused, idle, done seeking, or the window is hidden. */
function ensureTicking() {
  if (progTimer || !playing || document.hidden || lastDur <= 0) return;
  paintProgress();
  progTimer = window.setInterval(() => {
    if (!playing || document.hidden || lastDur <= 0) { stopTicking(); return; }
    paintProgress();
  }, 250);
}
// Pause the timer when the island is hidden (Win+D, another desktop, etc.).
document.addEventListener("visibilitychange", () => {
  if (document.hidden) {
    stopTicking();
  } else {
    paintProgress();
    ensureTicking();
    kickEq();
  }
});

/** Vivid accent from album art. */
function accentFromImage(src: string): Promise<string | null> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      const size = 36;
      const c = document.createElement("canvas");
      c.width = size;
      c.height = size;
      const ctx = c.getContext("2d");
      if (!ctx) return resolve(null);
      ctx.drawImage(img, 0, 0, size, size);
      let data: Uint8ClampedArray;
      try {
        data = ctx.getImageData(0, 0, size, size).data;
      } catch {
        return resolve(null);
      }
      let r = 0, g = 0, b = 0, wsum = 0;
      for (let i = 0; i < data.length; i += 4) {
        const R = data[i], G = data[i + 1], B = data[i + 2], A = data[i + 3];
        if (A < 128) continue;
        const max = Math.max(R, G, B), min = Math.min(R, G, B);
        const sat = max === 0 ? 0 : (max - min) / max;
        const lum = max / 255;
        if (lum < 0.12 || lum > 0.96) continue;
        const w = sat * sat * (1 - Math.abs(lum - 0.55)) + 0.02;
        r += R * w; g += G * w; b += B * w; wsum += w;
      }
      if (wsum === 0) return resolve(null);
      let cr = r / wsum, cg = g / wsum, cb = b / wsum;
      // Boost saturation so the accent (and the window glow) reads as a real
      // color, not a washed-out grey, even on muted album art.
      const mx = Math.max(cr, cg, cb), mn = Math.min(cr, cg, cb);
      if (mx > mn) {
        const boost = 1.4;
        const mid = (mx + mn) / 2;
        cr = Math.min(255, Math.max(0, mid + (cr - mid) * boost));
        cg = Math.min(255, Math.max(0, mid + (cg - mid) * boost));
        cb = Math.min(255, Math.max(0, mid + (cb - mid) * boost));
      }
      // Lift brightness so it stands out on the dark windows.
      const lum = Math.max(cr, cg, cb);
      if (lum < 170) {
        const k = 170 / Math.max(lum, 1);
        cr = Math.min(255, cr * k); cg = Math.min(255, cg * k); cb = Math.min(255, cb * k);
      }
      resolve(`rgb(${cr | 0}, ${cg | 0}, ${cb | 0})`);
    };
    img.onerror = () => resolve(null);
    img.src = src;
  });
}

let lastReported = "";
function setAccent(color: string) {
  document.documentElement.style.setProperty("--accent", color);
  eqAccent = color;
  // Mirror to the other windows for their glow (debounced by equality).
  if (color !== lastReported) {
    lastReported = color;
    invoke("report_accent", { color });
  }
}

/** Seamless marquee: if the title overflows, duplicate it and scroll the pair
 *  continuously so the second copy enters from the right as the first exits
 *  left — no pause at the end. Otherwise show a single static, centered copy. */
function setTitle(text: string) {
  titleEl.classList.remove("marquee");
  titleSeg.textContent = text;
  // Drop any duplicate copy from a previous track.
  while (titleSeg.nextElementSibling) titleSeg.nextElementSibling.remove();
  titleEl.style.removeProperty("--seg-w");

  requestAnimationFrame(() => {
    const overflow = titleSeg.scrollWidth - titleWrap.clientWidth;
    if (overflow > 4) {
      const gap = 44; // px between the two copies
      const dup = titleSeg.cloneNode(true) as HTMLElement;
      titleEl.appendChild(dup);
      const shift = titleSeg.scrollWidth + gap;
      titleEl.style.setProperty("--seg-w", `${shift}px`);
      // ~60px/sec, min 6s, so long titles aren't dizzying.
      titleEl.style.setProperty("--marquee-dur", `${Math.max(6, shift / 55)}s`);
      titleEl.classList.add("marquee");
    }
  });
}

let lastSnap: Snapshot | null = null;
let lastThumb: string | null = null; // data-uri of the current art, or null

async function render(ev: MediaEvent) {
  const m = ev.media;
  lastSnap = m;

  if (!m.has_session) {
    island.classList.add("idle");
    island.classList.remove("playing", "has-art");
    setTitle(t("island.noPlayer"));
    subtitleEl.textContent = "";
    lastDur = 0;
    artImg.removeAttribute("src");
    glowImg.removeAttribute("src");
    setAccent(configAccent);
    return;
  }

  island.classList.remove("idle");
  island.classList.toggle("playing", m.status === "playing");
  playing = m.status === "playing";

  if (titleSeg.textContent !== (m.title || t("island.untitled"))) {
    setTitle(m.title || t("island.untitled"));
  }
  subtitleEl.textContent = m.artist || m.album || "";
  sourceName.textContent = m.source || t("island.source");
  sourceIc.innerHTML = sourceGlyph(m.source);

  if (!seeking) {
    lastPos = m.position;
    lastDur = m.duration;
    lastStamp = performance.now();
  }
  // Paint the bar once for this update, and run the rAF loop only while
  // actually playing (it self-stops on pause via tick()).
  paintProgress();
  if (playing && !document.hidden) ensureTicking();

  if (ev.thumb !== null) {
    if (ev.thumb === "") {
      island.classList.remove("has-art");
      artImg.removeAttribute("src");
      glowImg.removeAttribute("src");
      lastThumb = null;
      pickAccent(m, null);
    } else {
      artImg.src = ev.thumb;
      glowImg.src = ev.thumb;
      island.classList.add("has-art");
      lastThumb = ev.thumb;
      pickAccent(m, ev.thumb);
    }
  } else {
    // source can change without thumb; refresh accent if no art-derived one.
    if (!island.classList.contains("has-art")) pickAccent(m, null);
  }
}

/** Priority: album-art color (if dynamic) → service brand color → config. */
async function pickAccent(m: Snapshot, thumb: string | null) {
  if (dynamicAccent && thumb) {
    const c = await accentFromImage(thumb);
    if (c) return setAccent(c);
  }
  const brand = sourceColor(m.source);
  setAccent(brand ?? configAccent);
}

function applyTheme(cfg: any) {
  setLang(cfg.language);
  applyI18n();
  const r = document.documentElement.style;
  r.setProperty("--w", `${cfg.width}px`);
  r.setProperty("--h", `${cfg.height}px`);
  r.setProperty("--radius", `${cfg.corner_radius}px`);
  r.setProperty("--bg", cfg.bg_color);
  r.setProperty("--text", cfg.text_color);
  r.setProperty("--opacity", String(cfg.opacity));
  if (cfg.pad != null) r.setProperty("--pad", `${cfg.pad}px`);
  dynamicAccent = !!cfg.dynamic_accent;
  configAccent = cfg.accent_color;
  // Re-derive the accent immediately from the current state so toggling
  // "color from artwork" applies at once (no waiting for the next track).
  if (lastSnap && lastSnap.has_session) {
    pickAccent(lastSnap, lastThumb);
  } else {
    setAccent(configAccent);
    setTitle(t("island.noPlayer"));
  }
}

// ---- Seek ----------------------------------------------------------------
function ratioFromEvent(e: PointerEvent): number {
  const rect = seekEl.getBoundingClientRect();
  return Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
}
seekEl.addEventListener("pointerdown", (e) => {
  if (lastDur <= 0) return;
  seeking = true;
  seekEl.setPointerCapture(e.pointerId);
  lastPos = ratioFromEvent(e) * lastDur;
  lastStamp = performance.now();
  paintProgress();
});
seekEl.addEventListener("pointermove", (e) => {
  if (!seeking) return;
  lastPos = ratioFromEvent(e) * lastDur;
  lastStamp = performance.now();
  paintProgress();
});
seekEl.addEventListener("pointerup", (e) => {
  if (!seeking) return;
  seeking = false;
  const target = ratioFromEvent(e) * lastDur;
  lastPos = target;
  lastStamp = performance.now();
  paintProgress();
  if (playing && !document.hidden) ensureTicking();
  invoke("media_seek", { position: target });
});

// ---- Events --------------------------------------------------------------
listen<MediaEvent>("media:update", (e) => render(e.payload));
listen<any>("config:update", (e) => applyTheme(e.payload));

// Real-time spectrum from the backend (WASAPI loopback → FFT). Each bar's
// height tracks its frequency band; CSS smooths between the ~45fps packets.
listen<number[]>("viz:levels", (e) => {
  // The equalizer only shows while playing; ignore packets otherwise.
  if (document.hidden || !playing) return;
  const lv = e.payload;
  for (let i = 0; i < EQ_BANDS; i++) eqTarget[i] = Math.max(0, Math.min(1, lv[i] ?? 0));
  kickEq();
});

$("toggle").addEventListener("click", (e) => { e.stopPropagation(); invoke("media_toggle"); });
$("next").addEventListener("click", (e) => { e.stopPropagation(); invoke("media_next"); });
$("prev").addEventListener("click", (e) => { e.stopPropagation(); invoke("media_previous"); });

// Open settings on double-click of the pill BACKGROUND only — not its buttons.
island.addEventListener("dblclick", (e) => {
  const t = e.target as HTMLElement;
  if (t.closest(".ctl, .progress, button, input")) return;
  invoke("open_settings_window");
});
island.addEventListener("contextmenu", (e) => {
  e.preventDefault();
  const t = e.target as HTMLElement;
  if (t.closest(".ctl, .progress, button, input")) return;
  invoke("open_assistant_window");
});

// ---- Custom tooltip ------------------------------------------------------
const tipEl = $("tip");
let tipTimer: number | undefined;
island.addEventListener("mouseenter", () => {
  window.clearTimeout(tipTimer);
  tipTimer = window.setTimeout(() => {
    if (island.classList.contains("idle")) return;
    tipEl.textContent = t("island.tip");
    island.classList.add("show-tip");
    syncSize();
  }, 600);
});
island.addEventListener("mouseleave", () => {
  window.clearTimeout(tipTimer);
  island.classList.remove("show-tip");
  setTimeout(syncSize, 0);
});
island.addEventListener("mousedown", () => {
  window.clearTimeout(tipTimer);
  island.classList.remove("show-tip");
});

// ---- Initial paint -------------------------------------------------------
invoke<Snapshot>("get_now_playing").then((m) => {
  render({ media: m, thumb: m.has_session ? null : "", thumbChanged: true });
});
invoke("request_theme");
// Initial size once layout settles, and a couple of follow-ups for fonts.
requestAnimationFrame(syncSize);
setTimeout(syncSize, 300);
setTimeout(syncSize, 1000);
