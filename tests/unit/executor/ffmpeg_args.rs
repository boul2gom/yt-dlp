use yt_dlp::executor::FfmpegArgs;

// ============================== FfmpegArgs::new ==============================

#[test]
fn ffmpeg_args_new_is_empty() {
    let args = FfmpegArgs::new().build();
    assert!(args.is_empty());
}

#[test]
fn ffmpeg_args_default() {
    let args = FfmpegArgs::default().build();
    assert!(args.is_empty());
}

// ============================== FfmpegArgs::input ==============================

#[test]
fn ffmpeg_args_single_input() {
    let args = FfmpegArgs::new().input("/tmp/input.mp4").build();
    assert_eq!(args, vec!["-i", "/tmp/input.mp4"]);
}

#[test]
fn ffmpeg_args_multiple_inputs() {
    let args = FfmpegArgs::new()
        .input("/tmp/video.mp4")
        .input("/tmp/audio.mp3")
        .build();
    assert_eq!(args, vec!["-i", "/tmp/video.mp4", "-i", "/tmp/audio.mp3"]);
}

// ============================== FfmpegArgs::codec_copy ==============================

#[test]
fn ffmpeg_args_codec_copy() {
    let args = FfmpegArgs::new().codec_copy().build();
    assert_eq!(args, vec!["-c", "copy"]);
}

// ============================== FfmpegArgs::overwrite ==============================

#[test]
fn ffmpeg_args_overwrite() {
    let args = FfmpegArgs::new().overwrite().build();
    assert_eq!(args, vec!["-y"]);
}

// ============================== FfmpegArgs::output ==============================

#[test]
fn ffmpeg_args_output_placed_last() {
    let args = FfmpegArgs::new()
        .input("/tmp/input.mp4")
        .codec_copy()
        .output("/tmp/output.mkv")
        .build();
    assert_eq!(args, vec!["-i", "/tmp/input.mp4", "-c", "copy", "/tmp/output.mkv"]);
}

#[test]
fn ffmpeg_args_overwrite_before_output() {
    let args = FfmpegArgs::new()
        .input("/tmp/input.mp4")
        .overwrite()
        .output("/tmp/output.mkv")
        .build();
    // -y should come before output
    let y_pos = args.iter().position(|a| a == "-y").unwrap();
    let out_pos = args.iter().position(|a| a == "/tmp/output.mkv").unwrap();
    assert!(y_pos < out_pos);
}

// ============================== FfmpegArgs::args ==============================

#[test]
fn ffmpeg_args_arbitrary_args() {
    let args = FfmpegArgs::new().args(["-map", "0:v", "-map", "1:a"]).build();
    assert_eq!(args, vec!["-map", "0:v", "-map", "1:a"]);
}

// ============================== FfmpegArgs::arg ==============================

#[test]
fn ffmpeg_args_single_arg() {
    let args = FfmpegArgs::new().arg("-nostdin").build();
    assert_eq!(args, vec!["-nostdin"]);
}

// ============================== Full builder chain ==============================

#[test]
fn ffmpeg_args_full_chain() {
    let args = FfmpegArgs::new()
        .input("/tmp/input.mp4")
        .input("/tmp/audio.mp3")
        .args(["-map", "0:v", "-map", "1:a"])
        .codec_copy()
        .overwrite()
        .output("/tmp/output.mkv")
        .build();

    assert_eq!(
        args,
        vec![
            "-i",
            "/tmp/input.mp4",
            "-i",
            "/tmp/audio.mp3",
            "-map",
            "0:v",
            "-map",
            "1:a",
            "-c",
            "copy",
            "-y",
            "/tmp/output.mkv"
        ]
    );
}

#[test]
fn ffmpeg_args_output_without_overwrite() {
    let args = FfmpegArgs::new()
        .input("/tmp/input.mp4")
        .output("/tmp/output.mp4")
        .build();
    assert!(!args.contains(&"-y".to_string()));
    assert_eq!(args.last().unwrap(), "/tmp/output.mp4");
}

#[test]
fn ffmpeg_args_no_output() {
    let args = FfmpegArgs::new().input("/tmp/input.mp4").codec_copy().build();
    assert!(!args.iter().any(|a| a.ends_with(".mp4") && a != "/tmp/input.mp4"));
}

#[test]
fn ffmpeg_args_with_string_owned() {
    let args = FfmpegArgs::new()
        .input(String::from("/tmp/video.mp4"))
        .output(String::from("/tmp/out.mkv"))
        .build();
    assert_eq!(args[0], "-i");
    assert_eq!(args[1], "/tmp/video.mp4");
    assert_eq!(args.last().unwrap(), "/tmp/out.mkv");
}

#[test]
fn ffmpeg_args_complex_filter() {
    let args = FfmpegArgs::new()
        .input("/tmp/input.mp4")
        .args(["-vf", "scale=1280:720,setsar=1"])
        .args(["-c:v", "libx264", "-c:a", "aac"])
        .overwrite()
        .output("/tmp/output.mp4")
        .build();

    assert!(args.contains(&"-vf".to_string()));
    assert!(args.contains(&"scale=1280:720,setsar=1".to_string()));
    assert!(args.contains(&"-c:v".to_string()));
    assert!(args.contains(&"libx264".to_string()));
}
