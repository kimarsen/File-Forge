# File Forge

Утилита следит за папками и сразу обрабатывает новые файлы: конвертирует через FFmpeg, переименовывает или перемещает.

Скачиваете `.ogg` в папку загрузок — File Forge кладёт рядом `.mp3`. Так можно готовить звук, например, для CapCut.

## Что входит

- Наблюдение за несколькими папками и ожидание, пока размер файла перестанет меняться.
- Правила: расширение, часть имени, шаблон `*` и `?`, своя папка и папка результата.
- Действия: конвертация, переименование (`{name}`, `{date}`, `{time}`, `{counter}`, `{ext}`, префикс, суффикс, замена) и перемещение.
- Пресеты: ogg/wav/flac → mp3, mkv/webm → mp4, webp/png → jpg.
- Разбор файлов, которые уже лежат в папке.
- Журнал операций в `operations.jsonl`.
- Окно на Tauri: правила, журнал, настройки качества, тёмная и светлая тема, трей, уведомления и автозапуск.

## Требования

- [Rust](https://rustup.rs/)
- [FFmpeg](https://ffmpeg.org/download.html) в `PATH`
- Для окна: [Node.js](https://nodejs.org/) и системные библиотеки Tauri. На Windows инсталлятор NSIS собирается командой `cargo tauri build`.

## Командная строка

Без подкоманды File Forge запускает наблюдение. Конфиг берётся из `--config`, затем из `FILE_FORGE_CONFIG`, затем из `./config.json`, иначе из `%APPDATA%/file-forge/config.json` (на Linux и macOS — `~/.config/file-forge/config.json`).

```text
file-forge watch --since day
file-forge convert song.wav --format mp3 --keep
file-forge batch --since all
file-forge log --limit 20
file-forge rules list
file-forge rules add convert --ext ogg --format mp3 --output D:\Music
file-forge rules add rename --ext txt --prefix DONE_ --name report
file-forge rules add rename --pattern "{date}_{counter}_{name}"
file-forge rules add move --ext pdf D:\Docs
file-forge rules preset audio --watch D:\Downloads --output D:\Music
file-forge rules remove 1
```

Пока наблюдение запущено, новые файлы обрабатываются по правилам сверху вниз. Имя вроде `song (1).ogg` при конвертации становится `song.mp3`. Удаление оригинала включается в настройках и может быть переопределено у конкретного правила.

## Окно

```text
npm install
npm run tauri dev
```

Сборка приложения:

```text
npm run tauri build
```

На Windows эта команда собирает инсталлятор NSIS. Окно можно свернуть в трей: закрытие прячет его, выход — в меню трея или в настройках.

Файлы можно перетащить в окно: они пройдут по тем же правилам.

## Конфиг

Старый `config.json` по-прежнему читается. Новые поля необязательны.

```json
{
  "watch_folder": "C:\\Users\\me\\Downloads",
  "watch_folders": [],
  "rules": [
    {
      "extension": "ogg",
      "name_contains": null,
      "action": { "Convert": { "target_format": "mp3" } }
    }
  ],
  "settings": {
    "delete_original": true,
    "scan_on_start": "last_day",
    "theme": "dark",
    "autostart": false,
    "watch_on_start": true,
    "counter": 1,
    "quality": {
      "mp3_qscale": 2,
      "audio_bitrate": "192k",
      "audio_codec": "",
      "video_codec": "",
      "video_crf": 23,
      "image_quality": 2
    }
  }
}
```

`mp3_qscale` 2 — это `-codec:a libmp3lame -qscale:a 2`. Пустые кодеки означают кодек формата по умолчанию.
