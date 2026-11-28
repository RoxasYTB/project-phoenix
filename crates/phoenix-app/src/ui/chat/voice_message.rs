use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine as _;
use gpui::{
    AnyElement, App, Bounds, Context, DispatchPhase, FocusHandle, FontWeight, Hitbox, HitboxBehavior, Hsla, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, SharedString, StatefulInteractiveElement,
    Styled, WeakEntity, Window, div, prelude::*, px, svg,
};
use phoenix_voice::audio::effects::{self, Handle, Pcm};
use phoenix_voice::audio::voice_message::{self as vm, Recorder, Recording, Source};

use crate::app::PhoenixApp;
use crate::model::{Id, VoiceMeta};
use crate::theme::{c, generated::idx, tok, z};
use crate::ui::states::{cubic_bezier, mix};

enum Phase {
    Requesting,
    Recording(Recorder),
    Finalizing,
    Recorded(Arc<Recording>),
}

struct Draft {
    channel: Id,
    phase: Phase,
    started: Instant,
    token: u64,
    frozen: Vec<u8>,
    frozen_ms: f32,
    collapse: Option<Instant>,
    entrance: Option<Instant>,
}

#[derive(Clone)]
enum Audio {
    Loading,
    Ready(Arc<Pcm>),
    Failed,
}

struct Playing {
    key: SharedString,
    handle: Handle,
    from: f32,
    at: Instant,
    rate: f32,
    duration: f32,
}

impl Playing {
    fn position(&self) -> f32 {
        (self.from + self.at.elapsed().as_secs_f32() * self.rate).min(self.duration)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Bar {
    value: f32,
    last: f32,
    start: Option<Instant>,
    reset: bool,
}

impl Bar {
    fn new(v: f32) -> Self {
        Self { value: v, last: v, start: None, reset: false }
    }

    fn current(&mut self, now: Instant) -> f32 {
        let Some(s) = self.start else { return self.value };
        let e = ms_between(s, now);
        let down = self.value < self.last;
        let d = if down { 150. } else { 500. };
        if e > d {
            self.start = None;
            return self.value;
        }
        if down { ease_out_quint(e, self.last, self.value, d) } else { ease_out_back(e, self.last, self.value, d, 4.) }
    }

    fn animate_to(&mut self, v: f32, now: Instant) {
        self.reset = false;
        if self.value != v {
            self.last = self.value;
            self.start = Some(now);
            self.value = v;
        }
    }

    fn reset(&mut self, now: Instant) {
        self.animate_to(0., now);
        self.reset = true;
    }
}

#[derive(Clone, Copy)]
struct Look {
    wave: (bool, bool),
    wave_at: Option<Instant>,
    wave_from: WaveColors,
    playing: bool,
    play_at: Option<Instant>,
    ripple_from: f32,
}

struct Seek {
    key: SharedString,
    duration: f32,
    resume: bool,
    bounds: Bounds<Pixels>,
}

struct Store {
    draft: Option<Draft>,
    playing: Option<Playing>,
    positions: HashMap<SharedString, f32>,
    audio: HashMap<SharedString, Audio>,
    stretched: HashMap<(SharedString, usize), Arc<Pcm>>,
    want: Option<SharedString>,
    rate_ix: usize,
    muted: HashSet<SharedString>,
    volume: f32,
    ticking: bool,
    next_token: u64,
    bars: HashMap<SharedString, Vec<Bar>>,
    looks: HashMap<SharedString, Look>,
    ended: HashMap<SharedString, Instant>,
    seek: Option<Seek>,
    vol_open: Option<SharedString>,
    vol_hide_at: Option<Instant>,
    vol_drag: Option<(SharedString, Bounds<Pixels>)>,
    vol_bar: Option<(SharedString, bool, Instant)>,
    vol_keys: Option<SharedString>,
    vol_in: (bool, bool),
    anim_until: Option<Instant>,
    modal: Option<(Id, Id)>,
    app: Option<WeakEntity<PhoenixApp>>,
    send_focus: Option<FocusHandle>,
    focus_send: bool,
    hovered: HashSet<SharedString>,
    left: HashMap<SharedString, Instant>,
    send_hover: bool,
    send_press: bool,
    fades: HashMap<&'static str, Fade>,
    meta: HashMap<SharedString, Option<f32>>,
    track_hover: Option<(SharedString, f32)>,
}

#[derive(Clone, Copy)]
struct Fade {
    from: Hsla,
    to: Hsla,
    at: Instant,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            draft: None,
            playing: None,
            positions: HashMap::new(),
            audio: HashMap::new(),
            stretched: HashMap::new(),
            want: None,
            rate_ix: 1,
            muted: HashSet::new(),
            volume: 1.,
            ticking: false,
            next_token: 0,
            bars: HashMap::new(),
            looks: HashMap::new(),
            ended: HashMap::new(),
            seek: None,
            vol_open: None,
            vol_hide_at: None,
            vol_drag: None,
            vol_bar: None,
            vol_keys: None,
            vol_in: (false, false),
            anim_until: None,
            modal: None,
            app: None,
            send_focus: None,
            focus_send: false,
            hovered: HashSet::new(),
            left: HashMap::new(),
            send_hover: false,
            send_press: false,
            fades: HashMap::new(),
            meta: HashMap::new(),
            track_hover: None,
        }
    }
}

impl gpui::Global for Store {}

fn store(cx: &mut App) -> &mut Store {
    cx.default_global::<Store>()
}

const VOLUME_PREF: &str = "media-volume";

fn ms_between(a: Instant, b: Instant) -> f32 {
    b.saturating_duration_since(a).as_secs_f32() * 1000.
}

pub fn ease_out_back(t: f32, from: f32, to: f32, d: f32, s: f32) -> f32 {
    let t = t / d - 1.;
    (to - from) * (t * t * ((s + 1.) * t + s) + 1.) + from
}

pub fn ease_out_quint(t: f32, from: f32, to: f32, d: f32) -> f32 {
    let t = t / d - 1.;
    (to - from) * (t * t * t * t * t + 1.) + from
}

pub fn perceptual(amplitude: f32) -> f32 {
    if amplitude <= 0. { 0. } else { amplitude.min(1.).powf(1. / 2.8) }
}

pub fn amplitude(perceptual: f32) -> f32 {
    if perceptual <= 0. { 0. } else { perceptual.min(1.).powf(2.8) }
}

fn bump(cx: &mut App, ms: u64) {
    let until = Instant::now() + Duration::from_millis(ms);
    let st = store(cx);
    if st.anim_until.is_none_or(|t| t < until) {
        st.anim_until = Some(until);
    }
    ensure_ticker(cx);
}

pub fn format_secs(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

pub fn waveform_b64(samples: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(vm::waveform(samples))
}

pub fn decode_waveform(b64: &str) -> Vec<f32> {
    base64::engine::general_purpose::STANDARD.decode(b64.trim()).map(|v| v.iter().map(|&b| f32::from(b) / 255.).collect()).unwrap_or_default()
}

pub fn canvas_width(duration: f64) -> f32 {
    let w = if duration <= 0.5 {
        40.
    } else if duration >= 45. {
        294.
    } else {
        (duration.min(45.) - 0.5) / 44.5 * 254. + 40.
    };
    6. * ((w as f32 + 4.) / 6.).floor() - 4.
}

pub fn fit_waveform(v: &[f32], n: usize, fluid: bool) -> Vec<f32> {
    if n == 0 {
        return Vec::new();
    }
    if v.len() >= n {
        return vm::downsample(v, n);
    }
    if !fluid {
        let mut out = v.to_vec();
        out.resize(n, 0.);
        return out;
    }
    match v.len() {
        0 => vec![0.; n],
        1 => vec![v[0]; n],
        len => (0..n)
            .map(|i| {
                let r = i as f32 * (len - 1) as f32 / (n - 1).max(1) as f32;
                let a = r.floor() as usize;
                let b = (a + 1).min(len - 1);
                let t = r - a as f32;
                v[a] * (1. - t) + v[b] * t
            })
            .collect(),
    }
}

fn source() -> Source {
    if let Some(secs) = std::env::var("PHOENIX_VOICE_MESSAGE_TONE").ok().and_then(|v| v.parse::<f32>().ok()) {
        return Source::Pcm { rate: 48_000, samples: vm::tone(48_000, secs.clamp(0.5, 60.), 440., 0.4) };
    }
    Source::Microphone(crate::voice::prefs::prefs().lock().input_device.clone())
}

pub fn active(cx: &App, channel: Id) -> bool {
    cx.try_global::<Store>().and_then(|s| s.draft.as_ref()).is_some_and(|d| d.channel == channel)
}

pub fn start(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let Some(channel) = app.selected_channel else { return };
    if app.nav.extras.get(&channel).is_some_and(|x| x.send_blocked) {
        return;
    }
    let existing = cx
        .try_global::<Store>()
        .and_then(|s| s.draft.as_ref())
        .map(|d| (d.channel, matches!(d.phase, Phase::Recorded(_)), matches!(d.phase, Phase::Recording(_))));
    if let Some((other, recorded, recording)) = existing {
        if other != channel && recorded {
            if app.channels.contains_key(&other) {
                store(cx).modal = Some((other, channel));
            } else {
                discard(cx, other);
            }
            cx.notify();
        } else if other != channel && recording {
            stop_recording(app, other, true, false, cx);
        }
        return;
    }
    let st = store(cx);
    st.next_token += 1;
    let token = st.next_token;
    st.draft = Some(Draft {
        channel,
        phase: Phase::Requesting,
        started: Instant::now(),
        token,
        frozen: Vec::new(),
        frozen_ms: 0.,
        collapse: None,
        entrance: None,
    });
    st.focus_send = true;
    let control = crate::voice::call::manager().control().clone();
    let src = source();
    ensure_ticker(cx);
    cx.spawn(async move |this, cx| {
        let res = cx.background_spawn(async move { Recorder::start(control, src) }).await;
        let _ = this.update(cx, |app, cx| {
            let st = store(cx);
            let ours = st.draft.as_ref().is_some_and(|d| d.token == token && matches!(d.phase, Phase::Requesting));
            match res {
                Ok(rec) if ours => {
                    if let Some(d) = st.draft.as_mut() {
                        d.phase = Phase::Recording(rec);
                        d.started = Instant::now();
                    }
                }
                Ok(rec) => {
                    cx.background_spawn(async move { rec.cancel() }).detach();
                }
                Err(e) => {
                    tracing::warn!("voice message recording: {e}");
                    if ours {
                        st.draft = None;
                        app.parity_toast(crate::t!("voice_msg.failed_start"), cx);
                    }
                }
            }
            cx.notify();
        });
    })
    .detach();
    cx.notify();
}

fn take_recorder(cx: &mut App, channel: Id) -> Option<(Recorder, u64)> {
    let d = store(cx).draft.as_mut().filter(|d| d.channel == channel)?;
    match std::mem::replace(&mut d.phase, Phase::Finalizing) {
        Phase::Recording(r) => {
            d.frozen = r.samples();
            d.frozen_ms = d.started.elapsed().as_secs_f32() * 1000.;
            Some((r, d.token))
        }
        other => {
            d.phase = other;
            None
        }
    }
}

fn stop_recording(app: &mut PhoenixApp, channel: Id, interrupted: bool, by_button: bool, cx: &mut Context<PhoenixApp>) {
    let Some((rec, token)) = take_recorder(cx, channel) else { return };
    if by_button && !crate::ui::settings::effects::reduced_motion() {
        let st = store(cx);
        if let Some(d) = st.draft.as_mut() {
            d.collapse = Some(Instant::now());
        }
        st.focus_send = true;
        bump(cx, 200);
    } else if by_button {
        store(cx).focus_send = true;
    }
    if interrupted {
        if !crate::sounds::is_disabled("voice_message_recording_stopped") {
            crate::sounds::play("voice_message_recording_stopped", 0.4);
        }
        app.parity_toast(crate::t!("voice_msg.stopped"), cx);
    }
    cx.spawn(async move |this, cx| {
        let res = cx.background_spawn(async move { rec.stop() }).await;
        let _ = this.update(cx, |app, cx| {
            let st = store(cx);
            let Some(d) = st.draft.as_mut().filter(|d| d.token == token) else { return };
            match res {
                Ok(r) => {
                    let r = Arc::new(r);
                    let key = draft_key(channel);
                    d.phase = Phase::Recorded(r.clone());
                    st.audio.insert(key.clone(), Audio::Loading);
                    let ogg = r.ogg.clone();
                    cx.spawn(async move |this, cx| {
                        let pcm = cx.background_spawn(async move { vm::decode_ogg(&ogg).ok() }).await;
                        let _ = this.update(cx, |_, cx| {
                            store(cx).audio.insert(key, pcm.map_or(Audio::Failed, |p| Audio::Ready(Arc::new(p))));
                            cx.notify();
                        });
                    })
                    .detach();
                }
                Err(e) => {
                    tracing::warn!("voice message finalize: {e}");
                    st.draft = None;
                    app.parity_toast(crate::t!("voice_msg.failed_send"), cx);
                }
            }
            cx.notify();
        });
    })
    .detach();
    cx.notify();
}

const DRAFT_PREFIX: &str = "voice-draft:";

pub const ATTACHMENT_PREFIX: &str = "audio-attachment:";

pub fn attachment_key(url: &str) -> SharedString {
    format!("{ATTACHMENT_PREFIX}{url}").into()
}

fn is_attachment(key: &str) -> bool {
    key.starts_with(ATTACHMENT_PREFIX)
}

fn normal_rate_ix() -> usize {
    vm::PLAYBACK_RATES.iter().position(|r| (r - 1.).abs() < 1e-3).unwrap_or(0)
}

fn rate_ix_for(st: &Store, key: &str) -> usize {
    if is_attachment(key) { normal_rate_ix() } else { st.rate_ix }
}

pub fn file_name(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn draft_key(channel: Id) -> SharedString {
    format!("{DRAFT_PREFIX}{}", channel.0).into()
}

fn forget_key(st: &mut Store, key: &SharedString) {
    if let Some(p) = st.playing.take_if(|p| &p.key == key) {
        p.handle.stop();
    }
    st.audio.remove(key);
    st.positions.remove(key);
    st.bars.remove(key);
    st.looks.remove(key);
    st.ended.remove(key);
    st.stretched.retain(|(k, _), _| k != key);
}

pub fn discard(cx: &mut App, channel: Id) {
    let key = draft_key(channel);
    let st = store(cx);
    forget_key(st, &key);
    let Some(d) = st.draft.take_if(|d| d.channel == channel) else { return };
    if let Phase::Recording(r) = d.phase {
        cx.background_spawn(async move { r.cancel() }).detach();
    }
}

pub fn send(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) -> bool {
    let Some(channel) = app.selected_channel else { return false };
    if !active(cx, channel) {
        return false;
    }
    if let Some((rec, token)) = take_recorder(cx, channel) {
        cx.spawn(async move |this, cx| {
            let res = cx.background_spawn(async move { rec.stop() }).await;
            let _ = this.update(cx, |app, cx| {
                if !store(cx).draft.as_ref().is_some_and(|d| d.token == token) {
                    return;
                }
                discard(cx, channel);
                match res {
                    Ok(r) => upload(app, channel, Arc::new(r), cx),
                    Err(_) => app.parity_toast(crate::t!("voice_msg.failed_send"), cx),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
        return true;
    }
    let rec = match cx.try_global::<Store>().and_then(|s| s.draft.as_ref()).map(|d| &d.phase) {
        Some(Phase::Recorded(r)) => r.clone(),
        _ => return true,
    };
    discard(cx, channel);
    upload(app, channel, rec, cx);
    cx.notify();
    true
}

fn upload(app: &mut PhoenixApp, channel: Id, rec: Arc<Recording>, cx: &mut Context<PhoenixApp>) {
    if rec.duration_secs < vm::MIN_DURATION_SECS {
        return;
    }
    let voice = crate::backend::VoiceUpload { ogg: rec.ogg.clone(), duration_secs: rec.duration_secs, waveform: waveform_b64(&rec.samples) };
    let reply = app.replying_to.take().map(|r| (r, app.chat.reply_mention));
    app.chat.reply_mention = true;
    let nonce = super::pending::make_nonce();
    let fut = app.backend.send_voice_message(channel, voice, reply, nonce);
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |app, cx| match res {
            Ok(mut m) => {
                m.nonce.get_or_insert(nonce);
                app.append_message(m, cx);
                if app.selected_channel == Some(channel) {
                    app.message_list.scroll_to_end();
                }
            }
            Err(e) => {
                tracing::warn!("voice message upload: {}", e.0);
                app.parity_toast(crate::t!("voice_msg.failed_send"), cx);
            }
        });
    })
    .detach();
}

pub fn on_enter(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) -> bool {
    let Some(channel) = app.selected_channel else { return false };
    let ready = cx
        .try_global::<Store>()
        .and_then(|s| s.draft.as_ref())
        .is_some_and(|d| d.channel == channel && matches!(d.phase, Phase::Recording(_) | Phase::Recorded(_)));
    if ready { send(app, cx) } else { active(cx, channel) }
}

pub fn on_channel_change(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let Some(d) = cx.try_global::<Store>().and_then(|s| s.draft.as_ref()) else { return };
    if Some(d.channel) == app.selected_channel {
        return;
    }
    let ch = d.channel;
    let (recording, requesting) = (matches!(d.phase, Phase::Recording(_)), matches!(d.phase, Phase::Requesting));
    if recording {
        stop_recording(app, ch, true, false, cx);
    } else if requesting {
        discard(cx, ch);
    }
}

fn ensure_ticker(cx: &mut App) {
    let st = store(cx);
    if st.ticking {
        return;
    }
    st.ticking = true;
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(Duration::from_millis(16)).await;
            if !cx.update(tick) {
                break;
            }
        }
    })
    .detach();
}

fn tick(cx: &mut App) -> bool {
    let now = Instant::now();
    let st = store(cx);
    if let Some(p) = &st.playing
        && (p.handle.is_stopped() || p.position() >= p.duration - 0.01)
    {
        let (key, duration) = (p.key.clone(), p.duration);
        st.playing = None;
        if key.starts_with(DRAFT_PREFIX) {
            st.positions.remove(&key);
        } else if is_attachment(&key) {
            st.positions.insert(key, duration);
        } else {
            st.positions.insert(key.clone(), duration);
            st.ended.insert(key, now);
        }
    }
    let done: Vec<SharedString> = st.ended.iter().filter(|(_, at)| ms_between(**at, now) >= 500.).map(|(k, _)| k.clone()).collect();
    for k in done {
        st.ended.remove(&k);
        st.positions.remove(&k);
    }
    if st.vol_drag.is_none() && st.vol_hide_at.is_some_and(|t| now >= t) {
        st.vol_hide_at = None;
        st.vol_open = None;
        st.vol_keys = None;
    }
    if st.anim_until.is_some_and(|t| now >= t) {
        st.anim_until = None;
    }
    let recording = st.draft.as_ref().is_some_and(|d| matches!(d.phase, Phase::Requesting | Phase::Recording(_) | Phase::Finalizing));
    let busy = st.playing.is_some()
        || recording
        || st.want.is_some()
        || !st.ended.is_empty()
        || st.vol_hide_at.is_some()
        || st.anim_until.is_some()
        || st.seek.is_some()
        || st.vol_drag.is_some();
    if !busy {
        st.ticking = false;
    }
    cx.refresh_windows();
    busy
}

fn audio_for(cx: &App, key: &SharedString) -> Option<Audio> {
    cx.try_global::<Store>().and_then(|s| s.audio.get(key).cloned())
}

fn ensure_loaded(key: &SharedString, url: &SharedString, cx: &mut App) {
    let st = store(cx);
    if st.audio.contains_key(key) {
        return;
    }
    st.audio.insert(key.clone(), Audio::Loading);
    let http = cx.http_client();
    let (k, u) = (key.clone(), url.to_string());
    cx.spawn(async move |cx| {
        use futures::AsyncReadExt;
        let mut bytes = Vec::new();
        let ok = match http.get(&u, Default::default(), true).await {
            Ok(mut r) => r.body_mut().read_to_end(&mut bytes).await.is_ok(),
            Err(_) => false,
        };
        let ok = ok && bytes.len() <= crate::media::audio_file::MAX_BYTES;
        let att = is_attachment(&k);
        let name = file_name(&u);
        let pcm = if ok {
            cx.background_spawn(async move { if att { crate::media::audio_file::decode(&bytes, &name) } else { vm::decode_ogg(&bytes).ok() } }).await
        } else {
            None
        };
        cx.update(|cx| {
            let failed = pcm.is_none();
            let st = store(cx);
            if let Some(p) = pcm.as_ref().filter(|_| att) {
                st.meta.insert(k.clone(), Some(p.duration().as_secs_f32()));
            }
            st.audio.insert(k.clone(), pcm.map_or(Audio::Failed, |p| Audio::Ready(Arc::new(p))));
            if st.want.as_ref() == Some(&k) {
                st.want = None;
                if !failed {
                    start_playback(&k, cx);
                }
            }
            cx.refresh_windows();
        });
    })
    .detach();
}

fn pause(cx: &mut App) {
    let st = store(cx);
    if let Some(p) = st.playing.take() {
        let pos = p.position();
        p.handle.stop();
        st.positions.insert(p.key, pos);
    }
}

fn stretched(key: &SharedString, ix: usize, cx: &mut App) -> Option<Arc<Pcm>> {
    let st = store(cx);
    let Some(Audio::Ready(pcm)) = st.audio.get(key).cloned() else { return None };
    let rate = vm::PLAYBACK_RATES[ix];
    if (rate - 1.).abs() < 1e-3 {
        return Some(pcm);
    }
    if let Some(p) = st.stretched.get(&(key.clone(), ix)) {
        return Some(p.clone());
    }
    st.want = Some(key.clone());
    let k = key.clone();
    cx.spawn(async move |cx| {
        let src = pcm.clone();
        let out = cx.background_spawn(async move { Pcm { rate: src.rate, channels: 1, samples: vm::time_stretch(&src.samples, rate, src.rate) } }).await;
        cx.update(|cx| {
            let st = store(cx);
            st.stretched.insert((k.clone(), ix), Arc::new(out));
            if st.want.as_ref() == Some(&k) {
                st.want = None;
                start_playback(&k, cx);
            }
        });
    })
    .detach();
    None
}

fn effective_volume(st: &Store, key: &SharedString) -> f32 {
    if st.muted.contains(key) { 0. } else { st.volume.clamp(0., 1.) }
}

fn start_playback(key: &SharedString, cx: &mut App) {
    pause(cx);
    let Some(Audio::Ready(pcm)) = audio_for(cx, key) else { return };
    let duration = pcm.duration().as_secs_f32();
    let ix = rate_ix_for(store(cx), key);
    let Some(src) = stretched(key, ix, cx) else { return };
    let st = store(cx);
    st.ended.remove(key);
    let mut from = st.positions.get(key).copied().unwrap_or(0.);
    if from >= duration - 0.05 {
        from = 0.;
    }
    let rate = vm::PLAYBACK_RATES[ix];
    let ch = usize::from(src.channels.max(1));
    let off = ((from / rate) * src.rate as f32) as usize * ch;
    let samples = src.samples.get(off..).map(<[f32]>::to_vec).unwrap_or_default();
    let volume = effective_volume(st, key);
    let handle = effects::play(Arc::new(Pcm { rate: src.rate, channels: src.channels, samples }), volume, false);
    let st = store(cx);
    st.positions.insert(key.clone(), from);
    st.playing = Some(Playing { key: key.clone(), handle, from, at: Instant::now(), rate, duration });
    ensure_ticker(cx);
}

fn toggle(key: &SharedString, url: Option<&SharedString>, cx: &mut App) {
    if cx.try_global::<Store>().and_then(|s| s.playing.as_ref()).is_some_and(|p| &p.key == key) {
        pause(cx);
        bump(cx, 520);
        return;
    }
    match audio_for(cx, key) {
        Some(Audio::Ready(_)) => start_playback(key, cx),
        Some(Audio::Loading) => store(cx).want = Some(key.clone()),
        Some(Audio::Failed) => {}
        None => {
            if let Some(u) = url {
                store(cx).want = Some(key.clone());
                ensure_loaded(key, u, cx);
            }
        }
    }
    ensure_ticker(cx);
}

fn seek_to(key: &SharedString, frac: f32, duration: f32, cx: &mut App) {
    let st = store(cx);
    st.ended.remove(key);
    st.positions.insert(key.clone(), frac.clamp(0., 1.) * duration);
}

fn seek_start(key: &SharedString, frac: f32, duration: f32, bounds: Bounds<Pixels>, cx: &mut App) {
    let playing = cx.try_global::<Store>().and_then(|s| s.playing.as_ref()).is_some_and(|p| &p.key == key);
    if playing {
        pause(cx);
    }
    store(cx).seek = Some(Seek { key: key.clone(), duration, resume: playing, bounds });
    seek_to(key, frac, duration, cx);
    ensure_ticker(cx);
}

fn seek_end(cx: &mut App) {
    let Some(s) = store(cx).seek.take() else { return };
    let pos = store(cx).positions.get(&s.key).copied().unwrap_or(0.);
    if pos >= s.duration - 0.01 && s.key.starts_with(DRAFT_PREFIX) {
        store(cx).positions.remove(&s.key);
    } else if pos >= s.duration - 0.01 {
        let st = store(cx);
        st.positions.insert(s.key.clone(), s.duration);
        if !is_attachment(&s.key) {
            st.ended.insert(s.key, Instant::now());
        }
        bump(cx, 520);
    } else if s.resume {
        start_playback(&s.key, cx);
    }
    cx.refresh_windows();
}

fn frac_in(b: Bounds<Pixels>, x: Pixels) -> f32 {
    (f32::from(x - b.origin.x) / f32::from(b.size.width).max(1.)).clamp(0., 1.)
}

fn cycle_rate(cx: &mut App) {
    let playing = cx.try_global::<Store>().and_then(|s| s.playing.as_ref().map(|p| p.key.clone()));
    if playing.is_some() {
        pause(cx);
    }
    let st = store(cx);
    st.rate_ix = (st.rate_ix + 1) % vm::PLAYBACK_RATES.len();
    if let Some(k) = playing {
        start_playback(&k, cx);
    }
}

fn apply_volume(cx: &mut App) {
    let st = store(cx);
    if let Some(p) = &st.playing {
        p.handle.set_volume(effective_volume(st, &p.key));
    }
}

fn persist_volume(cx: &mut App) {
    let v = store(cx).volume;
    let Some(app) = store(cx).app.as_ref().and_then(WeakEntity::upgrade) else { return };
    app.update(cx, |a, _| {
        a.prefs.local.ui.insert(VOLUME_PREF.into(), (v.clamp(0., 1.) * 1_000_000.).round() as u64);
        a.prefs.local.save();
    });
}

pub const CALL_VOLUME_PREFIX: &str = "call-volume:";

pub fn call_volume_key(user: Id, stream: bool) -> SharedString {
    format!("{CALL_VOLUME_PREFIX}{}:{}", user.0, u8::from(stream)).into()
}

fn call_target(key: &SharedString) -> Option<(Id, bool)> {
    let (u, s) = key.strip_prefix(CALL_VOLUME_PREFIX)?.split_once(':')?;
    Some((Id(u.parse().ok()?), s == "1"))
}

fn toggle_mute(key: &SharedString, cx: &mut App) {
    if let Some((user, stream)) = call_target(key) {
        crate::ui::call_view::toggle_participant_mute(user, stream);
        cx.refresh_windows();
        return;
    }
    let st = store(cx);
    if !st.muted.remove(key) {
        st.muted.insert(key.clone());
    }
    apply_volume(cx);
}

fn set_slider(key: &SharedString, value: f32, cx: &mut App) {
    if let Some((user, stream)) = call_target(key) {
        crate::ui::call_view::set_participant_slider(user, stream, value);
        cx.refresh_windows();
        return;
    }
    let amp = amplitude(value.clamp(0., 1.));
    let st = store(cx);
    st.volume = amp;
    if amp == 0. {
        st.muted.insert(key.clone());
    } else {
        st.muted.remove(key);
    }
    apply_volume(cx);
    persist_volume(cx);
}

pub fn media_volume(cx: &App) -> f32 {
    cx.try_global::<Store>().map_or(1., |s| s.volume.clamp(0., 1.))
}

pub fn set_media_volume(v: f32, cx: &mut App) {
    store(cx).volume = v.clamp(0., 1.);
    apply_volume(cx);
    persist_volume(cx);
}

fn slider_value(cx: &App, key: &SharedString) -> f32 {
    if let Some((user, stream)) = call_target(key) {
        return crate::ui::call_view::participant_slider(user, stream);
    }
    cx.try_global::<Store>().map_or(1., |s| if s.muted.contains(key) { 0. } else { perceptual(s.volume) })
}

pub fn rate_label(rate: f32) -> String {
    let s = format!("{rate}");
    format!("{}X", s.strip_prefix('0').unwrap_or(&s))
}

struct View {
    current: f32,
    playing: bool,
    played: bool,
}

fn view(cx: &App, key: &SharedString) -> View {
    let Some(st) = cx.try_global::<Store>() else { return View { current: 0., playing: false, played: false } };
    if let Some(p) = st.playing.as_ref().filter(|p| &p.key == key) {
        return View { current: p.position(), playing: true, played: true };
    }
    let dragging = st.seek.as_ref().is_some_and(|s| &s.key == key && s.resume);
    match st.positions.get(key) {
        Some(&pos) => View { current: pos, playing: dragging, played: true },
        None => View { current: 0., playing: dragging, played: false },
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WaveColors {
    bg: Hsla,
    active: Hsla,
    inactive: Hsla,
}

fn wave_colors(played: bool, playing: bool) -> WaveColors {
    let muted = c(tok(idx::BACKGROUND_MOD_MUTED));
    let bg = if playing { c(tok(idx::BRAND_430)) } else { muted };
    let active = if playing {
        c(tok(idx::WHITE))
    } else if played {
        c(tok(idx::INTERACTIVE_TEXT_ACTIVE))
    } else {
        c(tok(idx::INTERACTIVE_TEXT_DEFAULT))
    };
    let inactive = if played { bg } else { c(tok(idx::INTERACTIVE_TEXT_DEFAULT)) };
    WaveColors { bg, active, inactive }
}

fn opaque(h: Hsla) -> Hsla {
    Hsla { a: 1., ..h }
}

pub fn hex_lerp(from: Hsla, to: Hsla, t: f32) -> Hsla {
    if t >= 1. { to } else { mix(opaque(from), opaque(to), t.max(0.)) }
}

fn ripple_target(playing: bool) -> f32 {
    if playing { 1. } else { 0. }
}

fn ease_in_out(t: f32) -> f32 {
    cubic_bezier(0.42, 0., 0.58, 1.)(t)
}

fn ease_in(t: f32) -> f32 {
    cubic_bezier(0.42, 0., 1., 1.)(t)
}

fn look_ripple(l: &Look, now: Instant) -> f32 {
    let target = ripple_target(l.playing);
    match l.play_at {
        Some(at) => {
            let t = (ms_between(at, now) / 200.).min(1.);
            l.ripple_from + (target - l.ripple_from) * ease_in_out(t)
        }
        None => target,
    }
}

fn look_wave(l: &Look, now: Instant) -> WaveColors {
    let to = wave_colors(l.wave.0, l.wave.1);
    match l.wave_at {
        Some(at) => {
            let t = (ms_between(at, now) / 200.).min(1.);
            WaveColors { bg: hex_lerp(l.wave_from.bg, to.bg, t), active: hex_lerp(l.wave_from.active, to.active, t), inactive: hex_lerp(l.wave_from.inactive, to.inactive, t) }
        }
        None => to,
    }
}

fn update_look(cx: &mut App, key: &SharedString, played: bool, playing: bool, brand: bool) -> Look {
    let now = Instant::now();
    let wave = (played, playing && brand);
    let st = store(cx);
    let prev = st.looks.get(key).copied();
    let mut l = prev.unwrap_or(Look { wave, wave_at: None, wave_from: wave_colors(wave.0, wave.1), playing, play_at: None, ripple_from: ripple_target(playing) });
    let mut changed = false;
    if l.wave != wave {
        l.wave_from = look_wave(&l, now);
        l.wave = wave;
        l.wave_at = Some(now);
        changed = true;
    }
    if l.playing != playing {
        l.ripple_from = look_ripple(&l, now);
        l.playing = playing;
        l.play_at = Some(now);
        changed = true;
    }
    st.looks.insert(key.clone(), l);
    if changed {
        bump(cx, 260);
    }
    l
}

pub const HOVER_OUT_MS: f32 = 200.;
pub const SEND_FADE_MS: f32 = 250.;

fn set_hovered(cx: &mut App, id: &SharedString, over: bool) {
    let st = store(cx);
    if over {
        st.hovered.insert(id.clone());
        st.left.remove(id);
    } else if st.hovered.remove(id) {
        st.left.insert(id.clone(), Instant::now());
        bump(cx, HOVER_OUT_MS as u64 + 20);
    }
}

pub fn hover_out(left_ms: Option<f32>, hovered: bool, hover: Hsla, rest: Hsla, ease: fn(f32) -> f32) -> Hsla {
    if hovered {
        return hover;
    }
    match left_ms {
        Some(ms) if ms < HOVER_OUT_MS => mix(hover, rest, ease((ms / HOVER_OUT_MS).max(0.))),
        _ => rest,
    }
}

fn hover_fade(cx: &App, id: &SharedString, hover: Hsla, rest: Hsla, ease: fn(f32) -> f32) -> Hsla {
    let Some(st) = cx.try_global::<Store>() else { return rest };
    let left = st.left.get(id).map(|at| ms_between(*at, Instant::now()));
    hover_out(left, st.hovered.contains(id), hover, rest, ease)
}

fn fade_shown(f: &Fade, now: Instant, ms: f32, ease: fn(f32) -> f32) -> Hsla {
    let t = ms_between(f.at, now) / ms;
    if t >= 1. { f.to } else { mix(f.from, f.to, ease(t.max(0.))) }
}

fn faded(cx: &mut App, id: &'static str, target: Hsla, ms: f32, ease: fn(f32) -> f32) -> Hsla {
    let now = Instant::now();
    let st = store(cx);
    let (shown, restart) = match st.fades.get(id).copied() {
        None => (target, Some(Fade { from: target, to: target, at: now })),
        Some(f) if f.to == target => (fade_shown(&f, now, ms, ease), None),
        Some(f) => {
            let from = fade_shown(&f, now, ms, ease);
            (from, Some(Fade { from, to: target, at: now }))
        }
    };
    if let Some(f) = restart {
        let moving = f.from != f.to;
        st.fades.insert(id, f);
        if moving {
            bump(cx, ms as u64 + 20);
        }
    }
    shown
}

fn state_mix(cx: &App, key: &SharedString, playing: bool, rest: Hsla, on: Hsla, ease: fn(f32) -> f32) -> Hsla {
    let now = Instant::now();
    let target = if playing { on } else { rest };
    let Some(l) = cx.try_global::<Store>().and_then(|s| s.looks.get(key).copied()) else { return target };
    if l.playing != playing {
        return if playing { rest } else { on };
    }
    match l.play_at {
        Some(at) if ms_between(at, now) < 200. => {
            let t = ease(ms_between(at, now) / 200.);
            if playing { mix(rest, on, t) } else { mix(on, rest, t) }
        }
        _ => target,
    }
}

fn bar_quad(window: &mut Window, b: Bounds<Pixels>, x: f32, h: f32, color: Hsla) {
    if h <= 0. {
        return;
    }
    let bw = f32::from(z(2.));
    let top = (f32::from(b.size.height) - h) / 2.;
    let o = gpui::point(b.origin.x + px(x), b.origin.y + px(top));
    window.paint_quad(gpui::fill(Bounds::new(o, gpui::size(px(bw), px(h))), color).corner_radii(z(1.)));
}

fn mount_factor(elapsed: Option<f32>, i: usize, n: usize) -> f32 {
    let Some(l) = elapsed else { return 1. };
    let delay = if n > 1 { i as f32 / (n - 1) as f32 * 60. } else { 0. };
    let t = (l - delay).clamp(0., 100.);
    ease_out_back(t, 0., 1., 100., 2.).max(0.)
}

struct Wave {
    key: SharedString,
    values: Vec<f32>,
    fluid: bool,
    played: bool,
    frac: f32,
    playing: bool,
    brand: bool,
    entrance: Option<Instant>,
    duration: f32,
}

fn wave_canvas(w: Wave) -> gpui::Canvas<Hitbox> {
    gpui::canvas(
        |b, window, _| window.insert_hitbox(b, HitboxBehavior::Normal),
        move |b, hitbox, window, cx| {
            let Wave { key, values, fluid, played, frac, playing, brand, entrance, duration } = w;
            let now = Instant::now();
            let look = update_look(cx, &key, played, playing, brand);
            let colors = look_wave(&look, now);
            let pitch = f32::from(z(6.));
            let width = f32::from(b.size.width);
            let n = ((width + f32::from(z(4.))) / pitch).floor().max(0.) as usize;
            let vals = fit_waveform(&values, n, fluid);
            let upto = if played { (frac * n as f32).round().clamp(0., n as f32) as usize } else { n };
            let elapsed = entrance.map(|e| ms_between(e, now)).filter(|e| *e < 160.);
            let (k22, k24, k2) = (f32::from(z(22.)), f32::from(z(24.)), f32::from(z(2.)));
            let st = store(cx);
            let bars = st.bars.entry(key.clone()).or_default();
            if bars.len() != n {
                *bars = vals.iter().enumerate().map(|(i, v)| Bar::new(if i < upto { *v } else { 0. })).collect();
            }
            for (i, bar) in bars.iter_mut().enumerate() {
                if i < upto {
                    bar.animate_to(vals[i], now);
                } else {
                    bar.reset(now);
                }
            }
            for (i, v) in vals.iter().enumerate() {
                let t = mount_factor(elapsed, i, n);
                bar_quad(window, b, i as f32 * pitch, k22 * (v * t) + k2, colors.bg);
            }
            let mut animating = elapsed.is_some();
            for (i, bar) in bars.iter_mut().enumerate() {
                let t = mount_factor(elapsed, i, n);
                let v = bar.current(now).max(vals[i] - 0.1) * t;
                animating |= bar.start.is_some();
                if bar.reset {
                    bar_quad(window, b, i as f32 * pitch, k24 * v, colors.inactive);
                } else {
                    bar_quad(window, b, i as f32 * pitch, k22 * v + k2, colors.active);
                }
            }
            if animating {
                bump(cx, 32);
            }
            let k = key.clone();
            window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble && e.button == MouseButton::Left && hitbox.is_hovered(window) {
                    cx.stop_propagation();
                    seek_start(&k, frac_in(b, e.position.x), duration, b, cx);
                    window.refresh();
                }
            });
            if cx.try_global::<Store>().and_then(|s| s.seek.as_ref()).is_some_and(|s| s.key == key) {
                let drag_b = cx.try_global::<Store>().and_then(|s| s.seek.as_ref()).map_or(b, |s| s.bounds);
                window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture {
                        return;
                    }
                    let Some(s) = cx.try_global::<Store>().and_then(|s| s.seek.as_ref()) else { return };
                    let (k, d) = (s.key.clone(), s.duration);
                    seek_to(&k, frac_in(drag_b, e.position.x), d, cx);
                    window.refresh();
                });
                window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble {
                        seek_end(cx);
                    }
                });
            }
        },
    )
}

fn paint_ripple(window: &mut Window, b: Bounds<Pixels>, p: f32, reduced: bool, color: Hsla) {
    let inner = b;
    let (w, h) = (f32::from(inner.size.width), f32::from(inner.size.height));
    let radius = (f32::from(z(24.)) - f32::from(z(1.))).min(h / 2.).max(0.);
    if reduced || p >= 1. {
        let a = if reduced { p } else { 1. };
        if a > 0. {
            window.paint_quad(gpui::fill(inner, Hsla { a: color.a * a, ..color }).corner_radii(px(radius)));
        }
        return;
    }
    let r = p * w;
    if r <= 0. {
        return;
    }
    let (cx0, cy0) = (f32::from(z(24.)), h / 2.);
    let step = 1. / window.scale_factor();
    let mut y = 0.;
    while y < h {
        let ym = y + step / 2.;
        let dy = ym - cy0;
        if dy.abs() < r {
            let half = (r * r - dy * dy).sqrt();
            let (mut x0, mut x1) = ((cx0 - half).max(0.), (cx0 + half).min(w));
            let ry = if ym < radius {
                radius - ym
            } else if ym > h - radius {
                ym - (h - radius)
            } else {
                0.
            };
            if ry > 0. {
                let cut = radius - (radius * radius - ry * ry).max(0.).sqrt();
                x0 = x0.max(cut);
                x1 = x1.min(w - cut);
            }
            if x1 > x0 {
                let o = gpui::point(inner.origin.x + px(x0), inner.origin.y + px(y));
                window.paint_quad(gpui::fill(Bounds::new(o, gpui::size(px(x1 - x0), px(step.min(h - y)))), color));
            }
        }
        y += step;
    }
}

fn ripple_canvas(key: SharedString, reduced: bool) -> gpui::Canvas<()> {
    gpui::canvas(
        |_, _, _| (),
        move |b, _, window, cx| {
            let Some(l) = cx.try_global::<Store>().and_then(|s| s.looks.get(&key).copied()) else { return };
            let now = Instant::now();
            let p = if reduced {
                match l.play_at {
                    Some(at) => {
                        let t = ease_in_out((ms_between(at, now) / 200.).min(1.));
                        if l.playing { t } else { 1. - t }
                    }
                    None => ripple_target(l.playing),
                }
            } else {
                look_ripple(&l, now)
            };
            paint_ripple(window, b, p, reduced, c(tok(idx::BRAND_530)));
        },
    )
}

fn live_wave(levels: Vec<f32>, elapsed_ms: f32, recording: bool, reduced: bool, collapse_ms: Option<f32>, color: Hsla) -> gpui::Canvas<()> {
    gpui::canvas(
        |_, _, _| (),
        move |b, _, window, _| {
            let pitch = f32::from(z(6.));
            let w = f32::from(b.size.width);
            let n = (w / pitch).ceil() as usize + 2;
            let pushes = (elapsed_ms / 48.).floor().max(0.) as usize + 1;
            let mut d = 0f32;
            let mut targets: Vec<(f32, f32)> = Vec::with_capacity(pushes);
            for j in 0..pushes {
                let si = ((j as f32 * 48. / 20.) as usize).min(levels.len().saturating_sub(1));
                let r = levels.get(si).copied().unwrap_or(0.);
                d += (r - d) * 0.45;
                targets.push((d, j as f32 * 48.));
            }
            let tail = &targets[targets.len().saturating_sub(n)..];
            let pad = n - tail.len();
            let offset = if recording && !reduced { pitch * (1. - (elapsed_ms % 48.) / 48.) } else { 0. };
            let x0 = w - n as f32 * pitch + offset;
            let (k22, k2) = (f32::from(z(22.)), f32::from(z(2.)));
            let fade = 0.12 * w;
            for i in 0..n {
                let x = x0 + i as f32 * pitch;
                if x + pitch < 0. || x > w {
                    continue;
                }
                let v = match i.checked_sub(pad).map(|k| tail[k]) {
                    None => 0.,
                    Some((target, _)) if reduced => target,
                    Some((target, at)) => {
                        let e = (elapsed_ms - at).max(0.);
                        if e >= 500. || target == 0. { target } else { ease_out_back(e, 0., target, 500., 4.).max(0.) }
                    }
                };
                let a = collapse_ms.map_or(0., |cm| {
                    let r = if n > 1 { i as f32 / (n - 1) as f32 * 40. } else { 0. };
                    ((cm - r) / 60.).clamp(0., 1.)
                });
                let h = v * (1. - a * a) * k22 + k2;
                let mask = if fade > 0. { ((x + f32::from(z(1.))) / fade).clamp(0., 1.) } else { 1. };
                if mask > 0. {
                    bar_quad(window, b, x, h, Hsla { a: color.a * mask, ..color });
                }
            }
        },
    )
}

fn chat_button(id: impl Into<SharedString>, icon: &'static str, tip: SharedString) -> gpui::Stateful<gpui::Div> {
    let id: SharedString = id.into();
    let group: SharedString = format!("vm-{id}").into();
    div()
        .id(gpui::ElementId::Name(id))
        .group(group.clone())
        .size(z(32.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(8.))
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_SELECTED))))
        .tooltip(crate::ui::tooltip::tip(tip))
        .child(
            svg()
                .path(crate::icons::path(icon))
                .size(z(20.))
                .flex_none()
                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                .group_hover(group, |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER)))),
        )
}

fn timer_text(text: String, color: Hsla) -> gpui::Div {
    div().flex_none().min_w(z(32.)).text_size(z(14.)).line_height(z(18.)).text_color(color).child(SharedString::from(text))
}

pub fn bar(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let weak = cx.entity().downgrade();
    let saved = app.prefs.local.ui.get(VOLUME_PREF).copied();
    let fresh = cx.try_global::<Store>().is_none_or(|s| s.send_focus.is_none()).then(|| cx.focus_handle());
    let st = store(cx);
    if st.app.is_none() {
        st.app = Some(weak);
        if let Some(v) = saved {
            st.volume = (v as f32 / 1_000_000.).clamp(0., 1.);
        }
    }
    if let Some(h) = fresh {
        st.send_focus = Some(h);
    }
    let channel = app.selected_channel?;
    let st = cx.try_global::<Store>()?;
    let d = st.draft.as_ref().filter(|d| d.channel == channel)?;
    let key = draft_key(channel);
    let reduced = crate::ui::settings::effects::reduced_motion();
    let recording = matches!(d.phase, Phase::Recording(_));
    let can_send = matches!(d.phase, Phase::Recording(_) | Phase::Recorded(_));
    let elapsed = d.started.elapsed();
    let now = Instant::now();
    let collapsing = d.collapse.map(|t| ms_between(t, now)).filter(|ms| *ms < 100.);
    let recorded = match &d.phase {
        Phase::Recorded(r) if collapsing.is_none() => Some(r.clone()),
        _ => None,
    };
    let collapse_at = d.collapse;
    let needs_entrance = recorded.is_some() && d.collapse.is_some() && d.entrance.is_none();
    let entrance = d.entrance;
    let frozen: Vec<f32> = d.frozen.iter().map(|&s| f32::from(s) / 255.).collect();
    let frozen_ms = d.frozen_ms;
    let live: Vec<u8> = match &d.phase {
        Phase::Recording(r) => r.samples(),
        _ => Vec::new(),
    };
    let focus = st.send_focus.clone();
    let focus_now = std::mem::take(&mut store(cx).focus_send);
    if focus_now && let Some(h) = &focus {
        window.focus(h, cx);
    }
    if needs_entrance && let Some(dr) = store(cx).draft.as_mut() {
        dr.entrance = Some(now);
    }
    let entrance = if needs_entrance { Some(now) } else { entrance };
    if recording && elapsed >= vm::MAX_DURATION {
        cx.spawn(async move |this, cx| {
            let _ = this.update(cx, |a, cx| stop_recording(a, channel, false, false, cx));
        })
        .detach();
    }
    let capture: AnyElement = match recorded {
        Some(r) => {
            let dur = r.duration_secs as f32;
            let v = view(cx, &key);
            let values: Vec<f32> = vm::waveform(&r.samples).iter().map(|&b| f32::from(b) / 255.).collect();
            let shown = if v.played { (dur - v.current).max(0.).ceil() } else { dur.ceil() };
            let k1 = key.clone();
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(z(16.))
                .child(
                    chat_button("vm-preview-play", if v.playing { "pause" } else { "play" }, if v.playing { crate::t!("voice_msg.pause") } else { crate::t!("voice_msg.play") })
                        .on_click(move |_, _, cx| toggle(&k1, None, cx)),
                )
                .child(
                    div().id("vm-preview-wave").flex_1().min_w_0().ml(z(4.)).h(z(32.)).my(z(-4.)).cursor_pointer().child(
                        wave_canvas(Wave {
                            key: key.clone(),
                            values,
                            fluid: true,
                            played: v.played,
                            frac: v.current / dur.max(0.001),
                            playing: v.playing,
                            brand: false,
                            entrance: if reduced { None } else { entrance },
                            duration: dur,
                        })
                        .size_full(),
                    ),
                )
                .child(div().flex_none().flex().items_center().child(timer_text(format_secs(shown as u64), c(tok(idx::TEXT_DEFAULT)))))
                .into_any_element()
        }
        None => {
            let (levels, ms) = if recording {
                (live.iter().map(|&s| f32::from(s) / 255.).collect::<Vec<f32>>(), elapsed.as_secs_f32() * 1000.)
            } else {
                (frozen, frozen_ms)
            };
            let warn = if !recording {
                None
            } else if elapsed >= vm::WARN_IMMINENT {
                Some(250u128)
            } else if elapsed >= vm::WARN_APPROACHING {
                Some(500u128)
            } else {
                None
            };
            let blink = warn.is_some_and(|p| {
                let period = if reduced { 1000 } else { p };
                (elapsed.as_millis() / period) % 2 == 1
            });
            let secs = if recording { elapsed.as_secs() } else { 0 };
            let ch = channel;
            let has_levels = !levels.is_empty() || recording;
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(z(16.))
                .when(recording, |d| {
                    d.child(chat_button("vm-stop", "stop", crate::t!("voice_msg.stop")).on_click(cx.listener(move |a, _, _, cx| stop_recording(a, ch, false, true, cx))))
                })
                .child(
                    div().flex_1().min_w_0().h(z(32.)).my(z(-4.)).overflow_hidden().child(
                        live_wave(
                            levels,
                            if has_levels { ms } else { 0. },
                            recording,
                            reduced,
                            if reduced { None } else { collapsing.or(collapse_at.map(|_| 100.)) },
                            c(tok(idx::INTERACTIVE_TEXT_ACTIVE)),
                        )
                        .size_full(),
                    ),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .child(div().flex_none().size(z(4.)).rounded_full().when(recording, |d| d.bg(c(tok(idx::TEXT_FEEDBACK_CRITICAL)))))
                        .child(timer_text(format_secs(secs), c(tok(if blink { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_DEFAULT })))),
                )
                .into_any_element()
        }
    };
    let ch = channel;
    let send_focused = focus.as_ref().is_some_and(|h| h.is_focused(window));
    let (send_hover, send_press) = cx.try_global::<Store>().map_or((false, false), |s| (s.send_hover, s.send_press));
    let send_target = c(tok(if !can_send {
        idx::ICON_MUTED
    } else if send_press {
        idx::BRAND_500
    } else if send_focused || send_hover {
        idx::BRAND_430
    } else {
        idx::TEXT_BRAND
    }));
    let send_color = faded(cx, "vm-send", send_target, SEND_FADE_MS, ease_in_out);
    let mut send_btn = div()
        .id("vm-send")
        .group("vm-send")
        .size(z(32.))
        .p(z(4.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(8.))
        .tooltip(crate::ui::tooltip::tip(crate::t!("voice_msg.send")))
        .when(can_send, |d| {
            d.cursor_pointer().on_click(cx.listener(|a, _, _, cx| {
                send(a, cx);
            }))
        })
        .when(!can_send, |d| d.cursor(gpui::CursorStyle::OperationNotAllowed))
        .on_hover(|over, window, cx| {
            store(cx).send_hover = *over;
            window.refresh();
        })
        .on_mouse_down(MouseButton::Left, |_, window, cx| {
            store(cx).send_press = true;
            window.refresh();
        })
        .on_mouse_up(MouseButton::Left, |_, window, cx| {
            store(cx).send_press = false;
            window.refresh();
        })
        .on_mouse_up_out(MouseButton::Left, |_, window, cx| {
            store(cx).send_press = false;
            window.refresh();
        })
        .child(svg().path(crate::icons::path("send-message")).size(z(20.)).flex_none().text_color(send_color));
    if let Some(h) = &focus {
        send_btn = send_btn.track_focus(h);
    }
    Some(
        div()
            .flex_1()
            .min_w_0()
            .min_h(z(56.))
            .pl(z(12.))
            .flex()
            .items_center()
            .gap(z(8.))
            .child(capture)
            .child(div().flex_none().w(z(1.)).h(z(22.)).bg(c(tok(idx::BORDER_SUBTLE))))
            .child(chat_button("vm-trash", "chat/trash", crate::t!("voice_msg.delete")).on_click(cx.listener(move |_, _, _, cx| {
                discard(cx, ch);
                cx.notify();
            })))
            .child(send_btn)
            .into_any_element(),
    )
}

fn ring_key(part: &str, key: &SharedString) -> u64 {
    crate::ui::states::key(("vm-ring", part, key.as_ref()))
}

const VOICE_SLIDER_LEFT: f32 = slider_left(-8.);
const ATTACHMENT_SLIDER_LEFT: f32 = slider_left(-4.);

const fn slider_left(margin_inline_start: f32) -> f32 {
    70. - 78. + margin_inline_start
}

fn volume_slider(key: SharedString, left: f32, cx: &App) -> AnyElement {
    let value = slider_value(cx, &key);
    let now = Instant::now();
    let (bar_hover, bar_at) = cx
        .try_global::<Store>()
        .and_then(|s| s.vol_bar.as_ref().filter(|(k, _, _)| *k == key).map(|(_, h, at)| (*h, Some(*at))))
        .unwrap_or((false, None));
    let dragging = cx.try_global::<Store>().is_some_and(|s| s.vol_drag.as_ref().is_some_and(|(k, _)| *k == key));
    let lit = bar_hover || dragging;
    let grow = match bar_at {
        Some(at) if ms_between(at, now) < 250. => {
            let t = ease_in_out(ms_between(at, now) / 250.);
            if lit { t } else { 1. - t }
        }
        _ => {
            if lit {
                1.
            } else {
                0.
            }
        }
    };
    let (k_hover, k_bar) = (key.clone(), key.clone());
    let k_down = key.clone();
    let track = gpui::canvas(
        |b, window, _| window.insert_hitbox(b, HitboxBehavior::Normal),
        move |b, hitbox, window, cx| {
            let x0 = b.origin.x;
            let y0 = b.origin.y;
            let (tx, ty, tw, th) = (f32::from(z(4.)), f32::from(z(8.)), f32::from(z(6.)), f32::from(z(72.)));
            let edge = f32::from(z(3.));
            let track_b = Bounds::new(gpui::point(x0 + px(tx), y0 + px(ty - edge)), gpui::size(px(tw), px(th + 2. * edge)));
            if lit {
                let sh = Bounds::new(gpui::point(track_b.origin.x + z(1.), track_b.origin.y), track_b.size);
                window.paint_quad(gpui::fill(sh, c(tok(idx::OPACITY_BLACK_28))).corner_radii(px(edge)));
            }
            let base = c(tok(idx::PRIMARY_300));
            window.paint_quad(gpui::fill(track_b, Hsla { a: base.a * 0.3, ..base }).corner_radii(px(edge)));
            let fill_h = value * th;
            let prog = Bounds::new(gpui::point(x0 + px(tx), y0 + px(ty + th - fill_h - edge)), gpui::size(px(tw), px(fill_h + 2. * edge)));
            window.paint_quad(gpui::fill(prog, c(tok(idx::BRAND_500))).corner_radii(px(edge)));
            if grow > 0. {
                let d = f32::from(z(10.)) * grow;
                let (gx, gy) = (tx + tw / 2., ty + th - fill_h);
                let gb = Bounds::new(gpui::point(x0 + px(gx - d / 2.), y0 + px(gy - d / 2.)), gpui::size(px(d), px(d)));
                let col = mix(c(tok(idx::BRAND_500)), c(tok(idx::BRAND_560)), if lit { grow } else { 0. });
                window.paint_quad(gpui::fill(gb, col).corner_radii(px(d / 2.)));
            }
            let pill_h = f32::from(b.size.height);
            let value_at = move |y: Pixels| ((f32::from(b.origin.y) + pill_h - f32::from(y)) / pill_h.max(1.)).clamp(0., 1.);
            let k = k_down.clone();
            window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble && e.button == MouseButton::Left && hitbox.is_hovered(window) {
                    cx.stop_propagation();
                    store(cx).vol_drag = Some((k.clone(), b));
                    set_slider(&k, value_at(e.position.y), cx);
                    ensure_ticker(cx);
                    window.refresh();
                }
            });
            if cx.try_global::<Store>().is_some_and(|s| s.vol_drag.as_ref().is_some_and(|(kk, _)| *kk == k_down)) {
                let k2 = k_down.clone();
                window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble {
                        set_slider(&k2, value_at(e.position.y), cx);
                        window.refresh();
                    }
                });
                window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble {
                        store(cx).vol_drag = None;
                        ensure_ticker(cx);
                        window.refresh();
                    }
                });
            }
        },
    );
    gpui::deferred(
        div()
            .id(gpui::ElementId::Name(format!("vm-volslider-{key}").into()))
            .absolute()
            .left(z(left))
            .top(z(-144.))
            .w(z(54.))
            .h(z(140.))
            .occlude()
            .on_hover(move |over, window, cx| {
                vol_hover(&k_hover, None, Some(*over), cx);
                window.refresh();
            })
            .child(
                div()
                    .id(gpui::ElementId::Name(format!("vm-volpill-{key}").into()))
                    .absolute()
                    .left(z(20.))
                    .top(z(52.))
                    .w(z(14.))
                    .h(z(88.))
                    .rounded(z(8.))
                    .bg(c(tok(idx::OPACITY_BLACK_68)))
                    .cursor_pointer()
                    .on_hover(move |over, window, cx| {
                        let st = store(cx);
                        let prev = st.vol_bar.as_ref().filter(|(k, _, _)| *k == k_bar).is_some_and(|(_, h, _)| *h);
                        if prev != *over {
                            st.vol_bar = Some((k_bar.clone(), *over, Instant::now()));
                            bump(cx, 270);
                        }
                        window.refresh();
                    })
                    .child(track.size_full()),
            ),
    )
    .with_priority(5)
    .into_any_element()
}

pub fn call_volume(user: Id, stream: bool, cx: &App) -> AnyElement {
    let key = call_volume_key(user, stream);
    let value = slider_value(cx, &key);
    let muted = crate::ui::call_view::participant_muted(user, stream);
    let vol_open = cx.try_global::<Store>().is_some_and(|s| {
        let open = s.vol_open.as_ref() == Some(&key) && s.vol_hide_at.is_none_or(|t| Instant::now() < t);
        open || s.vol_drag.as_ref().is_some_and(|(k, _)| *k == key) || s.vol_keys.as_ref() == Some(&key)
    });
    let (k_click, k_enter, k_keys) = (key.clone(), key.clone(), key.clone());
    let id = |s: &str| gpui::ElementId::Name(format!("{s}-{key}").into());
    div()
        .id(id("callvol"))
        .occlude()
        .relative()
        .flex_none()
        .size(z(24.))
        .child(
            div()
                .id(id("callvolbtn"))
                .size(z(24.))
                .cursor_pointer()
                .child(svg().path(crate::icons::path(volume_icon(muted, value))).size(z(24.)).flex_none().text_color(c(tok(idx::WHITE))))
                .on_click(move |_, window, cx| {
                    toggle_mute(&k_click, cx);
                    window.refresh();
                }),
        )
        .on_hover(move |over, window, cx| {
            vol_hover(&k_enter, Some(*over), None, cx);
            window.refresh();
        })
        .on_key_down(move |e: &KeyDownEvent, window, cx| {
            let k = e.keystroke.key.as_str();
            match k {
                "up" | "down" => {
                    cx.stop_propagation();
                    let st = store(cx);
                    if st.vol_keys.as_ref() != Some(&k_keys) {
                        st.vol_keys = Some(k_keys.clone());
                    } else {
                        let cur = slider_value(cx, &k_keys);
                        let next = if k == "up" { (cur + 0.05).min(1.) } else { (cur - 0.05).max(0.) };
                        set_slider(&k_keys, next, cx);
                    }
                    window.refresh();
                }
                "escape" if store(cx).vol_keys.take().is_some() => {
                    cx.stop_propagation();
                    window.refresh();
                }
                _ => {}
            }
        })
        .when(vol_open, |d| d.child(volume_slider(key.clone(), CALL_SLIDER_LEFT, cx)))
        .into_any_element()
}

const CALL_SLIDER_LEFT: f32 = slider_left(0.);

fn vol_hover(key: &SharedString, button: Option<bool>, slider: Option<bool>, cx: &mut App) {
    let st = store(cx);
    if st.vol_open.as_ref() != Some(key) {
        st.vol_in = (false, false);
    }
    if let Some(b) = button {
        st.vol_in.0 = b;
    }
    if let Some(s) = slider {
        st.vol_in.1 = s;
    }
    if st.vol_in.0 || st.vol_in.1 {
        st.vol_open = Some(key.clone());
        st.vol_hide_at = None;
    } else if st.vol_open.as_ref() == Some(key) {
        st.vol_hide_at = Some(Instant::now() + Duration::from_millis(150));
        ensure_ticker(cx);
    }
}

fn volume_icon(muted: bool, value: f32) -> &'static str {
    if muted || value <= 0. {
        "volume-off"
    } else if value < 0.5 {
        "voice-low"
    } else {
        "speaker"
    }
}

pub fn player(url: &SharedString, meta: &VoiceMeta, cx: &App) -> AnyElement {
    let key = url.clone();
    let mut duration = meta.duration_secs as f32;
    if let Some(Audio::Ready(p)) = audio_for(cx, &key) {
        let d = p.duration().as_secs_f32();
        if d > 0. {
            duration = d;
        }
    }
    let duration = duration.max(0.001);
    let v = view(cx, &key);
    let rate_ix = cx.try_global::<Store>().map_or(1, |s| s.rate_ix);
    let muted = cx.try_global::<Store>().is_some_and(|s| s.muted.contains(&key));
    let slider = slider_value(cx, &key);
    let reduced = crate::ui::settings::effects::reduced_motion();
    let values = decode_waveform(&meta.waveform);
    let shown = if v.played { (duration - v.current).max(0.).ceil() } else { duration.ceil() };
    let white = c(tok(idx::WHITE));
    let (k1, k_vol) = (key.clone(), key.clone());
    let u = url.clone();
    let id = |s: &str| gpui::ElementId::Name(format!("vm-{s}-{key}").into());
    let play_bg = if v.playing {
        gpui::Background::from(white)
    } else {
        gpui::linear_gradient(151.11, gpui::linear_color_stop(c(tok(idx::BRAND_460)), 0.1655), gpui::linear_color_stop(c(tok(idx::BRAND_560)), 1.0436))
    };
    let play_hover = gpui::linear_gradient(151.11, gpui::linear_color_stop(c(tok(idx::BRAND_400)), 0.1778), gpui::linear_color_stop(c(tok(idx::BRAND_460)), 0.8222));
    let vol_open = cx.try_global::<Store>().is_some_and(|s| {
        let open = s.vol_open.as_ref() == Some(&key) && s.vol_hide_at.is_none_or(|t| Instant::now() < t);
        open || s.vol_drag.as_ref().is_some_and(|(k, _)| *k == key) || s.vol_keys.as_ref() == Some(&key)
    });
    let vol_color = state_mix(cx, &key, v.playing, c(tok(idx::INTERACTIVE_TEXT_DEFAULT)), c(tok(idx::BRAND_200)), ease_in_out);
    let vol_hover_fg = if v.playing { white } else { c(tok(idx::INTERACTIVE_TEXT_HOVER)) };
    let vol_active = if v.playing { c(tok(idx::BRAND_200)) } else { c(tok(idx::INTERACTIVE_TEXT_ACTIVE)) };
    let rate_group: SharedString = format!("vm-rate-{key}").into();
    let vol_group: SharedString = format!("vm-vol-{key}").into();
    let play_group: SharedString = format!("vm-play-{key}").into();
    let rate_bg = state_mix(cx, &key, v.playing, c(tok(idx::BACKGROUND_MOD_SUBTLE)), c(tok(idx::INTERACTIVE_BACKGROUND_ACTIVE)), ease_in);
    let rate_fg = state_mix(cx, &key, v.playing, c(tok(idx::INTERACTIVE_TEXT_DEFAULT)), c(tok(idx::BRAND_200)), ease_in);
    let rate_hover_fg = if v.playing { white } else { c(tok(idx::INTERACTIVE_TEXT_ACTIVE)) };
    let duration_color = state_mix(cx, &key, v.playing, c(tok(idx::INTERACTIVE_TEXT_DEFAULT)), white, ease_in_out);
    let rate_hover_bg = c(tok(idx::INTERACTIVE_BACKGROUND_HOVER));
    let rate_id: SharedString = format!("rate-{key}").into();
    let vol_id: SharedString = format!("vol-{key}").into();
    let rate_bg = hover_fade(cx, &rate_id, rate_hover_bg, rate_bg, ease_in);
    let rate_fg = hover_fade(cx, &rate_id, rate_hover_fg, rate_fg, ease_in);
    let vol_color = hover_fade(cx, &vol_id, vol_hover_fg, vol_color, ease_in_out);
    let pressed_bg = if v.playing { gpui::Background::from(c(tok(idx::BRAND_360))) } else { play_bg };
    let (icon_rest, icon_pressed) = if v.playing { (c(tok(idx::BRAND_500)), c(tok(idx::BRAND_500))) } else { (white, c(tok(idx::BRAND_360))) };
    let play = div()
        .id(id("play"))
        .group(play_group.clone())
        .relative()
        .size(z(32.))
        .ml(z(-4.))
        .my(z(-4.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .cursor_pointer()
        .child(
            div()
                .id("vm-play-circle")
                .relative()
                .size(z(32.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .overflow_hidden()
                .bg(play_bg)
                .when(!v.playing, |d| d.group_hover(play_group.clone(), move |s| s.bg(play_hover)))
                .group_active(play_group.clone(), move |s| s.size(z(28.)).bg(pressed_bg))
                .when(!v.playing, |d| {
                    d.child(
                        div()
                            .id("vm-play-shade")
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .rounded_full()
                            .group_active(play_group.clone(), |s| s.bg(gpui::hsla(0., 0., 0., 0.2))),
                    )
                })
                .child(
                    svg()
                        .path(crate::icons::path(if v.playing { "pause" } else { "play" }))
                        .size(z(18.))
                        .flex_none()
                        .text_color(icon_rest)
                        .id("vm-play-icon")
                        .group_active(play_group.clone(), move |s| s.size(z(15.75)).text_color(icon_pressed)),
                ),
        )
        .on_click(move |_, window, cx| {
            toggle(&k1, Some(&u), cx);
            window.refresh();
        });
    let rate = div()
        .id(id("rate"))
        .relative()
        .group(rate_group.clone())
        .flex_none()
        .py(z(4.))
        .cursor_pointer()
        .child(
            div()
                .w(z(32.))
                .rounded(z(4.))
                .bg(rate_bg)
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rate_fg)
                .flex()
                .justify_center()
                .group_hover(rate_group, move |s| s.bg(rate_hover_bg).text_color(rate_hover_fg))
                .child(SharedString::from(rate_label(vm::PLAYBACK_RATES[rate_ix]))),
        )
        .on_hover(move |over, window, cx| {
            set_hovered(cx, &rate_id, *over);
            window.refresh();
        })
        .on_click(|_, _, cx| {
            cycle_rate(cx);
            cx.refresh_windows();
        });
    let k_enter = key.clone();
    let k_keys = key.clone();
    let vol = div()
        .id(id("vol"))
        .relative()
        .flex_none()
        .size(z(24.))
        .child(
            div()
                .id(id("volbtn"))
                .group(vol_group.clone())
                .size(z(24.))
                .cursor_pointer()
                .child(
                    svg()
                        .path(crate::icons::path(volume_icon(muted, slider)))
                        .size(z(24.))
                        .flex_none()
                        .text_color(vol_color)
                        .group_hover(vol_group.clone(), move |s| s.text_color(vol_hover_fg))
                        .id("vm-vol-icon")
                        .group_active(vol_group, move |s| s.text_color(vol_active)),
                )
                .on_hover(move |over, window, cx| {
                    set_hovered(cx, &vol_id, *over);
                    window.refresh();
                })
                .on_click(move |_, window, cx| {
                    toggle_mute(&k_vol, cx);
                    window.refresh();
                }),
        )
        .on_hover(move |over, window, cx| {
            vol_hover(&k_enter, Some(*over), None, cx);
            window.refresh();
        })
        .on_key_down(move |e: &KeyDownEvent, window, cx| {
            let k = e.keystroke.key.as_str();
            match k {
                "up" | "down" => {
                    cx.stop_propagation();
                    let st = store(cx);
                    if st.vol_keys.as_ref() != Some(&k_keys) {
                        st.vol_keys = Some(k_keys.clone());
                    } else {
                        let cur = slider_value(cx, &k_keys);
                        let next = if k == "up" { (cur + 0.05).min(1.) } else { (cur - 0.05).max(0.) };
                        set_slider(&k_keys, next, cx);
                    }
                    window.refresh();
                }
                "escape" => {
                    let st = store(cx);
                    if st.vol_keys.take().is_some() {
                        cx.stop_propagation();
                        window.refresh();
                    }
                }
                _ => {}
            }
        })
        .when(vol_open, |d| d.child(volume_slider(key.clone(), VOICE_SLIDER_LEFT, cx)));
    div()
        .id(id("player"))
        .mt(z(4.))
        .self_start()
        .flex_none()
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .gap(z(12.))
        .p(z(12.))
        .max_w_full()
        .min_w_0()
        .rounded(z(24.))
        .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left_0()
                .right_0()
                .rounded(z(24.))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .child(ripple_canvas(key.clone(), reduced).size_full()),
        )
        .child(crate::ui::focus_ring::ring(play, ring_key("play", &key), 16.))
        .child(
            div().id(id("wave")).relative().flex_none().w(z(canvas_width(f64::from(duration)))).h(z(32.)).my(z(-4.)).cursor_pointer().child(
                wave_canvas(Wave {
                    key: key.clone(),
                    values,
                    fluid: false,
                    played: v.played,
                    frac: v.current / duration,
                    playing: v.playing,
                    brand: true,
                    entrance: None,
                    duration,
                })
                .size_full(),
            ),
        )
        .child(
            div()
                .relative()
                .flex_none()
                .text_size(z(14.))
                .line_height(z(18.))
                .text_color(duration_color)
                .child(SharedString::from(format_secs(shown as u64))),
        )
        .child(crate::ui::focus_ring::ring(rate, ring_key("rate", &key), 4.))
        .child(crate::ui::focus_ring::ring(vol, ring_key("vol", &key), 4.))
        .into_any_element()
}

pub fn shot_state(app: &PhoenixApp, spec: &str, cx: &mut App) {
    let Some(channel) = app.selected_channel else { return };
    let found = app.messages.get(&channel).and_then(|e| {
        e.msgs.iter().rev().find_map(|m| m.attachments.iter().find_map(|a| a.voice.as_ref().map(|v| (a.url.clone(), v.duration_secs as f32))))
    });
    for part in spec.split(',').map(str::trim) {
        let (what, arg) = part.split_once(':').unwrap_or((part, ""));
        let f = arg.parse::<f32>().unwrap_or(0.).clamp(0., 1.);
        match (what, &found) {
            ("pos", Some((key, dur))) => {
                store(cx).positions.insert(key.clone(), f * dur);
            }
            ("play", Some((key, dur))) => {
                let rate = 48_000;
                let silent = Pcm { rate, channels: 1, samples: vec![0.; rate as usize] };
                let handle = effects::play(Arc::new(silent), 0., true);
                let st = store(cx);
                st.muted.insert(key.clone());
                st.playing = Some(Playing { key: key.clone(), handle, from: f * dur, at: Instant::now(), rate: 0.0001, duration: *dur });
                ensure_ticker(cx);
            }
            ("volume", _) => {
                set_slider(&SharedString::default(), f, cx);
            }
            ("modal", _) => {
                if let Ok(from) = arg.parse::<u64>() {
                    store(cx).modal = Some((Id(from), channel));
                }
            }
            _ => {}
        }
    }
}

pub fn close_modal(cx: &mut App) -> bool {
    let taken = cx.try_global::<Store>().is_some_and(|s| s.modal.is_some());
    if taken {
        store(cx).modal = None;
    }
    taken
}

fn flow_text(text: &str) -> Vec<AnyElement> {
    text.split(' ')
        .filter(|w| !w.is_empty())
        .map(|w| {
            div().flex_none().whitespace_nowrap().child(SharedString::from(format!("{w}\u{a0}"))).into_any_element()
        })
        .collect()
}

pub fn draft_modal(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    use crate::ui::controls::{BtnKind, button};
    let (from, target) = cx.try_global::<Store>()?.modal?;
    let ch = app.channels.get(&from)?;
    let tgt_guild = app.channels.get(&target).and_then(|c| c.guild);
    let is_dm = ch.guild.is_none();
    let group = is_dm && crate::ui::dm_profile_panel::is_group_dm(app, from);
    let recipients: Vec<&crate::model::User> = ch.voice_users.iter().filter_map(|u| app.users.get(u)).collect();
    let peer = recipients.first().copied();
    let strong = c(tok(idx::TEXT_STRONG));
    let mut sub = div().flex().flex_wrap().items_center();
    if group {
        for w in flow_text(&crate::t!("voice_msg.unsent_body_group")) {
            sub = sub.child(w);
        }
    } else if is_dm {
        let name: SharedString = peer.map(|u| u.display_name.clone()).filter(|n| !n.is_empty()).or_else(|| peer.map(|u| u.name.clone())).unwrap_or_else(|| ch.name.clone());
        for w in flow_text(&crate::t!("voice_msg.unsent_body_dm")) {
            sub = sub.child(w);
        }
        sub = sub.child(div().flex_none().whitespace_nowrap().font_weight(FontWeight::BOLD).text_color(strong).child(name));
    } else {
        for w in flow_text(&crate::t!("voice_msg.unsent_body")) {
            sub = sub.child(w);
        }
        sub = sub.child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(z(4.))
                .whitespace_nowrap()
                .text_color(strong)
                .child(svg().path(crate::ui::sidebar::channel_icon(ch.kind).path()).size(z(12.)).flex_none().text_color(strong))
                .child(div().font_weight(FontWeight::BOLD).child(ch.name.clone())),
        );
    }
    let show_card = is_dm || ch.guild != tgt_guild;
    let entity = cx.entity();
    let go_back = {
        let e = entity.clone();
        std::rc::Rc::new(move |_: &mut Window, cx: &mut App| {
            store(cx).modal = None;
            e.update(cx, |a, cx| {
                a.open_channel_chat(from, cx);
                cx.notify();
            });
        })
    };
    let close: crate::ui::modal::Action = {
        let e = entity.clone();
        std::rc::Rc::new(move |_: &mut Window, cx: &mut App| {
            store(cx).modal = None;
            e.update(cx, |_, cx| cx.notify());
        })
    };
    let drop_draft = {
        let e = entity.clone();
        std::rc::Rc::new(move |_: &mut Window, cx: &mut App| {
            store(cx).modal = None;
            discard(cx, from);
            e.update(cx, |_, cx| cx.notify());
        })
    };
    let card_el = show_card.then(|| {
        let (icon, primary, secondary): (AnyElement, SharedString, Option<(bool, SharedString)>) = if let Some(gid) = ch.guild {
            let g = app.guilds.iter().find(|g| g.id == gid);
            (
                div()
                    .size(z(32.))
                    .rounded(z(8.))
                    .overflow_hidden()
                    .child(crate::ui::invite::guild_icon(g.map(|g| &g.icon), g.map_or("", |g| g.name.as_ref()), 32. * crate::theme::zoom()))
                    .into_any_element(),
                g.map(|g| g.name.clone()).unwrap_or_default(),
                Some((true, ch.name.clone())),
            )
        } else if group {
            let tags: Vec<String> = recipients.iter().map(|u| u.name.to_string()).collect();
            (crate::ui::group_dm::group_avatar(app, from, 32. * crate::theme::zoom(), gpui::transparent_black()), ch.name.clone(), Some((false, tags.join(", ").into())))
        } else {
            (
                crate::ui::avatar(peer, 32.),
                peer.map(|u| if u.display_name.is_empty() { u.name.clone() } else { u.display_name.clone() }).unwrap_or_else(|| ch.name.clone()),
                peer.map(|u| (false, u.name.clone())),
            )
        };
        let back = go_back.clone();
        crate::ui::focus_ring::ring(
            div()
                .id("vm-unsent-destination")
                .mx(z(-8.))
                .px(z(8.))
                .min_h(z(48.))
                .min_w_0()
                .rounded(z(8.))
                .flex()
                .items_center()
                .gap(z(12.))
                .overflow_hidden()
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                .on_click(move |_, w, cx| back(w, cx))
                .child(div().flex_none().child(icon))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .justify_center()
                        .gap(z(2.))
                        .overflow_hidden()
                        .child(
                            div().min_w_0().truncate().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_DEFAULT))).child(primary),
                        )
                        .children(secondary.filter(|(_, s)| !s.is_empty()).map(|(chan, s)| {
                            div()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(z(2.))
                                .overflow_hidden()
                                .text_color(c(tok(idx::TEXT_MUTED)))
                                .when(chan, |d| d.child(svg().path(crate::ui::sidebar::channel_icon(ch.kind).path()).size(z(12.)).flex_none().text_color(c(tok(idx::TEXT_MUTED)))))
                                .child(div().flex_1().min_w_0().truncate().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).child(s))
                        })),
                ),
            crate::ui::states::key("vm-unsent-destination"),
            8.,
        )
        .into_any_element()
    });
    let back2 = go_back.clone();
    let drop2 = drop_draft.clone();
    let close2 = close.clone();
    let card = div()
        .relative()
        .w(z(400.))
        .rounded(z(12.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.24),
            offset: gpui::point(z(0.), z(12.)),
            blur_radius: z(24.),
            spread_radius: z(0.),
            inset: false,
        }])
        .pt(z(8.))
        .pb(z(16.))
        .flex()
        .flex_col()
        .child(
            div()
                .pt(z(16.))
                .px(z(24.))
                .pb(z(8.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(div().pr(z(36.)).text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(strong).child(crate::t!("voice_msg.unsent_title")))
                .child(sub.text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_SUBTLE)))),
        )
        .children(card_el.map(|el| div().pt(z(16.)).pb(z(8.)).px(z(24.)).flex().flex_col().child(el)))
        .child(
            crate::ui::focus_ring::ring(
                div()
                    .id("vm-unsent-close")
                    .absolute()
                    .top(z(8.))
                    .right(z(8.))
                    .size(z(40.))
                    .rounded(z(8.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
                    .on_click(move |_, w, cx| close2(w, cx))
                    .child(svg().path(crate::icons::path("close")).size(z(20.)).text_color(c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT)))),
                crate::ui::states::key("vm-unsent-close"),
                8.,
            ),
        )
        .child(
            div()
                .pt(z(16.))
                .px(z(24.))
                .pb(z(8.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(crate::ui::focus_ring::ring(
                    button("vm-unsent-return", crate::t!("voice_msg.unsent_return"), BtnKind::Secondary).relative().w_full().on_click(move |_, w, cx| back2(w, cx)),
                    crate::ui::states::key("vm-unsent-return"),
                    8.,
                ))
                .child(crate::ui::focus_ring::ring(
                    button("vm-unsent-discard", crate::t!("voice_msg.unsent_discard"), BtnKind::Critical).relative().w_full().on_click(move |_, w, cx| drop2(w, cx)),
                    crate::ui::states::key("vm-unsent-discard"),
                    8.,
                )),
        );
    Some(crate::ui::modal::frame("vm-unsent-backdrop", close, card))
}

pub fn ensure_metadata(url: &SharedString, cx: &mut App) {
    let key = attachment_key(url);
    let st = store(cx);
    if st.meta.contains_key(&key) {
        return;
    }
    st.meta.insert(key.clone(), None);
    let http = cx.http_client();
    let u = url.to_string();
    cx.spawn(async move |cx| {
        use futures::AsyncReadExt;
        use gpui::http_client::{AsyncBody, Request};
        let req = Request::builder().uri(u.as_str()).header("Range", crate::media::audio_file::METADATA_RANGE).body(AsyncBody::default());
        let mut bytes = Vec::new();
        let ok = match req {
            Ok(req) => match http.send(req).await {
                Ok(r) if r.status().is_success() => r.into_body().read_to_end(&mut bytes).await.is_ok(),
                _ => false,
            },
            Err(_) => false,
        };
        let name = file_name(&u);
        let secs = if ok { cx.background_spawn(async move { crate::media::audio_file::probe_duration(&bytes, &name) }).await } else { None };
        cx.update(|cx| {
            if let Some(d) = secs {
                let st = store(cx);
                if st.meta.get(&key).is_some_and(Option::is_none) {
                    st.meta.insert(key.clone(), Some(d as f32));
                }
            }
            cx.refresh_windows();
        });
    })
    .detach();
}

pub struct AttachmentView {
    pub current: Option<f32>,
    pub duration: Option<f32>,
    pub playing: bool,
    pub ended: bool,
}

pub fn attachment_view(cx: &App, url: &str) -> AttachmentView {
    let key = attachment_key(url);
    let Some(st) = cx.try_global::<Store>() else { return AttachmentView { current: None, duration: None, playing: false, ended: false } };
    let duration = match st.audio.get(&key) {
        Some(Audio::Ready(p)) => Some(p.duration().as_secs_f32()),
        _ => st.meta.get(&key).copied().flatten(),
    };
    let v = view(cx, &key);
    let current = duration.map(|d| v.current.min(d));
    let ended = !v.playing && duration.is_some_and(|d| v.played && v.current >= d - 0.01);
    AttachmentView { current, duration, playing: v.playing, ended }
}

pub fn playback(cx: &App) -> Option<f32> {
    cx.try_global::<Store>()?.playing.as_ref().map(Playing::position)
}

pub fn attachment_icon(v: &AttachmentView) -> &'static str {
    if v.playing {
        "pause"
    } else if v.ended {
        "media/replay"
    } else {
        "play"
    }
}

pub fn attachment_toggle(url: &SharedString, cx: &mut App) {
    let key = attachment_key(url);
    toggle(&key, Some(url), cx);
}

fn control(id: String, icon: &'static str) -> gpui::Stateful<gpui::Div> {
    control_sized(id, icon, 24., 16.)
}

fn control_sized(id: String, icon: &'static str, size: f32, glyph: f32) -> gpui::Stateful<gpui::Div> {
    let group = SharedString::from(id);
    div()
        .id(gpui::ElementId::Name(group.clone()))
        .group(group.clone())
        .flex_none()
        .size(z(size))
        .p(z(4.))
        .cursor_pointer()
        .active(|s| s.mt(z(1.)).mb(z(-1.)))
        .child(svg().path(crate::icons::path(icon)).flex_none().size(z(glyph)).text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE))).group_hover(group, |s| s.text_color(gpui::white())))
}

fn attachment_track(key: SharedString, frac: f32, duration: Option<f32>, cx: &App) -> gpui::Canvas<Hitbox> {
    let hover = cx.try_global::<Store>().and_then(|s| s.track_hover.clone()).filter(|(k, _)| *k == key).map(|(_, f)| f);
    let dragging = cx.try_global::<Store>().and_then(|s| s.seek.as_ref()).is_some_and(|s| s.key == key);
    gpui::canvas(
        |b, window, _| window.insert_hitbox(b, HitboxBehavior::Normal),
        move |b, hitbox, window, cx| {
            let h6 = f32::from(z(6.));
            let edge = f32::from(z(3.));
            let w = f32::from(b.size.width);
            let y = f32::from(b.origin.y) + (f32::from(b.size.height) - h6) / 2.;
            let x0 = f32::from(b.origin.x);
            let base = c(tok(idx::PRIMARY_300));
            let bar = Bounds::new(gpui::point(px(x0 - edge), px(y)), gpui::size(px(w + 2. * edge), px(h6)));
            let lit = hover.is_some() || dragging;
            if lit {
                let sh = Bounds::new(gpui::point(bar.origin.x, bar.origin.y + z(1.)), bar.size);
                window.paint_quad(gpui::fill(sh, c(tok(idx::OPACITY_BLACK_28))).corner_radii(px(edge)));
            }
            window.paint_quad(gpui::fill(bar, Hsla { a: base.a * 0.3, ..base }).corner_radii(px(edge)));
            if let Some(hf) = hover.filter(|_| !dragging) {
                let pv = Bounds::new(bar.origin, gpui::size(px(w * hf + 2. * edge), px(h6)));
                window.paint_quad(gpui::fill(pv, Hsla { a: 0.3, ..gpui::white() }).corner_radii(px(edge)));
            }
            let prog = Bounds::new(bar.origin, gpui::size(px(w * frac + 2. * edge), px(h6)));
            window.paint_quad(gpui::fill(prog, c(tok(idx::BRAND_500))).corner_radii(px(edge)));
            if lit {
                let d = f32::from(z(10.));
                let gx = x0 + w * frac;
                let gb = Bounds::new(gpui::point(px(gx - d / 2.), px(y + h6 / 2. - d / 2.)), gpui::size(px(d), px(d)));
                window.paint_quad(gpui::fill(gb, c(tok(idx::BRAND_560))).corner_radii(px(d / 2.)));
            }
            let (k, hb) = (key.clone(), hitbox.clone());
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                let over = hb.is_hovered(window);
                let st = store(cx);
                let next = over.then(|| (k.clone(), frac_in(b, e.position.x)));
                let had = st.track_hover.as_ref().is_some_and(|(kk, _)| *kk == k);
                if next.is_some() || had {
                    st.track_hover = next;
                    window.refresh();
                }
            });
            let Some(duration) = duration else { return };
            let k = key.clone();
            window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble && e.button == MouseButton::Left && hitbox.is_hovered(window) {
                    cx.stop_propagation();
                    seek_start(&k, frac_in(b, e.position.x), duration, b, cx);
                    window.refresh();
                }
            });
            if cx.try_global::<Store>().and_then(|s| s.seek.as_ref()).is_some_and(|s| s.key == key) {
                let drag_b = cx.try_global::<Store>().and_then(|s| s.seek.as_ref()).map_or(b, |s| s.bounds);
                window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture {
                        return;
                    }
                    let Some(s) = cx.try_global::<Store>().and_then(|s| s.seek.as_ref()) else { return };
                    let (k, d) = (s.key.clone(), s.duration);
                    seek_to(&k, frac_in(drag_b, e.position.x), d, cx);
                    window.refresh();
                });
                window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble {
                        seek_end(cx);
                    }
                });
            }
        },
    )
}

pub fn track_bubble(text: String, frac: f32) -> AnyElement {
    div()
        .absolute()
        .top(z(-28.))
        .left(gpui::relative(frac))
        .child(
            div()
                .relative()
                .left(z(-24.))
                .w(z(48.))
                .flex()
                .flex_col()
                .items_center()
                .child(
                    div()
                        .h(z(18.))
                        .px(z(8.))
                        .rounded(z(3.))
                        .bg(c(tok(idx::BLACK)))
                        .text_size(z(12.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(tok(idx::PRIMARY_100)))
                        .child(text),
                )
                .child(
                    gpui::canvas(
                        |_, _, _| {},
                        |b, _, window, _| {
                            let (x, y, w, h) = (b.origin.x, b.origin.y, b.size.width, b.size.height);
                            let mut pb = gpui::PathBuilder::fill();
                            pb.move_to(gpui::point(x, y));
                            pb.line_to(gpui::point(x + w, y));
                            pb.line_to(gpui::point(x + w / 2., y + h));
                            pb.close();
                            if let Ok(path) = pb.build() {
                                window.paint_path(path, c(tok(idx::BLACK)));
                            }
                        },
                    )
                    .w(z(10.))
                    .h(z(5.)),
                ),
        )
        .into_any_element()
}

pub fn media_key_volume(cx: &App, key: &SharedString) -> f32 {
    cx.try_global::<Store>().map_or(1., |s| effective_volume(s, key))
}

pub fn media_volume_control(key: SharedString, id: String, size: f32, glyph: f32, cx: &App) -> AnyElement {
    let muted = cx.try_global::<Store>().is_some_and(|s| s.muted.contains(&key));
    let slider = slider_value(cx, &key);
    let vol_open = cx.try_global::<Store>().is_some_and(|s| {
        let open = s.vol_open.as_ref() == Some(&key) && s.vol_hide_at.is_none_or(|t| Instant::now() < t);
        open || s.vol_drag.as_ref().is_some_and(|(k, _)| *k == key) || s.vol_keys.as_ref() == Some(&key)
    });
    let (k_mute, k_enter, k_keys) = (key.clone(), key.clone(), key.clone());
    div()
        .id(gpui::ElementId::Name(format!("{id}-vol").into()))
        .relative()
        .flex_none()
        .size(z(size))
        .child(control_sized(format!("{id}-volbtn"), volume_icon(muted, slider), size, glyph).on_click(move |_, window, cx| {
            toggle_mute(&k_mute, cx);
            window.refresh();
        }))
        .on_hover(move |over, window, cx| {
            vol_hover(&k_enter, Some(*over), None, cx);
            window.refresh();
        })
        .on_key_down(move |e: &KeyDownEvent, window, cx| {
            let k = e.keystroke.key.as_str();
            if k == "up" || k == "down" {
                cx.stop_propagation();
                let cur = slider_value(cx, &k_keys);
                let next = if k == "up" { (cur + 0.05).min(1.) } else { (cur - 0.05).max(0.) };
                set_slider(&k_keys, next, cx);
                window.refresh();
            }
        })
        .when(vol_open, |d| d.child(volume_slider(key, ATTACHMENT_SLIDER_LEFT, cx)))
        .into_any_element()
}

pub fn attachment_controls(url: &SharedString, cx: &App) -> AnyElement {
    let key = attachment_key(url);
    let v = attachment_view(cx, url);
    let (cur, dur) = crate::media::audio_file::duration_text(v.current.map(f64::from), v.duration.map(f64::from));
    let frac = match (v.current, v.duration) {
        (Some(c), Some(d)) if d > 0. => (c / d).clamp(0., 1.),
        _ => 0.,
    };
    let hover = cx.try_global::<Store>().and_then(|s| s.track_hover.clone()).filter(|(k, _)| *k == key).map(|(_, f)| f);
    let dragging = cx.try_global::<Store>().and_then(|s| s.seek.as_ref()).is_some_and(|s| s.key == key);
    let bubble = match (if dragging { Some(frac) } else { hover }, v.duration) {
        (Some(f), Some(d)) => Some((crate::media::audio_file::format_time(f64::from(f * d)), f)),
        _ => None,
    };
    let u1 = url.clone();
    let play = control(format!("aplay-{url}"), attachment_icon(&v)).on_click(move |_, window, cx| {
        attachment_toggle(&u1, cx);
        window.refresh();
    });
    let time = div()
        .flex_none()
        .whitespace_nowrap()
        .m(z(4.))
        .h(z(12.))
        .flex()
        .font_family(crate::theme::FONT_MONO())
        .text_size(z(12.))
        .line_height(z(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(gpui::white())
        .child(cur)
        .child(div().mx(z(2.)).child("/"))
        .child(dur);
    let track = div()
        .relative()
        .flex_1()
        .h(z(24.))
        .mx(z(7.))
        .cursor_pointer()
        .child(attachment_track(key.clone(), frac, v.duration, cx).size_full())
        .children(bubble.map(|(t, f)| track_bubble(t, f)));
    let vol = media_volume_control(key.clone(), format!("a{url}"), 24., 16., cx);
    div()
        .id(gpui::ElementId::Name(format!("acontrols-{url}").into()))
        .mt(z(4.))
        .w_full()
        .flex()
        .items_center()
        .rounded(z(crate::theme::generated::global::RADIUS_SM))
        .bg(c(tok(idx::OPACITY_BLACK_40)))
        .child(play)
        .child(time)
        .child(track)
        .child(vol)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_format_matches_client() {
        assert_eq!(format_secs(0), "0:00");
        assert_eq!(format_secs(9), "0:09");
        assert_eq!(format_secs(75), "1:15");
        assert_eq!(format_secs(1199), "19:59");
    }

    #[test]
    fn canvas_width_follows_duration() {
        assert_eq!(canvas_width(0.2), 38.);
        assert_eq!(canvas_width(60.), 290.);
        assert_eq!(canvas_width(4.), 56.);
        assert_eq!((canvas_width(10.) + 4.) % 6., 0.);
    }

    #[test]
    fn rate_labels() {
        let l: Vec<String> = vm::PLAYBACK_RATES.iter().map(|&r| rate_label(r)).collect();
        assert_eq!(l, ["0.75X", "1X", "1.5X", "2X"].map(|s| s.strip_prefix('0').unwrap_or(s).to_string()));
        assert_eq!(rate_label(0.75), ".75X");
    }

    #[test]
    fn waveform_fit_pads_or_interpolates() {
        assert_eq!(fit_waveform(&[1., 0.5], 4, false), vec![1., 0.5, 0., 0.]);
        assert_eq!(fit_waveform(&[0., 1.], 3, true), vec![0., 0.5, 1.]);
        assert_eq!(fit_waveform(&[1., 1., 0., 0.], 2, false), vec![1., 0.]);
        assert_eq!(fit_waveform(&[], 2, true), vec![0., 0.]);
    }

    #[test]
    fn waveform_base64_roundtrip() {
        let s: Vec<u8> = (0..20).map(|i| i * 10).collect();
        let b = waveform_b64(&s);
        let back = decode_waveform(&b);
        assert_eq!(back.len(), 20);
        assert!((back[1] - 10. / 255.).abs() < 1e-6);
    }

    #[test]
    fn easings_match_the_client_library() {
        assert!((ease_out_back(500., 0., 1., 500., 4.) - 1.).abs() < 1e-6);
        assert!(ease_out_back(250., 0., 1., 500., 4.) > 1.);
        assert_eq!(ease_out_back(0., 0., 1., 100., 2.), 0.);
        assert!((ease_out_quint(150., 1., 0., 150.)).abs() < 1e-6);
        assert!((ease_out_quint(75., 1., 0., 150.) - 0.03125).abs() < 1e-6);
    }

    #[test]
    fn bars_grow_with_overshoot_and_shrink_fast() {
        let t0 = Instant::now();
        let mut b = Bar::new(0.);
        b.animate_to(0.5, t0);
        assert!(b.current(t0 + Duration::from_millis(200)) > 0.5);
        assert_eq!(b.current(t0 + Duration::from_millis(501)), 0.5);
        assert!(b.start.is_none());
        b.reset(t0);
        assert!(b.reset);
        assert!(b.current(t0 + Duration::from_millis(100)) < 0.01);
        assert_eq!(b.current(t0 + Duration::from_millis(151)), 0.);
    }

    #[test]
    fn volume_popout_offsets_follow_the_wrapper_margin() {
        assert_eq!(VOICE_SLIDER_LEFT, -16.);
        assert_eq!(ATTACHMENT_SLIDER_LEFT, -12.);
    }

    #[test]
    fn volume_curve_matches_the_client() {
        assert_eq!(perceptual(0.), 0.);
        assert!((perceptual(1.) - 1.).abs() < 1e-6);
        assert!((amplitude(perceptual(0.18)) - 0.18).abs() < 1e-5);
        assert!((perceptual(0.18) - 0.5419).abs() < 1e-3);
        assert_eq!(volume_icon(true, 1.), "volume-off");
        assert_eq!(volume_icon(false, 0.), "volume-off");
        assert_eq!(volume_icon(false, 0.4), "voice-low");
        assert_eq!(volume_icon(false, 0.5), "speaker");
    }

    #[test]
    fn transitions_drop_alpha_until_they_end() {
        let a = gpui::hsla(0., 0., 0.5, 0.2);
        let b = gpui::hsla(0., 0., 1., 1.);
        assert_eq!(hex_lerp(a, b, 0.5).a, 1.);
        assert_eq!(hex_lerp(b, a, 1.).a, 0.2);
    }

    #[test]
    fn entrance_staggers_bars_over_160_ms() {
        assert_eq!(mount_factor(None, 3, 10), 1.);
        assert_eq!(mount_factor(Some(0.), 0, 10), 0.);
        assert!(mount_factor(Some(50.), 0, 10) > mount_factor(Some(50.), 9, 10));
        assert!((mount_factor(Some(160.), 9, 10) - 1.).abs() < 1e-6);
    }

    #[test]
    fn hover_is_instant_and_leaving_fades_back_over_200_ms() {
        let hover = gpui::hsla(0., 0., 1., 1.);
        let rest = gpui::hsla(0., 0., 0., 1.);
        assert_eq!(hover_out(None, true, hover, rest, ease_in), hover);
        assert_eq!(hover_out(Some(0.), false, hover, rest, ease_in), hover);
        let mid = hover_out(Some(100.), false, hover, rest, ease_in);
        assert!(mid.l > 0. && mid.l < 1.);
        assert_eq!(hover_out(Some(HOVER_OUT_MS), false, hover, rest, ease_in), rest);
        assert_eq!(hover_out(None, false, hover, rest, ease_in), rest);
    }

    #[test]
    fn send_colour_fades_over_250_ms() {
        let now = Instant::now();
        let f = Fade { from: gpui::hsla(0., 0., 0., 1.), to: gpui::hsla(0., 0., 1., 1.), at: now };
        assert_eq!(fade_shown(&f, now, SEND_FADE_MS, ease_in_out), f.from);
        let half = fade_shown(&f, now + Duration::from_millis(125), SEND_FADE_MS, ease_in_out);
        assert!((half.l - 0.5).abs() < 0.05, "{half:?}");
        assert_eq!(fade_shown(&f, now + Duration::from_millis(250), SEND_FADE_MS, ease_in_out), f.to);
    }
}
