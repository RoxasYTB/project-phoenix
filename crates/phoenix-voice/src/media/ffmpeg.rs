use std::collections::VecDeque;
use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;

use super::{EncodedVideo, VideoEncoder, VideoFrame};
use crate::codec::{Codec, H264Profile, VideoEncoderConfig};
use crate::rtp::h264;
use crate::{Error, Result};

pub fn ffmpeg_command() -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new("ffmpeg");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

pub fn available() -> bool {
    ffmpeg_command().arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

pub fn split_access_units(buf: &mut Vec<u8>, out: &mut Vec<Vec<u8>>) {
    let aud = [0u8, 0, 0, 1, 9];
    let mut starts = Vec::new();
    let mut i = 0;
    while i + aud.len() <= buf.len() {
        if buf[i..i + aud.len()] == aud {
            starts.push(i);
            i += aud.len();
        } else {
            i += 1;
        }
    }
    if starts.len() < 2 {
        return;
    }
    for w in starts.windows(2) {
        out.push(buf[w[0]..w[1]].to_vec());
    }
    let last = *starts.last().unwrap_or(&0);
    buf.drain(..last);
}

pub struct FfmpegH264 {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: mpsc::Receiver<Vec<u8>>,
    config: VideoEncoderConfig,
    frames: u64,
    pending: AccessUnitQueue,
}

#[derive(Default)]
pub struct AccessUnitQueue {
    pts: VecDeque<u64>,
    units: VecDeque<Vec<u8>>,
}

impl AccessUnitQueue {
    pub fn submitted(&mut self, pts_us: u64) {
        self.pts.push_back(pts_us);
    }

    pub fn received(&mut self, au: Vec<u8>) {
        self.units.push_back(au);
    }

    pub fn next(&mut self, fallback_pts: u64) -> Option<EncodedVideo> {
        let data = self.units.pop_front()?;
        let pts_us = self.pts.pop_front().unwrap_or(fallback_pts);
        Some(EncodedVideo { codec: Codec::H264, keyframe: h264::is_keyframe(&data), data, pts_us })
    }
}

impl FfmpegH264 {
    pub fn spawn(config: &VideoEncoderConfig) -> Result<Self> {
        let kbps = (config.bitrate_bps / 1000).max(100);
        let profile = match config.h264_profile.unwrap_or(H264Profile::ConstrainedBaseline) {
            H264Profile::ConstrainedBaseline => "baseline",
            H264Profile::Main => "main",
        };
        let mut child = ffmpeg_command()
            .args(["-hide_banner", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s"])
            .arg(format!("{}x{}", config.width, config.height))
            .args(["-r", &config.fps.to_string(), "-i", "-", "-c:v", "libx264", "-preset", "ultrafast", "-tune", "zerolatency", "-profile:v", profile])
            .args(["-pix_fmt", "yuv420p", "-bf", "0", "-g", &config.keyframe_interval.max(1).to_string()])
            .args(["-b:v", &format!("{kbps}k"), "-maxrate", &format!("{kbps}k"), "-bufsize", &format!("{}k", kbps * 2)])
            .args(["-x264-params", "aud=1:repeat-headers=1:sliced-threads=0", "-f", "h264", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::Encoder(format!("ffmpeg: {e}")))?;
        let stdin = child.stdin.take();
        let mut stdout = child.stdout.take().ok_or(Error::Encoder("ffmpeg stdout".into()))?;
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("phoenix-h264-out".into())
            .spawn(move || {
                let mut buf = Vec::new();
                let mut chunk = vec![0u8; 64 * 1024];
                let mut aus = Vec::new();
                while let Ok(n) = stdout.read(&mut chunk) {
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    split_access_units(&mut buf, &mut aus);
                    for au in aus.drain(..) {
                        if tx.send(au).is_err() {
                            return;
                        }
                    }
                }
                if !buf.is_empty() {
                    let _ = tx.send(buf);
                }
            })
            .map_err(Error::Io)?;
        Ok(Self { child, stdin, rx, config: config.clone(), frames: 0, pending: AccessUnitQueue::default() })
    }
}

impl Drop for FfmpegH264 {
    fn drop(&mut self) {
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl VideoEncoder for FfmpegH264 {
    fn codec(&self) -> Codec {
        Codec::H264
    }

    fn encode(&mut self, frame: &VideoFrame, force_keyframe: bool) -> Result<Option<EncodedVideo>> {
        if frame.width != self.config.width || frame.height != self.config.height {
            return Err(Error::Encoder("frame size changed".into()));
        }
        if force_keyframe && self.frames > 0 {
            *self = Self::spawn(&self.config)?;
        }
        let stdin = self.stdin.as_mut().ok_or(Error::Encoder("ffmpeg closed".into()))?;
        stdin.write_all(&frame.data).map_err(|e| Error::Encoder(format!("ffmpeg write: {e}")))?;
        self.frames += 1;
        self.pending.submitted(frame.pts_us);
        while let Ok(au) = self.rx.try_recv() {
            self.pending.received(au);
        }
        Ok(self.pending.next(frame.pts_us))
    }

    fn set_bitrate(&mut self, bps: u32) -> Result<()> {
        self.config.bitrate_bps = bps;
        Ok(())
    }
}

pub const CAMERA_MAX: (u32, u32) = (1280, 720);
pub const STREAM_MAX: (u32, u32) = (1920, 1080);

#[cfg(target_os = "macos")]
const PLATFORM_HWACCEL: Option<&str> = Some("videotoolbox");
#[cfg(target_os = "windows")]
const PLATFORM_HWACCEL: Option<&str> = Some("d3d11va");
#[cfg(target_os = "linux")]
const PLATFORM_HWACCEL: Option<&str> = Some("auto");
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
const PLATFORM_HWACCEL: Option<&str> = None;

pub fn hwaccel() -> Option<String> {
    match std::env::var("PHOENIX_VIDEO_HWACCEL") {
        Ok(v) if v.is_empty() || v == "none" || v == "0" => None,
        Ok(v) => Some(v),
        Err(_) => PLATFORM_HWACCEL.map(str::to_string),
    }
}

pub fn decoder_hwaccel(codec: Codec) -> Option<String> {
    if codec == Codec::Av1 && std::env::var_os("PHOENIX_VIDEO_HWACCEL").is_none() {
        return None;
    }
    hwaccel()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DecoderSupport {
    pub codecs: Vec<Codec>,
    pub dav1d: bool,
}

pub fn parse_decoder_list(out: &str) -> DecoderSupport {
    let mut d = DecoderSupport::default();
    for line in out.lines() {
        let mut parts = line.split_whitespace();
        let (Some(flags), Some(name)) = (parts.next(), parts.next()) else { continue };
        if !flags.starts_with('V') {
            continue;
        }
        let codec = match name {
            "h264" => Some(Codec::H264),
            "vp8" | "libvpx" => Some(Codec::Vp8),
            "av1" | "libdav1d" | "libaom-av1" => Some(Codec::Av1),
            _ => None,
        };
        if name == "libdav1d" {
            d.dav1d = true;
        }
        if let Some(c) = codec
            && !d.codecs.contains(&c)
        {
            d.codecs.push(c);
        }
    }
    d.codecs.sort_by_key(|c| match c {
        Codec::Av1 => 0,
        Codec::H264 => 1,
        _ => 2,
    });
    d
}

pub fn decoder_support() -> &'static DecoderSupport {
    static S: std::sync::OnceLock<DecoderSupport> = std::sync::OnceLock::new();
    S.get_or_init(|| {
        ffmpeg_command()
            .args(["-hide_banner", "-decoders"])
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| parse_decoder_list(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default()
    })
}

pub fn ivf_fourcc(codec: Codec) -> Option<&'static [u8; 4]> {
    match codec {
        Codec::H264 => Some(b"H264"),
        Codec::Vp8 => Some(b"VP80"),
        Codec::Av1 => Some(b"AV01"),
        _ => None,
    }
}

pub fn ivf_header(codec: Codec, width: u16, height: u16) -> Option<[u8; 32]> {
    let mut h = [0u8; 32];
    h[0..4].copy_from_slice(b"DKIF");
    h[6..8].copy_from_slice(&32u16.to_le_bytes());
    h[8..12].copy_from_slice(ivf_fourcc(codec)?);
    h[12..14].copy_from_slice(&width.to_le_bytes());
    h[14..16].copy_from_slice(&height.to_le_bytes());
    h[16..20].copy_from_slice(&90_000u32.to_le_bytes());
    h[20..24].copy_from_slice(&1u32.to_le_bytes());
    Some(h)
}

pub fn ivf_frame(codec: Codec, frame: &[u8], pts: u64) -> Result<Vec<u8>> {
    let body = match codec {
        Codec::Av1 => {
            let mut v = vec![(crate::rtp::av1::OBU_TEMPORAL_DELIMITER << 3) | 0x02, 0];
            v.extend_from_slice(&crate::rtp::av1::with_obu_sizes(frame)?);
            std::borrow::Cow::Owned(v)
        }
        _ => std::borrow::Cow::Borrowed(frame),
    };
    let mut out = Vec::with_capacity(body.len() + 12);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&pts.to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn decode_args(codec: Codec, max: (u32, u32), hwaccel: Option<&str>, dav1d: bool) -> Vec<String> {
    let mut a: Vec<String> = ["-hide_banner", "-loglevel", "error"].map(String::from).to_vec();
    let software_av1 = codec == Codec::Av1 && hwaccel.is_none();
    let max = if software_av1 { (max.0.min(1280), max.1.min(720)) } else { max };
    if let Some(h) = hwaccel {
        a.extend(["-hwaccel".into(), h.into()]);
    }
    let threads = if software_av1 { "6" } else { "2" };
    a.extend(["-filter_threads", "1", "-threads", threads, "-probesize", "32", "-analyzeduration", "0", "-flags", "low_delay"].map(String::from));
    if codec == Codec::Av1 {
        if hwaccel.is_some() {
            a.extend(["-c:v", "av1"].map(String::from));
        } else if dav1d {
            a.extend(["-c:v", "libdav1d", "-max_frame_delay", "3"].map(String::from));
        }
    }
    a.extend(["-f", "ivf", "-i", "-", "-vf"].map(String::from));
    a.push(format!(
        "scale=w='min(iw,{})':h='min(ih,{})':force_original_aspect_ratio=decrease:force_divisible_by=2:flags=fast_bilinear,format=bgra,vflip",
        max.0, max.1
    ));
    a.extend(["-fps_mode", "passthrough", "-flush_packets", "1", "-f", "image2pipe", "-c:v", "bmp", "-threads", "1", "-pix_fmt", "bgra", "-"].map(String::from));
    a
}

fn read_video_bitmap(input: &mut impl Read) -> Option<(u32, u32, Vec<u8>)> {
    let mut header = [0; 54];
    input.read_exact(&mut header).ok()?;
    if &header[..2] != b"BM" || header[26..34] != [1, 0, 32, 0, 0, 0, 0, 0] {
        return None;
    }
    let field = |i| u32::from_le_bytes(header[i..i + 4].try_into().unwrap());
    let (width, height) = (field(18), field(22));
    if !(1..=8192).contains(&width) || !(1..=8192).contains(&height) {
        return None;
    }
    let bytes = width as usize * height as usize * 4;
    if field(10) != 54 || field(2) as usize != 54 + bytes {
        return None;
    }
    let mut pixels = vec![0; bytes];
    input.read_exact(&mut pixels).ok()?;
    Some((width, height, pixels))
}

pub fn read_pam(r: &mut impl std::io::BufRead) -> Option<(u32, u32, Vec<u8>)> {
    let (mut w, mut h, mut depth) = (0u32, 0u32, 0u32);
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let mut it = line.split_whitespace();
        match (it.next(), it.next()) {
            (Some("WIDTH"), Some(v)) => w = v.parse().ok()?,
            (Some("HEIGHT"), Some(v)) => h = v.parse().ok()?,
            (Some("DEPTH"), Some(v)) => depth = v.parse().ok()?,
            (Some("ENDHDR"), _) => break,
            _ => {}
        }
    }
    if depth != 4 || w == 0 || h == 0 || w > 8192 || h > 8192 {
        return None;
    }
    let mut buf = vec![0u8; (w * h * 4) as usize];
    r.read_exact(&mut buf).ok()?;
    for px in buf.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    Some((w, h, buf))
}

fn encode_lavfi(args: &[&str]) -> Option<Vec<u8>> {
    let o = ffmpeg_command()
        .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    (o.status.success() && !o.stdout.is_empty()).then_some(o.stdout)
}

pub fn ivf_payloads(data: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut o = 32;
    while o + 12 <= data.len() {
        let n = u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]) as usize;
        let Some(f) = data.get(o + 12..o + 12 + n) else { break };
        out.push(f.to_vec());
        o += 12 + n;
    }
    out
}

pub fn synthetic_frames(codec: Codec, source: &str, frames: u32) -> Option<Vec<Vec<u8>>> {
    let n = frames.to_string();
    let n = n.as_str();
    match codec {
        Codec::H264 => {
            let mut buf = encode_lavfi(&[source, "-frames:v", n, "-c:v", "libx264", "-bf", "0", "-pix_fmt", "yuv420p", "-x264-params", "aud=1", "-f", "h264", "-"])?;
            buf.extend_from_slice(&[0, 0, 0, 1, 9, 0xf0]);
            let mut out = Vec::new();
            split_access_units(&mut buf, &mut out);
            Some(out)
        }
        Codec::Vp8 => encode_lavfi(&[source, "-frames:v", n, "-c:v", "libvpx", "-deadline", "realtime", "-f", "ivf", "-"]).map(|d| ivf_payloads(&d)),
        Codec::Av1 => encode_lavfi(&[source, "-frames:v", n, "-c:v", "libaom-av1", "-cpu-used", "8", "-usage", "realtime", "-f", "ivf", "-"])
            .or_else(|| encode_lavfi(&[source, "-frames:v", n, "-c:v", "libsvtav1", "-preset", "12", "-f", "ivf", "-"]))
            .map(|d| ivf_payloads(&d)),
        _ => None,
    }
    .filter(|f| !f.is_empty())
}

static FRAME_SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub struct FfmpegDecoder {
    child: Child,
    input: mpsc::SyncSender<Vec<u8>>,
    codec: Codec,
    pts: u64,
    hwaccel: bool,
    pub latest: std::sync::Arc<std::sync::Mutex<Option<VideoFrame>>>,
    decoded: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl FfmpegDecoder {
    pub fn spawn(codec: Codec, max: (u32, u32)) -> Result<Self> {
        Self::spawn_with(codec, max, decoder_hwaccel(codec).as_deref())
    }

    pub fn spawn_with(codec: Codec, max: (u32, u32), hwaccel: Option<&str>) -> Result<Self> {
        let header = ivf_header(codec, max.0 as u16, max.1 as u16).ok_or_else(|| Error::Unsupported(format!("no {} decoder", codec.name())))?;
        let mut child = ffmpeg_command()
            .args(decode_args(codec, max, hwaccel, decoder_support().dav1d))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::Encoder(format!("ffmpeg decoder: {e}")))?;
        let mut stdin = child.stdin.take().ok_or(Error::Encoder("ffmpeg decoder stdin".into()))?;
        let stdout = child.stdout.take().ok_or(Error::Encoder("ffmpeg decoder stdout".into()))?;
        let capacity = if codec == Codec::Av1 { 24 } else { 12 };
        let (input, pending) = mpsc::sync_channel::<Vec<u8>>(capacity);
        let writer = std::thread::Builder::new().name("phoenix-video-out".into()).spawn(move || {
            if stdin.write_all(&header).is_ok() {
                for packet in pending {
                    if stdin.write_all(&packet).is_err() {
                        break;
                    }
                }
            }
        });
        if let Err(e) = writer {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Io(e));
        }
        let latest: std::sync::Arc<std::sync::Mutex<Option<VideoFrame>>> = Default::default();
        let decoded: std::sync::Arc<std::sync::atomic::AtomicU64> = Default::default();
        let (slot, count) = (latest.clone(), decoded.clone());
        let reader = std::thread::Builder::new()
            .name("phoenix-video-in".into())
            .spawn(move || {
                let mut r = std::io::BufReader::with_capacity(1 << 20, stdout);
                while let Some((w, h, buf)) = read_video_bitmap(&mut r) {
                    let serial = FRAME_SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let frame = VideoFrame { width: w, height: h, pts_us: serial, data: std::sync::Arc::from(buf) };
                    if let Ok(mut s) = slot.lock() {
                        let previous = s.replace(frame);
                        drop(s);
                        drop(previous);
                    }
                    count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            });
        if let Err(e) = reader {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Io(e));
        }
        Ok(Self { child, input, codec, pts: 0, hwaccel: hwaccel.is_some(), latest, decoded })
    }

    pub fn codec(&self) -> Codec {
        self.codec
    }

    pub fn hwaccel(&self) -> bool {
        self.hwaccel
    }

    pub fn decoded(&self) -> u64 {
        self.decoded.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn pushed(&self) -> u64 {
        self.pts
    }

    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn push(&mut self, frame: &[u8]) -> Result<()> {
        let packet = ivf_frame(self.codec, frame, self.pts)?;
        self.input.send(packet).map_err(|_| Error::Encoder("ffmpeg decoder closed".into()))?;
        self.pts += 1;
        Ok(())
    }

    pub fn try_push(&mut self, frame: &[u8]) -> Result<()> {
        let packet = ivf_frame(self.codec, frame, self.pts)?;
        self.input.try_send(packet).map_err(|e| Error::Encoder(format!("ffmpeg decoder input: {e}")))?;
        self.pts += 1;
        Ok(())
    }
}

impl Drop for FfmpegDecoder {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_access_unit_delimiters() {
        let mut buf = vec![0, 0, 0, 1, 9, 0x10, 0, 0, 0, 1, 0x65, 1, 2, 0, 0, 0, 1, 9, 0x10, 0, 0, 0, 1, 0x41, 3];
        let mut out = Vec::new();
        split_access_units(&mut buf, &mut out);
        assert_eq!(out.len(), 1);
        assert!(h264::is_keyframe(&out[0]));
        assert_eq!(buf[..5], [0, 0, 0, 1, 9]);
    }

    #[test]
    fn decoder_list_and_arguments() {
        let list = " V....D libdav1d             dav1d AV1 decoder\n VFS..D h264                 H.264\n A....D opus  Opus\n VFS..D vp8                  On2 VP8\n V....D av1                  AV1\n";
        let d = parse_decoder_list(list);
        assert_eq!(d.codecs, vec![Codec::Av1, Codec::H264, Codec::Vp8]);
        assert!(d.dav1d);
        assert!(parse_decoder_list(" A....D opus x\n").codecs.is_empty());
        let a = decode_args(Codec::Av1, (1280, 720), Some("auto"), true);
        let pos = |k: &str| a.iter().position(|x| x == k).unwrap();
        assert!(pos("-hwaccel") < pos("-i"));
        assert!(pos("av1") < pos("-i"));
        assert!(!a.iter().any(|x| x == "libdav1d"));
        let software = decode_args(Codec::Av1, (1280, 720), None, true);
        assert!(software.windows(2).any(|pair| pair == ["-c:v", "libdav1d"]));
        assert!(software.windows(2).any(|pair| pair == ["-threads", "6"]));
        assert_eq!(a[pos("-f") + 1], "ivf");
        assert!(a.iter().any(|x| x.contains("min(iw,1280)") && x.contains("force_original_aspect_ratio=decrease")));
        assert!(!decode_args(Codec::Vp8, (640, 360), None, true).iter().any(|x| x == "-hwaccel" || x == "libdav1d"));
    }

    #[test]
    fn ivf_framing() {
        let h = ivf_header(Codec::Vp8, 640, 360).unwrap();
        assert_eq!(&h[0..4], b"DKIF");
        assert_eq!(&h[8..12], b"VP80");
        assert_eq!(u16::from_le_bytes([h[12], h[13]]), 640);
        assert!(ivf_header(Codec::Vp9, 1, 1).is_none());
        let f = ivf_frame(Codec::Vp8, &[1, 2, 3], 7).unwrap();
        assert_eq!(f, [3, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3]);
        let bare_last_obu = [(6 << 3), 9, 9];
        let a = ivf_frame(Codec::Av1, &bare_last_obu, 0).unwrap();
        assert_eq!(&a[12..], &[0x12, 0, (6 << 3) | 2, 2, 9, 9]);
    }

    #[test]
    #[ignore]
    fn av1_decoder_keeps_up_with_a_full_hd_source_at_sixty_fps() {
        use std::time::{Duration, Instant};

        if !available() || !decoder_support().dav1d {
            return;
        }
        let Some(frames) = synthetic_frames(Codec::Av1, "testsrc2=s=1920x1080:r=60", 90) else { return };
        assert_eq!(frames.len(), 90);
        let mut decoder = FfmpegDecoder::spawn_with(Codec::Av1, STREAM_MAX, None).unwrap();
        let start = Instant::now();
        for (index, frame) in frames.iter().enumerate() {
            let due = start + Duration::from_nanos(index as u64 * 1_000_000_000 / 60);
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
            decoder.try_push(frame).unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while decoder.decoded() < 85 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(decoder.decoded() >= 85, "only {} AV1 frames decoded", decoder.decoded());
        let latest = decoder.latest.lock().unwrap().clone().unwrap();
        assert_eq!((latest.width, latest.height), CAMERA_MAX);
    }

    #[test]
    fn pam_frames_become_bgra() {
        let mut data = b"P7\nWIDTH 2\nHEIGHT 1\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n".to_vec();
        data.extend_from_slice(&[10, 20, 30, 255, 40, 50, 60, 255]);
        let mut r = std::io::Cursor::new(data);
        assert_eq!(read_pam(&mut r), Some((2, 1, vec![30, 20, 10, 255, 60, 50, 40, 255])));
        assert_eq!(read_pam(&mut r), None);
    }

    fn decode_through_rtp(codec: Codec, frames: &[Vec<u8>], max: (u32, u32)) -> Option<VideoFrame> {
        use crate::rtp::{Packetizer, RtpHeader};
        use crate::video_rx::{Layer, VideoReceiver};
        let mut rx = VideoReceiver::default();
        rx.set_layers(1, vec![Layer { ssrc: 9, rtx_ssrc: 0, quality: 100 }]);
        let now = std::time::Instant::now();
        let mut seq = 0u16;
        let mut out = Vec::new();
        rx.push(&RtpHeader::new(codec.payload_type(), seq, 0, 9, true), &[0x10, 0, 0, 0, 0x65], now);
        for (i, f) in frames.iter().enumerate() {
            let pl = match codec {
                Codec::H264 => crate::rtp::h264::H264Packetizer::new().packetize(f, 1100),
                Codec::Vp8 => crate::rtp::vp8::Vp8Packetizer::default().packetize(f, 1100),
                _ => crate::rtp::av1::Av1Packetizer.packetize(f, 1100),
            }
            .ok()?;
            let n = pl.len();
            for (k, p) in pl.into_iter().enumerate() {
                seq = seq.wrapping_add(1);
                let h = RtpHeader::new(codec.payload_type(), seq, (i as u32 + 1) * 9000, 9, k + 1 == n);
                out.extend(rx.push(&h, &p, now));
            }
        }
        assert_eq!(out.len(), frames.len());
        assert!(crate::video_rx::is_keyframe(codec, &out[0].data));
        let mut dec = FfmpegDecoder::spawn_with(codec, max, None).ok()?;
        for f in &out {
            assert!(rx.accept(f, crate::video_rx::is_keyframe(codec, &f.data)));
            dec.push(&f.data).ok()?;
        }
        let t = std::time::Instant::now();
        while dec.decoded() < out.len() as u64 && t.elapsed() < std::time::Duration::from_secs(20) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(dec.pushed(), out.len() as u64);
        dec.latest.lock().ok()?.clone()
    }

    #[test]
    fn decodes_generated_h264_vp8_and_av1_after_rtp() {
        if !available() {
            return;
        }
        for codec in [Codec::H264, Codec::Vp8, Codec::Av1] {
            if !decoder_support().codecs.contains(&codec) {
                continue;
            }
            let Some(frames) = synthetic_frames(codec, "color=c=red:s=160x90:r=10", 3) else { continue };
            let f = decode_through_rtp(codec, &frames, CAMERA_MAX).unwrap_or_else(|| panic!("{} did not decode", codec.name()));
            assert_eq!((f.width, f.height), (160, 90), "{}", codec.name());
            let px = &f.data[(45 * 160 + 80) * 4..][..4];
            assert!(px[2] > 200 && px[1] < 60 && px[0] < 60, "{} centre pixel {px:?} is not red in BGRA", codec.name());
        }
    }

    #[test]
    fn portrait_video_keeps_its_ratio() {
        if !available() || !decoder_support().codecs.contains(&Codec::Vp8) {
            return;
        }
        let Some(frames) = synthetic_frames(Codec::Vp8, "color=c=red:s=360x640:r=10", 3) else { return };
        let f = decode_through_rtp(Codec::Vp8, &frames, (320, 180)).expect("vp8 decode");
        assert_eq!(f.height, 180);
        assert!((100..=102).contains(&f.width), "{}x{}", f.width, f.height);
    }

    #[test]
    fn access_units_leave_one_at_a_time_with_their_own_timestamp() {
        let mut q = AccessUnitQueue::default();
        let idr = vec![0, 0, 0, 1, 9, 0x10, 0, 0, 0, 1, 0x67, 0x42, 0, 0, 0, 1, 0x68, 0xce, 0, 0, 0, 1, 0x65, 0x88];
        let p1 = vec![0, 0, 0, 1, 9, 0x30, 0, 0, 0, 1, 0x41, 0x9a];
        let p2 = vec![0, 0, 0, 1, 9, 0x30, 0, 0, 0, 1, 0x41, 0x9b];
        q.submitted(0);
        assert!(q.next(0).is_none());
        q.submitted(33_000);
        q.received(idr.clone());
        q.submitted(66_000);
        q.received(p1.clone());
        q.received(p2.clone());
        let a = q.next(66_000).unwrap();
        assert!(a.keyframe);
        assert_eq!((a.data, a.pts_us), (idr, 0));
        let b = q.next(66_000).unwrap();
        assert!(!b.keyframe);
        assert_eq!((b.data, b.pts_us), (p1, 33_000));
        let c = q.next(99_000).unwrap();
        assert_eq!((c.data, c.pts_us), (p2, 66_000));
        assert!(q.next(99_000).is_none());
    }

    #[test]
    #[ignore]
    fn encodes_a_test_pattern() {
        if !available() {
            return;
        }
        let cfg = VideoEncoderConfig::new(Codec::H264, 320, 180, 30, 500_000, false);
        let mut enc = FfmpegH264::spawn(&cfg).unwrap();
        let mut src = super::super::testsrc::TestPattern::new(320, 180, 30);
        let mut got = 0;
        for _ in 0..60 {
            let f = src.frame();
            if enc.encode(&f, false).unwrap().is_some() {
                got += 1;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(got > 0);
    }
}
