use crate::config::resolve_config_path;
use crate::engine::{self, Engine, describe_rule};
use crate::log::format_record;
use crate::rules::{Action, PresetKind, Rule};
use crate::scanner::TimeFilter;
use crate::watcher::spawn_watcher;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(
    name = "file-forge",
    version,
    about = "Следит за папками и по правилам конвертирует, переименовывает или перемещает файлы"
)]
struct Cli {
    #[arg(long, global = true, help = "Путь к config.json")]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    #[command(about = "Следить за папками и обрабатывать новые файлы")]
    Watch {
        #[arg(long, help = "Не обрабатывать уже лежащие файлы")]
        no_scan: bool,
        #[arg(long, value_name = "WHEN", help = "all, day, week или month")]
        since: Option<String>,
    },
    #[command(about = "Конвертировать один файл через FFmpeg")]
    Convert {
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long)]
        format: Option<String>,
        #[arg(long, help = "Удалить исходный файл после успеха")]
        delete: bool,
        #[arg(long, help = "Оставить исходный файл")]
        keep: bool,
    },
    #[command(about = "Показать, добавить или удалить правила")]
    Rules {
        #[command(subcommand)]
        action: RulesCmd,
    },
    #[command(about = "Обработать файлы, которые уже лежат в папках")]
    Batch {
        #[arg(long, value_name = "WHEN", default_value = "day")]
        since: String,
    },
    #[command(about = "Показать журнал операций")]
    Log {
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
}

#[derive(Subcommand, Debug)]
enum RulesCmd {
    #[command(about = "Список правил")]
    List,
    #[command(about = "Удалить правило по номеру или id")]
    Remove { id: String },
    #[command(about = "Включить правило")]
    Enable { id: String },
    #[command(about = "Выключить правило")]
    Disable { id: String },
    #[command(about = "Добавить пресет: audio, video или images")]
    Preset {
        kind: String,
        #[arg(long)]
        watch: Option<String>,
        #[arg(long)]
        output: Option<String>,
    },
    #[command(about = "Добавить правило")]
    Add {
        #[command(subcommand)]
        action: AddCmd,
    },
}

#[derive(Subcommand, Debug)]
enum AddCmd {
    #[command(about = "Конвертация в другой формат")]
    Convert {
        #[arg(long)]
        ext: Option<String>,
        #[arg(long)]
        format: String,
        #[arg(long)]
        watch: Option<String>,
        #[arg(long)]
        output: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        glob: Option<String>,
        #[arg(long)]
        delete: bool,
        #[arg(long)]
        keep: bool,
    },
    #[command(about = "Переименование")]
    Rename {
        #[arg(long)]
        ext: Option<String>,
        #[arg(long)]
        watch: Option<String>,
        #[arg(long)]
        output: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        glob: Option<String>,
        #[arg(long)]
        prefix: Option<String>,
        #[arg(long)]
        suffix: Option<String>,
        #[arg(long)]
        find: Option<String>,
        #[arg(long)]
        replace: Option<String>,
        #[arg(long)]
        pattern: Option<String>,
    },
    #[command(about = "Перемещение")]
    Move {
        #[arg(long)]
        ext: Option<String>,
        #[arg(long)]
        watch: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        glob: Option<String>,
        dest: String,
    },
}

pub fn run() -> i32 {
    init_tracing();
    let cli = Cli::parse();
    let config = cli.config;
    let result = match cli.command {
        None => watch(config, false, None),
        Some(Cmd::Watch { no_scan, since }) => watch(config, no_scan, since),
        Some(Cmd::Convert {
            input,
            output,
            format,
            delete,
            keep,
        }) => convert(config, input, output, format, delete, keep),
        Some(Cmd::Rules { action }) => rules(config, action),
        Some(Cmd::Batch { since }) => batch(config, since),
        Some(Cmd::Log { limit }) => log(config, limit),
    };
    match result {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("{err}");
            1
        }
    }
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

fn open(config: Option<PathBuf>) -> Result<Engine, String> {
    Engine::open(resolve_config_path(config))
}

fn watch(config: Option<PathBuf>, no_scan: bool, since: Option<String>) -> Result<(), String> {
    let mut engine = open(config)?;
    let filter = match since {
        Some(value) => TimeFilter::parse(&value).ok_or_else(|| {
            format!("неизвестный период: {value}. Используйте all, day, week или month")
        })?,
        None => engine.config().settings.scan_on_start,
    };
    let dirs = engine.prepare_watch_dirs()?;
    println!("Конфиг: {}", engine.config_path().display());
    println!(
        "Наблюдение: {}",
        dirs.iter()
            .map(|dir| dir.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    if !no_scan {
        print_records(&engine.process_existing(filter));
    }
    let session = spawn_watcher(dirs)?;
    loop {
        match session.recv_timeout(Duration::from_millis(400)) {
            Ok(path) => print_records(&engine.process_path(&path)),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}

fn convert(
    config: Option<PathBuf>,
    input: PathBuf,
    output: Option<PathBuf>,
    format: Option<String>,
    delete: bool,
    keep: bool,
) -> Result<(), String> {
    if delete && keep {
        return Err("укажите только один из флагов --delete или --keep".to_string());
    }
    if !input.is_file() {
        return Err(format!("файл не найден: {}", input.display()));
    }
    let format = match format {
        Some(value) => value,
        None => output
            .as_ref()
            .and_then(|path| path.extension())
            .and_then(|value| value.to_str())
            .map(str::to_string)
            .ok_or_else(|| "укажите --format или --output с расширением".to_string())?,
    };
    let deletion = if delete {
        Some(true)
    } else if keep {
        Some(false)
    } else {
        None
    };
    let mut engine = open(config)?;
    let record = engine.convert_file(&input, output.as_deref(), &format, deletion);
    println!("{}", format_record(&record));
    if record.success {
        Ok(())
    } else {
        Err(record.message)
    }
}

fn batch(config: Option<PathBuf>, since: String) -> Result<(), String> {
    let filter = TimeFilter::parse(&since).ok_or_else(|| {
        format!("неизвестный период: {since}. Используйте all, day, week или month")
    })?;
    let mut engine = open(config)?;
    let _ = engine.prepare_watch_dirs()?;
    let records = engine.process_existing(filter);
    print_records(&records);
    println!("Готово: {} операций", records.len());
    Ok(())
}

fn log(config: Option<PathBuf>, limit: usize) -> Result<(), String> {
    let engine = open(config)?;
    let records = engine.recent_log(limit)?;
    if records.is_empty() {
        println!("Журнал пуст ({})", engine.log_path().display());
        return Ok(());
    }
    print_records(&records);
    Ok(())
}

fn rules(config: Option<PathBuf>, action: RulesCmd) -> Result<(), String> {
    let mut engine = open(config)?;
    match action {
        RulesCmd::List => {
            if engine.config().rules.is_empty() {
                println!("Правил пока нет");
                return Ok(());
            }
            for (offset, rule) in engine.config().rules.iter().enumerate() {
                println!("{}", describe_rule(offset + 1, rule));
            }
        }
        RulesCmd::Remove { id } => {
            let index = engine::rule_by_token(&engine.config().rules, &id)
                .map(|(index, _)| index)
                .ok_or_else(|| format!("правило не найдено: {id}"))?;
            engine.update(|config| {
                config.rules.remove(index);
            })?;
            println!("Правило удалено");
        }
        RulesCmd::Enable { id } => set_enabled(&mut engine, &id, true)?,
        RulesCmd::Disable { id } => set_enabled(&mut engine, &id, false)?,
        RulesCmd::Preset {
            kind,
            watch,
            output,
        } => {
            let kind = PresetKind::parse(&kind).ok_or_else(|| {
                format!("неизвестный пресет: {kind}. Используйте audio, video или images")
            })?;
            let added = engine.add_preset(kind, watch, output)?;
            println!("Добавлен пресет {}: {} правил", kind.title(), added.len());
        }
        RulesCmd::Add { action } => {
            let rule = rule_from_add(action)?;
            let id = rule.id.clone();
            engine.update(|config| config.rules.push(rule))?;
            println!("Добавлено правило {id}");
        }
    }
    Ok(())
}

fn set_enabled(engine: &mut Engine, id: &str, enabled: bool) -> Result<(), String> {
    let index = engine::rule_by_token(&engine.config().rules, id)
        .map(|(index, _)| index)
        .ok_or_else(|| format!("правило не найдено: {id}"))?;
    engine.update(|config| {
        config.rules[index].enabled = enabled;
    })?;
    println!(
        "Правило {}",
        if enabled {
            "включено"
        } else {
            "выключено"
        }
    );
    Ok(())
}

fn rule_from_add(action: AddCmd) -> Result<Rule, String> {
    let mut rule = match action {
        AddCmd::Convert {
            ext,
            format,
            watch,
            output,
            name,
            glob,
            delete,
            keep,
        } => {
            if delete && keep {
                return Err("укажите только один из флагов --delete или --keep".to_string());
            }
            let delete_original = if delete {
                Some(true)
            } else if keep {
                Some(false)
            } else {
                None
            };
            Rule {
                id: String::new(),
                watch_dir: watch,
                extension: ext,
                name_contains: name,
                name_pattern: glob,
                action: Action::Convert {
                    target_format: format,
                    delete_original,
                },
                output_dir: output,
                enabled: true,
            }
        }
        AddCmd::Rename {
            ext,
            watch,
            output,
            name,
            glob,
            prefix,
            suffix,
            find,
            replace,
            pattern,
        } => {
            if present_text(&prefix).is_none()
                && present_text(&suffix).is_none()
                && present_text(&find).is_none()
                && present_text(&pattern).is_none()
            {
                return Err(
                    "укажите --prefix, --suffix, --find или --pattern для переименования"
                        .to_string(),
                );
            }
            Rule {
                id: String::new(),
                watch_dir: watch,
                extension: ext,
                name_contains: name,
                name_pattern: glob,
                action: Action::Rename {
                    pattern,
                    prefix,
                    suffix,
                    find,
                    replace,
                },
                output_dir: output,
                enabled: true,
            }
        }
        AddCmd::Move {
            ext,
            watch,
            name,
            glob,
            dest,
        } => {
            if dest.trim().is_empty() {
                return Err("укажите папку назначения".to_string());
            }
            Rule {
                id: String::new(),
                watch_dir: watch,
                extension: ext,
                name_contains: name,
                name_pattern: glob,
                action: Action::Move { dest },
                output_dir: None,
                enabled: true,
            }
        }
    };
    if rule.id.is_empty() {
        rule.id = crate::rules::new_id();
    }
    Ok(rule)
}

fn present_text(value: &Option<String>) -> Option<&str> {
    value
        .as_ref()
        .map(|text| text.trim())
        .filter(|text| !text.is_empty())
}

fn print_records(records: &[crate::log::OperationRecord]) {
    for record in records {
        println!("{}", format_record(record));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rule_commands() {
        let cli = Cli::try_parse_from([
            "file-forge",
            "--config",
            "custom.json",
            "rules",
            "add",
            "rename",
            "--ext",
            "txt",
            "--prefix",
            "DONE_",
            "--name",
            "report",
        ])
        .unwrap();
        assert_eq!(cli.config.unwrap(), PathBuf::from("custom.json"));
        match cli.command {
            Some(Cmd::Rules {
                action:
                    RulesCmd::Add {
                        action: AddCmd::Rename { prefix, name, .. },
                    },
            }) => {
                assert_eq!(prefix.as_deref(), Some("DONE_"));
                assert_eq!(name.as_deref(), Some("report"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
