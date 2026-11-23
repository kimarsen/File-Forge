const STORAGE_KEY = "file-forge-demo";

const PRESETS = {
  audio: ["ogg", "wav", "flac"],
  video: ["mkv", "webm"],
  images: ["webp", "png"],
};

const TARGETS = {
  audio: "mp3",
  video: "mp4",
  images: "jpg",
};

export function hasTauri() {
  return Boolean(window.__TAURI__?.core?.invoke);
}

export async function loadConfig() {
  if (hasTauri()) {
    return invoke("load_config");
  }
  return clone(readMock());
}

export async function saveConfig(config) {
  if (hasTauri()) {
    return invoke("save_config", { config });
  }
  const next = clone(config);
  const seen = new Set();
  next.rules.forEach((rule) => {
    if (!rule.id || seen.has(rule.id)) {
      rule.id = `demo-${Date.now()}-${Math.random().toString(16).slice(2)}`;
    }
    seen.add(rule.id);
  });
  writeMock(next);
  return clone(next);
}

export async function watchStatus() {
  if (hasTauri()) {
    return invoke("watch_status");
  }
  return Boolean(readMock().watching);
}

export async function startWatch() {
  if (hasTauri()) {
    return invoke("start_watch");
  }
  const config = readMock();
  config.watching = true;
  writeMock(config);
  return [];
}

export async function stopWatch() {
  if (hasTauri()) {
    return invoke("stop_watch");
  }
  const config = readMock();
  config.watching = false;
  writeMock(config);
}

export async function processExisting(since) {
  if (hasTauri()) {
    return invoke("process_existing", { since });
  }
  throw new Error("Пакетная обработка запускает FFmpeg в приложении File Forge");
}

export async function processFiles(paths) {
  if (hasTauri()) {
    return invoke("process_files", { paths });
  }
  return paths.map((path) =>
    record("drop", path, null, true, "Демо: файл отмечен, обработка выполняется в приложении"),
  );
}

export async function recentLog(limit) {
  if (hasTauri()) {
    return invoke("recent_log", { limit });
  }
  return readMock().log.slice(-limit);
}

export async function pickFolder() {
  if (hasTauri()) {
    return invoke("pick_folder");
  }
  return "/tmp/file-forge";
}

export async function addPreset(kind, watch, output) {
  if (hasTauri()) {
    return invoke("add_preset", { kind, watch, output });
  }
  const config = readMock();
  const extensions = PRESETS[kind] || [];
  const target = TARGETS[kind] || "mp3";
  extensions.forEach((extension, index) => {
    config.rules.push({
      id: `demo-${extension}-${Date.now()}-${index}`,
      watch_dir: watch,
      extension,
      name_contains: null,
      name_pattern: null,
      action: { Convert: { target_format: target, delete_original: null } },
      output_dir: output,
      enabled: true,
    });
  });
  writeMock(config);
  return clone(config);
}

export async function configPath() {
  if (hasTauri()) {
    return invoke("config_path");
  }
  return "браузерное хранилище";
}

export async function quit() {
  if (hasTauri()) {
    return invoke("quit");
  }
}

export async function onOperation(callback) {
  if (!hasTauri()) {
    return;
  }
  await window.__TAURI__.event.listen("operation", (event) => callback(event.payload));
}

export async function onDragDrop(callback) {
  if (!hasTauri()) {
    return;
  }
  await window.__TAURI__.event.listen("tauri://drag-drop", (event) => {
    const paths = event.payload?.paths || [];
    if (paths.length) {
      callback(paths);
    }
  });
}

function invoke(command, args) {
  return window.__TAURI__.core.invoke(command, args);
}

function readMock() {
  const raw = localStorage.getItem(STORAGE_KEY);
  if (raw) {
    return JSON.parse(raw);
  }
  return {
    watch_folder: "/tmp/file-forge",
    watch_folders: [],
    rules: [
      {
        id: "demo-ogg",
        watch_dir: null,
        extension: "ogg",
        name_contains: null,
        name_pattern: null,
        action: { Convert: { target_format: "mp3", delete_original: null } },
        output_dir: null,
        enabled: true,
      },
      {
        id: "demo-report",
        watch_dir: null,
        extension: "txt",
        name_contains: "report",
        name_pattern: null,
        action: {
          Rename: {
            pattern: null,
            prefix: "DONE_",
            suffix: null,
            find: null,
            replace: null,
          },
        },
        output_dir: null,
        enabled: true,
      },
    ],
    settings: {
      delete_original: true,
      scan_on_start: "last_day",
      theme: "dark",
      autostart: false,
      watch_on_start: false,
      quality: {
        mp3_qscale: 2,
        audio_bitrate: "192k",
        audio_codec: "",
        video_codec: "",
        video_crf: 23,
        image_quality: 2,
      },
      counter: 1,
    },
    watching: false,
    log: [],
  };
}

function writeMock(config) {
  const current = readMock();
  config.watching = config.watching ?? current.watching;
  config.log = config.log ?? current.log;
  localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
}

function record(action, source, destination, success, message) {
  const entry = {
    timestamp: new Date().toISOString(),
    source,
    destination,
    action,
    success,
    message,
  };
  const config = readMock();
  config.log.push(entry);
  writeMock(config);
  return entry;
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}
