# File Forge

Утилита для автоматической конвертации и переименования файлов в реальном времени.

Следит за выбранными папками и мгновенно обрабатывает новые файлы по заданным правилам.

## Пример использования

> Скачиваешь `.ogg` в папку `Downloads` - File Forge автоматически конвертирует в `.mp3`
> и кладёт в `C:\Music\Converted`. Готово, например для CapCut.

## Стек

- **Backend:** Rust
- **GUI:** Tauri v2 (Rust + HTML/CSS/JS фронтенд)
- **File Watcher:** `notify` crate
- **Конвертация:** FFmpeg (subprocess)
- **Конфигурация:** JSON (serde)

## Зависимости

- [Rust](https://rustup.rs/) (rustup)
- [Node.js](https://nodejs.org/) (для Tauri фронтенда)
- [FFmpeg](https://ffmpeg.org/download.html) (для конвертации форматов)

## Структура проекта

```
file-forge/
├── src-tauri/           # Rust backend (Tauri)
│   ├── src/
│   │   ├── main.rs      # Точка входа
│   │   ├── watcher.rs   # File watcher (notify)
│   │   ├── rules.rs     # Движок правил
│   │   ├── converter.rs # Обёртка над FFmpeg
│   │   ├── renamer.rs   # Логика переименования
│   │   └── config.rs    # Загрузка/сохранение конфигурации
│   └── Cargo.toml
├── src/                 # Frontend (HTML/CSS/JS)
│   ├── index.html
│   ├── styles/
│   └── scripts/
├── docs/
│   └── roadmap.md
└── README.md
```
