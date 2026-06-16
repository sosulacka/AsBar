<div align="center">

# AsBar

**A Dynamic Island for Windows.**

Now‑playing media, transport controls, and a built‑in AI assistant —
docked at the top of your screen, out of the way until you need it.

[![CI](https://github.com/sosulacka/AsBar/actions/workflows/build.yml/badge.svg)](https://github.com/sosulacka/AsBar/actions/workflows/build.yml)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-0078D6)](#)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB)](https://tauri.app)
[![License](https://img.shields.io/badge/license-MIT-555)](#license)

[English](#overview) · [Русский](#asbar--русская-версия)

</div>

---

## Overview

AsBar renders a compact, glass‑morphic pill at the top‑center of your desktop. It
reads whatever is playing through the Windows media bus (SMTC) — Spotify, YouTube
in any browser, Yandex Music, native players — and surfaces the track, album art,
a live audio‑reactive equalizer, a seek bar, and transport controls. Double‑click
opens settings; right‑click (or `Ctrl + Space`) opens the AI assistant.

The app lives in the system tray and runs quietly in the background.

## Features

- **Universal now‑playing** — any SMTC‑aware source, auto‑detected, with brand
  colors and per‑service glyphs.
- **High‑resolution artwork** — pulls crisp album art (including real YouTube
  thumbnails when a watch tab is open), cached on disk.
- **Live equalizer** — a real spectrum from a WASAPI loopback FFT, not a canned
  animation.
- **Dynamic theming** — the island tints itself with the dominant color of the
  current artwork; the glow propagates to the settings and AI windows.
- **AI assistant** — chat panel with multiple models and **internet tools**: web
  search, page fetching, and reading this project's own source from GitHub.
- **Localization** — full Russian / English UI with a flag‑based switcher; no
  hardcoded strings.
- **Tasteful customization** — size, position, corner radius, opacity, colors,
  always‑on‑top, and launch‑on‑startup.

## Architecture

| Layer | Stack | Responsibility |
|------|-------|----------------|
| **Shell** | Tauri 2, Rust | Windows, tray, geometry, SMTC, audio, autostart |
| **UI** | Vanilla TypeScript, Vite | Island, settings, and assistant webviews |
| **AI relay** | Python (stdlib only) | Multi‑provider chat + agentic tool loop |

Three transparent, undecorated windows (`island`, `settings`, `assistant`) are
sized to their exact visible content so transparent pixels never block the
desktop. Windows‑specific integration uses the `windows` crate: SMTC for media,
the Audio Session API for per‑app volume, UI Automation to read the active
browser tab, and an `IShellLink` shortcut for startup.

```
asbar/
├─ src/                  # webview front-ends
│  ├─ island.ts          # the pill: media, equalizer, seek, theming
│  ├─ settings.ts        # preferences + language picker
│  ├─ assistant.ts       # AI chat panel
│  └─ i18n.ts            # RU/EN dictionary + apply engine
├─ src-tauri/src/
│  ├─ lib.rs             # orchestrator: windows, tray, poller, commands
│  ├─ media.rs           # SMTC read + transport control
│  ├─ audio.rs           # per-app volume (Audio Session API)
│  ├─ browser.rs         # active-tab URL via UI Automation (YouTube art)
│  ├─ ai.rs              # thin client to the AI relay
│  ├─ autostart.rs       # Startup-folder shortcut
│  └─ config.rs          # persisted settings (C:/AsBar/config.json)
└─ AI бэкенд/            # the relay (see below)
```

## Getting started

### Prerequisites

- Windows 10 / 11
- [Rust](https://rustup.rs/) (stable) with the MSVC build tools
- [Node.js](https://nodejs.org/) 18+

### Clone

```bash
git clone https://github.com/sosulacka/AsBar.git
cd AsBar
```

> The `AI бэкенд/` relay is **not** part of the public repository — it is
> server‑side only and holds the encrypted provider keys.

### Run in development

```bash
npm install
npm run tauri dev
```

### Build a release

```bash
npm run tauri build
```

The installer and standalone binary are emitted under
`src-tauri/target/release/`.

## AI backend

The desktop app never holds a provider key. It talks only to a relay, which
selects a key from an **encrypted, in‑memory pool** and calls the upstream
provider. Keys live as encrypted blobs inside the server source — never on disk.

| Model | Provider | Endpoint |
|-------|----------|----------|
| `qwen/qwen3-32b` | Groq | OpenAI‑compatible |
| `gemini-2.5-flash` / `…-lite` | Puter | OpenAI‑compatible |

The relay implements an **agentic tool loop**: when a model requests a tool, the
relay executes it and feeds the result back until a final answer is produced.

- `web_search` — keyless web search
- `fetch_url` — open a page and read its HTML / CSS / text
- `github_read` — read this project's source from GitHub

It is **pure Python standard library** — no `pip install` required.

```bash
# 1. Encrypt your provider keys locally (plaintext never touches disk)
python "AI бэкенд/keygen.py" gsk_yourGroqKey...

# 2. Paste the printed blob into AI бэкенд/server.py (GROQ_KEYS_BLOB / PUTER_KEYS_BLOB)

# 3. Run the relay
python "AI бэкенд/server.py"        # listens on 0.0.0.0:25573
```

Health check: `GET /health` → `{"status":"ok","keys":{"groq":N,"puter":M}}`.

## Configuration

Settings persist to `C:/AsBar/config.json` and apply live. Album art is cached
under `C:/AsBar/Assets/Icons/`.

| Setting | Description |
|---------|-------------|
| Position · offset · margin | Where the island docks |
| Width · height · radius | Pill geometry |
| Opacity | Background translucency |
| Colors | Background, text, accent |
| Color from artwork | Tint the accent from album art |
| Windows accent | Follow the system accent color |
| Always on top | Keep panels above other windows |
| Launch on startup | Managed via a Startup‑folder shortcut |
| Language | Russian / English |

## Continuous integration

Every push and pull request triggers a Windows build via GitHub Actions
([`.github/workflows/build.yml`](.github/workflows/build.yml)): it installs the
toolchain, runs `npm run tauri build`, and uploads the bundled installer as an
artifact. Trigger it manually from the **Actions** tab (`workflow_dispatch`).

## License

MIT — see [`LICENSE`](LICENSE).

---

<div align="center">

# AsBar — Русская версия

**Динамический остров для Windows.**

Текущий трек, управление воспроизведением и встроенный AI‑ассистент —
закреплены сверху по центру экрана и не мешают, пока не понадобятся.

[English](#overview) · **Русский**

</div>

## Обзор

AsBar рисует компактную «стеклянную» пилюлю сверху по центру рабочего стола. Он
читает то, что играет через системную шину Windows (SMTC) — Spotify, YouTube в
любом браузере, Yandex Music, нативные плееры — и показывает трек, обложку,
живой эквалайзер, полосу перемотки и кнопки управления. Двойной клик открывает
настройки; правый клик (или `Ctrl + Space`) — AI‑ассистента.

Приложение живёт в системном трее и тихо работает в фоне.

## Возможности

- **Любой источник** — автоопределение всего, что отдаётся в SMTC, с фирменными
  цветами и иконками сервисов.
- **Обложки высокого качества** — подтягивает чёткий арт (включая настоящие
  превью YouTube, когда открыта вкладка с видео), с кэшем на диске.
- **Живой эквалайзер** — реальный спектр через WASAPI loopback + FFT, а не
  зацикленная анимация.
- **Динамическая тема** — остров красится в доминирующий цвет обложки; свечение
  расходится на окна настроек и AI.
- **AI‑ассистент** — чат с несколькими моделями и **интернет‑инструментами**:
  веб‑поиск, чтение страниц и чтение исходников проекта с GitHub.
- **Локализация** — полный интерфейс на русском и английском с переключателем по
  флагам; без захардкоженных строк.
- **Гибкая настройка** — размер, положение, скругление, прозрачность, цвета,
  «поверх всех окон», автозапуск.

## Архитектура

| Слой | Стек | Назначение |
|------|------|-----------|
| **Оболочка** | Tauri 2, Rust | Окна, трей, геометрия, SMTC, звук, автозапуск |
| **UI** | Vanilla TypeScript, Vite | Окна острова, настроек и ассистента |
| **AI‑релей** | Python (только stdlib) | Мультипровайдерный чат + цикл инструментов |

## Установка

### Требования

- Windows 10 / 11
- [Rust](https://rustup.rs/) (stable) и сборочные инструменты MSVC
- [Node.js](https://nodejs.org/) 18+

### Клонирование

```bash
git clone https://github.com/sosulacka/AsBar.git
cd AsBar
```

> Релей `AI бэкенд/` **не входит** в публичный репозиторий — он только на сервере
> и хранит зашифрованные ключи провайдеров.

### Запуск

```bash
npm install
npm run tauri dev      # разработка
npm run tauri build    # релизная сборка (.msi / .exe)
```

Готовый бинарь и установщик лежат в `src-tauri/target/release/`.

## AI‑бэкенд

Приложение никогда не держит ключи провайдеров. Оно общается только с релеем,
который выбирает ключ из **зашифрованного пула в памяти** и вызывает провайдера.
Ключи лежат зашифрованными блобами прямо в коде сервера — никогда на диске.

| Модель | Провайдер |
|--------|-----------|
| `qwen/qwen3-32b` | Groq |
| `gemini-2.5-flash` / `…-lite` | Puter |

Релей реализует **агентный цикл инструментов**: модель запрашивает инструмент →
релей выполняет его и возвращает результат, пока не будет готов финальный ответ
(`web_search` — веб‑поиск, `fetch_url` — чтение страницы, `github_read` — чтение
исходников проекта). Всё на **чистой стандартной библиотеке Python** — без
`pip install`.

```bash
python "AI бэкенд/keygen.py" gsk_yourGroqKey...   # зашифровать ключи локально
# вставить блоб в server.py (GROQ_KEYS_BLOB / PUTER_KEYS_BLOB)
python "AI бэкенд/server.py"                        # слушает 0.0.0.0:25573
```

## Лицензия

MIT — см. [`LICENSE`](LICENSE).

<div align="center">
<sub>AsBar — сделано на Tauri, Rust и TypeScript.</sub>
</div>
