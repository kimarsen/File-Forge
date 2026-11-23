import * as api from "./api.js";

const state = {
  config: null,
  log: [],
  watching: false,
  view: "rules",
  editor: null,
  path: "",
};

const $ = (id) => document.getElementById(id);

boot().catch((error) => toast(text(error), true));

async function boot() {
  if (!api.hasTauri()) {
    $("demo-banner").hidden = false;
  }
  await api.onOperation((record) => {
    state.log.push(record);
    toast(summary(record), !record.success);
    if (state.view === "log") {
      renderLog();
    }
  });
  await api.onDragDrop(async (paths) => {
    await guard(async () => {
      const records = await api.processFiles(paths);
      state.log.push(...records);
      toast(`Обработано файлов: ${records.length}`);
      if (state.view === "log") {
        renderLog();
      }
    });
  });
  state.config = await api.loadConfig();
  state.log = await api.recentLog(200);
  state.watching = await api.watchStatus();
  state.path = await api.configPath();
  applyTheme(state.config.settings.theme);
  bind();
  renderStatus();
  showView("rules");
  if (state.config.settings.watch_on_start && !state.watching && api.hasTauri()) {
    await toggleWatch(true);
  }
}

function bind() {
  document.querySelectorAll("[data-view]").forEach((button) => {
    button.addEventListener("click", () => showView(button.dataset.view));
  });
  $("watch-toggle").addEventListener("click", () => {
    guard(() => toggleWatch(!state.watching));
  });
  $("batch-button").addEventListener("click", () => guard(batch));
  $("new-rule").addEventListener("click", () => openEditor({ mode: "create" }));
  $("editor-form").addEventListener("submit", (event) => {
    event.preventDefault();
    guard(saveRule);
  });
  $("editor-cancel").addEventListener("click", closeEditor);
  $("action").addEventListener("change", syncActionFields);
  document.querySelectorAll("[data-preset]").forEach((button) => {
    button.addEventListener("click", () => guard(() => addPreset(button.dataset.preset)));
  });
  document.querySelectorAll("[data-browse]").forEach((button) => {
    button.addEventListener("click", () => guard(() => browse(button.dataset.browse)));
  });
  $("settings-form").addEventListener("submit", (event) => {
    event.preventDefault();
    guard(saveSettings);
  });
  $("theme").addEventListener("change", () => applyTheme($("theme").value));
  $("quit").addEventListener("click", () => {
    if (api.hasTauri()) {
      api.quit();
    } else {
      toast("В установленном приложении эта кнопка завершает File Forge");
    }
  });
  let dragDepth = 0;
  window.addEventListener("dragenter", (event) => {
    event.preventDefault();
    dragDepth += 1;
    $("drop-overlay").hidden = false;
  });
  window.addEventListener("dragover", (event) => {
    event.preventDefault();
  });
  window.addEventListener("dragleave", () => {
    dragDepth = Math.max(0, dragDepth - 1);
    if (dragDepth === 0) {
      hideDrop();
    }
  });
  window.addEventListener("drop", (event) => {
    event.preventDefault();
    dragDepth = 0;
    hideDrop();
    if (api.hasTauri()) {
      return;
    }
    const paths = [...event.dataTransfer.files].map((file) => file.name);
    if (!paths.length) {
      return;
    }
    guard(async () => {
      const records = await api.processFiles(paths);
      state.log.push(...records);
      toast(`Отмечено файлов: ${paths.length}`);
      if (state.view === "log") {
        renderLog();
      }
    });
  });
}

function showView(name) {
  if (state.view === "settings" && name !== "settings") {
    applyTheme(state.config.settings.theme);
  }
  state.view = name;
  for (const view of ["rules", "log", "settings"]) {
    $(`view-${view}`).hidden = view !== name;
  }
  document.querySelectorAll("[data-view]").forEach((button) => {
    button.classList.toggle("active", button.dataset.view === name);
  });
  if (name === "rules") {
    renderRules();
  }
  if (name === "log") {
    renderLog();
  }
  if (name === "settings") {
    fillSettings();
  }
}

function renderStatus() {
  $("status-line").textContent = state.watching
    ? "Наблюдение включено"
    : "Наблюдение остановлено";
  $("watch-toggle").textContent = state.watching ? "Пауза" : "Следить";
}

function renderRules() {
  const list = $("rule-list");
  list.replaceChildren();
  if (!state.config.rules.length) {
    list.append(empty("Правил пока нет. Добавьте своё или возьмите пресет."));
    return;
  }
  state.config.rules.forEach((rule) => list.append(ruleCard(rule)));
}

function ruleCard(rule) {
  const card = document.createElement("article");
  card.className = rule.enabled ? "card" : "card off";
  const title = document.createElement("h3");
  title.textContent = ruleTitle(rule);
  const meta = document.createElement("p");
  meta.className = "meta";
  meta.textContent = ruleMeta(rule);
  const actions = document.createElement("div");
  actions.className = "card-actions";
  const toggle = document.createElement("label");
  toggle.className = "check";
  const checkbox = document.createElement("input");
  checkbox.type = "checkbox";
  checkbox.checked = rule.enabled;
  checkbox.addEventListener("change", () => {
    guard(() => setEnabled(rule.id, checkbox.checked));
  });
  toggle.append(checkbox, document.createTextNode("Включено"));
  const buttons = document.createElement("div");
  buttons.className = "card-actions";
  const edit = document.createElement("button");
  edit.type = "button";
  edit.textContent = "Изменить";
  edit.addEventListener("click", () => openEditor({ mode: "edit", id: rule.id }));
  const remove = document.createElement("button");
  remove.type = "button";
  remove.textContent = "Удалить";
  remove.addEventListener("click", () => guard(() => removeRule(rule.id)));
  buttons.append(edit, remove);
  actions.append(toggle, buttons);
  card.append(title, meta, actions);
  return card;
}

function renderLog() {
  const list = $("log-list");
  list.replaceChildren();
  if (!state.log.length) {
    list.append(empty("Журнал пуст. Запустите наблюдение или разберите уже лежащие файлы."));
    return;
  }
  [...state.log].reverse().forEach((record) => {
    const row = document.createElement("article");
    row.className = "log-row";
    const pill = document.createElement("span");
    pill.className = record.success ? "pill ok" : "pill bad";
    pill.textContent = record.success ? record.action : "fail";
    const body = document.createElement("div");
    const route = document.createElement("div");
    route.className = "route";
    route.textContent = record.destination
      ? `${record.source} → ${record.destination}`
      : record.source;
    const when = document.createElement("div");
    when.className = "when";
    when.textContent = record.message
      ? `${record.timestamp} · ${record.message}`
      : record.timestamp;
    body.append(route, when);
    row.append(pill, body);
    list.append(row);
  });
}

function fillSettings() {
  const settings = state.config.settings;
  $("theme").value = settings.theme;
  $("scan-on-start").value = settings.scan_on_start;
  $("watch-folder").value = state.config.watch_folder || "";
  $("watch-folders").value = (state.config.watch_folders || []).join("\n");
  $("mp3-qscale").value = settings.quality.mp3_qscale;
  $("audio-bitrate").value = settings.quality.audio_bitrate;
  $("audio-codec").value = settings.quality.audio_codec;
  $("video-codec").value = settings.quality.video_codec;
  $("video-crf").value = settings.quality.video_crf;
  $("image-quality").value = settings.quality.image_quality;
  $("delete-original").checked = settings.delete_original;
  $("watch-on-start").checked = settings.watch_on_start;
  $("autostart").checked = settings.autostart;
  $("config-path").textContent = state.path;
}

function openEditor(editor) {
  state.editor = editor;
  const rule =
    editor.mode === "edit"
      ? state.config.rules.find((item) => item.id === editor.id)
      : blankRule();
  $("editor-title").textContent = editor.mode === "edit" ? "Изменить правило" : "Новое правило";
  $("rule-enabled").checked = rule.enabled !== false;
  $("rule-ext").value = rule.extension || "";
  $("rule-name").value = rule.name_contains || "";
  $("rule-glob").value = rule.name_pattern || "";
  $("rule-watch").value = rule.watch_dir || "";
  $("rule-output").value = rule.output_dir || "";
  const action = rule.action || {};
  if (action.Rename) {
    $("action").value = "rename";
    $("prefix").value = action.Rename.prefix || "";
    $("suffix").value = action.Rename.suffix || "";
    $("find").value = action.Rename.find || "";
    $("replace").value = action.Rename.replace || "";
    $("pattern").value = action.Rename.pattern || "";
  } else if (action.Move) {
    $("action").value = "move";
    $("move-dest").value = action.Move.dest || "";
  } else {
    $("action").value = "convert";
    $("target-format").value = action.Convert?.target_format || "";
    const deletion = action.Convert?.delete_original;
    $("delete-mode").value = deletion === true ? "yes" : deletion === false ? "no" : "inherit";
  }
  syncActionFields();
  $("editor-form").hidden = false;
  $("rule-ext").focus();
}

function closeEditor() {
  state.editor = null;
  $("editor-form").hidden = true;
}

function syncActionFields() {
  const action = $("action").value;
  $("fields-convert").hidden = action !== "convert";
  $("fields-rename").hidden = action !== "rename";
  $("fields-move").hidden = action !== "move";
}

async function saveRule() {
  const rule = readRule();
  const rules = state.config.rules.slice();
  if (state.editor?.mode === "edit") {
    const index = rules.findIndex((item) => item.id === state.editor.id);
    if (index < 0) {
      throw new Error("Правило не найдено");
    }
    rules[index] = { ...rules[index], ...rule, id: state.editor.id };
  } else {
    rules.push(rule);
  }
  state.config = await api.saveConfig({ ...state.config, rules });
  closeEditor();
  renderRules();
  toast("Правило сохранено");
}

function readRule() {
  const kind = $("action").value;
  let action;
  if (kind === "convert") {
    const format = $("target-format").value.trim();
    if (!format) {
      throw new Error("Укажите формат, например mp3");
    }
    const mode = $("delete-mode").value;
    action = {
      Convert: {
        target_format: format,
        delete_original: mode === "yes" ? true : mode === "no" ? false : null,
      },
    };
  } else if (kind === "rename") {
    const prefix = emptyToNull($("prefix").value);
    const suffix = emptyToNull($("suffix").value);
    const find = emptyToNull($("find").value);
    const pattern = emptyToNull($("pattern").value);
    if (!prefix && !suffix && !find && !pattern) {
      throw new Error("Укажите префикс, суффикс, замену или шаблон");
    }
    action = {
      Rename: {
        prefix,
        suffix,
        find,
        replace: emptyToNull($("replace").value),
        pattern,
      },
    };
  } else {
    const dest = $("move-dest").value.trim();
    if (!dest) {
      throw new Error("Укажите папку назначения");
    }
    action = { Move: { dest } };
  }
  return {
    id: "",
    enabled: $("rule-enabled").checked,
    extension: emptyToNull($("rule-ext").value),
    name_contains: emptyToNull($("rule-name").value),
    name_pattern: emptyToNull($("rule-glob").value),
    watch_dir: emptyToNull($("rule-watch").value),
    output_dir: emptyToNull($("rule-output").value),
    action,
  };
}

async function setEnabled(id, enabled) {
  const rules = state.config.rules.map((rule) => (rule.id === id ? { ...rule, enabled } : rule));
  state.config = await api.saveConfig({ ...state.config, rules });
  renderRules();
}

async function removeRule(id) {
  state.config = await api.saveConfig({
    ...state.config,
    rules: state.config.rules.filter((rule) => rule.id !== id),
  });
  renderRules();
  toast("Правило удалено");
}

async function addPreset(kind) {
  state.config = await api.addPreset(
    kind,
    emptyToNull($("preset-watch").value),
    emptyToNull($("preset-output").value),
  );
  renderRules();
  toast("Пресет добавлен");
}

async function saveSettings() {
  const next = readSettings();
  state.config = await api.saveConfig(next);
  applyTheme(state.config.settings.theme);
  toast("Настройки сохранены");
}

function readSettings() {
  const quality = state.config.settings.quality;
  return {
    ...state.config,
    watch_folder: $("watch-folder").value.trim(),
    watch_folders: $("watch-folders")
      .value.split("\n")
      .map((line) => line.trim())
      .filter(Boolean),
    settings: {
      ...state.config.settings,
      theme: $("theme").value,
      scan_on_start: $("scan-on-start").value,
      delete_original: $("delete-original").checked,
      watch_on_start: $("watch-on-start").checked,
      autostart: $("autostart").checked,
      quality: {
        ...quality,
        mp3_qscale: clamp($("mp3-qscale").value, 0, 9),
        audio_bitrate: $("audio-bitrate").value.trim() || "192k",
        audio_codec: $("audio-codec").value.trim(),
        video_codec: $("video-codec").value.trim(),
        video_crf: clamp($("video-crf").value, 0, 51),
        image_quality: clamp($("image-quality").value, 1, 31),
      },
    },
  };
}

async function batch() {
  const records = await api.processExisting($("since").value);
  state.log.push(...records);
  toast(records.length ? `Операций: ${records.length}` : "Подходящих файлов нет");
  if (state.view === "log") {
    renderLog();
  }
}

async function toggleWatch(shouldRun) {
  if (shouldRun) {
    const records = await api.startWatch();
    state.watching = true;
    state.log.push(...records);
    toast("Наблюдение запущено");
  } else {
    await api.stopWatch();
    state.watching = false;
    toast("Наблюдение остановлено");
  }
  renderStatus();
  if (state.view === "log") {
    renderLog();
  }
}

async function browse(inputId) {
  const picked = await api.pickFolder();
  if (picked) {
    $(inputId).value = picked;
  }
}

function applyTheme(theme) {
  document.documentElement.dataset.theme = theme === "light" ? "light" : "dark";
}

function hideDrop() {
  $("drop-overlay").hidden = true;
}

function toast(message, bad = false) {
  const node = document.createElement("div");
  node.className = bad ? "toast bad" : "toast";
  node.textContent = message;
  $("toasts").append(node);
  setTimeout(() => node.remove(), 4200);
}

async function guard(work) {
  try {
    await work();
  } catch (error) {
    toast(text(error), true);
  }
}

function empty(message) {
  const node = document.createElement("div");
  node.className = "empty";
  node.textContent = message;
  return node;
}

function blankRule() {
  return {
    enabled: true,
    extension: "",
    action: { Convert: { target_format: "mp3", delete_original: null } },
  };
}

function ruleTitle(rule) {
  const ext = String(rule.extension || "*").replace(/^\./, "");
  if (rule.action?.Convert) {
    return `*.${ext} → ${rule.action.Convert.target_format}`;
  }
  if (rule.action?.Rename) {
    return `*.${ext} → новое имя`;
  }
  if (rule.action?.Move) {
    return `*.${ext} → ${rule.action.Move.dest}`;
  }
  return `*.${ext}`;
}

function ruleMeta(rule) {
  const parts = [];
  if (rule.name_contains) {
    parts.push(`имя содержит «${rule.name_contains}»`);
  }
  if (rule.name_pattern) {
    parts.push(`шаблон «${rule.name_pattern}»`);
  }
  if (rule.watch_dir) {
    parts.push(`папка ${rule.watch_dir}`);
  }
  if (rule.output_dir) {
    parts.push(`выход ${rule.output_dir}`);
  }
  const rename = rule.action?.Rename;
  if (rename?.prefix) {
    parts.push(`префикс ${rename.prefix}`);
  }
  if (rename?.suffix) {
    parts.push(`суффикс ${rename.suffix}`);
  }
  if (rename?.pattern) {
    parts.push(`шаблон ${rename.pattern}`);
  }
  if (rename?.find) {
    parts.push(`«${rename.find}» → «${rename.replace || ""}»`);
  }
  return parts.join(" · ") || "без дополнительных фильтров";
}

function summary(record) {
  if (!record.success) {
    return record.message || "Ошибка операции";
  }
  return record.destination
    ? `${record.action}: ${fileName(record.source)} → ${fileName(record.destination)}`
    : `${record.action}: ${fileName(record.source)}`;
}

function fileName(path) {
  const parts = String(path).split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

function emptyToNull(value) {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function clamp(value, min, max) {
  const number = Number(value);
  if (Number.isNaN(number)) {
    return min;
  }
  return Math.min(max, Math.max(min, number));
}

function text(error) {
  if (typeof error === "string") {
    return error;
  }
  return error?.message || "Неизвестная ошибка";
}
