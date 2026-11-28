use crate::theme::z;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AnyElement, Bounds, Context, DispatchPhase, Div, ElementId, FontWeight, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, SharedString, Stateful, Styled, canvas, deferred, div, prelude::*,
    relative,
};
use phoenix_voice::audio::device::MicTest;

use super::Page;
use crate::app::PhoenixApp;
use crate::assets::Icon;
use crate::theme::{Palette, c, generated::idx, tok};
use crate::ui::icon;
use crate::voice::call::{PortalState, manager, portal_state};
use crate::voice::prefs::{InputMode, PROFILE_CUSTOM, PROFILE_STUDIO, VoicePrefs, prefs, update};
use crate::voice::{VoiceStatus, ptt};

type Cx<'a> = Context<'a, PhoenixApp>;
pub(crate) type OnSlide = Rc<dyn Fn(&mut PhoenixApp, f32, bool, &mut Cx)>;

const TRACK: u32 = 0x4E5058;
const SENS_LOW: u32 = 0xF0B232;
const SENS_HIGH: u32 = 0x23A55A;
pub const DB_MIN: f32 = -100.;

#[derive(Clone, Copy, PartialEq, Eq)]
enum MicTestPlace {
    Settings,
    NoisePopout,
}

thread_local! {
    static BOUNDS: RefCell<HashMap<&'static str, Bounds<Pixels>>> = RefCell::new(HashMap::new());
    static DRAG: Cell<Option<&'static str>> = const { Cell::new(None) };
    static OPEN_SELECT: Cell<Option<&'static str>> = const { Cell::new(None) };
    static MIC_TEST: RefCell<Option<(MicTestPlace, MicTest)>> = const { RefCell::new(None) };
    static MIC_ERROR: RefCell<Option<(MicTestPlace, String)>> = const { RefCell::new(None) };
    static DEVICES: RefCell<Option<(Vec<String>, Vec<String>)>> = const { RefCell::new(None) };
    static LOADING: Cell<bool> = const { Cell::new(false) };
    static TICKING: Cell<bool> = const { Cell::new(false) };
}

fn eid(s: impl Into<String>) -> ElementId {
    ElementId::Name(SharedString::from(s.into()))
}

fn h1(t: SharedString) -> Div {
    crate::ui::controls::section_title(t)
}


fn h5(t: impl Into<SharedString>) -> Div {
    div()
        .pb(z(8.))
        .text_size(z(16.))
        .line_height(z(20.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(idx::TEXT_STRONG)))
        .child(t.into())
}


fn note(t: impl Into<SharedString>) -> Div {
    div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(t.into())
}


fn divider() -> Div {
    div().my(z(50.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))
}


fn knob(id: ElementId, on: bool, disabled: bool) -> gpui::Stateful<Div> {
    crate::ui::controls::switch(id, on).when(disabled, |d| d.opacity(0.5))
}


fn switch_row(
    id: &'static str,
    title: SharedString,
    desc: SharedString,
    on: bool,
    disabled: bool,
    f: impl Fn(&mut VoicePrefs) + 'static,
    cx: &mut Cx,
) -> Div {
    div().child(
        crate::ui::a11y::switch(div().id(eid(format!("vs-switch-{id}"))), title.clone(), on)
            .when(!disabled, |d| d.group(crate::ui::controls::SWITCH_ROW))
            .flex()
            .gap(z(12.))
            .mx(z(-12.))
            .px(z(12.))
            .py(z(10.))
            .rounded(z(16.))
            .when(!disabled, |d| {
                d.cursor_pointer().on_click(cx.listener(move |_, _, _, cx| {
                    update(&f, true);
                    cx.notify();
                }))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(z(4.))
                    .child(
                        div()
                            .text_size(z(16.))
                            .line_height(z(20.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(tok(if disabled { idx::TEXT_MUTED } else { idx::TEXT_STRONG })))
                            .child(title),
                    )
                    .when(!desc.is_empty(), |d| d.child(note(desc))),
            )
            .child(knob(eid(format!("vs-knob-{id}")), on, disabled)),
    )
}

fn frac_at(id: &'static str, x: Pixels) -> Option<f32> {
    BOUNDS.with(|b| {
        let b = *b.borrow().get(id)?;
        let w = f32::from(b.size.width).max(1.);
        Some(((f32::from(x) - f32::from(b.origin.x)) / w).clamp(0., 1.))
    })
}

pub(crate) fn slider(
    id: &'static str,
    frac: f32,
    bubble: SharedString,
    track: Option<(u32, u32)>,
    on_change: OnSlide,
    cx: &mut Cx,
) -> Stateful<Div> {
    let frac = frac.clamp(0., 1.);
    let dragging = DRAG.with(Cell::get) == Some(id);
    let weak = cx.entity().downgrade();
    let paint_cb = on_change.clone();
    let probe = canvas(
        move |bounds, _, _| {
            BOUNDS.with(|b| b.borrow_mut().insert(id, bounds));
        },
        move |_, _, window, _| {
            if DRAG.with(Cell::get) != Some(id) {
                return;
            }
            let (w1, f1) = (weak.clone(), paint_cb.clone());
            window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble || DRAG.with(Cell::get) != Some(id) {
                    return;
                }
                if let Some(f) = frac_at(id, ev.position.x) {
                    let _ = w1.update(cx, |a, cx| {
                        f1(a, f, false, cx);
                        cx.notify();
                    });
                }
            });
            let (w2, f2) = (weak.clone(), paint_cb.clone());
            window.on_mouse_event(move |ev: &MouseUpEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble || DRAG.with(Cell::get) != Some(id) {
                    return;
                }
                DRAG.with(|d| d.set(None));
                let f = frac_at(id, ev.position.x);
                let _ = w2.update(cx, |a, cx| {
                    if let Some(f) = f {
                        f2(a, f, true, cx);
                    }
                    cx.notify();
                });
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    let (lo, hi) = track.unwrap_or((Palette::BLURPLE, TRACK));
    div()
        .id(eid(format!("slider-{id}")))
        .when(crate::ui::a11y::on(), |d| {
            d.role(gpui::Role::Slider)
                .aria_value(bubble.clone())
                .aria_numeric_value((frac * 100.).round() as f64)
                .aria_min_numeric_value(0.)
                .aria_max_numeric_value(100.)
        })
        .relative()
        .w_full()
        .h(z(24.))
        .cursor_pointer()
        .child(probe)
        .child(
            div()
                .absolute()
                .top(z(10.))
                .left_0()
                .w_full()
                .h(z(4.))
                .rounded(z(4.))
                .bg(c(hi))
                .child(div().h_full().w(relative(frac)).rounded(z(4.)).bg(c(lo))),
        )
        .child(
            div().absolute().top(z(4.)).left(relative(frac)).w(z(0.)).flex().justify_center().child(
                div()
                    .id(eid(format!("slider-{id}-grab")))
                    .relative()
                    .flex_none()
                    .size(z(16.))
                    .rounded_full()
                    .bg(gpui::white())
                    .border_1()
                    .border_color(c(tok(idx::BORDER_NORMAL)))
                    .shadow_sm()
                    .hover(|d| d.size(z(20.)).m(z(-2.)))
                    .when(dragging, |d| d.size(z(20.)).m(z(-2.)))
                    .when(dragging, |d| {
                        d.child(
                            div().absolute().bottom(z(30.)).left(z(-27.)).w(z(64.)).flex().justify_center().child(
                                div()
                                    .px(z(8.))
                                    .py(z(4.))
                                    .rounded(z(4.))
                                    .bg(c(Palette::BG_FLOATING))
                                    .text_size(z(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c(Palette::TEXT_NORMAL))
                                    .whitespace_nowrap()
                                    .child(bubble),
                            ),
                        )
                    }),
            ),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |a, ev: &MouseDownEvent, _, cx| {
                DRAG.with(|d| d.set(Some(id)));
                if let Some(f) = frac_at(id, ev.position.x) {
                    on_change(a, f, false, cx);
                }
                cx.stop_propagation();
                cx.notify();
            }),
        )
}

type SetPref = Rc<dyn Fn(&mut VoicePrefs)>;

fn select(
    id: &'static str,
    current: Option<String>,
    options: Vec<String>,
    f: impl Fn(&mut VoicePrefs, Option<String>) + 'static,
    cx: &mut Cx,
) -> Div {
    let default_label: SharedString = match options.first() {
        Some(d) => format!("{}: {d}", crate::t!("voice.system_default")).into(),
        None => crate::t!("voice.default"),
    };
    let label: SharedString = current.clone().map(SharedString::from).unwrap_or_else(|| default_label.clone());
    let f = Rc::new(f);
    let mut entries: Vec<Option<String>> = vec![None];
    entries.extend(options.into_iter().map(Some));
    let entries = entries
        .into_iter()
        .map(|opt| {
            let text: SharedString = opt.clone().map(SharedString::from).unwrap_or_else(|| default_label.clone());
            let f = f.clone();
            let selected = opt == current;
            let set: SetPref = Rc::new(move |p: &mut VoicePrefs| f(p, opt.clone()));
            (text, selected, set)
        })
        .collect();
    select_box(id, label, entries, cx)
}

fn select_box(id: &'static str, label: SharedString, entries: Vec<(SharedString, bool, SetPref)>, cx: &mut Cx) -> Div {
    let open = OPEN_SELECT.with(Cell::get) == Some(id);
    let mut list = div()
        .id(eid(format!("{id}-list")))
        .absolute()
        .top(z(44.))
        .left_0()
        .w_full()
        .max_h(z(300.))
        .overflow_y_scroll()
        .p(z(4.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
        .shadow_lg()
        .occlude()
        .flex()
        .flex_col();
    for (ix, (text, selected, f)) in entries.into_iter().enumerate() {
        list = list.child(
            div()
                .id(eid(format!("{id}-opt-{ix}")))
                .h(z(40.))
                .px(z(8.))
                .rounded(z(4.))
                .flex()
                .items_center()
                .cursor_pointer()
                .text_size(z(16.))
                .line_height(z(20.))
                .text_color(c(tok(idx::TEXT_STRONG)))
                .when(selected, |d| d.bg(c(tok(idx::INTERACTIVE_BACKGROUND_SELECTED))))
                .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                .child(div().flex_1().min_w_0().text_ellipsis().child(text))
                .on_click(cx.listener(move |_, _, _, cx| {
                    OPEN_SELECT.with(|o| o.set(None));
                    let f = f.clone();
                    update(move |p| f(p), true);
                    restart_mic_test();
                    cx.notify();
                })),
        );
    }
    div()
        .relative()
        .child(
            div()
                .id(eid(format!("{id}-box")))
                .h(z(40.))
                .pl(z(12.))
                .pr(z(8.))
                .rounded(z(8.))
                .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
                .border_1()
                .border_color(c(tok(if open { idx::INPUT_BORDER_ACTIVE } else { idx::INPUT_BORDER_DEFAULT })))
                .flex()
                .items_center()
                .cursor_pointer()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .mr(z(14.3))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::TEXT_STRONG)))
                        .child(label),
                )
                .child(icon(Icon::Chevron, 20., c(tok(idx::TEXT_STRONG))))
                .on_click(cx.listener(move |_, _, _, cx| {
                    OPEN_SELECT.with(|o| o.set(if open { None } else { Some(id) }));
                    cx.notify();
                })),
        )
        .when(open, |d| d.child(deferred(list).with_priority(2)))
}

fn button(id: &'static str, label: SharedString, bg: u32, _hover: u32) -> gpui::Stateful<Div> {
    let kind = if bg == Palette::BLURPLE { crate::ui::controls::BtnKind::Primary } else { crate::ui::controls::BtnKind::Secondary };
    crate::ui::controls::button(id, label, kind)
}

pub fn meter_bar(level_db: f32, active: bool, height: f32, frame: usize) -> Div {
    let frac = if active { ((level_db - DB_MIN) / -DB_MIN).clamp(0., 1.) } else { 0. };
    div().flex_1().h(z(height)).child(
        gpui::canvas(
            |_, _, _| {},
            move |b, _, window, _| {
                let cell = f32::from(z(8.));
                let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
                let n = (w / cell).round() as usize;
                if n == 0 || h <= 0. {
                    return;
                }
                let width = n as f32 * cell;
                let lit = n - ((n as f32 * (1. - frac)).round() as usize).min(n);
                window.paint_quad(gpui::fill(gpui::Bounds::new(b.origin, gpui::size(gpui::px(width), b.size.height)), c(tok(frame))));
                let (from, to) = (gpui::Rgba::from(c(tok(idx::YELLOW_260))), gpui::Rgba::from(c(tok(idx::GREEN_360))));
                let inset = f32::from(z(2.));
                for i in 0..n {
                    let color = if i < lit {
                        let t = (i as f32 + 0.5) * cell / width;
                        gpui::Hsla::from(gpui::Rgba { r: from.r + (to.r - from.r) * t, g: from.g + (to.g - from.g) * t, b: from.b + (to.b - from.b) * t, a: 1. })
                    } else {
                        c(tok(idx::GRADIENT_PROGRESS_PILL_BACKGROUND))
                    };
                    let o = gpui::point(b.origin.x + gpui::px(i as f32 * cell + inset), b.origin.y + gpui::px(inset));
                    window.paint_quad(gpui::fill(gpui::Bounds::new(o, gpui::size(gpui::px(cell - 2. * inset), gpui::px(h - 2. * inset))), color).corner_radii(z(2.)));
                }
            },
        )
        .size_full(),
    )
}

pub fn live_level() -> Option<(f32, bool)> {
    let t = MIC_TEST.with(|m| m.borrow().as_ref().map(|(_, t)| (t.level_db(), t.gate_open())));
    t.or_else(|| {
        let c = manager().control();
        (c.input_level_db() > DB_MIN).then(|| (c.input_level_db(), c.transmitting()))
    })
}

fn mic_test_place() -> Option<MicTestPlace> {
    MIC_TEST.with(|m| m.borrow().as_ref().map(|(place, _)| *place))
}

fn stop_mic_test(place: MicTestPlace) {
    if mic_test_place() == Some(place)
        && let Some((_, t)) = MIC_TEST.with(|m| m.borrow_mut().take())
    {
        std::thread::spawn(move || t.stop());
    }
}

pub(crate) fn stop_quick_mic_test() {
    stop_mic_test(MicTestPlace::NoisePopout);
    MIC_ERROR.with(|error| {
        let mut error = error.borrow_mut();
        if error.as_ref().is_some_and(|(place, _)| *place == MicTestPlace::NoisePopout) {
            *error = None;
        }
    });
}

fn start_mic_test(place: MicTestPlace) {
    if mic_test_place().is_some() {
        return;
    }
    let p = prefs().lock().clone();
    match MicTest::start(Arc::clone(manager().control()), p.input_device, p.output_device, true) {
        Ok(t) => {
            MIC_ERROR.with(|e| *e.borrow_mut() = None);
            MIC_TEST.with(|m| *m.borrow_mut() = Some((place, t)));
        }
        Err(e) => MIC_ERROR.with(|x| *x.borrow_mut() = Some((place, e.to_string()))),
    }
}

fn restart_mic_test() {
    if let Some((place, t)) = MIC_TEST.with(|m| m.borrow_mut().take()) {
        t.stop();
        start_mic_test(place);
    }
}

fn ensure_ticker(cx: &mut Cx, settings_open: bool) {
    if TICKING.with(Cell::get) {
        return;
    }
    TICKING.with(|t| t.set(true));
    cx.spawn(async move |this, cx| {
        let mut settings_was_open = settings_open;
        loop {
            cx.background_executor().timer(Duration::from_millis(50)).await;
            let (settings_open, quick_open) = this
                .update(cx, |a, cx| {
                    let settings = a.settings.as_ref().is_some_and(|s| s.page == Page::VoiceVideo);
                    let quick = crate::ui::voice_panel::noise_popout_open(a);
                    if settings || (quick && mic_test_place() == Some(MicTestPlace::NoisePopout)) {
                        cx.notify();
                    }
                    (settings, quick)
                })
                .unwrap_or_default();
            if settings_was_open && !settings_open {
                OPEN_SELECT.with(|o| o.set(None));
                ptt::set_recording(false);
            }
            if !settings_open {
                stop_mic_test(MicTestPlace::Settings);
            }
            if !quick_open {
                stop_quick_mic_test();
            }
            if !settings_open && (!quick_open || mic_test_place() != Some(MicTestPlace::NoisePopout)) {
                break;
            }
            settings_was_open = settings_open;
        }
        TICKING.with(|t| t.set(false));
    })
    .detach();
}

fn ensure_devices(cx: &mut Cx) {
    if DEVICES.with(|d| d.borrow().is_some()) || LOADING.with(Cell::get) {
        return;
    }
    LOADING.with(|l| l.set(true));
    let task = cx.background_executor().spawn(async { crate::voice::call::devices() });
    cx.spawn(async move |this, cx| {
        let list = task.await;
        DEVICES.with(|d| *d.borrow_mut() = Some(list));
        LOADING.with(|l| l.set(false));
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

fn pct_slider(
    id: &'static str,
    value: u32,
    max: u32,
    f: impl Fn(&mut VoicePrefs, u32) + 'static,
    cx: &mut Cx,
) -> Stateful<Div> {
    let f = Rc::new(f);
    slider(
        id,
        value as f32 / max as f32,
        format!("{value}%").into(),
        None,
        Rc::new(move |_, frac, done, _| {
            let v = (frac * max as f32).round() as u32;
            let f = f.clone();
            update(move |p| f(p, v), done);
        }),
        cx,
    )
}

fn column(children: impl IntoIterator<Item = AnyElement>) -> Div {
    div().flex_1().min_w_0().flex().flex_col().children(children)
}

fn keybind(p: &VoicePrefs, cx: &mut Cx) -> Div {
    let recording = ptt::is_recording();
    let label: SharedString = match (&p.ptt_key, recording) {
        (_, true) => crate::t!("voice.press_key"),
        (Some(k), false) => ptt::label(k).into(),
        (None, false) => crate::t!("voice.no_keybind"),
    };
    let portal: Option<(SharedString, u32)> = match portal_state().lock().clone() {
        PortalState::Off => None,
        PortalState::Pending => Some((crate::t!("voice.portal_pending"), Palette::TEXT_MUTED)),
        PortalState::Bound(d) if d.is_empty() => Some((crate::t!("voice.portal_bound"), Palette::TEXT_MUTED)),
        PortalState::Bound(d) => Some((crate::t!("voice.portal_active", d = d), Palette::TEXT_MUTED)),
        PortalState::Unavailable(_) => Some((crate::t!("voice.portal_unavailable"), Palette::YELLOW)),
    };
    let delay = p.ptt_delay_ms;
    div()
        .flex()
        .gap(z(16.))
        .child(column([
            h5(crate::t!("voice.shortcut")).into_any_element(),
            div()
                .h(z(40.))
                .pl(z(10.))
                .pr(z(4.))
                .rounded(z(8.))
                .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
                .border_1()
                .border_color(c(tok(if recording { idx::BORDER_FEEDBACK_CRITICAL } else { idx::INPUT_BORDER_DEFAULT })))
                .flex()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .text_size(z(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(if p.ptt_key.is_some() || recording { Palette::TEXT_NORMAL } else { Palette::TEXT_MUTED }))
                        .child(label),
                )
                .child(
                    button(
                        "vs-record",
                        if recording { crate::t!("voice.stop_recording") } else if p.ptt_key.is_some() { crate::t!("voice.edit_keybind") } else { crate::t!("voice.record_keybind") },
                        if recording { 0xDA373C } else { 0x4E5058 },
                        if recording { 0xA12D2F } else { 0x6D6F78 },
                    )
                    .min_w(z(0.))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        ptt::set_recording(!recording);
                        cx.notify();
                    })),
                )
                .into_any_element(),
            div().when_some(portal, |d, (t, col)| d.mt(z(8.)).text_size(z(12.)).text_color(c(col)).child(t)).into_any_element(),
        ]))
        .child(column([
            h5(crate::t!("voice.ptt_delay")).into_any_element(),
            slider(
                "ptt-delay",
                delay as f32 / 2000.,
                format!("{delay}ms").into(),
                None,
                Rc::new(|_, f, done, _| {
                    let v = (f * 2000.).round() as u32;
                    update(move |p| p.ptt_delay_ms = v, done);
                }),
                cx,
            )
            .mt(z(8.))
            .into_any_element(),
            div()
                .mt(z(8.))
                .text_size(z(12.))
                .text_color(c(Palette::TEXT_MUTED))
                .child(SharedString::from(format!("{delay}ms")))
                .into_any_element(),
        ]))
}

fn sensitivity(p: &VoicePrefs, cx: &mut Cx) -> Div {
    let auto = p.auto_sensitivity;
    let level = live_level();
    let lvl_frac = level.map(|(db, _)| ((db - DB_MIN) / -DB_MIN).clamp(0., 1.)).unwrap_or(0.);
    let bar: AnyElement = if auto {
        let speaking = level.is_some_and(|(_, open)| open);
        div()
            .mt(z(4.))
            .h(z(8.))
            .rounded(z(4.))
            .bg(c(TRACK))
            .child(div().h_full().w(relative(lvl_frac)).rounded(z(4.)).bg(c(if speaking { SENS_HIGH } else { SENS_LOW })))
            .into_any_element()
    } else {
        let t = p.sensitivity_db;
        let frac = (t - DB_MIN) / -DB_MIN;
        div()
            .relative()
            .child(slider(
                "sensitivity",
                frac,
                format!("{t:.0}dB").into(),
                Some((SENS_LOW, SENS_HIGH)),
                Rc::new(|_, f, done, _| {
                    let db = (DB_MIN + f * -DB_MIN).round();
                    update(move |p| p.sensitivity_db = db, done);
                }),
                cx,
            ))
            .child(
                div()
                    .absolute()
                    .top(z(8.))
                    .left(relative(lvl_frac))
                    .right_0()
                    .h(z(8.))
                    .rounded_r(z(4.))
                    .bg(gpui::black().opacity(0.45)),
            )
            .into_any_element()
    };
    div()
        .flex()
        .flex_col()
        .child(h5(crate::t!("voice.sensitivity")))
        .child(
            div()
                .id("vs-auto-sens")
                .group(crate::ui::controls::SWITCH_ROW)
                .mb(z(16.))
                .flex()
                .items_center()
                .cursor_pointer()
                .child(
                    div()
                        .flex_1()
                        .text_size(z(16.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(Palette::TEXT_NORMAL))
                        .child(crate::t!("voice.auto_sensitivity")),
                )
                .child(knob("vs-auto-knob".into(), auto, false))
                .on_click(cx.listener(move |_, _, _, cx| {
                    update(|p| p.auto_sensitivity = !auto, true);
                    cx.notify();
                })),
        )
        .child(bar)
        .when(auto && level.is_none(), |d| {
            d.child(div().mt(z(8.)).child(note(crate::t!("voice.level_hint"))))
        })
}

fn mic_test_controls(place: MicTestPlace, in_call: bool, cx: &mut Cx) -> Div {
    let compact = place == MicTestPlace::NoisePopout;
    let testing = mic_test_place() == Some(place);
    let level = live_level().map(|(db, _)| db).unwrap_or(DB_MIN);
    let err = MIC_ERROR.with(|e| e.borrow().as_ref().filter(|(p, _)| *p == place).map(|(_, e)| e.clone()));
    let label = if testing {
        crate::t!("voice.stop_testing")
    } else if compact {
        crate::t!("voice.quick_mic_test")
    } else {
        crate::t!("voice.lets_check")
    };
    let button = if compact {
        let button = crate::ui::controls::button_sm("voice-noise-mic-test", label.clone(), crate::ui::controls::BtnKind::Secondary)
            .min_w(z(68.))
            .role(gpui::Role::Button)
            .aria_label(label);
        crate::ui::focus_ring::ring(button, crate::ui::states::key("voice-noise-mic-test"), 8.)
    } else {
        button("vs-mic-test", label, if testing { 0x4E5058 } else { Palette::BLURPLE }, if testing { 0x6D6F78 } else { 0x4752C4 })
    };
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(if compact { 10. } else { 16. }))
                .child(
                    button.flex_none().on_click(cx.listener(move |_, _, _, cx| {
                        if testing {
                            stop_mic_test(place);
                        } else {
                            start_mic_test(place);
                            if compact && mic_test_place() == Some(place) {
                                ensure_ticker(cx, false);
                            }
                        }
                        cx.notify();
                    })),
                )
                .child(meter_bar(level, testing || in_call, if compact { 18. } else { 28. }, idx::BACKGROUND_BASE_LOW)),
        )
        .when(compact && testing, |d| {
            d.child(
                div()
                    .mt(z(8.))
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .text_color(c(tok(idx::TEXT_SUBTLE)))
                    .child(crate::t!("voice.quick_mic_paused")),
            )
        })
        .when(!compact, |d| {
            d.child(
                div()
                    .mt(z(36.))
                    .flex()
                    .gap(z(4.))
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .text_color(c(tok(idx::TEXT_SUBTLE)))
                    .child(crate::t!("voice.help"))
                    .child(div().text_color(c(tok(idx::TEXT_LINK))).child(format!("{}.", crate::t!("voice.help_link")))),
            )
        })
        .when_some(err, |d, e| d.child(div().mt(z(8.)).text_size(z(12.)).text_color(c(Palette::RED)).child(SharedString::from(e))))
}

pub(crate) fn quick_mic_test(cx: &mut Cx) -> Div {
    mic_test_controls(MicTestPlace::NoisePopout, false, cx)
}

pub fn render(app: &PhoenixApp, cx: &mut Cx) -> Div {
    ensure_ticker(cx, true);
    ensure_devices(cx);
    let p = prefs().lock().clone();
    let (inputs, outputs) = DEVICES.with(|d| d.borrow().clone()).unwrap_or_default();
    let top = top_rows(&super::view::query(app, cx));
    div()
        .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| {
            if OPEN_SELECT.with(|o| o.take()).is_some() {
                cx.notify();
            }
        }))
        .when(top.iter().any(|v| *v), |d| d.child(h1(crate::t!("sp.vo.t1"))))
        .when(top[0], |d| {
            d.child(
                div()
                    .mt(z(11.))
                    .flex()
                    .gap(z(16.))
                    .child(column([
                        h5(crate::t!("voice.input_device")).into_any_element(),
                        select("input", p.input_device.clone(), inputs, |p, v| p.input_device = v, cx).into_any_element(),
                    ]))
                    .child(column([
                        h5(crate::t!("voice.output_device")).into_any_element(),
                        select("output", p.output_device.clone(), outputs, |p, v| p.output_device = v, cx).into_any_element(),
                    ])),
            )
        })
        .when(top[1], |d| {
            d.child(
                div()
                    .mt(z(30.))
                    .flex()
                    .gap(z(16.))
                    .child(column([
                        h5(crate::t!("voice.input_volume")).into_any_element(),
                        pct_slider("input-volume", p.input_volume, 100, |p, v| p.input_volume = v, cx).into_any_element(),
                    ]))
                    .child(column([
                        h5(crate::t!("voice.output_volume")).into_any_element(),
                        pct_slider("output-volume", p.output_volume, 200, |p, v| p.output_volume = v, cx).into_any_element(),
                    ])),
            )
        })
        .when(top[2], |d| {
            d.child(div().mt(z(35.)).child(mic_test_controls(MicTestPlace::Settings, app.voice.status == VoiceStatus::Connected, cx)))
        })
        .when(top.iter().any(|v| *v), |d| d.child(divider()))
        .child(super::pages::render(app, LOWER, cx))
}

use super::pages::{B, Bind};

pub fn top_rows(q: &str) -> [bool; 3] {
    let hit = |k: &str| crate::t!(k).to_lowercase().contains(q);
    if q.is_empty() || hit("sp.vo.t1") || hit("settings.page.voice_video") {
        return [true; 3];
    }
    [hit("voice.input_device") || hit("voice.output_device"), hit("voice.input_volume") || hit("voice.output_volume"), hit("voice.mic_test")]
}

static LOWER: &[B] = &[
    B::Custom("sp.vo.profile", profile_row),
    B::Custom("sp.vo.ptt", ptt_row),
    B::Expand(
        "vo-adv",
        "sp.vo.adv",
        "sp.vo.adv_d",
        &[
            B::Custom("voice.agc", agc_row),
            B::Custom("voice.vad_krisp", vad_krisp_row),
            B::Switch("vo-suggest", "sp.vo.suggest", "sp.vo.suggest_d", Bind::Pref(true)),
            B::Switch("vo-silent", "sp.vo.silent", "sp.vo.silent_d", Bind::Pref(true)),
            B::Switch("vo-switch-warn", "sp.vo.switch_warn", "sp.vo.switch_warn_d", Bind::Pref(true)),
            B::Switch("vo-qos", "sp.vo.qos", "sp.vo.qos_d", Bind::Pref(false)),
        ],
    ),
    B::Custom("", adv_divider),
    B::Title("sp.vo.t2"),
    B::Custom("sp.vo.test_video", camera_preview),
    B::Switch("vo-always-preview", "sp.vo.always_preview", "sp.vo.always_preview_d", Bind::Proto),
    B::Custom("sp.vo.camera", camera_select),
    B::Custom("sp.vo.bg", backgrounds),
    B::Divider,
    B::Title("sp.vo.t3"),
    B::Switch("vo-stream-preview", "sp.vo.stream_preview", "sp.vo.stream_preview_d", Bind::Proto),
    B::Expand(
        "vo-stream-adv",
        "sp.vo.stream_adv",
        "sp.vo.stream_adv_d",
        &[B::Custom("sp.vo.stream_att", stream_attenuation), B::Custom("sp.vo.stream_att_force", stream_strength)],
    ),
    B::Divider,
    B::Title("sp.vo.sounds"),
    B::Custom("sp.snd.deafen", voice_sounds),
    B::Related("bell", "settings.page.notifications", "sp.vo.sounds_d", super::Page::Notifications),
    B::Divider,
    B::Title("settings.sub.soundboard"),
    B::Custom("soundboard.volume", soundboard_settings),
];

fn soundboard_settings(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let volume = app.settings.as_ref().and_then(|s| s.soundboard_volume_preview)
        .unwrap_or_else(|| crate::voice::soundboard::setting_volume(app));
    let save_volume: OnSlide = Rc::new(|app, fraction, done, cx| {
        let volume = (fraction * 100.).round();
        if let Some(s) = app.settings.as_mut() {
            s.soundboard_volume_preview = (!done).then_some(volume);
        }
        if done {
            super::save(app, super::SettingsPatch::Proto("sb-volume", u64::from(volume.to_bits())), cx);
        }
    });
    div().flex().flex_col()
        .child(h5(crate::t!("soundboard.volume")).pb(z(4.)))
        .child(soundboard_help("sb-volume-help", crate::t!("sp.vo.soundboard_volume_desc")))
        .child(slider("settings-soundboard", volume / 100., format!("{volume:.0}%").into(), None, save_volume, cx))
        .child(div().mt(z(22.)).text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD)
            .text_color(c(tok(idx::TEXT_STRONG))).child(crate::t!("sp.vo.entrance_sounds")))
        .child(div().mt(z(4.)).child(soundboard_help("sb-entrance-help", crate::t!("sp.vo.entrance_sounds_desc"))))
        .child(entrance_sound_fields(app, cx))
        .into_any_element()
}

fn soundboard_help(id: &'static str, text: SharedString) -> Div {
    let style = gpui::HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() };
    let (text, highlights) = super::account::bracket_runs(&text, style);
    let links = highlights.iter().map(|(range, _)| range.clone()).collect();
    note("").child(
        gpui::InteractiveText::new(id, gpui::StyledText::new(text).with_highlights(highlights))
            .on_click(links, |_, _, cx| cx.open_url("https://support.discord.com/hc/articles/12612888127767")),
    )
}

fn entrance_sound_fields(app: &PhoenixApp, cx: &mut Cx) -> Div {
    use crate::ui::{controls, pickers::{Pickers, soundboard}};
    use phoenix_api::rest::settings::JoinSound;

    let premium = app.backend.premium_tier2();
    let mut guilds: Vec<_> = app.guilds.iter().filter(|g| !crate::ui::join_flow::is_lurking(g.id)).collect();
    guilds.sort_by_cached_key(|g| g.name.to_lowercase());
    let mut scopes = vec![0];
    let mut guild_names = vec![crate::t!("sp.vo.entrance_all_servers")];
    for guild in &guilds {
        scopes.push(guild.id.0);
        guild_names.push(guild.name.clone());
    }
    let target = app.settings.as_ref().map_or(0, |s| s.soundboard_target);
    let scope = scopes.iter().position(|id| *id == target).unwrap_or(0);
    let target = scopes[scope];
    let guild_field = super::pages::dropdown_with(
        app, "entrance-guild", controls::select_field("entrance-guild-field", guild_names[scope].clone()),
        guild_names, scope, relative(1.).into(), true,
        move |a, i, cx| {
            if let Some(settings) = a.settings.as_mut() {
                settings.soundboard_target = scopes[i];
                settings.open_select = None;
            }
            cx.notify();
        },
        cx,
    );

    let current = app.prefs.synced.join_sounds.get(&target).copied();
    let selecting = app.settings.as_ref().is_some_and(|s| s.open_select == Some("entrance-sound"));
    if current.is_some() || selecting {
        soundboard::ensure_loaded(app, cx);
    }
    let catalog = &cx.global::<Pickers>().soundboard;
    let mut names = vec![crate::t!("sp.vo.entrance_none")];
    let mut choices = vec![None];
    for sound in catalog.defaults.iter().flatten().filter(|s| s.available) {
        if let Some(name) = &sound.name {
            names.push(name.clone());
            choices.push(Some(JoinSound { sound: sound.id.0, source_guild: 0 }));
        }
    }
    for guild in guilds {
        for sound in catalog.guild_sounds.get(&guild.id).into_iter().flatten().filter(|s| s.available) {
            if let Some(name) = &sound.name {
                names.push(format!("{name} · {}", guild.name).into());
                choices.push(Some(JoinSound { sound: sound.id.0, source_guild: guild.id.0 }));
            }
        }
    }
    let selected = choices.iter().position(|sound| *sound == current);
    let name = selected.map(|i| names[i].clone()).unwrap_or_else(|| {
        if catalog.loading { crate::t!("sp.vo.entrance_loading") } else { crate::t!("sp.vo.entrance_unavailable") }
    });
    let field = div().id("entrance-sound-field")
        .h(z(40.)).px(z(12.)).flex().items_center().gap(z(4.)).rounded(z(8.)).border_1()
        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT))).bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .when(premium, |d| d.cursor_pointer().hover(|s| s.border_color(c(tok(idx::INPUT_BORDER_HOVER)))))
        .text_size(z(16.)).text_color(c(tok(idx::TEXT_STRONG)))
        .child(div().min_w_0().overflow_hidden().whitespace_nowrap().child(name))
        .child(icon(Icon::Speaker, 18., c(tok(idx::TEXT_SUBTLE))))
        .child(div().flex_1())
        .child(gpui::svg().path(crate::icons::path("pen")).size(z(20.)).flex_none().text_color(c(tok(idx::TEXT_SUBTLE))));
    let sound_field = if premium {
        super::pages::dropdown_with(
            app, "entrance-sound", field, names, selected.unwrap_or(usize::MAX), z(321.).into(), true,
            move |a, i, cx| {
                if let Some(settings) = a.settings.as_mut() {
                    settings.open_select = None;
                }
                if a.prefs.synced.join_sounds.get(&target).copied() != choices[i] {
                    super::save(a, super::SettingsPatch::JoinSound(target, choices[i]), cx);
                }
                cx.notify();
            },
            cx,
        )
    } else {
        field.w(z(321.)).max_w_full().opacity(0.6).into_any_element()
    };
    div()
        .child(div().mt(z(32.)).child(h5(crate::t!("sp.vo.entrance_server"))).child(guild_field))
        .child(div().mt(z(46.))
            .child(div().flex().items_start().gap(z(4.))
                .child(h5(crate::t!("sp.vo.entrance_sound")))
                .child(div().pt(z(2.)).child(icon(Icon::Nitro, 14., c(tok(idx::TEXT_STRONG))))))
            .child(sound_field)
            .when(!premium, |d| d.child(note(crate::t!("sp.vo.entrance_nitro")).mt(z(6.)))))
}

fn voice_sounds(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let notice = (!app.prefs.local.notification_sounds).then(|| {
        div()
            .p(z(8.))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .text_size(z(14.))
            .line_height(z(18.))
            .text_color(c(tok(idx::TEXT_DEFAULT)))
            .child(crate::t!("sp.snd.off_notice"))
    });
    div().flex().flex_col().gap(z(10.)).children(notice).child(super::pages::render(app, super::pages::VOICE_SOUNDS, cx)).into_any_element()
}

fn adv_divider(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    div().mt(z(40.)).mb(z(50.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))).into_any_element()
}

fn profile_row(_: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let p = prefs().lock().clone();
    let opts: [(&str, &str); 3] = [("sp.vo.iso", "sp.vo.iso_d"), ("sp.vo.studio", "sp.vo.studio_d"), ("sp.vo.custom", "sp.vo.custom_d")];
    let group = div().pt(z(17.)).flex().flex_col().gap(z(16.)).children(opts.iter().enumerate().map(|(i, (t, d))| {
        let sel = p.audio_profile == i as u32;
        div()
            .id(eid(format!("vo-profile-{i}")))
            .flex()
            .gap(z(12.))
            .cursor_pointer()
            .on_click(cx.listener(move |_, _, _, cx| {
                update(move |p| p.audio_profile = i as u32, true);
                cx.notify();
            }))
            .child(crate::ui::controls::radio_indicator(sel))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(z(4.))
                    .child(div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(crate::t!(*t)))
                    .child(note(crate::t!(*d))),
            )
    }));
    let mut col = div().pt(z(19.)).pb(z(15.)).flex().flex_col().child(h5(crate::t!("sp.vo.profile")).pb_0()).child(group);
    if p.audio_profile == PROFILE_CUSTOM {
        col = col
            .child(div().mt(z(24.)).child(sensitivity(&p, cx)))
            .child(div().mt(z(16.)).child(switch_row("echo", crate::t!("voice.echo"), "".into(), p.echo_cancellation, false, |p| p.echo_cancellation = !p.echo_cancellation, cx)))
            .child(noise_row(&p, cx));
    }
    col.into_any_element()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NoiseMode {
    Rnnoise,
    Standard,
    Off,
}

impl NoiseMode {
    const ALL: [Self; 3] = [Self::Rnnoise, Self::Standard, Self::Off];

    pub(crate) fn effective(p: &VoicePrefs) -> Self {
        let processing = p.processing();
        if processing.noise_cancellation {
            Self::Rnnoise
        } else if processing.noise_suppression {
            Self::Standard
        } else {
            Self::Off
        }
    }

    pub(crate) fn apply(self, p: &mut VoicePrefs) {
        p.noise_cancellation = self == Self::Rnnoise;
        p.noise_suppression = self == Self::Standard;
    }

    pub(crate) fn label(self) -> SharedString {
        crate::t!(match self {
            Self::Rnnoise => "voice.noise_krisp",
            Self::Standard => "voice.noise_standard",
            Self::Off => "voice.noise_none",
        })
    }
}

fn noise_row(p: &VoicePrefs, cx: &mut Cx) -> Div {
    let now = NoiseMode::effective(p);
    let entries = NoiseMode::ALL
        .iter()
        .map(|m| {
            let m = *m;
            let set: SetPref = Rc::new(move |p: &mut VoicePrefs| m.apply(p));
            (m.label(), m == now, set)
        })
        .collect();
    div()
        .py(z(10.))
        .flex()
        .items_center()
        .gap(z(16.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(crate::t!("voice.noise")))
                .child(note(crate::t!("voice.noise_select_note"))),
        )
        .child(
            div()
                .id("voice-noise-mode-selector")
                .w(z(220.))
                .flex_none()
                .when(now == NoiseMode::Rnnoise, |d| {
                    let engine = crate::t!("voice.noise_krisp_engine");
                    d.aria_description(engine.clone()).tooltip(crate::ui::tooltip::tip(engine))
                })
                .child(select_box("noise-mode", now.label(), entries, cx)),
        )
}

fn agc_row(_: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let p = prefs().lock().clone();
    if p.audio_profile != PROFILE_CUSTOM {
        return div().into_any_element();
    }
    switch_row("agc", crate::t!("voice.agc"), crate::t!("voice.agc_note"), p.agc, false, |p| p.agc = !p.agc, cx).into_any_element()
}

fn vad_krisp_row(_: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let p = prefs().lock().clone();
    if p.audio_profile != PROFILE_CUSTOM {
        return div().into_any_element();
    }
    let disabled = p.input_mode != InputMode::VoiceActivity || !p.auto_sensitivity;
    switch_row("vad-krisp", crate::t!("voice.vad_krisp"), crate::t!("voice.vad_krisp_note"), p.vad_use_krisp, disabled, |p| p.vad_use_krisp = !p.vad_use_krisp, cx)
        .into_any_element()
}

fn ptt_row(_: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let p = prefs().lock().clone();
    if p.audio_profile == PROFILE_STUDIO {
        return div().into_any_element();
    }
    let on = p.input_mode == InputMode::PushToTalk;
    div()
        .pb(z(3.))
        .flex()
        .flex_col()
        .child(switch_row(
            "ptt",
            crate::t!("sp.vo.ptt"),
            "".into(),
            on,
            false,
            move |p| p.input_mode = if on { InputMode::VoiceActivity } else { InputMode::PushToTalk },
            cx,
        ))
        .when(on, |d| d.child(div().pt(z(10.)).child(keybind(&p, cx))))
        .into_any_element()
}

fn camera_preview(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    div()
        .py(z(10.))
        .child(
            div()
                .h(z(220.))
                .rounded(z(4.))
                .border_1()
                .border_color(gpui::black())
                .bg(gpui::black())
                .flex()
                .items_center()
                .justify_center()
                .child(crate::ui::controls::button("vo-test-video", crate::t!("sp.vo.test_video"), crate::ui::controls::BtnKind::Primary)),
        )
        .into_any_element()
}

#[cfg(windows)]
pub(crate) fn cameras() -> Vec<String> {
    phoenix_voice::media::camera::windows_cameras()
}

#[cfg(not(windows))]
pub(crate) fn cameras() -> Vec<String> {
    let mut nodes: Vec<(u32, String)> = std::fs::read_dir("/sys/class/video4linux")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n: u32 = e.file_name().to_str()?.strip_prefix("video")?.parse().ok()?;
            let index = std::fs::read_to_string(e.path().join("index")).ok().and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(0);
            let name = std::fs::read_to_string(e.path().join("name")).ok()?.trim().to_string();
            (index == 0).then_some((n, name))
        })
        .collect();
    nodes.sort();
    let mut out: Vec<String> = Vec::new();
    for (_, n) in nodes {
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

fn camera_select(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    thread_local!(static CAMS: RefCell<Option<Vec<String>>> = const { RefCell::new(None) });
    let cams = CAMS.with(|c| c.borrow_mut().get_or_insert_with(cameras).clone());
    let opts: Vec<SharedString> =
        if cams.is_empty() { vec![crate::t!("voice.no_video")] } else { cams.into_iter().map(SharedString::from).collect() };
    let cur = app.settings.as_ref().and_then(|s| s.picks.get("vo-camera").copied()).unwrap_or(0).min(opts.len() - 1);
    let dd = super::pages::dropdown(
        app,
        "vo-camera",
        opts,
        cur,
        696.,
        |a, i, cx| {
            if let Some(s) = a.settings.as_mut() {
                s.picks.insert("vo-camera", i);
                s.open_select = None;
            }
            cx.notify();
        },
        cx,
    );
    div().py(z(10.)).flex().flex_col().gap(z(8.)).child(h5(crate::t!("sp.vo.camera")).pb_0()).child(dd).into_any_element()
}

const PRESETS: &[(&str, Option<&str>, bool)] = &[
    ("sp.vo.bg1", None, true),
    ("sp.vo.bg2", None, true),
    ("sp.vo.bg3", None, true),
    ("sp.vo.bg4", None, true),
    ("sp.vo.bg5", Some("https://cdn.discordapp.com/assets/content/e321ddf99270fad41d4c5b21de89a82984d8469c892a3f9a924f5996764d33b4.png"), false),
    ("sp.vo.bg6", Some("https://cdn.discordapp.com/assets/content/435961b527cedf836cfdfb34cf7c4c7353b248f93be15c30a9a3b778b7d20c76.png"), false),
    ("sp.vo.bg7", Some("https://cdn.discordapp.com/assets/content/cc4247f477caba753bfda3b66dd480a417c59ccd741b3809fedbfc3c8bdd49fd.png"), false),
    ("sp.vo.bg8", Some("https://cdn.discordapp.com/assets/content/857242c637737acc689e8e06fdde1ce120fc595fd65833f4f86944eaea46527e.png"), false),
];
const BLUR_IMG: &str = "https://cdn.discordapp.com/assets/content/9b1bb92a5685e3f1d67378826197687bb24f8cf1319a4ab2375fbc8a3fb7d0fe.png";

fn new_badge() -> Div {
    div()
        .absolute()
        .top(z(-3.))
        .left(z(-6.))
        .p(z(3.))
        .rounded_full()
        .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
        .child(
            div()
                .h(z(16.))
                .px(z(8.))
                .rounded_full()
                .bg(gpui::white())
                .flex()
                .items_center()
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(gpui::black())
                .child(crate::t!("sp.vo.new")),
        )
}

fn backgrounds(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let cur = app.settings.as_ref().and_then(|s| s.picks.get("vo-bg").copied()).unwrap_or(0);
    let label = |icon: &'static str, t: SharedString| {
        div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(div().h(z(24.)).flex().items_center().child(gpui::svg().path(crate::icons::path(icon)).size(z(18.)).text_color(gpui::white())))
            .child(div().px(z(4.)).text_size(z(14.)).line_height(z(18.)).text_color(gpui::white()).child(t))
    };
    let tile = |i: usize, inner: Div, badge: bool, selectable: bool, cx: &mut Cx| {
        div()
            .id(eid(format!("vo-bg-{i}")))
            .relative()
            .w(z(162.))
            .h(z(94.))
            .when(selectable, |d| {
                d.cursor_pointer().on_click(cx.listener(move |a, _, _, cx| {
                    if let Some(s) = a.settings.as_mut() {
                        s.picks.insert("vo-bg", i);
                    }
                    cx.notify();
                }))
            })
            .child(inner.absolute().inset_0().rounded(z(4.)).overflow_hidden())
            .when(cur == i, |d| {
                d.child(div().absolute().top(z(-4.)).left(z(-4.)).w(z(170.)).h(z(102.)).rounded(z(4.)).border_2().border_color(c(0x5865F2)))
            })
            .when(badge, |d| d.child(new_badge()))
    };
    let mut tiles: Vec<AnyElement> = Vec::new();
    tiles.push(tile(0, div().bg(c(0x1E1F22)).child(label("deny", crate::t!("sp.vo.bg_none"))), false, true, cx).into_any_element());
    tiles.push(
        tile(
            1,
            div().bg(c(0x1E1F22)).child(gpui::img(BLUR_IMG).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover)).child(label("blur", crate::t!("sp.vo.bg_blur"))),
            false,
            true,
            cx,
        )
        .into_any_element(),
    );
    let custom = div()
        .bg(gpui::linear_gradient(90., gpui::linear_color_stop(c(0x8547C6), 0.), gpui::linear_color_stop(c(0xAB5D8A), 1.)))
        .child(div().absolute().inset_0().bg(c(0x5F606A)).opacity(0.2))
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(div().h(z(24.)).flex().items_center().child(gpui::svg().path(crate::icons::path("image-plus")).size(z(18.)).text_color(gpui::white())))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .child(gpui::svg().path(crate::icons::path("nitro")).size(z(14.)).text_color(gpui::white()))
                        .child(div().text_size(z(14.)).line_height(z(18.)).text_color(gpui::white()).child(crate::t!("sp.vo.bg_custom"))),
                ),
        );
    tiles.push(tile(2, custom, true, false, cx).into_any_element());
    for (k, (_, img, badge)) in PRESETS.iter().enumerate() {
        let i = k + 3;
        let inner = match img {
            Some(u) => div().bg(c(0x1E1F22)).child(gpui::img(*u).size_full().object_fit(gpui::ObjectFit::Cover)),
            None => div().bg(c(0x1E1F22)).child(
                div().absolute().top(z(4.)).right(z(4.)).size(z(16.)).flex().items_center().justify_center().child(
                    gpui::svg().path(crate::icons::path("play")).size(z(12.)).text_color(gpui::white()),
                ),
            ),
        };
        tiles.push(tile(i, inner, *badge, true, cx).into_any_element());
    }
    div()
        .py(z(10.))
        .flex()
        .flex_col()
        .child(h5(crate::t!("sp.vo.bg")).pb_0())
        .child(div().pt(z(24.)).flex().flex_wrap().gap_x(z(16.)).gap_y(z(15.)).children(tiles))
        .into_any_element()
}

fn stream_attenuation(_: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let p = prefs().lock().clone();
    switch_row(
        "stream-att",
        crate::t!("sp.vo.stream_att"),
        crate::t!("sp.vo.stream_att_d"),
        p.attenuate_others,
        false,
        |p| p.attenuate_others = !p.attenuate_others,
        cx,
    )
    .into_any_element()
}

fn stream_strength(_: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let p = prefs().lock().clone();
    div()
        .py(z(10.))
        .flex()
        .flex_col()
        .child(h5(crate::t!("sp.vo.stream_att_force")))
        .child(pct_slider("attenuation", p.attenuation, 100, |p, v| p.attenuation = v, cx))
        .into_any_element()
}

#[cfg(test)]
mod search_tests {
    #[test]
    fn the_top_rows_follow_the_settings_search() {
        assert_eq!(super::top_rows(""), [true; 3]);
        let mic = crate::t!("voice.input_device").to_lowercase();
        assert!(super::top_rows(&mic)[0]);
        let test = crate::t!("voice.mic_test").to_lowercase();
        assert!(super::top_rows(&test)[2]);
        let vol = crate::t!("voice.output_volume").to_lowercase();
        assert!(super::top_rows(&vol)[1]);
        assert_eq!(super::top_rows("zzzz-nothing"), [false; 3]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_select_maps_to_the_effective_processing() {
        let mut p = VoicePrefs::default();
        assert_eq!(NoiseMode::effective(&p), NoiseMode::Rnnoise);
        NoiseMode::Standard.apply(&mut p);
        assert!(p.noise_suppression && !p.noise_cancellation);
        assert_eq!(NoiseMode::effective(&p), NoiseMode::Standard);
        NoiseMode::Off.apply(&mut p);
        assert!(!p.noise_suppression && !p.noise_cancellation);
        assert_eq!(NoiseMode::effective(&p), NoiseMode::Off);
        assert_eq!(crate::i18n::t_in("fr", "voice.noise_krisp", &[]).as_ref(), "Krisp");
        assert_eq!(crate::i18n::t_in("en-US", "voice.noise_none", &[]).as_ref(), "None");
    }
}
