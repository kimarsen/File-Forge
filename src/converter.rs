use crate::config::QualitySettings;
use crate::paths::ensure_parent;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Copy)]
enum FormatKind {
    Mp3,
    Aac,
    Vorbis,
    Opus,
    Flac,
    Wav,
    Video { faststart: bool },
    Webm,
    Jpeg,
    Webp,
    Png,
}

struct FormatProfile {
    names: &'static [&'static str],
    kind: FormatKind,
}

fn profiles() -> &'static [FormatProfile] {
    &[
        FormatProfile {
            names: &["mp3"],
            kind: FormatKind::Mp3,
        },
        FormatProfile {
            names: &["m4a", "aac"],
            kind: FormatKind::Aac,
        },
        FormatProfile {
            names: &["ogg", "oga"],
            kind: FormatKind::Vorbis,
        },
        FormatProfile {
            names: &["opus"],
            kind: FormatKind::Opus,
        },
        FormatProfile {
            names: &["flac"],
            kind: FormatKind::Flac,
        },
        FormatProfile {
            names: &["wav"],
            kind: FormatKind::Wav,
        },
        FormatProfile {
            names: &["mp4", "m4v", "mov"],
            kind: FormatKind::Video { faststart: true },
        },
        FormatProfile {
            names: &["mkv", "avi"],
            kind: FormatKind::Video { faststart: false },
        },
        FormatProfile {
            names: &["webm"],
            kind: FormatKind::Webm,
        },
        FormatProfile {
            names: &["jpg", "jpeg"],
            kind: FormatKind::Jpeg,
        },
        FormatProfile {
            names: &["webp"],
            kind: FormatKind::Webp,
        },
        FormatProfile {
            names: &["png"],
            kind: FormatKind::Png,
        },
    ]
}

pub fn ffmpeg_args(format: &str, quality: &QualitySettings) -> Vec<String> {
    let format = normalize_format(format);
    let kind = profiles()
        .iter()
        .find(|profile| profile.names.iter().any(|name| *name == format))
        .map(|profile| profile.kind);
    match kind {
        Some(FormatKind::Mp3) => vec![
            "-codec:a".into(),
            chosen(&quality.audio_codec, "libmp3lame"),
            "-qscale:a".into(),
            quality.mp3_qscale.to_string(),
        ],
        Some(FormatKind::Aac) => vec![
            "-codec:a".into(),
            chosen(&quality.audio_codec, "aac"),
            "-b:a".into(),
            quality.audio_bitrate.clone(),
        ],
        Some(FormatKind::Vorbis) => vec![
            "-codec:a".into(),
            chosen(&quality.audio_codec, "libvorbis"),
            "-qscale:a".into(),
            quality.mp3_qscale.to_string(),
        ],
        Some(FormatKind::Opus) => vec![
            "-codec:a".into(),
            chosen(&quality.audio_codec, "libopus"),
            "-b:a".into(),
            quality.audio_bitrate.clone(),
        ],
        Some(FormatKind::Flac) => vec!["-codec:a".into(), chosen(&quality.audio_codec, "flac")],
        Some(FormatKind::Wav) => {
            vec!["-codec:a".into(), chosen(&quality.audio_codec, "pcm_s16le")]
        }
        Some(FormatKind::Video { faststart }) => {
            let mut args = vec![
                "-codec:v".into(),
                chosen(&quality.video_codec, "libx264"),
                "-crf".into(),
                quality.video_crf.to_string(),
                "-codec:a".into(),
                chosen(&quality.audio_codec, "aac"),
                "-b:a".into(),
                quality.audio_bitrate.clone(),
            ];
            if faststart {
                args.push("-movflags".into());
                args.push("+faststart".into());
            }
            args
        }
        Some(FormatKind::Webm) => vec![
            "-codec:v".into(),
            chosen(&quality.video_codec, "libvpx-vp9"),
            "-crf".into(),
            quality.video_crf.to_string(),
            "-b:v".into(),
            "0".into(),
            "-codec:a".into(),
            chosen(&quality.audio_codec, "libopus"),
            "-b:a".into(),
            quality.audio_bitrate.clone(),
        ],
        Some(FormatKind::Jpeg) => vec!["-q:v".into(), quality.image_quality.to_string()],
        Some(FormatKind::Webp) => vec![
            "-codec:v".into(),
            "libwebp".into(),
            "-quality".into(),
            webp_quality(quality.image_quality).to_string(),
        ],
        Some(FormatKind::Png) => Vec::new(),
        None => Vec::new(),
    }
}

fn chosen(custom: &str, default_codec: &str) -> String {
    let trimmed = custom.trim();
    if trimmed.is_empty() {
        default_codec.to_string()
    } else {
        trimmed.to_string()
    }
}

fn webp_quality(image_quality: u8) -> u8 {
    let qscale = image_quality.clamp(1, 31);
    100u8
        .saturating_sub(qscale.saturating_mul(2))
        .clamp(30, 100)
}

pub fn normalize_format(format: &str) -> String {
    format.trim().trim_start_matches('.').to_ascii_lowercase()
}

pub fn clean_duplicate_stem(stem: &str) -> String {
    let bytes = stem.as_bytes();
    if bytes.len() < 4 || !stem.ends_with(')') {
        return stem.to_string();
    }
    let Some(open) = stem.rfind('(') else {
        return stem.to_string();
    };
    if open == 0 {
        return stem.to_string();
    }
    let inside = &stem[open + 1..stem.len() - 1];
    if inside.is_empty() || !inside.chars().all(|ch| ch.is_ascii_digit()) {
        return stem.to_string();
    }
    stem[..open].trim_end().to_string()
}

pub fn output_path(input: &Path, format: &str, output_dir: Option<&Path>) -> PathBuf {
    let parent = input.parent().unwrap_or(Path::new(""));
    let dir = output_dir.unwrap_or(parent);
    let stem = input
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let stem = clean_duplicate_stem(stem);
    dir.join(format!("{stem}.{}", normalize_format(format)))
}

pub fn convert(
    input: &Path,
    output: &Path,
    format: &str,
    quality: &QualitySettings,
) -> Result<(), String> {
    if input == output {
        return Err("выходной файл совпадает с исходным".to_string());
    }
    ensure_parent(output)?;
    let mut command = Command::new("ffmpeg");
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(input);
    for arg in ffmpeg_args(format, quality) {
        command.arg(arg);
    }
    command.arg(output);
    command.stdin(Stdio::null());
    let result = command.output().map_err(|err| {
        format!("не удалось запустить ffmpeg: {err}. Установите FFmpeg и добавьте его в PATH")
    })?;
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        let stderr = tail(stderr.trim(), 800);
        return Err(match result.status.code() {
            Some(code) => format!("ffmpeg завершился с кодом {code}: {stderr}"),
            None => format!("ffmpeg завершился с ошибкой: {stderr}"),
        });
    }
    if !output.exists() {
        return Err("ffmpeg не создал выходной файл".to_string());
    }
    Ok(())
}

fn tail(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        text.to_string()
    } else {
        text.chars().skip(count - max_chars).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn mp3_uses_lame_and_qscale() {
        let args = ffmpeg_args("mp3", &QualitySettings::default());
        assert_eq!(
            args,
            vec!["-codec:a", "libmp3lame", "-qscale:a", "2"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn cleans_download_duplicate_suffix() {
        assert_eq!(clean_duplicate_stem("song (1)"), "song");
        assert_eq!(clean_duplicate_stem("song (live)"), "song (live)");
        assert_eq!(
            output_path(Path::new("/music/song (2).ogg"), "MP3", None),
            PathBuf::from("/music/song.mp3")
        );
    }

    #[test]
    fn converts_wav_and_reports_ffmpeg_errors() {
        let root = std::env::temp_dir().join(format!(
            "file-forge-convert-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let wav = root.join("tone.wav");
        let mp3 = root.join("tone.mp3");
        let synth = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=0.2",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(&wav)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            synth.status.success(),
            "{}",
            String::from_utf8_lossy(&synth.stderr)
        );
        convert(&wav, &mp3, "mp3", &QualitySettings::default()).unwrap();
        assert!(mp3.metadata().unwrap().len() > 0);

        let junk = root.join("notes.ogg");
        fs::write(&junk, b"this is not audio").unwrap();
        let error = convert(
            &junk,
            &root.join("notes.mp3"),
            "mp3",
            &QualitySettings::default(),
        );
        assert!(error.is_err());
        assert!(error.unwrap_err().contains("ffmpeg"));
        let _ = fs::remove_dir_all(root);
    }
}
