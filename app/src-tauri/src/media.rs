//! Conservative MP4 preview pipeline. Outputs a separate 1080p file; never
//! replaces an installed game's video. A game-specific compatibility profile
//! and a completed backup transaction are required before replacement exists.
use serde_json::Value;
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}
#[derive(Debug)]
pub struct Preview {
    pub path: PathBuf,
    pub original_bytes: u64,
    pub output_bytes: u64,
}

fn execute(
    executable: &Path,
    args: &[&std::ffi::OsStr],
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".into());
    }
    if !executable.is_absolute() || !executable.is_file() {
        return Err("A verified local FFmpeg/FFprobe installation is required.".into());
    }
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::{
            BELOW_NORMAL_PRIORITY_CLASS, CREATE_NO_WINDOW,
        };
        command.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().ok_or("Cannot read media tool output")?;
    let (send, recv) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(1_048_577)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = send.send(result);
    });
    let started = Instant::now();
    let mut output = None;
    let result = loop {
        if cancel.load(Ordering::Relaxed) {
            break Err("cancelled".into());
        }
        if started.elapsed() >= timeout {
            break Err("Media tool timed out.".into());
        }
        if output.is_none()
            && let Ok(result) = recv.try_recv()
        {
            match result {
                Ok(bytes) if bytes.len() <= 1_048_576 => output = Some(bytes),
                _ => {
                    break Err(
                        "Media metadata exceeded the supported size or could not be read.".into(),
                    );
                }
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    break Err(
                        "The media tool could not process this video. Original files were kept."
                            .into(),
                    );
                }
                if let Some(bytes) = output.take() {
                    break Ok(bytes);
                }
            }
            Ok(None) => {}
            Err(e) => break Err(e.to_string()),
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    if result.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    let _ = reader.join();
    result
}

fn args<'a>(strings: &'a [&'a str]) -> Vec<&'a std::ffi::OsStr> {
    strings.iter().map(std::ffi::OsStr::new).collect()
}

impl Tools {
    fn probe(&self, path: &Path, cancel: &AtomicBool) -> Result<Value, String> {
        let mut command = args(&[
            "-v",
            "error",
            "-protocol_whitelist",
            "file",
            "-f",
            "mov",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
        ]);
        command.push(path.as_os_str());
        serde_json::from_slice(&execute(
            &self.ffprobe,
            &command,
            cancel,
            Duration::from_secs(30),
        )?)
        .map_err(|e| e.to_string())
    }

    pub fn preview_1080p(
        &self,
        input: &Path,
        output_directory: &Path,
        cancel: &AtomicBool,
    ) -> Result<Preview, String> {
        if input
            .extension()
            .and_then(|v| v.to_str())
            .is_none_or(|v| !v.eq_ignore_ascii_case("mp4"))
        {
            return Err("Only standalone MP4 sample previews are supported. Bink and packed archives are not supported.".into());
        }
        let metadata = fs::symlink_metadata(input).map_err(|e| e.to_string())?;
        if !metadata.is_file() || crate::filesystem::is_link(&metadata) {
            return Err("Choose a regular video file.".into());
        }
        // Keep the original stable throughout probing and encoding on Windows.
        let _source_dirs =
            crate::backups::pin_directories(input.parent().ok_or("Missing video directory")?)?;
        let original: File = crate::backups::read_locked(input, false)?;
        let source = self.probe(input, cancel)?;
        let source_video = validate_source(&source)?;
        if !output_directory.is_absolute() || !output_directory.is_dir() {
            return Err("Choose an existing absolute preview directory.".into());
        }
        let _output_dirs = crate::backups::pin_directories(output_directory)?;
        let directory = output_directory.join(format!("gamezip-preview-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).map_err(|e| e.to_string())?;
        let output = directory.join("preview-1080p.mp4");
        let mut command = args(&[
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-n",
            "-protocol_whitelist",
            "file",
            "-f",
            "mov",
            "-i",
        ]);
        command.push(input.as_os_str());
        command.extend(args(&[
            "-map",
            "0:v:0",
            "-map",
            "0:a?",
            "-map_metadata",
            "0",
            "-map_chapters",
            "0",
            "-vf",
            "scale=1920:1080:flags=lanczos",
            "-c:v",
            "libx264",
            "-preset",
            "medium",
            "-crf",
            "20",
            "-pix_fmt",
            "yuv420p",
            "-threads",
            "2",
            "-c:a",
            "copy",
            "-movflags",
            "+faststart",
        ]));
        command.push(output.as_os_str());
        execute(&self.ffmpeg, &command, cancel, Duration::from_secs(1800))?;
        let converted = self.probe(&output, cancel)?;
        validate_output(&source, source_video, &converted)?;
        let mut verify = args(&[
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-xerror",
            "-protocol_whitelist",
            "file",
            "-i",
        ]);
        verify.push(output.as_os_str());
        verify.extend(args(&["-map", "0", "-threads", "2", "-f", "null", "-"]));
        execute(&self.ffmpeg, &verify, cancel, Duration::from_secs(1800))?;
        let output_bytes = fs::metadata(&output).map_err(|e| e.to_string())?.len();
        let original_bytes = original.metadata().map_err(|e| e.to_string())?.len();
        Ok(Preview {
            path: output,
            original_bytes,
            output_bytes,
        })
    }
}

fn streams(value: &Value) -> Result<&Vec<Value>, String> {
    value["streams"]
        .as_array()
        .ok_or("Video has no stream metadata.".into())
}
fn duration(value: &Value) -> Result<f64, String> {
    let duration = value["format"]["duration"]
        .as_str()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.)
        .ok_or("Cannot verify video duration.")?;
    Ok(duration)
}
fn validate_source(value: &Value) -> Result<&Value, String> {
    let streams = streams(value)?;
    if streams.is_empty()
        || streams.len() > 16
        || streams
            .iter()
            .any(|s| !matches!(s["codec_type"].as_str(), Some("video" | "audio")))
    {
        return Err("This sample has unsupported tracks.".into());
    }
    let video: Vec<_> = streams
        .iter()
        .filter(|s| s["codec_type"] == "video")
        .collect();
    if video.len() != 1 {
        return Err("A single video track is required.".into());
    }
    let video = video[0];
    if video["codec_name"] != "h264"
        || video["width"] != 3840
        || video["height"] != 2160
        || video["pix_fmt"] != "yuv420p"
    {
        return Err("Preview currently requires a 3840×2160, 8-bit H.264 MP4 sample.".into());
    }
    if video["side_data_list"]
        .as_array()
        .is_some_and(|v| !v.is_empty())
        || video["tags"]["rotate"].as_str().is_some_and(|s| s != "0")
        || video["sample_aspect_ratio"]
            .as_str()
            .is_some_and(|s| s != "1:1")
        || matches!(
            video["color_transfer"].as_str(),
            Some("smpte2084" | "arib-std-b67")
        )
    {
        return Err(
            "Rotated, HDR or non-square-pixel video needs a separate compatibility profile.".into(),
        );
    }
    duration(value)?;
    if video["avg_frame_rate"].as_str().is_none_or(|s| s == "0/0")
        || video["avg_frame_rate"] != video["r_frame_rate"]
    {
        return Err(
            "Variable or unknown frame rates require a separate compatibility profile.".into(),
        );
    }
    Ok(video)
}
fn validate_output(source: &Value, source_video: &Value, output: &Value) -> Result<(), String> {
    let output_streams = streams(output)?;
    let videos: Vec<_> = output_streams
        .iter()
        .filter(|s| s["codec_type"] == "video")
        .collect();
    if videos.len() != 1
        || videos[0]["width"] != 1920
        || videos[0]["height"] != 1080
        || videos[0]["codec_name"] != "h264"
        || videos[0]["avg_frame_rate"] != source_video["avg_frame_rate"]
        || (duration(source)? - duration(output)?).abs() > 0.1
    {
        return Err("Converted video failed resolution, frame-rate or duration validation.".into());
    }
    let before: Vec<_> = streams(source)?
        .iter()
        .filter(|s| s["codec_type"] == "audio")
        .collect();
    let after: Vec<_> = output_streams
        .iter()
        .filter(|s| s["codec_type"] == "audio")
        .collect();
    if before.len() != after.len()
        || before.iter().zip(after).any(|(a, b)| {
            ["codec_name", "sample_rate", "channels", "channel_layout"]
                .iter()
                .any(|key| a[*key] != b[*key])
        })
    {
        return Err("Converted video failed audio-track validation.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_video_profiles_are_rejected() {
        let mut value = serde_json::json!({"streams":[{"codec_type":"video","codec_name":"h264","width":3840,"height":2160,"pix_fmt":"yuv420p","avg_frame_rate":"24/1","r_frame_rate":"24/1"}],"format":{"duration":"1.0"}});
        assert!(validate_source(&value).is_ok());
        value["streams"][0]["codec_name"] = "binkvideo".into();
        assert!(validate_source(&value).is_err());
        value["streams"][0]["codec_name"] = "h264".into();
        value["streams"][0]["color_transfer"] = "smpte2084".into();
        assert!(validate_source(&value).is_err());
    }

    #[test]
    #[ignore = "requires verified local FFmpeg tools via GAMEZIP_TEST_FFMPEG and GAMEZIP_TEST_FFPROBE"]
    fn sample_conversion_keeps_original_and_audio_and_produces_1080p() {
        let tools = Tools {
            ffmpeg: std::env::var_os("GAMEZIP_TEST_FFMPEG")
                .expect("FFmpeg path")
                .into(),
            ffprobe: std::env::var_os("GAMEZIP_TEST_FFPROBE")
                .expect("FFprobe path")
                .into(),
        };
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("sample.mp4");
        let cancel = AtomicBool::new(false);
        let mut command = args(&[
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=3840x2160:r=24:d=1",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=1",
            "-c:v",
            "libx264",
            "-threads",
            "2",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
        ]);
        command.push(input.as_os_str());
        execute(&tools.ffmpeg, &command, &cancel, Duration::from_secs(120)).unwrap();
        let original = fs::read(&input).unwrap();
        let preview = tools.preview_1080p(&input, temp.path(), &cancel).unwrap();
        assert!(preview.output_bytes > 0);
        assert_eq!(fs::read(&input).unwrap(), original);
        let audio_hash = |path: &Path| {
            let mut command = args(&["-nostdin", "-v", "error", "-i"]);
            command.push(path.as_os_str());
            command.extend(args(&[
                "-map", "0:a:0", "-c", "copy", "-f", "hash", "-hash", "sha256", "-",
            ]));
            execute(&tools.ffmpeg, &command, &cancel, Duration::from_secs(30)).unwrap()
        };
        assert_eq!(audio_hash(&input), audio_hash(&preview.path));
        assert_eq!(
            tools.probe(&preview.path, &cancel).unwrap()["streams"][0]["height"],
            1080
        );
        cancel.store(true, Ordering::Relaxed);
        assert!(tools.preview_1080p(&input, temp.path(), &cancel).is_err());
    }
}
