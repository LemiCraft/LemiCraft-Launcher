<p align="center">
  <img src="src-tauri/icons/128x128.png" alt="LemiCraft" width="96"/>
</p>

<h3 align="center">Официальный лаунчер для Minecraft проекта LemiCraft</h3>

<p align="center">
  <img src="https://img.shields.io/github/v/release/LemiCraft/LemiCraft-Launcher" alt="version"/>
  <a href="https://github.com/LemiCraft/LemiCraft-Launcher/actions/workflows/test.yml"><img src="https://github.com/LemiCraft/LemiCraft-Launcher/actions/workflows/test.yml/badge.svg" alt="Tests"/></a>
  <img src="https://img.shields.io/github/license/LemiCraft/LemiCraft-Launcher" alt="license"/>
  <img src="https://img.shields.io/badge/Tauri-2-24C8DB" alt="Tauri 2"/>
  <img src="https://img.shields.io/badge/Vue-3-42b883" alt="Vue 3"/>
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey" alt="Windows | Linux | macOS"/>
</p>

---

## Возможности

- 🎮 **Автоматическая установка** Minecraft (Fabric) и модов, с прогресс-баром и докачкой при сбоях
- 🔐 **Авторизация** через Microsoft и Ely.by с поддержкой authlib-injector
- 🧩 **Менеджер модов** - каталог, официальная сборка LemiSborka, импорт по коду и наборы Modrinth (`.mrpack`)
- 📰 **Новости** прямо в лаунчере, с уведомлением о новых постах
- ⚙️ **Гибкие настройки** Java, RAM, пути установки
- 📊 **Мониторинг** статуса сервера в реальном времени: пинг, версия, кто онлайн
- 🧭 **Подбор подключения** - проверка путей до сервера и выбор лучшего
- 👥 **Профиль** игрока: свой и других игроков, со статистикой
- 🎨 **Современный UI** с тёмной темой и плавными анимациями, по желанию с полупрозрачным окном (Mica/Acrylic на Windows 11, vibrancy на macOS)
- 👤 **Скины** - 3D-превью, загрузка, применение и удаление
- 🛡️ **Анализатор крашей** - при аварийном закрытии Minecraft предлагает открыть лог
- 💬 **Discord Rich Presence** - в профиле видно, чем вы заняты в лаунчере и в игре, отключается в настройках

## Платформы

| Платформа | Что скачать | Обновление |
|---|---|---|
| Windows 10/11 | установщик или портативный `.exe` | автоматическое |
| Linux (x86_64) | `.AppImage`, `.deb`, `.rpm` или `.flatpak` | `.AppImage` обновляется сам, для остальных лаунчер сообщает о новой версии и ведёт на страницу загрузки |
| macOS 11+ (Intel и Apple Silicon) | универсальный `.dmg` из опубликованного релиза | автоматическое обновление |

> [!NOTE]
> macOS-сборка подписана ad-hoc и не нотарифицирована Apple. При первом запуске Gatekeeper может её заблокировать. Если вы скачали её из официального релиза и доверяете источнику, попробуйте открыть приложение, затем разрешите запуск через **Системные настройки → Конфиденциальность и безопасность → Всё равно открыть**. Подробнее — в инструкции Apple: [Safely open apps on your Mac](https://support.apple.com/en-us/102445).

Полупрозрачное окно на Windows работает только на Windows 11

## Установка

**Windows**

Скачайте со страницы [Releases](../../releases/latest) один из вариантов:

- установщик `LemiCraft-Launcher-*-installer.exe` (рекомендуется) - запустите и следуйте инструкциям
- портативную версию `LemiCraft-Launcher-*-portable.exe` - запускается без установки, можно использовать с флешки

> [!TIP]
> Если Windows или антивирус при первом запуске предупреждает о неизвестном издателе, это ожидаемо: сборки пока не подписаны сертификатом. Скачивайте файлы только со страницы [Releases](../../releases/latest). В SmartScreen нажмите «Подробнее» - «Выполнить в любом случае»

**Linux**

Со страницы [Releases](../../releases/latest) подойдёт любой из форматов: `.AppImage` (портативный, один файл без установки), `.deb`, `.rpm` или `.flatpak`

```
chmod +x LemiCraft-Launcher-*.AppImage && ./LemiCraft-Launcher-*.AppImage
flatpak install --user LemiCraft-Launcher-*.flatpak
```

**macOS**

Скачайте универсальный файл `LemiCraft-Launcher-*-macos-universal.dmg` со страницы [Releases](../../releases/latest). Откройте образ и перетащите `LemiCraft Launcher.app` в `Applications`. Одна сборка подходит для Mac с Intel и Apple Silicon. Если опубликованного релиза пока нет, соберите приложение из исходников по инструкции ниже.

## Сборка из исходников

Требования: [Rust](https://rust-lang.org) (stable), [Node.js](https://nodejs.org) 20.19+ (или 22+), npm

Системные зависимости:

- **Windows** - [Build Tools для Visual Studio](https://visualstudio.microsoft.com/visual-cpp-build-tools/) с компонентом «Desktop development with C++» и WebView2, он уже есть в Windows 10 (версия 1803 и новее) и Windows 11
- **Linux** - для Debian/Ubuntu:
  ```
  sudo apt install build-essential curl wget file libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```
  <details>
  <summary>Fedora, Arch, openSUSE</summary>

  Fedora:
  ```
  sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel libxdo-devel
  sudo dnf group install "c-development"
  ```
  Arch:
  ```
  sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libappindicator-gtk3 librsvg xdotool
  ```
  openSUSE:
  ```
  sudo zypper in webkit2gtk3-devel libopenssl-devel curl wget file libappindicator3-1 librsvg-devel
  sudo zypper in -t pattern devel_basis
  ```
  </details>

  Для остальных дистрибутивов смотрите [требования Tauri](https://v2.tauri.app/start/prerequisites/)
- **macOS** - Xcode Command Line Tools: `xcode-select --install`

```
git clone https://github.com/LemiCraft/LemiCraft-Launcher
cd LemiCraft-Launcher
npm install
```

**Запуск в режиме отладки**

```
npm run tauri dev
```

Отладочная сборка хранит данные в отдельной папке `LemiCraftLauncherDev`, поэтому не трогает установленный лаунчер

**Сборка релиза**

```
npm run tauri build
```

Готовые файлы появятся в `src-tauri/target/release/bundle/`: установщик и `.exe` на Windows, `.deb`/`.rpm`/`.AppImage` на Linux

**macOS**

```
npm run build:mac:arm64      # Apple Silicon
npm run build:mac:x86        # Intel
npm run build:mac:universal  # оба процессора в одной сборке
# npm run build:mac:all       # обе отдельные сборки и universal
```

Перед кросс-сборкой установите нужные Rust-цели (для universal нужны обе):

```sh
rustup target add aarch64-apple-darwin   # Apple Silicon
rustup target add x86_64-apple-darwin    # Intel
```

Команды запускаются из корня репозитория. Приложения `.app` появятся в `dist-macos/arm64/`, `dist-macos/x86_64/` и `dist-macos/universal/`, а ZIP-архивы — в корне `dist-macos/`. Локальный скрипт не создаёт `.dmg`. Для локальных сборок используется ad-hoc подпись, не Developer ID.

**Flatpak**

Пакет собирается из готового `.deb`, нужен `flatpak-builder`:

```
bash packaging/flatpak/build.sh путь/к/LemiCraft-Launcher.deb
```

## Устройство проекта

```
src/                   интерфейс (Vue 3)
  components/          экраны и элементы интерфейса
  store/               состояние: аккаунт, моды, новости, профиль, обновления
  composables/         общая логика (вход в аккаунт)
src-tauri/src/         бэкенд (Rust)
  auth.rs, auth_ely.rs         вход через Microsoft и Ely.by
  game.rs                      установка и запуск Minecraft
  mods.rs, mrpack.rs           каталог модов, сборка LemiSborka, импорт по коду и наборы Modrinth
  skin.rs, skin_elyby.rs       скины
  profile.rs                   профиль игрока (свой и чужие)
  entry_points.rs              подбор подключения к серверу
  server_status.rs             статус и пинг сервера
  notify.rs                    уведомления и автозапуск
  http.rs                      один общий HTTP-клиент
  update.rs                    самообновление
  window_effect.rs             полупрозрачное окно
  config.rs                    адреса сервера и API, версия Minecraft
packaging/flatpak/     пакет Flatpak
packaging/macos/       фон и иконки образа macOS, сборка иконки
scripts/               сборка под macOS
```

Новости, каталог модов, правила и сведения об обновлениях лаунчер получает из API lemicraft.ru, адрес задан в `src-tauri/src/config.rs`

## Стек

- **Оболочка** - Tauri 2 (Rust)
- **Интерфейс** - Vue 3, Vite
- **Minecraft** - [`portablemc`](https://crates.io/crates/portablemc)
- **Авторизация** - Microsoft: ручной OAuth в окне входа; Ely.by: вход через системный браузер и API lemicraft.ru; authlib-injector
- **Хранение токенов** - DPAPI на Windows, зашифрованный файл на Linux и macOS
- **3D-скин** - [`skinview3d`](https://github.com/bs-community/skinview3d)

## Участие

Баги и идеи - в [Issues](../../issues). Правки кода присылайте через pull request: форк, отдельная ветка, PR в `main`. Перед отправкой убедитесь, что собираются фронтенд (`npm run build`) и бэкенд (`cargo check` в папке `src-tauri`)

Как выпускаются версии - в [RELEASING.md](RELEASING.md)

Проект распространяется под лицензией [MIT](LICENSE.txt)

## Ссылки

[Сайт](https://lemicraft.ru) · [Discord](https://discord.gg/ybC6QM8WTM) · [Wiki](https://wiki.lemicraft.ru) · [Правила](https://lemicraft.ru/rules)
