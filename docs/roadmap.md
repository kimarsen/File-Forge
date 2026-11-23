# Роадмап разработки File Forge (Rust)

## Концепция
Hot-folder утилита: фоновый процесс следит за папками в реальном времени
и автоматически конвертирует / переименовывает файлы по заданным правилам.

## Стек
- **Rust** — бэкенд
- **Tauri v2** — GUI (Rust + веб-фронтенд)
- **notify** — file system watcher
- **serde + serde_json** — сериализация правил
- **std::process::Command** — вызов FFmpeg
- **tracing** — логирование

---

## Этап 1: Ядро (чистый Rust, без GUI)

### 1.1 File Watcher (`watcher.rs`)
- [x] Добавить `notify` crate в Cargo.toml
- [x] Создать watcher, подписанный на события Create в указанной папке
- [x] Фильтровать события: реагировать только на новые файлы (не папки)
- [x] Обработка race condition: файл ещё скачивается → ждать стабилизации размера
- [x] Поддержка нескольких наблюдаемых папок

### 1.2 Rules Engine (`rules.rs`)
- [x] Структура `Rule`: watch_dir, filter (extension, name_pattern), action, output_dir
- [x] Enum `Action`: Convert { target_format }, Rename { pattern }, Move { dest }
- [x] Функция `matches(rule, file_path) -> bool`
- [x] Функция `find_matching_rules(rules, file_path) -> Vec<&Rule>`
- [x] Пресеты: audio (ogg/wav/flac→mp3), video (mkv/webm→mp4), images (webp/png→jpg)

### 1.3 Converter (`converter.rs`)
- [x] Функция `convert(input: &Path, output: &Path, format: &str) -> Result<()>`
- [x] Внутри: сборка команды `ffmpeg -i input -flags output`
- [x] Маппинг форматов → FFmpeg-флаги (mp3: `-codec:a libmp3lame -qscale:a 2`)
- [x] Обработка ошибок FFmpeg (exit code, stderr)
- [x] Опция: удалить оригинал после успешной конвертации

### 1.4 Renamer (`renamer.rs`)
- [x] Замена подстроки в имени файла
- [x] Добавление префикса / суффикса
- [x] Шаблон с переменными: `{name}`, `{date}`, `{counter}`

### 1.5 Config (`config.rs`)
- [x] Структура `Config`: Vec<Rule>, settings
- [x] Загрузка из JSON файла (serde_json)
- [x] Сохранение в JSON файл
- [x] Путь по умолчанию: `%APPDATA%/file-forge/config.json`

### 1.6 Операционный лог
- [x] Структура `OperationLog`: timestamp, source, destination, action, success
- [x] Запись в файл (для undo / истории)

---

## Этап 2: CLI-обёртка (для отладки)
- [x] `main.rs`: парсинг CLI аргументов (clap)
- [x] Команда `watch` — запуск watcher с правилами из конфига
- [x] Команда `convert <input> <output>` — разовая конвертация
- [x] Команда `rules list / add / remove` — управление правилами
- [x] Вывод лога в stdout в реальном времени

---

## Этап 3: GUI (Tauri v2)
- [x] Инициализация Tauri v2 проекта (`cargo tauri init`)
- [x] Tauri commands: обёртки над Rust-функциями для вызова из JS
- [x] Экран «Правила» — CRUD для правил
- [x] Создание правила через форму: папка, фильтры, действие, выход
- [x] Пресеты — кнопки быстрого создания (Audio→MP3, Video→MP4)
- [x] Лог операций — live-список конвертаций (Tauri events → JS)
- [x] System Tray — сворачивание в трей, иконка, контекстное меню
- [x] Уведомления (toast) при конвертации
- [x] Автозапуск при старте Windows

---

## Этап 4: Полировка
- [x] Настройки качества (битрейт, кодек)
- [x] Batch-режим: обработать существующие файлы по правилам
- [x] Drag & Drop
- [x] Тёмная / светлая тема
- [x] Иконка приложения
- [x] Инсталлятор (NSIS через Tauri)

---

## Ключевые крейты (Cargo.toml)

```toml
[dependencies]
notify = "8"              # File system watcher
serde = { version = "1", features = ["derive"] }
serde_json = "1"          # JSON config
tracing = "0.1"           # Logging
tracing-subscriber = "0.3"
clap = { version = "4", features = ["derive"] }  # CLI args (этап 2)

[dependencies.tauri]      # Добавится на этапе 3
```

## Полезные ссылки
- [The Rust Book (рус)](https://doc.rust-lang.ru/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [notify crate docs](https://docs.rs/notify)
- [Tauri v2 Getting Started](https://v2.tauri.app/start/)
- [serde.rs](https://serde.rs/)
- [FFmpeg Encode/MP3](https://trac.ffmpeg.org/wiki/Encode/MP3)
