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
- **AI assistant** — a chat panel with multiple models, built right into the
  island.
- **Localization** — full Russian / English UI with a flag‑based switcher; no
  hardcoded strings.
- **Tasteful customization** — size, position, corner radius, opacity, colors,
  always‑on‑top, and launch‑on‑startup.

## Architecture

| Layer | Stack | Responsibility |
|------|-------|----------------|
| **Shell** | Tauri 2, Rust | Windows, tray, geometry, SMTC, audio capture, autostart |
| **UI** | Vanilla TypeScript, Vite | Island, settings, and assistant webviews |

Three transparent, undecorated windows (`island`, `settings`, `assistant`) are
sized to their exact visible content so transparent pixels never block the
desktop. Windows‑specific integration uses the `windows` crate: SMTC for media,
WASAPI loopback for the equalizer spectrum, UI Automation to read the active
browser tab, and an `IShellLink` shortcut for startup.

### What's in this repository

These are the files that are actually committed and published to GitHub.

```
AsBar/
├─ index.html               # island window entry
├─ settings.html            # settings window entry
├─ assistant.html           # AI assistant window entry
├─ package.json             # npm scripts + deps
├─ vite.config.ts           # multi-entry Vite config
├─ tsconfig.json
│
├─ src/                     # webview front-ends (TypeScript)
│  ├─ island.ts             # the pill: media, equalizer, seek, theming
│  ├─ island.css
│  ├─ settings.ts           # preferences + language picker
│  ├─ settings.css
│  ├─ assistant.ts          # AI chat panel
│  ├─ assistant.css
│  ├─ i18n.ts               # RU/EN dictionary + apply engine
│  ├─ fonts.ts              # bundles the Involve typeface into the document
│  ├─ vite-env.d.ts
│  └─ assets/               # vite.svg, tauri.svg, typescript.svg
│
├─ src-tauri/               # Rust shell (Tauri 2)
│  ├─ src/
│  │  ├─ main.rs            # binary entry point
│  │  ├─ lib.rs             # orchestrator: windows, tray, poller, commands
│  │  ├─ media.rs           # SMTC read + transport control
│  │  ├─ visualizer.rs      # WASAPI loopback FFT → equalizer levels
│  │  ├─ browser.rs         # active-tab URL via UI Automation (YouTube art)
│  │  ├─ ai.rs              # thin client to the AI relay
│  │  ├─ autostart.rs       # Startup-folder shortcut (.lnk)
│  │  └─ config.rs          # persisted settings (C:/AsBar/config.json)
│  ├─ capabilities/default.json
│  ├─ icons/                # app icons (.ico / .png / .icns)
│  ├─ build.rs
│  ├─ Cargo.toml · Cargo.lock
│  └─ tauri.conf.json
│
├─ Assets/                  # UI icons (.png) + FONTS/ (Involve typeface)
├─ .github/workflows/build.yml   # CI: Windows build via GitHub Actions
├─ LICENSE
└─ README.md
```

> **Not published:** the server‑side relay, `prim/`, `node_modules/`, `dist/`, and
> `src-tauri/target/` are all excluded by [`.gitignore`](.gitignore).

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

## AI assistant

The AI assistant is powered by a relay that simply forwards your requests to the
AI models and returns their answers. That's all it does.

- **No data is collected.** Your messages are not stored or logged — they are only
  passed through to generate a response.
- The relay runs server‑side and is not part of this repository.

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
- **AI‑ассистент** — чат с несколькими моделями, встроенный прямо в остров.
- **Локализация** — полный интерфейс на русском и английском с переключателем по
  флагам; без захардкоженных строк.
- **Гибкая настройка** — размер, положение, скругление, прозрачность, цвета,
  «поверх всех окон», автозапуск.

## Архитектура

| Слой | Стек | Назначение |
|------|------|-----------|
| **Оболочка** | Tauri 2, Rust | Окна, трей, геометрия, SMTC, захват звука, автозапуск |
| **UI** | Vanilla TypeScript, Vite | Окна острова, настроек и ассистента |

Три прозрачных окна без рамок (`island`, `settings`, `assistant`) подгоняются под
точный размер видимого контента, чтобы прозрачные пиксели не перехватывали клики
по рабочему столу. Интеграция с Windows идёт через крейт `windows`: SMTC для
медиа, WASAPI loopback для спектра эквалайзера, UI Automation для чтения активной
вкладки браузера и ярлык `IShellLink` для автозапуска.

### Что лежит в этом репозитории

Это файлы, которые реально закоммичены и публикуются на GitHub.

```
AsBar/
├─ index.html               # окно острова
├─ settings.html            # окно настроек
├─ assistant.html           # окно AI‑ассистента
├─ package.json             # npm‑скрипты и зависимости
├─ vite.config.ts           # multi-entry конфиг Vite
├─ tsconfig.json
│
├─ src/                     # фронтенд вебвью (TypeScript)
│  ├─ island.ts             # пилюля: медиа, эквалайзер, перемотка, тема
│  ├─ island.css
│  ├─ settings.ts           # настройки + переключатель языка
│  ├─ settings.css
│  ├─ assistant.ts          # панель AI‑чата
│  ├─ assistant.css
│  ├─ i18n.ts               # словарь RU/EN + движок применения
│  ├─ fonts.ts              # подключает шрифт Involve в документ
│  ├─ vite-env.d.ts
│  └─ assets/               # vite.svg, tauri.svg, typescript.svg
│
├─ src-tauri/               # Rust‑оболочка (Tauri 2)
│  ├─ src/
│  │  ├─ main.rs            # точка входа бинаря
│  │  ├─ lib.rs             # оркестратор: окна, трей, поллер, команды
│  │  ├─ media.rs           # чтение SMTC + управление воспроизведением
│  │  ├─ visualizer.rs      # WASAPI loopback FFT → уровни эквалайзера
│  │  ├─ browser.rs         # URL активной вкладки через UI Automation (арт YouTube)
│  │  ├─ ai.rs              # тонкий клиент к AI‑релею
│  │  ├─ autostart.rs       # ярлык в папке «Автозагрузка» (.lnk)
│  │  └─ config.rs          # сохранённые настройки (C:/AsBar/config.json)
│  ├─ capabilities/default.json
│  ├─ icons/                # иконки приложения (.ico / .png / .icns)
│  ├─ build.rs
│  ├─ Cargo.toml · Cargo.lock
│  └─ tauri.conf.json
│
├─ Assets/                  # иконки интерфейса (.png) + FONTS/ (шрифт Involve)
├─ .github/workflows/build.yml   # CI: сборка под Windows через GitHub Actions
├─ LICENSE
└─ README.md
```

> **Не публикуется:** серверный релей, `prim/`, `node_modules/`, `dist/` и
> `src-tauri/target/` — всё исключено через [`.gitignore`](.gitignore).

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

### Запуск

```bash
npm install
npm run tauri dev      # разработка
npm run tauri build    # релизная сборка (.msi / .exe)
```

Готовый бинарь и установщик лежат в `src-tauri/target/release/`.

## AI‑ассистент

AI‑ассистент работает через релей, который просто передаёт ваши запросы к
нейросетям и возвращает их ответы. Этим всё и ограничивается.

- **Данные не собираются.** Ваши сообщения не хранятся и не логируются — они лишь
  передаются дальше, чтобы получить ответ.
- Релей работает на сервере и не входит в этот репозиторий.

## Конфигурация

Настройки сохраняются в `C:/AsBar/config.json` и применяются на лету. Обложки
кэшируются в `C:/AsBar/Assets/Icons/`.

| Настройка | Описание |
|-----------|----------|
| Позиция · смещение · отступ | Где закрепляется остров |
| Ширина · высота · скругление | Геометрия пилюли |
| Прозрачность | Полупрозрачность фона |
| Цвета | Фон, текст, акцент |
| Цвет из обложки | Брать акцент из обложки трека |
| Акцент Windows | Следовать системному акцентному цвету |
| Поверх всех окон | Держать панели над другими окнами |
| Автозапуск | Через ярлык в папке «Автозагрузка» |
| Язык | Русский / английский |

## Непрерывная интеграция

Каждый push и pull request запускает сборку под Windows через GitHub Actions
([`.github/workflows/build.yml`](.github/workflows/build.yml)): ставится
тулчейн, выполняется `npm run tauri build`, и готовый установщик выкладывается
как артефакт. Запуск вручную — со вкладки **Actions** (`workflow_dispatch`).

## Лицензия

MIT — см. [`LICENSE`](LICENSE).

<div align="center">
<sub>AsBar — сделано на Tauri, Rust и TypeScript.</sub>
</div>
