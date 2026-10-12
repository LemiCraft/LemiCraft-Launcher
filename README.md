<p align="center">
  <img src="src-tauri/icons/128x128.png" alt="LemiCraft" width="96"/>
</p>

<h3 align="center">Официальный лаунчер для Minecraft проекта LemiCraft</h3>

<p align="center">
  <img src="https://img.shields.io/github/v/release/LemiCraft/LemiCraft-Launcher" alt="version"/>
  <img src="https://img.shields.io/github/license/LemiCraft/LemiCraft-Launcher" alt="license"/>
  <img src="https://img.shields.io/badge/Tauri-2-24C8DB" alt="Tauri 2"/>
  <img src="https://img.shields.io/badge/Vue-3-42b883" alt="Vue 3"/>
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey" alt="Windows | Linux | macOS"/>
</p>

---

## Возможности

- 🎮 **Автоматическая установка** Minecraft (Fabric) и модов, с прогресс-баром и докачкой при сбоях
- 🔐 **Авторизация** через Microsoft и Ely.by с поддержкой authlib-injector
- 🧩 **Менеджер модов** - каталог с индивидуальными переключателями, официальная сборка LemiSborka, импорт по коду или через ссылку `lemicraft://`
- 📰 **Новости** и обновления прямо в лаунчере
- ⚙️ **Гибкие настройки** Java, RAM, пути установки
- 📊 **Мониторинг** статуса сервера в реальном времени
- 🎨 **Современный UI** с тёмной темой и плавными анимациями, по желанию с полупрозрачным окном (Mica/Acrylic на Windows 11, vibrancy на macOS)
- 👤 **Скины** - 3D-превью, загрузка, применение и удаление
- 🛡️ **Анализатор крашей** - при аварийном закрытии Minecraft предлагает открыть лог
- 💬 **Discord Rich Presence** - в профиле видно, чем вы заняты в лаунчере и в игре, отключается в настройках

## Платформы

| Платформа | Что скачать | Обновление |
|---|---|---|
| Windows 10/11 | установщик или портативный `.exe` | автоматическое |
| Linux (x86_64) | `.AppImage`, `.deb`, `.rpm` или `.flatpak` | `.AppImage` обновляется сам, для остальных лаунчер сообщает о новой версии и ведёт на страницу загрузки |
| macOS | готовых сборок пока нет, только из исходников | - |

> [!NOTE]
> Поддержка macOS добавлена недавно и пока проверена мало. Сборки без подписи, поэтому при первом запуске откройте приложение через правый клик - «Открыть»

Полупрозрачное окно на Windows работает только на Windows 11

## Установка

**Windows**

Скачайте со страницы [Releases](../../releases/latest) один из вариантов:

- установщик `LemiCraft-Launcher-*-installer.exe` (рекомендуется) - запустите и следуйте инструкциям
- портативную версию `LemiCraft-Launcher-*-portable.exe` - запускается без установки, можно использовать с флешки

**Linux**

Со страницы [Releases](../../releases/latest) подойдёт любой из форматов: `.AppImage` (портативный, один файл без установки), `.deb`, `.rpm` или `.flatpak`

```
chmod +x LemiCraft-Launcher-*.AppImage && ./LemiCraft-Launcher-*.AppImage
flatpak install --user LemiCraft-Launcher-*.flatpak
```

**macOS**

Готовых сборок пока нет, но лаунчер собирается из исходников, см. [Сборка из исходников](#сборка-из-исходников). Результат - приложение `.app` (и `.zip` с ним), установка не нужна: положите его в любую папку и запускайте

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
```

Результат и `.zip` с `.app` ложатся в `dist-macos/`

**Flatpak**

Пакет собирается из готового `.deb`, нужен `flatpak-builder`:

```
bash packaging/flatpak/build.sh путь/к/LemiCraft-Launcher.deb
```

## Устройство проекта

```
src/                   интерфейс (Vue 3)
  components/          экраны и элементы интерфейса
  store/               состояние: аккаунт, моды, новости, обновления
  composables/         общая логика (вход в аккаунт)
src-tauri/src/         бэкенд (Rust)
  auth.rs, auth_ely.rs         вход через Microsoft и Ely.by
  game.rs                      установка и запуск Minecraft
  mods.rs                      каталог модов, сборка LemiSborka, импорт по коду
  skin.rs, skin_elyby.rs       скины
  update.rs                    самообновление
  window_effect.rs             полупрозрачное окно
  config.rs                    адреса сервера и API, версия Minecraft
packaging/flatpak/     пакет Flatpak
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
