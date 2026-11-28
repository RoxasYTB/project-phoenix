use std::io::Read;
use std::process::{Child, Stdio};
#[cfg(unix)]
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::ffmpeg::FfmpegH264;
use super::testsrc::TestPattern;
use super::{EncodedVideo, VideoEncoder, VideoFrame};
use crate::codec::{Codec, VideoEncoderConfig};
use crate::{Error, Result};

pub const TEST_PATTERN: &str = "phoenix-test-pattern";

#[derive(Debug)]
pub enum Input {
    TestPattern,
    Device(String),
    #[cfg(windows)]
    Selected(super::desktop::Target),
    #[cfg(unix)]
    PipeWire { fd: std::os::fd::OwnedFd, node: u32 },
}

impl PartialEq for Input {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::TestPattern, Self::TestPattern) => true,
            (Self::Device(a), Self::Device(b)) => a == b,
            #[cfg(windows)]
            (Self::Selected(a), Self::Selected(b)) => a == b,
            #[cfg(unix)]
            (Self::PipeWire { node: a, .. }, Self::PipeWire { node: b, .. }) => a == b,
            _ => false,
        }
    }
}

impl Input {
    pub fn from_name(name: Option<&str>) -> Self {
        match name {
            Some(TEST_PATTERN) => Self::TestPattern,
            #[cfg(windows)]
            other => Self::Device(other.map(str::to_owned).unwrap_or_default()),
            #[cfg(not(windows))]
            Some(d) if d.starts_with("/dev/video") => Self::Device(d.to_owned()),
            #[cfg(target_os = "linux")]
            Some(label) => Self::Device(v4l2_node(std::path::Path::new("/sys/class/video4linux"), label).unwrap_or_else(|| "/dev/video0".into())),
            #[cfg(not(windows))]
            _ => Self::Device("/dev/video0".into()),
        }
    }

    fn reads_device(&self) -> bool {
        #[cfg(windows)]
        if matches!(self, Self::Selected(_)) {
            return false;
        }
        !matches!(self, Self::TestPattern)
    }
}

#[cfg(target_os = "linux")]
fn v4l2_node(sysfs: &std::path::Path, label: &str) -> Option<String> {
    std::fs::read_dir(sysfs)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let n: u32 = e.file_name().to_str()?.strip_prefix("video")?.parse().ok()?;
            let index = std::fs::read_to_string(e.path().join("index")).ok().and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(0);
            let name = std::fs::read_to_string(e.path().join("name")).ok()?;
            (index == 0 && name.trim() == label).then_some(n)
        })
        .min()
        .map(|n| format!("/dev/video{n}"))
}

#[cfg(windows)]
pub fn windows_cameras() -> Vec<String> {
    let Ok(output) = super::ffmpeg::ffmpeg_command()
        .args(["-hide_banner", "-list_devices", "true", "-f", "dshow", "-i", "dummy"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    parse_dshow_devices(&String::from_utf8_lossy(&output.stderr))
}

pub fn parse_dshow_devices(stderr: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut video_section = false;
    for line in stderr.lines() {
        let line = line.trim_end();
        if line.contains("DirectShow video devices") {
            video_section = true;
            continue;
        }
        if line.contains("DirectShow audio devices") {
            video_section = false;
            continue;
        }
        if line.contains("Alternative name") {
            continue;
        }
        let quoted = match (line.strip_suffix("\" (video)"), line.strip_suffix('"')) {
            (Some(rest), _) => rest,
            (None, Some(rest)) if video_section => rest,
            _ => continue,
        };
        let Some((_, name)) = quoted.split_once('"') else { continue };
        if !name.is_empty() && !out.iter().any(|n| n == name) {
            out.push(name.to_owned());
        }
    }
    out
}

pub type Preview = Arc<Mutex<Option<VideoFrame>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureParams {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
}

pub const KEYFRAME_MIN_INTERVAL: Duration = Duration::from_secs(1);
pub const WARMUP_KEYFRAME_AFTER: Duration = Duration::from_secs(1);

pub struct Camera {
    stop: Arc<AtomicBool>,
    keyframe: Arc<AtomicBool>,
    reconfig: Arc<Mutex<Option<CaptureParams>>>,
    thread: Option<JoinHandle<()>>,
    pub preview: Preview,
}

struct DeviceReader {
    child: Child,
    stdout: std::process::ChildStdout,
}

impl DeviceReader {
    fn open(input: &Input, width: u32, height: u32, fps: u32) -> Result<Self> {
        #[cfg(unix)]
        if let Input::PipeWire { fd, node } = input {
            return Self::pipewire(fd, *node, width, height, fps);
        }
        let mut cmd = super::ffmpeg::ffmpeg_command();
        cmd.args(["-hide_banner", "-loglevel", "error"]);
        let Input::Device(dev) = input else { return Err(Error::Encoder("invalid capture source".into())) };
        #[cfg(windows)]
        {
            let name = if dev.is_empty() { windows_cameras().into_iter().next().ok_or_else(|| Error::Encoder("no video capture device".into()))? } else { dev.clone() };
            cmd.args(["-f", "dshow", "-i", &format!("video={name}")]);
        }
        #[cfg(not(windows))]
        cmd.args(["-f", "v4l2", "-framerate", &fps.to_string(), "-i", dev]);
        let fit = format!("scale={width}:{height}:force_original_aspect_ratio=increase,crop={width}:{height}");
        #[cfg(windows)]
        let filter = format!("fps={fps},{fit}");
        #[cfg(not(windows))]
        let filter = fit;
        let mut child = cmd
            .args(["-vf", &filter])
            .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::Encoder(format!("ffmpeg capture: {e}")))?;
        let stdout = child.stdout.take().ok_or(Error::Encoder("ffmpeg capture stdout".into()))?;
        Ok(Self { child, stdout })
    }

    #[cfg(unix)]
    fn pipewire(fd: &std::os::fd::OwnedFd, node: u32, width: u32, height: u32, fps: u32) -> Result<Self> {
        let dup = fd.try_clone().map_err(Error::Io)?;
        let caps = format!("video/x-raw,format=RGBA,width={width},height={height},framerate={fps}/1,pixel-aspect-ratio=1/1");
        let mut child = Command::new("gst-launch-1.0")
            .args(["-q", "pipewiresrc", "fd=0", &format!("path={node}"), "do-timestamp=true", "keepalive-time=1000", "always-copy=true"])
            .args(["!", "videoconvert", "!", "videoscale", "add-borders=true", "!", "videorate", "drop-only=true", "!", &caps, "!", "fdsink", "fd=1", "sync=false"])
            .stdin(Stdio::from(dup))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::Encoder(format!("gst-launch pipewiresrc: {e}")))?;
        let stdout = child.stdout.take().ok_or(Error::Encoder("pipewiresrc stdout".into()))?;
        Ok(Self { child, stdout })
    }

    fn frame(&mut self, len: usize) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.stdout.read_exact(&mut buf).ok()?;
        Some(buf)
    }
}

impl Drop for DeviceReader {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Camera {
    pub fn start(
        input: Input,
        width: u32,
        height: u32,
        fps: u32,
        on_frame: impl FnMut(EncodedVideo) + Send + 'static,
    ) -> Result<Self> {
        Self::start_with(input, width, height, fps, 1_500_000, false, on_frame)
    }

    pub fn start_with(
        input: Input,
        width: u32,
        height: u32,
        fps: u32,
        bitrate: u32,
        screen: bool,
        on_frame: impl FnMut(EncodedVideo) + Send + 'static,
    ) -> Result<Self> {
        Self::start_with_factory(
            input,
            CaptureParams { width, height, fps, bitrate },
            screen,
            |cfg| Ok(Box::new(FfmpegH264::spawn(cfg)?)),
            on_frame,
        )
    }

    pub fn start_with_factory<F>(
        input: Input,
        params: CaptureParams,
        screen: bool,
        factory: F,
        mut on_frame: impl FnMut(EncodedVideo) + Send + 'static,
    ) -> Result<Self>
    where
        F: Fn(&VideoEncoderConfig) -> Result<Box<dyn VideoEncoder>> + Send + 'static,
    {
        let CaptureParams { width, height, fps, bitrate } = params;
        let stop = Arc::new(AtomicBool::new(false));
        let keyframe = Arc::new(AtomicBool::new(false));
        let reconfig: Arc<Mutex<Option<CaptureParams>>> = Arc::new(Mutex::new(None));
        let preview: Preview = Arc::new(Mutex::new(None));
        let cfg = VideoEncoderConfig::new(Codec::H264, width, height, fps, bitrate, screen);
        let mut enc = factory(&cfg)?;
        #[cfg(windows)]
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let needs_reader = input.reads_device();
        let initial_reader = if needs_reader { Some(DeviceReader::open(&input, width, height, fps)?) } else { None };
        let (s2, r2, p2, k2) = (stop.clone(), reconfig.clone(), preview.clone(), keyframe.clone());
        let thread = std::thread::Builder::new()
            .name("phoenix-camera".into())
            .spawn(move || {
                #[cfg(windows)]
                let selected = {
                    let source = match &input {
                        Input::Selected(id) => super::desktop::Capture::open(*id).map(Some),
                        _ => Ok(None),
                    };
                    match source {
                        Ok(handle) => {
                            let _ = ready_tx.send(Ok(()));
                            handle
                        }
                        Err(error) => {
                            let _ = ready_tx.send(Err(error));
                            return;
                        }
                    }
                };
                let mut current = params;
                let mut pattern = TestPattern::new(width, height, fps);
                let live = !matches!(input, Input::TestPattern);
                let mut device = initial_reader;
                let started = Instant::now();
                let mut period = Duration::from_micros(1_000_000 / u64::from(fps.max(1)));
                let mut next = Instant::now();
                let mut first = true;
                let mut last_key = Instant::now();
                let mut warmup_key = true;
                while !s2.load(Ordering::Relaxed) {
                    if let Some(target) = r2.lock().ok().and_then(|mut slot| slot.take())
                        && target != current
                    {
                        let new_cfg = VideoEncoderConfig::new(Codec::H264, target.width, target.height, target.fps, target.bitrate, screen);
                        match factory(&new_cfg) {
                            Ok(new_enc) => {
                                enc = new_enc;
                                if (target.width, target.height, target.fps) != (current.width, current.height, current.fps) {
                                    pattern = TestPattern::new(target.width, target.height, target.fps);
                                    drop(device.take());
                                    if needs_reader {
                                        device = DeviceReader::open(&input, target.width, target.height, target.fps).ok();
                                    }
                                    period = Duration::from_micros(1_000_000 / u64::from(target.fps.max(1)));
                                }
                                current = target;
                                first = true;
                                warmup_key = true;
                            }
                            Err(e) => tracing::warn!(error = %e, "camera: reconfigure failed"),
                        }
                    }
                    let frame = match (live, device.as_mut()) {
                        (true, Some(d)) => match d.frame((current.width * current.height * 4) as usize) {
                            Some(data) => VideoFrame { width: current.width, height: current.height, pts_us: started.elapsed().as_micros() as u64, data: Arc::from(data) },
                            None => break,
                        },
                        #[cfg(windows)]
                        (true, None) if selected.is_some() => {
                            let Some(data) = selected.as_ref().and_then(|source| source.frame(current.width, current.height)) else {
                                std::thread::sleep(period);
                                continue;
                            };
                            VideoFrame { width: current.width, height: current.height, pts_us: started.elapsed().as_micros() as u64, data: Arc::from(data) }
                        }
                        (true, None) => break,
                        (false, _) => pattern.frame(),
                    };
                    if let Ok(mut p) = p2.lock() {
                        *p = Some(frame.clone());
                    }
                    let warmup = warmup_key && !first && last_key.elapsed() >= WARMUP_KEYFRAME_AFTER;
                    let force = first || warmup || (k2.load(Ordering::Relaxed) && last_key.elapsed() >= KEYFRAME_MIN_INTERVAL);
                    if warmup {
                        warmup_key = false;
                    }
                    if force {
                        k2.store(false, Ordering::Relaxed);
                        last_key = Instant::now();
                    }
                    match enc.encode(&frame, force) {
                        Ok(Some(e)) => on_frame(e),
                        Ok(None) => {}
                        Err(e) => {
                            tracing::warn!(error = %e, "camera: encoder failed");
                            break;
                        }
                    }
                    first = false;
                    if !needs_reader {
                        next += period;
                        let now = Instant::now();
                        if next > now {
                            std::thread::sleep(next - now);
                        } else {
                            next = now;
                        }
                    }
                }
            })
            .map_err(Error::Io)?;
        #[cfg(windows)]
        ready_rx.recv().map_err(|e| Error::Encoder(e.to_string()))??;
        Ok(Self { stop, keyframe, reconfig, thread: Some(thread), preview })
    }

    pub fn reconfigure(&self, width: u32, height: u32, fps: u32, bitrate: u32) {
        if let Ok(mut slot) = self.reconfig.lock() {
            *slot = Some(CaptureParams { width, height, fps, bitrate });
        }
    }

    pub fn keyframe_flag(&self) -> Arc<AtomicBool> {
        self.keyframe.clone()
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_names() {
        assert_eq!(Input::from_name(Some(TEST_PATTERN)), Input::TestPattern);
        assert_eq!(Input::from_name(Some("/dev/video2")), Input::Device("/dev/video2".into()));
        #[cfg(not(windows))]
        assert_eq!(Input::from_name(None), Input::Device("/dev/video0".into()));
        #[cfg(windows)]
        {
            assert_eq!(Input::from_name(None), Input::Device(String::new()));
            assert_eq!(Input::from_name(Some("USB Camera")), Input::Device("USB Camera".into()));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn v4l2_names_map_to_their_capture_node() {
        let root = std::env::temp_dir().join(format!("phoenix-v4l2-{}", std::process::id()));
        for (node, name, index) in [("video0", "Integrated Camera", 0), ("video1", "Integrated Camera", 1), ("video2", "USB Camera", 0), ("video3", "USB Camera", 1)] {
            let dir = root.join(node);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("name"), format!("{name}\n")).unwrap();
            std::fs::write(dir.join("index"), format!("{index}\n")).unwrap();
        }
        assert_eq!(v4l2_node(&root, "USB Camera").as_deref(), Some("/dev/video2"));
        assert_eq!(v4l2_node(&root, "Integrated Camera").as_deref(), Some("/dev/video0"));
        assert_eq!(v4l2_node(&root, "Missing"), None);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn dshow_device_lists_keep_video_inputs_only() {
        let modern = "[dshow @ 000001] \"Integrated Camera\" (video)\n[dshow @ 000001]   Alternative name \"@device_pnp_\\\\?\\usb\"\n[dshow @ 000001] \"Microphone (Realtek)\" (audio)\n[dshow @ 000001] \"OBS Virtual Camera\" (video)\n";
        assert_eq!(parse_dshow_devices(modern), vec!["Integrated Camera", "OBS Virtual Camera"]);
        let legacy = "[dshow @ 01] DirectShow video devices (some may be both video and audio devices)\n[dshow @ 01]  \"USB2.0 HD UVC WebCam\"\n[dshow @ 01]     Alternative name \"@device_pnp_x\"\n[dshow @ 01] DirectShow audio devices\n[dshow @ 01]  \"Microphone\"\n";
        assert_eq!(parse_dshow_devices(legacy), vec!["USB2.0 HD UVC WebCam"]);
    }

    #[test]
    fn hot_reconfigure_recreates_encoder_and_forces_keyframe() {
        struct FakeEncoder {
            log: Arc<Mutex<Vec<(u32, u32, bool)>>>,
        }
        impl VideoEncoder for FakeEncoder {
            fn codec(&self) -> Codec {
                Codec::H264
            }
            fn encode(&mut self, frame: &VideoFrame, force_keyframe: bool) -> Result<Option<EncodedVideo>> {
                self.log.lock().unwrap().push((frame.width, frame.height, force_keyframe));
                Ok(Some(EncodedVideo { codec: Codec::H264, keyframe: force_keyframe, data: vec![0, 0, 0, 1, 0x65], pts_us: frame.pts_us }))
            }
            fn set_bitrate(&mut self, _bps: u32) -> Result<()> {
                Ok(())
            }
        }

        type Log<T> = Arc<Mutex<Vec<T>>>;
        let configs: Log<(u32, u32, u32, u32)> = Arc::new(Mutex::new(Vec::new()));
        let frames: Arc<Mutex<Vec<(u32, u32, bool)>>> = Arc::new(Mutex::new(Vec::new()));
        let (c2, f2) = (configs.clone(), frames.clone());
        let cam = Camera::start_with_factory(
            Input::TestPattern,
            CaptureParams { width: 640, height: 360, fps: 60, bitrate: 1_500_000 },
            true,
            move |cfg| {
                c2.lock().unwrap().push((cfg.width, cfg.height, cfg.fps, cfg.bitrate_bps));
                Ok(Box::new(FakeEncoder { log: f2.clone() }))
            },
            |_| {},
        )
        .unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        while frames.lock().unwrap().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!frames.lock().unwrap().is_empty());

        cam.reconfigure(1280, 720, 60, 4_500_000);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !frames.lock().unwrap().iter().any(|&(w, h, _)| (w, h) == (1280, 720)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }

        cam.reconfigure(1280, 720, 60, 4_500_000);
        std::thread::sleep(Duration::from_millis(40));
        cam.stop();

        let cfgs = configs.lock().unwrap().clone();
        assert_eq!(cfgs, vec![(640, 360, 60, 1_500_000), (1280, 720, 60, 4_500_000)]);
        let fl = frames.lock().unwrap();
        let first_new = fl.iter().find(|&&(w, h, _)| (w, h) == (1280, 720)).copied();
        assert_eq!(first_new, Some((1280, 720, true)));
    }

    #[test]
    fn keyframe_requests_force_a_throttled_keyframe() {
        struct Keys(Arc<Mutex<Vec<bool>>>);
        impl VideoEncoder for Keys {
            fn codec(&self) -> Codec {
                Codec::H264
            }
            fn encode(&mut self, frame: &VideoFrame, force_keyframe: bool) -> Result<Option<EncodedVideo>> {
                self.0.lock().unwrap().push(force_keyframe);
                Ok(Some(EncodedVideo { codec: Codec::H264, keyframe: force_keyframe, data: vec![0, 0, 0, 1, 0x65], pts_us: frame.pts_us }))
            }
            fn set_bitrate(&mut self, _bps: u32) -> Result<()> {
                Ok(())
            }
        }
        let log = Arc::new(Mutex::new(Vec::new()));
        let l2 = log.clone();
        let cam = Camera::start_with_factory(
            Input::TestPattern,
            CaptureParams { width: 64, height: 36, fps: 50, bitrate: 500_000 },
            true,
            move |_| Ok(Box::new(Keys(l2.clone()))),
            |_| {},
        )
        .unwrap();
        let flag = cam.keyframe_flag();
        std::thread::sleep(Duration::from_millis(200));
        flag.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(200));
        assert!(flag.load(Ordering::Relaxed), "held back until the minimum interval passes");
        let deadline = Instant::now() + KEYFRAME_MIN_INTERVAL + Duration::from_secs(2);
        while flag.load(Ordering::Relaxed) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(100));
        cam.stop();
        let keys = log.lock().unwrap().clone();
        assert!(!flag.load(Ordering::Relaxed));
        assert!(keys[0]);
        assert_eq!(keys.iter().filter(|k| **k).count(), 2);
    }

    #[test]
    fn a_second_keyframe_follows_the_start_without_any_request() {
        struct Keys(Arc<Mutex<Vec<bool>>>);
        impl VideoEncoder for Keys {
            fn codec(&self) -> Codec {
                Codec::H264
            }
            fn encode(&mut self, frame: &VideoFrame, force_keyframe: bool) -> Result<Option<EncodedVideo>> {
                self.0.lock().unwrap().push(force_keyframe);
                Ok(Some(EncodedVideo { codec: Codec::H264, keyframe: force_keyframe, data: vec![0, 0, 0, 1, 0x65], pts_us: frame.pts_us }))
            }
            fn set_bitrate(&mut self, _bps: u32) -> Result<()> {
                Ok(())
            }
        }
        let log = Arc::new(Mutex::new(Vec::new()));
        let l2 = log.clone();
        let cam = Camera::start_with_factory(
            Input::TestPattern,
            CaptureParams { width: 64, height: 36, fps: 50, bitrate: 500_000 },
            true,
            move |_| Ok(Box::new(Keys(l2.clone()))),
            |_| {},
        )
        .unwrap();
        std::thread::sleep(WARMUP_KEYFRAME_AFTER + Duration::from_millis(600));
        cam.stop();
        let keys = log.lock().unwrap().clone();
        assert!(keys[0]);
        assert_eq!(keys.iter().filter(|k| **k).count(), 2);
    }
}
