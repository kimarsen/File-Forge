use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("file-forge-flow-{nanos}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_config(root: &Path) -> PathBuf {
    let config = root.join("config.json");
    let folder = root.join("inbox");
    fs::create_dir_all(&folder).unwrap();
    let body = format!(
        r#"{{
  "watch_folder": "{}",
  "watch_folders": [],
  "rules": [],
  "settings": {{
    "delete_original": true,
    "scan_on_start": "all",
    "theme": "dark",
    "autostart": false,
    "watch_on_start": false,
    "quality": {{
      "mp3_qscale": 2,
      "audio_bitrate": "192k",
      "audio_codec": "",
      "video_codec": "",
      "video_crf": 23,
      "image_quality": 2
    }},
    "counter": 1
  }}
}}"#,
        folder.display().to_string().replace('\\', "\\\\")
    );
    fs::write(&config, body).unwrap();
    config
}

fn forge(config: &Path, args: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_file-forge"))
        .arg("--config")
        .arg(config)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "file-forge {}\n{}",
        args.join(" "),
        explain(&output)
    );
    output
}

fn explain(output: &Output) -> String {
    format!(
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn ffmpeg(args: &[&str]) {
    let output = Command::new("ffmpeg")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "ffmpeg {}\n{}",
        args.join(" "),
        explain(&output)
    );
}

#[test]
fn cli_renames_converts_and_logs() {
    let root = temp_dir();
    let config = write_config(&root);
    let inbox = root.join("inbox");

    forge(
        &config,
        &[
            "rules", "add", "rename", "--ext", "txt", "--prefix", "DONE_", "--name", "report",
        ],
    );
    fs::write(inbox.join("report.txt"), b"quarter").unwrap();
    fs::write(inbox.join("notes.txt"), b"leave").unwrap();
    let batch = forge(&config, &["batch", "--since", "all"]);
    let batch_text = String::from_utf8_lossy(&batch.stdout);
    assert!(batch_text.contains("[ok]"));
    assert!(batch_text.contains("rename"));
    assert!(inbox.join("DONE_report.txt").is_file());
    assert!(inbox.join("notes.txt").is_file());
    assert!(!inbox.join("report.txt").exists());

    let again = forge(&config, &["batch", "--since", "all"]);
    let again_text = String::from_utf8_lossy(&again.stdout);
    assert!(again_text.contains("Готово: 0 операций"));

    let wav = inbox.join("tone.wav");
    ffmpeg(&[
        "-y",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=0.2",
        "-c:a",
        "pcm_s16le",
        &wav.to_string_lossy(),
    ]);
    let converted = forge(
        &config,
        &[
            "convert",
            &wav.to_string_lossy(),
            "--format",
            "mp3",
            "--keep",
        ],
    );
    let converted_text = String::from_utf8_lossy(&converted.stdout);
    assert!(converted_text.contains("libmp3lame") || converted_text.contains("[ok]"));
    assert!(inbox.join("tone.mp3").is_file());
    assert!(wav.is_file());

    let png = inbox.join("frame.png");
    ffmpeg(&[
        "-y",
        "-f",
        "lavfi",
        "-i",
        "color=c=red:s=16x16:d=0.05",
        "-frames:v",
        "1",
        &png.to_string_lossy(),
    ]);
    forge(
        &config,
        &[
            "rules", "add", "convert", "--ext", "png", "--format", "jpg", "--keep",
        ],
    );
    forge(&config, &["batch", "--since", "all"]);
    assert!(inbox.join("frame.jpg").is_file());
    assert!(png.is_file());

    let log = forge(&config, &["log", "--limit", "20"]);
    let log_text = String::from_utf8_lossy(&log.stdout);
    assert!(log_text.contains("rename"));
    assert!(log_text.contains("convert"));

    let list = forge(&config, &["rules", "list"]);
    let list_text = String::from_utf8_lossy(&list.stdout);
    assert!(list_text.contains("DONE_"));
    assert!(list_text.contains("jpg"));
    forge(&config, &["rules", "remove", "1"]);
    let listed = forge(&config, &["rules", "list"]);
    let listed_text = String::from_utf8_lossy(&listed.stdout);
    assert!(!listed_text.contains("DONE_"));

    let _ = fs::remove_dir_all(root);
}
