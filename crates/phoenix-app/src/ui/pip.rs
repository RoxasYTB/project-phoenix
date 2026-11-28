use std::cell::Cell;

use gpui::{
    AnyElement, Context, FontWeight, Hsla, MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement, SharedString,
    Styled, Window, div, prelude::*,
};

use crate::app::PhoenixApp;
use crate::assets::Icon;
use crate::model::Id;
use crate::theme::{DEFAULT_AVATAR, c, generated::idx, tok, z, zoom};
use crate::ui::tooltip::Tipped as _;

pub const PIP_W: f32 = 320.;
pub const PIP_H: f32 = 180.;
pub const EDGE_TOP: f32 = 80.;
pub const EDGE_BOTTOM: f32 = 132.;
pub const EDGE_LEFT: f32 = 80.;
pub const EDGE_RIGHT: f32 = 80.;
const DRAG_THRESHOLD_SQ: f32 = 9.;
const THROW_MS: f32 = 200.;
pub const FOLLOW: (f64, f64) = (80., 8.);
pub const SETTLE: (f64, f64) = (40., 7.);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub pos: f64,
    pub vel: f64,
    pub to: f64,
    tension: f64,
    friction: f64,
    last_ms: i64,
    pub active: bool,
}

pub fn from_origami(tension: f64, friction: f64) -> (f64, f64) {
    ((tension - 30.) * 3.62 + 194., (friction - 8.) * 3. + 25.)
}

impl Spring {
    pub const fn at(pos: f64) -> Self {
        Self { pos, vel: 0., to: pos, tension: 0., friction: 0., last_ms: 0, active: false }
    }

    pub fn set(&mut self, pos: f64) {
        *self = Self::at(pos);
    }

    pub fn animate_to(&mut self, to: f64, origami: (f64, f64), now_ms: i64) {
        if !self.active {
            self.vel = 0.;
            self.last_ms = now_ms;
        }
        let (tension, friction) = from_origami(origami.0, origami.1);
        self.to = to;
        self.tension = tension;
        self.friction = friction;
        self.active = true;
        self.update(now_ms);
    }

    pub fn update(&mut self, now_ms: i64) {
        if !self.active {
            return;
        }
        let now = now_ms.min(self.last_ms + 64);
        let steps = (now - self.last_ms).max(0);
        let (k, c, to) = (self.tension, self.friction, self.to);
        let (mut e, mut t) = (self.pos, self.vel);
        let (mut r, mut n) = (self.pos, self.vel);
        for _ in 0..steps {
            let l = t;
            let u = k * (to - r) - c * n;
            r = e + 0.001 * l / 2.;
            n = t + 0.001 * u / 2.;
            let cv = n;
            let f = k * (to - r) - c * n;
            r = e + 0.001 * cv / 2.;
            n = t + 0.001 * f / 2.;
            let d = n;
            let p = k * (to - r) - c * n;
            r = e + 0.001 * d / 2.;
            n = t + 0.001 * p / 2.;
            let h = n;
            let m = k * (to - r) - c * n;
            r = e + 0.001 * d / 2.;
            n = t + 0.001 * p / 2.;
            let v = (u + 2. * (f + p) + m) / 6.;
            e += (l + 2. * (cv + d) + h) / 6. * 0.001;
            t += 0.001 * v;
        }
        self.last_ms = now;
        self.pos = e;
        self.vel = t;
        if t.abs() <= 0.001 && (to - e).abs() <= 0.001 {
            self.pos = to;
            self.active = false;
        }
    }
}

#[derive(Clone, Copy)]
struct Motion {
    x: Spring,
    y: Spring,
    pressed: bool,
    dragging: bool,
    drag_start: (f32, f32),
    offset: (f32, f32),
    last_move: (f32, f32),
    last_move_ms: i64,
    velocity: (f32, f32),
}

impl Motion {
    const fn new() -> Self {
        Self {
            x: Spring::at(0.),
            y: Spring::at(0.),
            pressed: false,
            dragging: false,
            drag_start: (0., 0.),
            offset: (0., 0.),
            last_move: (0., 0.),
            last_move_ms: 0,
            velocity: (0., 0.),
        }
    }

    fn animating(&self) -> bool {
        self.x.active || self.y.active
    }
}

thread_local! {
    static CORNER: Cell<Corner> = const { Cell::new(Corner::BottomRight) };
    static MOTION: Cell<Motion> = const { Cell::new(Motion::new()) };
    static EPOCH: std::time::Instant = std::time::Instant::now();
}

fn now_ms() -> i64 {
    EPOCH.with(|e| e.elapsed().as_millis() as i64)
}

fn t(i: usize) -> Hsla {
    c(tok(i))
}

pub fn corner() -> Corner {
    CORNER.with(Cell::get)
}

pub fn set_corner(corner: Corner) {
    CORNER.with(|c| c.set(corner));
}

pub fn translate(x: f32, y: f32, vw: f32, vh: f32) -> (f32, f32) {
    let x = if x < 0. { 0. } else if x > vw - PIP_W { vw - PIP_W } else { x };
    let y = if y < 0. { 0. } else if y > vh - PIP_H { vh - PIP_H } else { y };
    (x, y)
}

pub fn corner_position(corner: Corner, vw: f32, vh: f32) -> (f32, f32) {
    let top = EDGE_TOP.round();
    let bottom = (vh - EDGE_BOTTOM - PIP_H).round();
    let left = EDGE_LEFT.round();
    let right = (vw - EDGE_RIGHT - PIP_W).round();
    let (x, y) = match corner {
        Corner::TopLeft => (left, top),
        Corner::TopRight => (right, top),
        Corner::BottomLeft => (left, bottom),
        Corner::BottomRight => (right, bottom),
    };
    translate(x, y, vw, vh)
}

pub fn release_corner(mouse: (f32, f32), velocity: (f32, f32), vw: f32, vh: f32) -> Corner {
    let x = mouse.0 + THROW_MS * velocity.0;
    let y = mouse.1 + THROW_MS * velocity.1;
    match (x > vw / 2., y > vh / 2.) {
        (false, false) => Corner::TopLeft,
        (true, false) => Corner::TopRight,
        (false, true) => Corner::BottomLeft,
        (true, true) => Corner::BottomRight,
    }
}

pub fn has_watched_stream(app: &PhoenixApp, ch_id: Id) -> bool {
    app.channels.get(&ch_id).is_some_and(|ch| {
        ch.voice_users.iter().any(|&u| {
            let f = crate::ui::voice_panel::flags(u);
            f.stream && crate::ui::call_view::stream_key(app, u).is_some_and(|k| app.voice.watching.contains(&k))
        })
    })
}

pub fn has_active_video(app: &PhoenixApp, ch_id: Id) -> bool {
    app.channels.get(&ch_id).is_some_and(|ch| {
        ch.voice_users.iter().any(|&u| {
            crate::ui::voice_panel::flags(u).video && !crate::voice::prefs::prefs().lock().user(u).video_disabled
        })
    })
}

pub fn active(app: &PhoenixApp) -> bool {
    app.voice_channel.is_some_and(|ch| has_watched_stream(app, ch) || has_active_video(app, ch))
        && !crate::ui::call_view::active(app)
        && !crate::ui::call_popout::is_open()
}

pub fn select_subject(app: &PhoenixApp, ch_id: Id) -> Option<(Id, bool)> {
    let ch = app.channels.get(&ch_id)?;
    let users = &ch.voice_users;
    if users.is_empty() {
        return Some((app.me.id, false));
    }
    if let Some(&u) = users.iter().find(|&&u| crate::ui::voice_panel::flags(u).stream && crate::ui::call_view::stream_key(app, u).is_some_and(|k| app.voice.watching.contains(&k))) {
        return Some((u, true));
    }
    if let Some(&u) = users.iter().find(|&&u| crate::ui::voice_panel::flags(u).video) {
        return Some((u, false));
    }
    if let Some(&u) = users.iter().find(|&&u| app.voice.is_speaking(u)) {
        return Some((u, false));
    }
    if let Some(&u) = users.iter().find(|&&u| u != app.me.id) {
        return Some((u, false));
    }
    Some((users[0], false))
}

fn tile_color(app: &PhoenixApp, user: Id) -> Hsla {
    match app.users.get(&user).map(|u| &u.avatar) {
        Some(crate::model::ImageRef::Url(url)) => crate::ui::avatar_color::get(url).map(c).unwrap_or(t(idx::BACKGROUND_BASE_LOWER)),
        Some(crate::model::ImageRef::Default(ix)) => c(DEFAULT_AVATAR[*ix as usize % DEFAULT_AVATAR.len()]),
        None => t(idx::BACKGROUND_BASE_LOWER),
    }
}

fn pip_btn(id: &'static str, icon: Icon, danger: bool, active_state: bool, tip: SharedString) -> gpui::Stateful<gpui::Div> {
    let bg = if danger {
        t(idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_DEFAULT)
    } else if active_state {
        t(idx::OPACITY_RED_16)
    } else {
        t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT)
    };
    let hover = if danger {
        t(idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_HOVER)
    } else if active_state {
        t(idx::OPACITY_RED_12)
    } else {
        t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_HOVER)
    };
    let fg = if active_state && !danger { t(idx::RED_400) } else { t(idx::WHITE) };
    div()
        .id(id)
        .size(z(32.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(bg)
        .hover(move |s| s.bg(hover))
        .cursor_pointer()
        .tip(tip)
        .child(crate::ui::icon(icon, 16. * zoom(), fg))
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    crate::ui::call_view::release_video_images(window, cx);
    if !active(app) {
        MOTION.with(|m| m.set(Motion::new()));
        return None;
    }
    let ch_id = app.voice_channel?;
    let ch = app.channels.get(&ch_id)?;
    let (user_id, is_stream) = select_subject(app, ch_id)?;
    let k = zoom();
    let vp = window.viewport_size();
    let vw = f32::from(vp.width) / k;
    let vh = f32::from(vp.height) / k;

    let mut motion = MOTION.with(Cell::get);
    if motion.animating() {
        let now = now_ms();
        motion.x.update(now);
        motion.y.update(now);
        if !motion.animating() && !motion.dragging {
            let (x, y) = corner_position(corner(), vw, vh);
            motion.x.set(f64::from(x));
            motion.y.set(f64::from(y));
        }
        window.request_animation_frame();
    } else if !motion.dragging {
        let (x, y) = corner_position(corner(), vw, vh);
        motion.x.set(f64::from(x));
        motion.y.set(f64::from(y));
    }
    MOTION.with(|m| m.set(motion));
    let (left, top) = (motion.x.pos as f32, motion.y.pos as f32);
    let pressed = motion.pressed;
    let dragging = motion.dragging;

    let me = user_id == app.me.id;
    let flags = crate::ui::voice_panel::flags(user_id);
    let speaking = !is_stream && app.voice.is_speaking(user_id);
    let user = app.users.get(&user_id);
    let name = crate::ui::sidebar::voice_user_name(app, user_id);
    let video = if is_stream {
        let skey = crate::ui::call_view::stream_key(app, user_id);
        let watching = skey.as_ref().is_some_and(|k| app.voice.watching.contains(k));
        if me || watching {
            crate::ui::call_view::stream_image(user_id, me)
        } else {
            None
        }
    } else if flags.video && !crate::voice::prefs::prefs().lock().user(user_id).video_disabled {
        if me {
            crate::ui::call_view::camera_image()
        } else {
            crate::ui::call_view::remote_image(user_id)
        }
    } else {
        None
    };

    if video.is_some() {
        crate::ui::call_view::schedule_video_refresh(window, cx);
    }
    crate::voice::call::set_pip_view();
    if is_stream
        && !me
        && let Some(key) = crate::ui::call_view::stream_key(app, user_id)
    {
        let (pw, ph) = crate::ui::call_view::device_pixels(PIP_W * k, PIP_H * k, window.scale_factor());
        crate::voice::call::set_stream_view(&key, pw, ph);
    }

    let muted = app.mic_muted || app.deafened;
    let white = t(idx::WHITE);
    let channel_name = ch.name.clone();

    let card = div()
        .id("voice-pip-card")
        .group("pip-card")
        .absolute()
        .left(z(left))
        .top(z(top))
        .w(z(PIP_W))
        .h(z(PIP_H))
        .rounded(z(8.))
        .overflow_hidden()
        .bg(if video.is_some() || is_stream {
            t(idx::BACKGROUND_BASE_LOWER)
        } else {
            tile_color(app, user_id)
        })
        .border_1()
        .border_color(if speaking { t(idx::STATUS_POSITIVE) } else { t(idx::BORDER_SUBTLE) })
        .shadow(crate::ui::inbox::popout_shadow())
        .map(|d| if dragging { d.cursor_grabbing() } else { d.cursor_grab() })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_, ev: &gpui::MouseDownEvent, _, cx| {
                let mx = f32::from(ev.position.x) / k;
                let my = f32::from(ev.position.y) / k;
                MOTION.with(|m| {
                    let mut st = m.get();
                    st.pressed = true;
                    st.dragging = false;
                    st.drag_start = (mx, my);
                    st.offset = (mx - st.x.pos as f32, my - st.y.pos as f32);
                    m.set(st);
                });
                cx.stop_propagation();
                cx.notify();
            }),
        )
        .flex()
        .items_center()
        .justify_center()
        .map(|d| match video {
            Some(img) => d.child(gpui::img(img).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Contain)),
            None if is_stream => d.child(gpui::svg().path("icons/voice/screen.svg").size(z(36.)).text_color(white)),
            None => d.child(crate::ui::avatar(user, 64. * k)),
        })
        .when(is_stream, |d| {
            d.child(
                div()
                    .absolute()
                    .top(z(8.))
                    .right(z(8.))
                    .px(z(6.))
                    .rounded(z(4.))
                    .bg(t(idx::STATUS_DANGER))
                    .text_size(z(11.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(white)
                    .child(crate::t!("voice.live")),
            )
        })
        .child(
            div()
                .absolute()
                .left(z(8.))
                .bottom(z(8.))
                .px(z(8.))
                .py(z(4.))
                .rounded(z(6.))
                .bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT))
                .text_color(white)
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::MEDIUM)
                .group_hover("pip-card", |s| s.opacity(0.))
                .child(name),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .p(z(8.))
                .flex()
                .flex_col()
                .justify_between()
                .bg(gpui::hsla(0., 0., 0., 0.45))
                .opacity(0.)
                .group_hover("pip-card", |s| s.opacity(1.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(z(6.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(z(13.))
                                .line_height(z(18.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(white)
                                .text_ellipsis()
                                .child(channel_name),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(z(4.))
                                .child(
                                    div()
                                        .id("pip-return")
                                        .size(z(28.))
                                        .rounded(z(6.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT))
                                        .hover(|s| s.bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_HOVER)))
                                        .cursor_pointer()
                                        .tip(crate::t!("pip.return_to_call"))
                                        .child(gpui::svg().path("icons/call/fullscreen.svg").size(z(16.)).text_color(white))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                        .on_click(cx.listener(|a, _, _, cx| a.jump_to_call(cx))),
                                )
                                .child(
                                    div()
                                        .id("pip-popout")
                                        .size(z(28.))
                                        .rounded(z(6.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT))
                                        .hover(|s| s.bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_HOVER)))
                                        .cursor_pointer()
                                        .tip(crate::t!("call.popout"))
                                        .child(gpui::svg().path("icons/call/popout.svg").size(z(16.)).text_color(white))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                        .on_click(cx.listener(|_, _, _, cx| {
                                            let app_entity = cx.entity();
                                            cx.defer(move |cx| crate::ui::call_popout::open(app_entity, cx));
                                        })),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(z(8.))
                        .child(
                            pip_btn(
                                "pip-mic",
                                if muted { Icon::MicOff } else { Icon::Mic },
                                false,
                                muted,
                                if muted { crate::t!("voice.unmute") } else { crate::t!("voice.mute") },
                            )
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|a, _, _, cx| {
                                a.mic_muted = !a.mic_muted;
                                crate::voice::sync_self_state(a);
                                cx.notify();
                            })),
                        )
                        .child(
                            pip_btn(
                                "pip-deafen",
                                if app.deafened { Icon::HeadphonesOff } else { Icon::Headphones },
                                false,
                                app.deafened,
                                if app.deafened { crate::t!("voice.undeafen") } else { crate::t!("voice.deafen") },
                            )
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|a, _, _, cx| {
                                a.deafened = !a.deafened;
                                crate::voice::sync_self_state(a);
                                cx.notify();
                            })),
                        )
                        .child(
                            pip_btn("pip-hangup", Icon::PhoneHangup, true, false, crate::t!("voice.disconnect"))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(|a, _, _, cx| a.join_voice(None, cx))),
                        ),
                ),
        );

    if pressed {
        Some(
            div()
                .id("voice-pip-drag-layer")
                .absolute()
                .inset_0()
                .when(dragging, |d| d.cursor_grabbing())
                .on_mouse_move(cx.listener(move |_, ev: &MouseMoveEvent, _, cx| {
                    let mut st = MOTION.with(Cell::get);
                    if !st.pressed {
                        return;
                    }
                    let mx = f32::from(ev.position.x) / k;
                    let my = f32::from(ev.position.y) / k;
                    let now = now_ms();
                    if !st.dragging {
                        let dx = mx - st.drag_start.0;
                        let dy = my - st.drag_start.1;
                        if dx * dx + dy * dy > DRAG_THRESHOLD_SQ {
                            st.dragging = true;
                            st.velocity = (0., 0.);
                            st.last_move = (mx, my);
                            st.last_move_ms = now;
                        }
                    }
                    if st.dragging {
                        let (x, y) = translate(mx - st.offset.0, my - st.offset.1, vw, vh);
                        st.x.animate_to(f64::from(x), FOLLOW, now);
                        st.y.animate_to(f64::from(y), FOLLOW, now);
                        let dt = now - st.last_move_ms;
                        if dt != 0 {
                            st.velocity = ((mx - st.last_move.0) / dt as f32, (my - st.last_move.1) / dt as f32);
                            st.last_move = (mx, my);
                            st.last_move_ms = now;
                        }
                    }
                    MOTION.with(|m| m.set(st));
                    cx.notify();
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |a, ev: &MouseUpEvent, _, cx| {
                        let mut st = MOTION.with(Cell::get);
                        st.pressed = false;
                        if st.dragging {
                            st.dragging = false;
                            let mouse = (f32::from(ev.position.x) / k, f32::from(ev.position.y) / k);
                            let corner = release_corner(mouse, st.velocity, vw, vh);
                            set_corner(corner);
                            let (x, y) = corner_position(corner, vw, vh);
                            let now = now_ms();
                            st.x.animate_to(f64::from(x), SETTLE, now);
                            st.y.animate_to(f64::from(y), SETTLE, now);
                            MOTION.with(|m| m.set(st));
                        } else {
                            MOTION.with(|m| m.set(st));
                            a.jump_to_call(cx);
                        }
                        cx.notify();
                    }),
                )
                .child(card)
                .into_any_element(),
        )
    } else {
        Some(card.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Corner, EDGE_BOTTOM, EDGE_LEFT, EDGE_RIGHT, EDGE_TOP, FOLLOW, PIP_H, PIP_W, SETTLE, Spring, corner_position, release_corner,
        translate,
    };

    fn run(origami: (f64, f64), samples: &[i64]) -> Vec<f64> {
        let mut s = Spring::at(0.);
        s.animate_to(100., origami, 0);
        let mut out = Vec::new();
        let mut ms = 0;
        while ms < *samples.last().unwrap() {
            ms += 16;
            s.update(ms);
            if samples.contains(&ms) {
                out.push(s.pos);
            }
        }
        out
    }

    #[test]
    fn settle_spring_matches_client_integrator() {
        let got = run(SETTLE, &[16, 48, 96, 160, 240, 320, 480, 640, 960]);
        let want = [2.59977872001, 18.4409859835, 50.5006026014, 83.8317732577, 101.368366466, 103.535348338, 100.348850833, 99.8830818878, 100.003668043];
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 1e-8, "{g} vs {w}");
        }
    }

    #[test]
    fn follow_spring_matches_client_integrator() {
        let got = run(FOLLOW, &[16, 48, 96, 160, 240, 320, 480, 640, 960]);
        let want = [4.16209564415, 28.1731551584, 70.4452962788, 101.798881165, 106.253902272, 101.478920730, 99.6792224200, 100.035389364, 99.9995515070];
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 1e-8, "{g} vs {w}");
        }
    }

    #[test]
    fn settle_spring_overshoots_then_rests() {
        let mut s = Spring::at(0.);
        s.animate_to(100., SETTLE, 0);
        let mut peak: f64 = 0.;
        let mut ms = 0;
        while s.active {
            ms += 1;
            s.update(ms);
            peak = peak.max(s.pos);
        }
        assert!((peak - 103.6840426).abs() < 1e-6);
        assert_eq!(ms, 1182);
        assert_eq!(s.pos, 100.);
    }

    #[test]
    fn frame_gap_is_capped_at_64_ms() {
        let mut s = Spring::at(0.);
        s.animate_to(100., SETTLE, 0);
        s.update(200);
        assert!((s.pos - 28.9437605205).abs() < 1e-8);
    }

    #[test]
    fn retarget_keeps_velocity() {
        let mut s = Spring::at(0.);
        s.animate_to(100., FOLLOW, 0);
        s.update(48);
        let v = s.vel;
        s.animate_to(300., SETTLE, 48);
        assert_eq!(s.vel, v);
    }

    #[test]
    fn release_corner_uses_cursor_and_throw() {
        let (vw, vh) = (1280., 800.);
        assert_eq!(release_corner((100., 100.), (0., 0.), vw, vh), Corner::TopLeft);
        assert_eq!(release_corner((1100., 100.), (0., 0.), vw, vh), Corner::TopRight);
        assert_eq!(release_corner((100., 700.), (0., 0.), vw, vh), Corner::BottomLeft);
        assert_eq!(release_corner((1100., 700.), (0., 0.), vw, vh), Corner::BottomRight);
        assert_eq!(release_corner((640., 400.), (0., 0.), vw, vh), Corner::TopLeft);
        assert_eq!(release_corner((500., 300.), (1., 0.6), vw, vh), Corner::BottomRight);
        assert_eq!(release_corner((1000., 600.), (-2., -1.5), vw, vh), Corner::TopLeft);
    }

    #[test]
    fn corner_positions_use_client_edge_offsets() {
        let (vw, vh) = (1280., 800.);
        assert_eq!(corner_position(Corner::TopLeft, vw, vh), (EDGE_LEFT, EDGE_TOP));
        assert_eq!(corner_position(Corner::TopRight, vw, vh), (vw - EDGE_RIGHT - PIP_W, EDGE_TOP));
        assert_eq!(corner_position(Corner::BottomLeft, vw, vh), (EDGE_LEFT, vh - EDGE_BOTTOM - PIP_H));
        assert_eq!(corner_position(Corner::BottomRight, vw, vh), (vw - EDGE_RIGHT - PIP_W, vh - EDGE_BOTTOM - PIP_H));
    }

    #[test]
    fn drag_is_clamped_to_the_window() {
        assert_eq!(translate(-50., -10., 1280., 800.), (0., 0.));
        assert_eq!(translate(2000., 2000., 1280., 800.), (1280. - PIP_W, 800. - PIP_H));
    }
}
