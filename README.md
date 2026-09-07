<p align="center">
  <img src="src-tauri/icons/128x128.png" alt="LemiCraft" width="96"/>
</p>

<h3 align="center">Официальный лаунчер для Minecraft проекта LemiCraft</h3>

<p align="center">
  <img src="https://img.shields.io/github/v/release/LemiCraft/LemiCraft-Launcher" alt="version"/>
  <img src="https://img.shields.io/badge/Tauri-2-24C8DB" alt="Tauri 2"/>
  <img src="https://img.shields.io/badge/Vue-3-42b883" alt="Vue 3"/>
  <img src="https://img.shields.io/badge/platform-Windows-lightgrey" alt="Windows"/>
</p>

---

## Возможности

- 🎮 **Автоматическая установка** Minecraft (Fabric) и модов, с прогресс-баром и докачкой при сбоях
- 🔐 **Авторизация** через Microsoft и Ely.by с поддержкой authlib-injector
- 🧩 **Менеджер модов** - каталог с индивидуальными переключателями, официальная сборка LemiSborka, импорт по коду или через ссылку `lemicraft://`
- 📰 **Новости** и обновления прямо в лаунчере
- ⚙️ **Гибкие настройки** Java, RAM, пути установки
- 📊 **Мониторинг** статуса сервера в реальном времени
- 🎨 **Современный UI** с тёмной темой и плавными анимациями
- 👤 **Скины** - 3D-превью, загрузка, применение и удаление
- 🛡️ **Анализатор крашей** - при аварийном закрытии Minecraft предлагает открыть лог

## Установка

**Через установщик (рекомендуется)**

Скачайте установщик со страницы [Releases](../../releases/latest), запустите и следуйте инструкциям

**Портативная версия**

Скачайте `LemiCraft-Launcher.exe` - запускается без установки, можно использовать с флешки

## Сборка из исходников

Требования: [Rust](https://rust-lang.org) (stable), [Node.js](https://nodejs.org) 20+, npm

```
git clone https://github.com/LemiCraft/LemiCraft-Launcher
cd LemiCraft-Launcher
npm install
```

**Запуск в режиме отладки**

```
npm run tauri dev
```

**Сборка релиза**

```
npm run tauri build
```

Готовые файлы (`.exe`/установщик) появятся в `src-tauri/target/release/bundle/`

## Стек

| | |
|---|---|
| Оболочка | Tauri 2 (Rust) |
| Интерфейс | Vue 3, Vite |
| Minecraft | [`portablemc`](https://crates.io/crates/portablemc) |
| Авторизация | ручной OAuth (Microsoft legacy-флоу, Ely.by через бэкенд lemicraft.ru), authlib-injector |
| 3D-скин | [`skinview3d`](https://github.com/bs-community/skinview3d) |

## Ссылки

[Сайт](https://lemicraft.ru) · [Discord](https://discord.gg/ybC6QM8WTM) · [Wiki](https://wiki.lemicraft.ru) · [Правила](https://lemicraft.ru/rules)
