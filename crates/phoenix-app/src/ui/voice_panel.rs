use crate::ui::tooltip::Tipped as _;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, Entity, FontWeight, Hsla, IntoElement, MouseButton, ParentElement, Pixels, Point,
    SharedString, Styled, Window, div, prelude::*, px, svg,
};

use crate::app::PhoenixApp;
use crate::assets::Icon;
use crate::theme::{Palette, c, generated::idx, tok, z};

fn t(i: usize) -> Hsla {
    c(tok(i))
}
use crate::model::{Id, VoiceFlags};
use crate::icons::lottie_icons::Lottie;
use crate::ui::context_menu::{self, MenuItem};
use crate::ui::icon;
use crate::ui::settings::voice::{NoiseMode, quick_mic_test, stop_quick_mic_test};
use crate::voice::prefs::{prefs, update};
use crate::voice::{Quality, VoiceStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Connection,
    Privacy,
}

thread_local! {
    static USER_MENU: Cell<Option<(Id, Point<Pixels>)>> = const { Cell::new(None) };
    static POPOUT: Cell<Option<Tab>> = const { Cell::new(None) };
    static NOISE_POPOUT: Cell<Option<Id>> = const { Cell::new(None) };
    static NOISE_ANCHOR: Cell<Option<gpui::Bounds<Pixels>>> = const { Cell::new(None) };
    static AUDIO_MENU: RefCell<Option<AudioMenu>> = const { RefCell::new(None) };
    static AUDIO_FULL: Cell<bool> = const { Cell::new(false) };
    static AUDIO_GROUP: Cell<[Option<gpui::Bounds<Pixels>>; 2]> = const { Cell::new([None; 2]) };
    static CAMERA_MENU: RefCell<Option<(Point<Pixels>, Vec<String>)>> = const { RefCell::new(None) };
    static CAMERA_PICK: RefCell<Option<String>> = const { RefCell::new(None) };
    static FLAGS: RefCell<HashMap<Id, VoiceFlags>> = RefCell::new(HashMap::new());
}

pub fn set_flags(user: Id, flags: VoiceFlags) {
    FLAGS.with(|f| f.borrow_mut().insert(user, flags));
}

pub fn flags(user: Id) -> VoiceFlags {
    FLAGS.with(|f| f.borrow().get(&user).copied().unwrap_or_default())
}

pub fn channel_has_video(users: &[Id]) -> bool {
    users.iter().any(|u| flags(*u).video)
}

type AudioMenu = (Audio, Point<Pixels>, Vec<String>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audio {
    Mic,
    Deafen,
}

const CAMERA: &str = "icons/voice/camera.svg";
const SCREEN: &str = "icons/voice/screen.svg";
const SOUNDBOARD: &str = "icons/voice/soundboard.svg";
const ACTIVITY: &str = "icons/voice/activity.svg";
const SHIELD: &str = "icons/voice/shield.svg";
const NOISE: &str = "icons/voice/noise.svg";
const NOISE_OFF: &str = "icons/voice/noise-off.svg";
const KRISP_WORDMARK: &str = "icons/voice/krisp-wordmark.svg";
const VIDEO: &str = "icons/voice/video-small.svg";

pub fn svg_asset(path: &str) -> Option<&'static str> {
    Some(match path {
        KRISP_WORDMARK => r##"<svg width="176" height="82" viewBox="0 0 176 82" fill="none" xmlns="http://www.w3.org/2000/svg">
<path d="M85.7001 16.2816C89.9614 16.2816 93.4159 12.8165 93.4159 8.54216C93.4159 4.26779 89.9614 0.802734 85.7001 0.802734C81.4388 0.802734 77.9844 4.26779 77.9844 8.54216C77.9844 12.8165 81.4388 16.2816 85.7001 16.2816Z" fill="#131032"/>
<path d="M20.5192 41.2251L40.3121 21.3714C40.6528 21.0296 40.4105 20.4372 39.9259 20.4372H23.9341C23.7902 20.4372 23.6539 20.498 23.5479 20.5967L13.2426 30.9337V3.78111C13.2426 3.39376 12.8489 3.12793 12.493 3.27224L1.18058 7.86729C0.976138 7.95084 0.839844 8.14831 0.839844 8.37616V61.8534C0.839844 62.3547 1.24873 62.7648 1.74847 62.7648H12.334C12.8337 62.7648 13.2426 62.3547 13.2426 61.8534V51.5165L24.3127 62.6129C24.4111 62.7117 24.555 62.7724 24.6989 62.7724L40.6982 62.7648C41.1828 62.7648 41.4251 62.1724 41.0844 61.8306L20.5192 41.2251Z" fill="#131032"/>
<path d="M80.4052 62.7648H90.9907C91.4904 62.7648 91.8993 62.3547 91.8993 61.8534V21.3562C91.8993 20.855 91.4904 20.4448 90.9907 20.4448H80.4052C79.9055 20.4448 79.4966 20.855 79.4966 21.3562V61.861C79.5041 62.3623 79.9055 62.7648 80.4052 62.7648Z" fill="#131032"/>
<path fill-rule="evenodd" clip-rule="evenodd" d="M155.412 20.4904C165.816 21.2043 174.244 29.6577 174.97 40.0858C175.826 52.4659 166.073 62.7648 153.928 62.7724C150.824 62.7724 147.878 62.0889 145.228 60.8813V74.5905C145.228 74.9626 145.008 75.2968 144.66 75.4335L133.575 79.9374C133.219 80.0817 132.825 79.8235 132.825 79.4285V41.6048H132.833C132.833 29.4147 143.085 19.6398 155.412 20.4904ZM145.228 41.6048C145.228 46.4201 149.12 50.3316 153.928 50.3316C158.729 50.3316 162.621 46.4201 162.621 41.6048C162.621 36.7895 158.729 32.878 153.928 32.878C149.12 32.878 145.228 36.7895 145.228 41.6048Z" fill="#131032"/>
<path d="M112.805 30.6375C112.843 30.6375 112.889 30.6375 112.926 30.6451C114.963 30.5615 116.796 31.8679 118.287 33.911C118.53 34.2452 118.969 34.3591 119.347 34.2072L127.873 30.7666C128.343 30.5767 128.57 30.0375 128.373 29.5666C126.23 24.3867 119.711 20.4448 112.798 20.4448C103.772 20.4448 96.4425 26.2019 96.4425 33.3034C96.4425 37.1085 98.6534 40.4124 101.69 42.5694C104.416 44.5062 107.672 45.7594 110.882 46.5948C110.935 46.61 110.988 46.6233 111.041 46.6366C111.094 46.6499 111.147 46.6632 111.2 46.6784C115.531 47.643 116.152 48.7898 116.152 49.9215C116.152 51.395 114.373 52.5874 112.184 52.5874C109.92 52.7317 107.914 51.1747 106.339 48.805C106.104 48.448 105.642 48.3189 105.249 48.4784L96.8211 51.9418C96.3592 52.1317 96.1471 52.6481 96.3213 53.119C98.7367 59.6812 105.249 62.78 112.184 62.78C121.21 62.78 128.54 57.0305 128.54 49.9215C128.54 48.3721 128.191 46.891 127.548 45.5163C126.934 44.2024 125.337 41.4074 121.203 39.4706C120.172 38.9902 118.345 38.3508 116.425 37.6791C113.944 36.8111 111.309 35.8892 110.042 35.187C109.269 34.7616 108.83 34.0553 108.83 33.3034C108.83 31.8299 110.617 30.6375 112.805 30.6375Z" fill="#131032"/>
<path d="M75.1276 21.1967C75.2715 20.8398 75.0064 20.4448 74.6203 20.4448H66.6319C55.0091 20.4448 45.5821 29.9008 45.5821 41.5593V61.8534C45.5821 62.3547 45.991 62.7648 46.4907 62.7648H57.0762C57.576 62.7648 57.9773 62.3623 57.9773 61.861V41.5593C57.9773 36.7591 61.8541 32.8704 66.6395 32.8704H69.9863C70.2059 32.8704 70.4027 32.7413 70.486 32.5363L75.1276 21.1967Z" fill="#131032"/>
</svg>"##,
        CAMERA => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<path d="M4 5h11a2 2 0 0 1 2 2v2.4l3.6-2.6A1 1 0 0 1 22 7.6v8.8a1 1 0 0 1-1.4.8L17 14.6V17a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z"/>"#,
            "</svg>"
        ),
        SCREEN => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<path fill-rule="evenodd" d="M4 3h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2h-6v2h3a1 1 0 1 1 0 2H7a1 1 0 1 1 0-2h3v-2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zm8 3-4 4h3v3h2v-3h3z"/>"#,
            "</svg>"
        ),
        SOUNDBOARD => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<path d="M12 3a9 9 0 0 1 9 9 1 1 0 1 1-2 0 7 7 0 0 0-14 0v2h2a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a9 9 0 0 1 9-9zm3 9h2a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2z"/>"#,
            "</svg>"
        ),
        ACTIVITY => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<rect x="2.5" y="2.5" width="8.5" height="8.5" rx="2.5"/><path d="M15.9 2.8a1.3 1.3 0 0 1 2.2 0l3.6 6.2a1.3 1.3 0 0 1-1.1 2H13.4a1.3 1.3 0 0 1-1.1-2z"/>"#,
            r#"<path d="M6.75 13l1.2 1.3 1.7-.4.4 1.7 1.7.6-.6 1.6.9 1.5-1.6.8.1 1.7-1.7-.2-1 1.4-1.1-1.4-1.7.2.1-1.7-1.6-.8.9-1.5-.6-1.6 1.7-.6.4-1.7 1.7.4z"/><path d="M17.25 12.5c.5 2.6 1.6 3.7 4.25 4.25-2.65.5-3.75 1.6-4.25 4.25-.5-2.65-1.6-3.75-4.25-4.25 2.65-.55 3.75-1.65 4.25-4.25z"/>"#,
            "</svg>"
        ),
        NOISE => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<rect x="3" y="9" width="2" height="6" rx="1"/><rect x="7" y="6" width="2" height="12" rx="1"/>"#,
            r#"<rect x="11" y="3" width="2" height="18" rx="1"/><rect x="15" y="6" width="2" height="12" rx="1"/>"#,
            r#"<rect x="19" y="9" width="2" height="6" rx="1"/>"#,
            "</svg>"
        ),
        VIDEO => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<path d="M4 6h10a2 2 0 0 1 2 2v1.5l4.4-2.9A1 1 0 0 1 22 7.4v9.2a1 1 0 0 1-1.6.8L16 14.5V16a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2z"/>"#,
            "</svg>"
        ),
        SHIELD => concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black">"#,
            r#"<path d="M12 2 4 5v6c0 5 3.4 9.4 8 11 4.6-1.6 8-6 8-11V5z"/>"#,
            "</svg>"
        ),
        _ => return None,
    })
}

fn asset_icon(path: &'static str, size: f32, color: Hsla) -> gpui::Svg {
    svg().path(SharedString::new_static(path)).size(px(size)).flex_none().text_color(color)
}

thread_local! {
    static ENLARGE: Cell<Option<bool>> = const { Cell::new(None) };
    static ENLARGE_FETCHING: Cell<bool> = const { Cell::new(false) };
}

pub fn set_target_size_experiment(list: &[(u32, i64)]) {
    let on = phoenix_api::rest::experiments::bucket(list, phoenix_api::rest::experiments::A11Y_TARGET_SIZE) >= 1;
    ENLARGE.with(|e| e.set(Some(on)));
}

pub fn enlarge_targets(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> bool {
    if let Some(on) = ENLARGE.with(Cell::get) {
        return on;
    }
    if !ENLARGE_FETCHING.with(|f| f.replace(true)) {
        let fut = app.backend.user_experiments();
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |_, cx| {
                match res {
                    Ok(list) => set_target_size_experiment(&list),
                    Err(_) => ENLARGE.with(|e| e.set(Some(false))),
                }
                cx.notify();
            });
        })
        .detach();
    }
    false
}

pub struct PanelBox {
    pub pad_top: f32,
    pub pad_bottom: f32,
    pub gap: f32,
    pub ping_top: f32,
    pub label_h: f32,
    pub label_top: f32,
    pub buttons_top: f32,
}

pub fn panel_box(enlarge: bool) -> PanelBox {
    if enlarge {
        PanelBox { pad_top: 8., pad_bottom: 14., gap: 10., ping_top: 6., label_h: 24., label_top: -1., buttons_top: 6. }
    } else {
        PanelBox { pad_top: 12., pad_bottom: 12., gap: 12., ping_top: 0., label_h: 20., label_top: 0., buttons_top: 0. }
    }
}

pub fn panel_button(id: impl Into<gpui::ElementId>, glyph: AnyElement, red: bool) -> gpui::Stateful<gpui::Div> {
    let id: gpui::ElementId = id.into();
    let key = crate::ui::states::key(("panel-btn", id.to_string(), red));
    let (rest, hover) = if red {
        (t(idx::OPACITY_RED_12), t(idx::OPACITY_RED_24))
    } else {
        (gpui::transparent_black(), t(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))
    };
    crate::ui::states::track(div().id(id), key)
        .relative()
        .child(crate::ui::states::fade_layer(key, rest, hover, 200, 8.))
        .size(z(32.))
        .flex_none()
        .rounded(z(8.))
        .when(crate::theme::high_contrast(), |d| {
            d.border_1().border_color(if red { t(idx::OPACITY_RED_52) } else { gpui::transparent_black() })
        })
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .group("panel-btn")
        .child(div().relative().child(glyph))
        .map(|d| crate::ui::focus_ring::ring_inset_edges(d, key, 8., -1., -1., 1., 1.))
}

pub fn panel_key(id: &str, red: bool) -> u64 {
    crate::ui::states::key(("panel-btn", gpui::ElementId::from(SharedString::from(id.to_owned())).to_string(), red))
}

pub fn lottie_glyph(key: u64, kind: Lottie, on: bool, red: bool, fallback: impl FnOnce() -> AnyElement) -> AnyElement {
    let rest = t(if red { idx::ICON_FEEDBACK_CRITICAL } else { idx::ICON_SUBTLE });
    let hover = if red { rest } else { t(idx::INTERACTIVE_TEXT_HOVER) };
    crate::ui::hover_icon::icon(key, kind, on, 20. * crate::theme::zoom(), (rest, hover), fallback)
}

pub fn panel_glyph(i: Icon, red: bool) -> AnyElement {
    let g = icon(i, 20. * crate::theme::zoom(), t(if red { idx::ICON_FEEDBACK_CRITICAL } else { idx::ICON_SUBTLE }));
    if red { g.into_any_element() } else { g.group_hover("panel-btn", |s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER))).into_any_element() }
}

pub fn audio_button(
    kind: Audio,
    i: Icon,
    red: bool,
    tip: SharedString,
    on_click: impl Fn(&mut PhoenixApp, &mut Context<PhoenixApp>) + 'static,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let group: SharedString = format!("audio-{kind:?}").into();
    let base = |d: gpui::Stateful<gpui::Div>| {
        d.h(z(32.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .when(red, |d| d.bg(t(idx::OPACITY_RED_12)).hover(|s| s.bg(t(idx::OPACITY_RED_24))))
            .when(!red, |d| {
                d.group_hover(group.clone(), |s| s.bg(t(idx::BACKGROUND_MOD_MUTED)))
                    .hover(|s| s.bg(t(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)))
            })
    };
    let open = AUDIO_MENU.with(|m| m.borrow().as_ref().is_some_and(|(k, _, _)| *k == kind));
    let hk = crate::ui::states::key(("audio-btn", format!("{kind:?}")));
    let lottie = match kind {
        Audio::Mic => Lottie::Mute,
        Audio::Deafen => Lottie::Deafen,
    };
    div()
        .group(group.clone())
        .relative()
        .flex()
        .flex_none()
        .gap(z(1.))
        .child(
            gpui::canvas(
                move |b, _, _| {
                    AUDIO_GROUP.with(|g| {
                        let mut all = g.get();
                        all[kind as usize] = Some(b);
                        g.set(all);
                    })
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(
            base(crate::ui::states::track(div().id(SharedString::from(format!("audio-btn-{kind:?}"))), hk))
                .when(crate::ui::a11y::on(), |d| {
                    d.role(gpui::Role::Switch)
                        .aria_label(match kind {
                            Audio::Mic => crate::t!("a11y.mute"),
                            Audio::Deafen => crate::t!("a11y.deafen"),
                        })
                        .aria_toggled(if red { gpui::Toggled::True } else { gpui::Toggled::False })
                })
                .w(z(32.))
                .rounded_l(z(8.))
                .child(lottie_glyph(hk, lottie, red, red, || panel_glyph(i, red)))
                .tip(tip)
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |_, ev: &gpui::MouseDownEvent, _, cx| {
                        let at = AUDIO_GROUP.with(|g| g.get()[kind as usize]).map_or(ev.position, |b| gpui::point(b.origin.x, b.origin.y - z(4.)));
                        toggle_audio_menu(kind, at);
                        cx.notify();
                    }),
                )
                .on_click(cx.listener(move |a, _, _, cx| on_click(a, cx))),
        )
        .child(
            base(div().id(SharedString::from(format!("audio-chev-{kind:?}"))))
                .when(crate::ui::a11y::on(), |d| {
                    d.role(gpui::Role::Button).aria_label(match kind {
                        Audio::Mic => crate::t!("voice.input_options"),
                        Audio::Deafen => crate::t!("voice.output_options"),
                    })
                })
                .w(z(16.))
                .rounded_r(z(8.))
                .when(open && !red, |d| d.bg(t(idx::BACKGROUND_MOD_MUTED)))
                .child(
                    gpui::svg()
                        .path(crate::icons::path(if open { "chevron-up" } else { "chevron" }))
                        .size(z(16.))
                        .flex_none()
                        .text_color(t(if red { idx::ICON_FEEDBACK_CRITICAL } else { idx::ICON_SUBTLE })),
                )
                .tip(match kind {
                    Audio::Mic => crate::t!("voice.input_options"),
                    Audio::Deafen => crate::t!("voice.output_options"),
                })
                .on_click(cx.listener(move |_, ev: &gpui::ClickEvent, _, cx| {
                    let at = AUDIO_GROUP.with(|g| g.get()[kind as usize]).map_or(ev.position(), |b| gpui::point(b.origin.x, b.origin.y - z(4.)));
                    toggle_audio_menu(kind, at);
                    cx.notify();
                })),
        )
        .into_any_element()
}

pub fn toggle_audio_menu(kind: Audio, pos: Point<Pixels>) {
    let now = AUDIO_MENU.with(|m| m.borrow().as_ref().map(|(k, _, _)| *k));
    let next = (now != Some(kind)).then(|| {
        let (inputs, outputs) = crate::voice::call::devices();
        (kind, pos, if kind == Audio::Mic { inputs } else { outputs })
    });
    AUDIO_MENU.with(|m| *m.borrow_mut() = next);
    AUDIO_FULL.with(|f| f.set(false));
}

pub fn toggle_call_audio_menu(pos: Point<Pixels>) {
    toggle_audio_menu(Audio::Mic, pos);
    AUDIO_FULL.with(|f| f.set(true));
}

pub const PROFILES: [&str; 3] = ["voice.profile_iso", "voice.profile_studio", "voice.profile_custom"];
pub use crate::voice::prefs::PROFILE_STUDIO;

fn device_submenu(kind: Audio, devices: Vec<String>) -> MenuItem {
    let current = {
        let p = prefs().lock();
        if kind == Audio::Mic { p.input_device.clone() } else { p.output_device.clone() }
    };
    let mut list = vec![MenuItem::radio(crate::t!("voice.default"), current.is_none(), move |_, _, _| {
        update(move |p| if kind == Audio::Mic { p.input_device = None } else { p.output_device = None }, true)
    })];
    for d in devices {
        let on = current.as_deref() == Some(d.as_str());
        let name = d.clone();
        list.push(MenuItem::radio(d, on, move |_, _, _| {
            let name = name.clone();
            update(move |p| if kind == Audio::Mic { p.input_device = Some(name) } else { p.output_device = Some(name) }, true)
        }));
    }
    MenuItem::Submenu {
        label: if kind == Audio::Mic { crate::t!("voice.input_device_menu") } else { crate::t!("voice.output_device_menu") },
        items: list,
        icon: None,
        subtext: Some(current.map(Into::into).unwrap_or_else(|| crate::t!("voice.default"))),
        danger: false,
        on_click: None,
    }
}

fn profile_submenu() -> MenuItem {
    let now = prefs().lock().audio_profile.min(2);
    let items = PROFILES
        .iter()
        .enumerate()
        .map(|(i, k)| {
            MenuItem::radio(crate::t!(*k), now == i as u32, move |_, _, _| {
                update(move |p| p.audio_profile = i as u32, true)
            })
        })
        .collect();
    MenuItem::Submenu { label: crate::t!("voice.input_profile_menu"), items, icon: None, subtext: Some(crate::t!(PROFILES[now as usize])), danger: false, on_click: None }
}

fn volume_row(kind: Audio, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let vol_now = {
        let p = prefs().lock();
        if kind == Audio::Mic { p.input_volume } else { p.output_volume }
    };
    let max = if kind == Audio::Mic { 100 } else { 200 };
    let frac = (vol_now as f32 / max as f32).clamp(0., 1.);
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .p(z(8.))
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(t(idx::TEXT_STRONG))
                .child(if kind == Audio::Mic { crate::t!("voice.input_volume_menu") } else { crate::t!("voice.output_volume_menu") }),
        )
        .child(div().px(z(8.)).mt(z(-4.)).child(crate::ui::settings::voice::slider(
            if kind == Audio::Mic { "menu-input-volume" } else { "menu-output-volume" },
            frac,
            format!("{vol_now}%").into(),
            Some((tok(idx::SLIDER_INDICATOR_BACKGROUND), tok(idx::SLIDER_TRACK_BACKGROUND))),
            Rc::new(move |_, f, done, _| {
                let v = (f * max as f32).round() as u32;
                update(move |p| if kind == Audio::Mic { p.input_volume = v } else { p.output_volume = v }, done);
            }),
            cx,
        )))
        .pb(z(4.))
        .into_any_element()
}

fn meter_row() -> AnyElement {
    let level = crate::ui::settings::voice::live_level();
    div()
        .flex()
        .flex_col()
        .child(div().p(z(8.)).text_size(z(14.)).line_height(z(18.)).font_weight(gpui::FontWeight::MEDIUM).text_color(t(idx::TEXT_STRONG)).child(crate::t!("voice.input_level_menu")))
        .child(div().mt(z(-4.)).mx(z(6.)).mb(z(16.)).flex().child(crate::ui::settings::voice::meter_bar(level.map_or(crate::ui::settings::voice::DB_MIN, |(db, _)| db), level.is_some(), 28., idx::BACKGROUND_SURFACE_HIGHER)))
        .when(level.is_some(), |d| d.child(gpui::canvas(|_, _, _| {}, |_, _, w, _| w.request_animation_frame()).size_0()))
        .into_any_element()
}

pub fn media_failure_modal(failure: &crate::voice::MediaFailure, screen: bool) {
    let title = if screen { crate::t!("golive.unavailable") } else { crate::t!("voice.camera_start_failed") };
    let (title, body) = match failure {
        crate::voice::MediaFailure::NoDevice => (crate::t!("voice.camera_unavailable"), crate::t!("voice.no_camera_detected")),
        crate::voice::MediaFailure::NoFfmpeg => (title, crate::t!("voice.ffmpeg_missing")),
        crate::voice::MediaFailure::Other(e) => (title, SharedString::from(e.clone())),
    };
    crate::actions::open_modal(
        crate::ui::modals::Modal::Form(crate::ui::modals::Form {
            title,
            subtitle: Some(body),
            fields: Vec::new(),
            choices: Vec::new(),
            choice: None,
            check: None,
            submit: crate::t!("common.okay"),
            danger: false,
            preview: None,
            width: 440.,
            on_submit: Rc::new(|_, _, _, _| {}),
        }),
        None,
    );
}

pub fn toggle_camera_menu(pos: Point<Pixels>) {
    let open = CAMERA_MENU.with(|m| m.borrow().is_some());
    let next = (!open).then(|| (pos, crate::ui::settings::voice::cameras()));
    CAMERA_MENU.with(|m| *m.borrow_mut() = next);
}

pub fn camera_menu_open() -> bool {
    CAMERA_MENU.with(|m| m.borrow().is_some())
}

pub fn picked_camera(cams: &[String], pick: Option<&str>) -> Option<usize> {
    pick.and_then(|p| cams.iter().position(|c| c == p)).or_else(|| (!cams.is_empty()).then_some(0))
}

fn camera_menu(pos: Point<Pixels>, cams: Vec<String>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let e = cx.entity();
    let dismiss: context_menu::Dismiss = Rc::new(move |_: &mut Window, cx: &mut App| {
        CAMERA_MENU.with(|m| *m.borrow_mut() = None);
        e.update(cx, |_, cx| cx.notify());
    });
    let current = CAMERA_PICK.with(|p| p.borrow().clone());
    let on = picked_camera(&cams, current.as_deref());
    let mut items: Vec<MenuItem> = Vec::new();
    if cams.is_empty() {
        items.push(MenuItem::disabled(crate::t!("voice.no_video"), None));
    }
    for (i, name) in cams.into_iter().enumerate() {
        let e = cx.entity();
        let pick = name.clone();
        items.push(MenuItem::radio(name, on == Some(i), move |_, _, cx: &mut App| {
            CAMERA_PICK.with(|p| *p.borrow_mut() = Some(pick.clone()));
            CAMERA_MENU.with(|m| *m.borrow_mut() = None);
            let pick = pick.clone();
            e.update(cx, |a, cx| {
                drop(a.backend.voice_control(crate::voice::VoiceControl::SetCameraDevice(Some(pick))));
                cx.notify();
            });
        }));
    }
    items.push(MenuItem::Separator);
    let e = cx.entity();
    items.push(MenuItem::action(crate::t!("voice.video_settings"), Some(crate::icons::path("gear")), move |_, _, cx| {
        CAMERA_MENU.with(|m| *m.borrow_mut() = None);
        e.update(cx, |a, cx| {
            crate::ui::settings::open(a, cx);
            if let Some(s) = a.settings.as_mut() {
                s.page = crate::ui::settings::Page::VoiceVideo;
            }
        });
    }));
    context_menu::render_anchored(pos - gpui::point(px(0.), px(12.)), gpui::Anchor::BottomLeft, items, dismiss)
}

pub fn audio_menu_open(kind: Audio) -> bool {
    AUDIO_MENU.with(|m| m.borrow().as_ref().is_some_and(|(k, _, _)| *k == kind))
}

fn audio_menu(kind: Audio, pos: Point<Pixels>, devices: Vec<String>, deafened: bool, in_voice: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let e = cx.entity();
    let dismiss: context_menu::Dismiss = Rc::new(move |_: &mut Window, cx: &mut App| {
        AUDIO_MENU.with(|m| *m.borrow_mut() = None);
        e.update(cx, |_, cx| cx.notify());
    });
    let full = kind == Audio::Mic && AUDIO_FULL.with(Cell::get);
    let e = cx.entity();
    let ptt = (kind == Audio::Mic).then(|| {
        let p = prefs().lock();
        p.ptt_key.as_ref().map(|_| (p.input_mode == crate::voice::prefs::InputMode::PushToTalk, p.audio_profile == PROFILE_STUDIO))
    });
    let mut items = vec![device_submenu(kind, devices)];
    if kind == Audio::Mic {
        items.push(profile_submenu());
    }
    if full {
        items.push(device_submenu(Audio::Deafen, crate::voice::call::devices().1));
    }
    items.push(MenuItem::Separator);
    items.push(MenuItem::Custom(volume_row(kind, cx)));
    if full || (kind == Audio::Mic && in_voice) {
        items.push(MenuItem::Custom(meter_row()));
    }
    if full {
        items.push(MenuItem::Custom(volume_row(Audio::Deafen, cx)));
    }
    items.push(MenuItem::Separator);
    if let Some(Some((on, studio))) = ptt {
        if studio {
            items.push(MenuItem::Disabled { label: crate::t!("voice.ptt_menu"), icon: None });
        } else {
            items.push(MenuItem::check(crate::t!("voice.ptt_menu"), on, move |_, _, _| {
                update(move |p| p.input_mode = ptt_toggled(on), true)
            }));
        }
    }
    if full {
        let d = cx.entity();
        items.push(MenuItem::check(crate::t!("voice.deafen"), deafened, move |_, _, cx: &mut App| {
            d.update(cx, |a, cx| {
                a.deafened = !a.deafened;
                crate::voice::sync_self_state(a);
                cx.notify();
            })
        }));
    }
    items.push(MenuItem::action(crate::t!("voice.voice_settings"), Some(crate::icons::path("gear")), move |_, _, cx| {
        AUDIO_MENU.with(|m| *m.borrow_mut() = None);
        e.update(cx, |a, cx| {
            crate::ui::settings::open(a, cx);
            if let Some(s) = a.settings.as_mut() {
                s.page = crate::ui::settings::Page::VoiceVideo;
            }
        });
    }));
    if full {
        return context_menu::render_anchored(pos - gpui::point(px(0.), px(8.)), gpui::Anchor::BottomCenter, items, dismiss);
    }
    context_menu::render_anchored(pos, gpui::Anchor::BottomLeft, items, dismiss)
}

fn toggle_advanced_noise_suppression(p: &mut crate::voice::prefs::VoicePrefs) {
    if p.audio_profile == crate::voice::prefs::PROFILE_CUSTOM {
        let next = if NoiseMode::effective(p) == NoiseMode::Rnnoise { NoiseMode::Standard } else { NoiseMode::Rnnoise };
        next.apply(p);
    }
}

pub fn ptt_toggled(on: bool) -> crate::voice::prefs::InputMode {
    use crate::voice::prefs::InputMode;
    if on { InputMode::VoiceActivity } else { InputMode::PushToTalk }
}

fn action_button(id: &'static str, path: &'static str, label: SharedString, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let hk = crate::ui::states::key(id);
    let fg = t(idx::CONTROL_SECONDARY_TEXT_DEFAULT);
    crate::ui::states::track(div().id(id), hk)
        .flex_1()
        .min_w_0()
        .h(z(32.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(Palette::BORDER_MUTED))
        .bg(t(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
        .hover(|s| s.bg(t(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .child(crate::ui::hover_icon::icon(hk, Lottie::Activities, false, 20. * crate::theme::zoom(), (fg, fg), || {
            asset_icon(path, 20. * crate::theme::zoom(), fg).into_any_element()
        }))
        .tip(label.clone())
        .on_click(cx.listener(move |a, _, _, cx| {
            crate::ui::hover_icon::clicked(hk);
            a.stub(&label, cx)
        }))
}

fn camera_button(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let on = flags(app.me.id).video;
    let (bg, hover, fg) = if on {
        (t(idx::OPACITY_GREEN_12), t(idx::OPACITY_GREEN_24), t(idx::GREEN_300))
    } else {
        (t(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT), t(idx::CONTROL_SECONDARY_BACKGROUND_HOVER), t(idx::CONTROL_SECONDARY_TEXT_DEFAULT))
    };
    let hk = crate::ui::states::key("voice-camera");
    crate::ui::states::track(div().id("voice-camera"), hk)
        .flex_1()
        .min_w_0()
        .h(z(32.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(Palette::BORDER_MUTED))
        .bg(bg)
        .hover(move |s| s.bg(hover))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .child(crate::ui::hover_icon::icon(hk, Lottie::Camera, on, 20. * crate::theme::zoom(), (fg, fg), || {
            asset_icon(if on { CAMERA } else { "icons/call/camera-off.svg" }, 20. * crate::theme::zoom(), fg).into_any_element()
        }))
        .tip(if on { crate::t!("voice.camera_off") } else { crate::t!("voice.camera_on") })
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(|_, ev: &gpui::MouseDownEvent, _, cx| {
                toggle_camera_menu(ev.position);
                cx.notify();
            }),
        )
        .on_click(cx.listener(move |a, _, _, cx| {
            drop(a.backend.voice_control(crate::voice::VoiceControl::SetCamera(!on)));
            cx.notify();
        }))
}

fn screen_button(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let on = crate::ui::go_live::live(app);
    let (bg, hover, fg) = if on {
        (t(idx::OPACITY_GREEN_12), t(idx::OPACITY_GREEN_24), t(idx::GREEN_300))
    } else {
        (t(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT), t(idx::CONTROL_SECONDARY_BACKGROUND_HOVER), t(idx::CONTROL_SECONDARY_TEXT_DEFAULT))
    };
    let hk = crate::ui::states::key("voice-screen");
    crate::ui::states::track(div().id("voice-screen"), hk)
        .flex_1()
        .min_w_0()
        .h(z(32.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(Palette::BORDER_MUTED))
        .bg(bg)
        .hover(move |s| s.bg(hover))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .child(crate::ui::hover_icon::icon(hk, Lottie::Stream, on, 20. * crate::theme::zoom(), (fg, fg), || {
            asset_icon(SCREEN, 20. * crate::theme::zoom(), fg).into_any_element()
        }))
        .tip(crate::ui::go_live::label(app))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |_, _, _, cx| {
                if on {
                    crate::ui::call_menus::toggle(crate::ui::call_menus::TrayMenu::Streams);
                    cx.notify();
                }
            }),
        )
        .on_click(cx.listener(|a, _, _, cx| crate::ui::go_live::toggle(a, cx)))
}

pub fn connection_icon(q: Quality) -> (&'static str, usize) {
    match q {
        Quality::Good => ("connection-fine", idx::ICON_FEEDBACK_POSITIVE),
        Quality::Fair => ("connection-average", idx::ICON_FEEDBACK_WARNING),
        Quality::Bad => ("connection-bad", idx::ICON_FEEDBACK_CRITICAL),
        Quality::Unknown => ("connection-unknown", idx::INTERACTIVE_ICON_DEFAULT),
    }
}

fn large_ping(q: Quality, bg: Hsla) -> impl IntoElement {
    let (name, fg) = connection_icon(q);
    div()
        .size(z(32.))
        .flex_none()
        .rounded(z(8.))
        .bg(bg)
        .border_1()
        .border_color(c(Palette::BORDER_MUTED))
        .flex()
        .items_center()
        .justify_center()
        .child(svg().path(crate::icons::path(name)).size(z(18.)).flex_none().text_color(t(fg)))
}

thread_local! {
    static SPEAK_FADE: std::cell::RefCell<std::collections::HashMap<u64, (bool, Option<std::time::Instant>)>> = Default::default();
}

pub const SPEAK_FADE_MS: u64 = 100;

pub fn speaking_fade(user: Id, active: bool) -> (f32, bool) {
    SPEAK_FADE.with(|m| {
        let mut m = m.borrow_mut();
        let entry = m.entry(user.0).or_insert((active, None));
        if entry.0 != active {
            entry.0 = active;
            entry.1 = Some(std::time::Instant::now());
        }
        let Some(since) = entry.1 else {
            return (if active { 1. } else { 0. }, false);
        };
        let p = (since.elapsed().as_secs_f32() / (SPEAK_FADE_MS as f32 / 1000.)).clamp(0., 1.);
        let eased = 1. - (1. - p) * (1. - p);
        if p >= 1. {
            entry.1 = None;
            (if active { 1. } else { 0. }, false)
        } else {
            (if active { eased } else { 1. - eased }, true)
        }
    })
}

pub fn speaking_ring(avatar: AnyElement, size: f32, speaking: bool) -> AnyElement {
    speaking_ring_alpha(avatar, size, if speaking { 1. } else { 0. }, false)
}

pub fn speaking_ring_alpha(avatar: AnyElement, size: f32, alpha: f32, soundboard: bool) -> AnyElement {
    let color = t(if soundboard { idx::BRAND_500 } else { idx::STATUS_SPEAKING });
    div()
        .size(px(size))
        .flex_none()
        .relative()
        .child(avatar)
        .when(alpha > 0.001, |d| {
            d.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded_full()
                    .opacity(alpha)
                    .border(z(2.))
                    .border_color(color)
                    .child(div().size_full().rounded_full().border(z(1.)).border_color(t(idx::BACKGROUND_BASE_LOWER))),
            )
        })
        .into_any_element()
}

pub fn self_speaking_inset(db: f32, reduced: bool) -> f32 {
    if reduced || !db.is_finite() || db <= crate::ui::settings::voice::DB_MIN {
        return 2.;
    }
    0.5 + 3.5 / (1. + (-0.03 * (db + 45.)).exp())
}

pub fn self_speaking_overlay(inset: f32, soundboard: bool) -> AnyElement {
    let color = t(if soundboard { idx::BRAND_500 } else { idx::STATUS_SPEAKING });
    div()
        .absolute()
        .inset_0()
        .rounded_full()
        .border(z(inset))
        .border_color(color)
        .child(div().size_full().rounded_full().border(z(1.)).border_color(t(idx::BACKGROUND_BASE_LOWER)))
        .into_any_element()
}

pub fn self_speaking_ring(app: &PhoenixApp) -> Option<AnyElement> {
    app.voice_channel?;
    let me = app.me.id;
    let soundboard = crate::voice::soundboard::is_user_playing(me);
    if !app.voice.is_speaking(me) && !soundboard {
        return None;
    }
    let db = crate::voice::call::manager().control().input_level_db();
    Some(self_speaking_overlay(self_speaking_inset(db, crate::ui::settings::effects::reduced_motion()), soundboard))
}

pub fn voice_panel(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let ch = app.voice_channel.and_then(|v| app.channels.get(&v));
    let guild = ch
        .and_then(|c| c.guild)
        .and_then(|g| app.guilds.iter().find(|x| x.id == g))
        .map(|g| g.name.clone())
        .unwrap_or_default();
    let name = ch.map(|c| crate::ui::dm_profile_panel::channel_name(app, c).to_string()).unwrap_or_default();
    let label: SharedString = if guild.is_empty() { name.into() } else { format!("{name} / {guild}").into() };
    let v = &app.voice;
    const OK: (usize, usize) = (idx::TEXT_FEEDBACK_POSITIVE, idx::BACKGROUND_FEEDBACK_MOD_POSITIVE);
    const WARN: (usize, usize) = (idx::TEXT_FEEDBACK_WARNING, idx::BACKGROUND_FEEDBACK_MOD_WARNING);
    const BAD: (usize, usize) = (idx::TEXT_FEEDBACK_CRITICAL, idx::BACKGROUND_FEEDBACK_MOD_CRITICAL);
    let (key, kind) = v.status.label(channel_has_video(ch.map_or(&[][..], |c| &c.voice_users[..])));
    let title = crate::t!(key);
    let (fg, bg) = match kind {
        crate::voice::StatusKind::Connected => match v.quality() {
            Quality::Fair => WARN,
            Quality::Bad => BAD,
            _ => OK,
        },
        crate::voice::StatusKind::Connecting => WARN,
        crate::voice::StatusKind::Error => BAD,
    };
    let color = t(fg);
    let ping: SharedString = match (v.ping_ms, &v.last_error) {
        (_, Some(e)) if matches!(v.status, VoiceStatus::Failed | VoiceStatus::NoRoute) => e.clone().into(),
        (Some(p), _) => crate::t!("voice.ping", p = p),
        (None, _) => crate::t!("voice.waiting_server"),
    };
    let noise = NoiseMode::effective(&prefs().lock()) == NoiseMode::Rnnoise;
    let noise_available = voice_status_has_noise_popout(app);
    let noise_open = noise_popout_open(app);
    let enlarge = enlarge_targets(app, cx);
    let bx = panel_box(enlarge);
    div()
        .flex_none()
        .border_b_1()
        .border_color(c(Palette::BORDER_MUTED))
        .px(z(12.))
        .pt(z(bx.pad_top))
        .pb(z(bx.pad_bottom))
        .flex()
        .flex_col()
        .gap(z(bx.gap))
        .child(
            div()
                .when(!enlarge, |d| d.h(z(32.)).items_center())
                .when(enlarge, |d| d.items_start())
                .flex()
                .gap(z(4.))
                .child(
                    div()
                        .id("voice-status")
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .when(!enlarge, |d| d.items_center())
                        .when(enlarge, |d| d.items_start())
                        .gap(z(8.))
                        .cursor_pointer()
                        .tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(ping.clone()).build(window, cx))
                        .on_click(cx.listener(|_, _, _, cx| {
                            POPOUT.with(|p| p.set(if p.get().is_some() { None } else { Some(Tab::Connection) }));
                            cx.notify();
                        }))
                        .child(div().flex_none().mt(z(bx.ping_top)).child(large_ping(v.quality(), t(bg))))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .mt(z(bx.label_top))
                                .when(enlarge, |d| d.pb(z(1.)))
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .h(z(bx.label_h))
                                        .pt(z(bx.label_h - 20.))
                                        .flex()
                                        .items_center()
                                        .gap(z(4.))
                                        .child(
                                            div()
                                                .text_size(z(16.))
                                                .line_height(z(20.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(color)
                                                .text_ellipsis()
                                                .child(title),
                                        )
                                )
                                .child(
                                    div()
                                        .id("voice-channel-link")
                                        .debug_selector(|| "voice-channel-link".to_owned())
                                        .text_size(z(12.))
                                        .line_height(z(16.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(c(Palette::TEXT_MUTED))
                                        .text_ellipsis()
                                        .hover(|s| s.underline())
                                        .on_click(cx.listener(|a, _, _, cx| {
                                            cx.stop_propagation();
                                            a.open_voice_channel(cx);
                                        }))
                                        .child(label),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .mt(z(bx.buttons_top))
                        .gap(z(4.))
                        .child(
                            panel_button(
                                "voice-noise",
                                lottie_glyph(panel_key("voice-noise", false), Lottie::Krisp, noise, false, || {
                                    asset_icon(if noise { NOISE } else { NOISE_OFF }, 20. * crate::theme::zoom(), t(idx::ICON_SUBTLE)).into_any_element()
                                }),
                                false,
                            )
                            .when(noise_available, |button| {
                                button
                                    .role(gpui::Role::Button)
                                    .aria_label(crate::t!("voice.noise"))
                                    .aria_expanded(noise_open)
                                    .child(
                                        gpui::canvas(
                                            |bounds, window, _| {
                                                let previous = NOISE_ANCHOR.with(|anchor| anchor.replace(Some(bounds)));
                                                if previous != Some(bounds) && NOISE_POPOUT.with(Cell::get).is_some() {
                                                    window.on_next_frame(|window, _| window.refresh());
                                                }
                                            },
                                            |_, _, _, _| {},
                                        )
                                        .absolute()
                                        .size_full(),
                                    )
                                    .tip(crate::t!("voice.noise"))
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        if !close_noise_popout() && voice_status_has_noise_popout(app) {
                                            POPOUT.with(|open| open.set(None));
                                            NOISE_POPOUT.with(|open| open.set(app.voice_channel));
                                        }
                                        cx.notify();
                                    }))
                            })
                            .when(!noise_available, |button| button.cursor(gpui::CursorStyle::OperationNotAllowed)),
                        )
                        .child(
                            panel_button(
                                "hangup",
                                lottie_glyph(panel_key("hangup", false), Lottie::Disconnect, false, false, || panel_glyph(Icon::PhoneHangup, false)),
                                false,
                            )
                                .tip(crate::t!("voice.disconnect"))
                                .on_click(cx.listener(|a, _, _, cx| a.join_voice(None, cx))),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .gap(z(8.))
                .child(camera_button(app, cx))
                .child(screen_button(app, cx))
                .child(action_button("voice-activity", ACTIVITY, crate::t!("voice.start_activity"), cx))
                .child(crate::ui::pickers::soundboard::panel_button(app, cx)),
        )
}

pub fn server_tag(tag: &(SharedString, Option<SharedString>), size: f32) -> AnyElement {
    div()
        .flex_shrink(1.)
        .min_w_0()
        .overflow_hidden()
        .ml(z(4.))
        .h(z(16.))
        .px(z(4.))
        .rounded(z(4.))
        .bg(t(idx::BACKGROUND_MOD_STRONG))
        .flex()
        .items_center()
        .gap(z(2.))
        .when_some(tag.1.clone(), |d, url| d.child(gpui::img(url).size(z(size)).flex_none()))
        .child(
            div()
                .flex_none()
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(Palette::TEXT_NORMAL))
                .child(tag.0.clone()),
        )
        .into_any_element()
}

pub fn voice_user_row(app: &PhoenixApp, entity: &Entity<PhoenixApp>, user: Id) -> AnyElement {
    let u = app.users.get(&user);
    let me = user == app.me.id;
    let in_call = app.voice_channel.is_some_and(|ch| app.channels.get(&ch).is_some_and(|c| c.voice_users.contains(&user)));
    let local_muted = !me && prefs().lock().user(user).muted;
    let speaking = app.voice.is_speaking(user) && !local_muted;
    let mut f = flags(user);
    if me && app.voice.status != crate::voice::VoiceStatus::Idle {
        f.self_mute = app.mic_muted;
        f.self_deaf = app.deafened;
    }
    let state = t(idx::CHANNELS_DEFAULT);
    let red = t(idx::ICON_FEEDBACK_CRITICAL);
    let tag = u.and_then(|u| u.tag.clone());
    let e2 = entity.clone();
    let selected = app.people.popout.as_ref().is_some_and(|p| p.user == user);
    let guild = app.selected_guild;
    let name_color = if speaking || selected { t(idx::INTERACTIVE_TEXT_ACTIVE) } else { state };
    div()
        .id(("voice-user-row", user.0))
        .group("voice-user-row")
        .cursor_pointer()
        .child(
            gpui::canvas(move |b, _, _| crate::ui::voice_row::record(user, b), |_, _, _, _| {})
                .absolute()
                .size_full(),
        )
        .ml(z(38.))
        .mr(z(8.))
        .h(z(32.))
        .px(z(10.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .gap(z(8.))
        .relative()
        .when(selected, |d| d.bg(t(idx::INTERACTIVE_BACKGROUND_SELECTED)))
        .when(!selected, |d| d.hover(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_HOVER))))
        .active(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_ACTIVE)))
        .on_click(move |ev, window, cx| {
            let at = crate::ui::voice_row::popout_anchor(user, ev.position());
            e2.update(cx, |a, cx| a.open_profile(user, guild, at, window, cx));
        })
        .on_hover({
            let e = entity.clone();
            move |h: &bool, _, cx| {
                if crate::ui::voice_row::set_stream_hover(user, *h) {
                    let hovered = *h;
                    e.update(cx, |a, cx| {
                        if hovered
                            && flags(user).stream
                            && let Some(k) = crate::ui::call_view::stream_key_in(a, user)
                        {
                            crate::ui::voice_row::fetch_preview(a, k, cx);
                        }
                        cx.notify()
                    });
                }
            }
        })
        .children(crate::ui::voice_row::stream_preview(app, user, entity.clone()))
        .child(speaking_ring_alpha(
            crate::ui::avatar_cdn(u, 24., 32),
            24.,
            speaking_fade(user, speaking || crate::voice::soundboard::is_user_playing(user)).0,
            crate::voice::soundboard::is_user_playing(user),
        ))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .child(
                    crate::fonts::styled_name(
                        div()
                            .min_w_0()
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(name_color)
                            .group_hover("voice-user-row", |s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER)))
                            .text_ellipsis(),
                        u,
                        false,
                    )
                    .child(crate::ui::emoji_text::label(crate::ui::sidebar::voice_user_name(app, user), 14.)),
                )
                .when_some(tag, |d, t| d.child(server_tag(&t, 12.))),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(z(4.))
                .when(f.video, |d| d.child(asset_icon(VIDEO, 16., state)))
                .when(local_muted, |d| d.child(svg().path(crate::icons::path("mic-deny")).size(z(16.)).flex_none().text_color(red)))
                .when(f.server_mute && !local_muted, |d| d.child(svg().path(crate::icons::path("mic-deny")).size(z(16.)).flex_none().text_color(red)))
                .when((f.self_mute || f.self_deaf) && !f.server_mute && !local_muted, |d| {
                    d.child(icon(Icon::MicOff, 16., state))
                })
                .when(f.server_deaf, |d| d.child(svg().path(crate::icons::path("headphones-deny")).size(z(16.)).flex_none().text_color(red)))
                .when(f.self_deaf && !f.server_deaf, |d| d.child(icon(Icon::HeadphonesOff, 16., state)))
                .when(f.stream, |d| {
                    d.child(
                        div()
                            .h(z(16.))
                            .px(z(8.))
                            .rounded_full()
                            .bg(t(idx::BACKGROUND_FEEDBACK_NOTIFICATION))
                            .flex()
                            .items_center()
                            .text_size(z(12.))
                            .line_height(z(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(gpui::white())
                            .child(crate::t!("voice.live")),
                    )
                }),
        )
        .child(
            div()
                .hidden()
                .group_hover("voice-user-row", |s| s.flex())
                .child(crate::ui::voice_row::options_button(user, guild, in_call)),
        )
        .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
            cx.stop_propagation();
            crate::ui::user_menu::open(user, guild, ev.position, window);
        })
        .into_any_element()
}

fn check_row(label: SharedString, on: bool, f: impl Fn(&mut PhoenixApp) + 'static, cx: &mut Context<PhoenixApp>) -> MenuItem {
    let e = cx.entity();
    MenuItem::check(label, on, move |_, _, cx: &mut App| {
        e.update(cx, |a, cx| {
            f(a);
            cx.notify();
        })
    })
}

pub fn user_menu_items(app: &PhoenixApp, user: Id, cx: &mut Context<PhoenixApp>) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if user == app.me.id {
        items.push(check_row(
            crate::t!("voice.mute"),
            app.mic_muted,
            |a| {
                a.mic_muted = !a.mic_muted;
                crate::voice::sync_self_state(a);
            },
            cx,
        ));
        items.push(check_row(
            crate::t!("voice.deafen"),
            app.deafened,
            |a| {
                a.deafened = !a.deafened;
                crate::voice::sync_self_state(a);
            },
            cx,
        ));
        let e = cx.entity();
        items.push(MenuItem::Separator);
        items.push(MenuItem::action(crate::t!("voice.voice_settings"), None, move |_, _, cx| {
            e.update(cx, |a, cx| {
                crate::ui::settings::open(a, cx);
                if let Some(s) = a.settings.as_mut() {
                    s.page = crate::ui::settings::Page::VoiceVideo;
                }
            });
        }));
        return items;
    }
    let ua = prefs().lock().user(user);
    let vol = ua.volume;
    items.push(MenuItem::Custom(
        div()
            .px(z(8.))
            .py(z(6.))
            .flex()
            .flex_col()
            .gap(z(8.))
            .child(div().text_size(z(14.)).font_weight(FontWeight::MEDIUM).text_color(t(idx::TEXT_DEFAULT)).child(crate::t!("voice.user_volume")))
            .child(crate::ui::settings::voice::slider(
                "user-volume",
                vol as f32 / 200.,
                format!("{vol}%").into(),
                None,
                Rc::new(move |_, f, done, _| {
                    let v = (f * 200.).round() as u32;
                    update(move |p| p.users.entry(user.0).or_default().volume = v, done);
                }),
                cx,
            ))
            .into_any_element(),
    ));
    items.push(MenuItem::Separator);
    items.push(check_row(
        crate::t!("voice.mute"),
        ua.muted,
        move |_| {
            update(
                |p| {
                    let u = p.users.entry(user.0).or_default();
                    u.muted = !u.muted;
                },
                true,
            );
        },
        cx,
    ));
    items.push(check_row(
        crate::t!("voice.mute_soundboard"),
        ua.soundboard_muted,
        move |_| {
            update(
                |p| {
                    let u = p.users.entry(user.0).or_default();
                    u.soundboard_muted = !u.soundboard_muted;
                },
                true,
            );
        },
        cx,
    ));
    items.push(check_row(
        crate::t!("voice.disable_video"),
        ua.video_disabled,
        move |_| {
            update(
                |p| {
                    let u = p.users.entry(user.0).or_default();
                    u.video_disabled = !u.video_disabled;
                },
                true,
            );
            crate::voice::call::refresh_video_preferences();
        },
        cx,
    ));
    if let Some(channel) = app.voice_channel
        && app.channels.get(&channel).is_some_and(|ch| ch.voice_users.contains(&user))
    {
        if app.voice.e2ee && app.voice.privacy_code.is_some() {
            let entity = cx.entity();
            items.push(MenuItem::action(crate::t!("voice.show_call_code"), None, move |_, _, cx| {
                POPOUT.with(|p| p.set(Some(Tab::Privacy)));
                entity.update(cx, |_, cx| cx.notify());
            }));
        }
        let entity = cx.entity();
        items.push(MenuItem::action(crate::t!("voice.focus_participant"), None, move |_, _, cx| {
            entity.update(cx, |app, cx| {
                if app.voice_channel == Some(channel) {
                    app.select_channel(channel, cx);
                    crate::ui::call_view::show(channel);
                    crate::ui::call_view::set_focus(crate::ui::call_view::Focus::Tile(user, false));
                }
            });
        }));
    }
    items
}

fn user_menu(app: &PhoenixApp, user: Id, pos: Point<Pixels>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let e = cx.entity();
    let dismiss: context_menu::Dismiss = Rc::new(move |_: &mut Window, cx: &mut App| {
        USER_MENU.with(|m| m.set(None));
        e.update(cx, |_, cx| cx.notify());
    });
    context_menu::render(pos, user_menu_items(app, user, cx), dismiss)
}

fn ping_graph(history: &[u32]) -> impl IntoElement {
    let n = crate::voice::PING_HISTORY;
    let max = history.iter().copied().max().unwrap_or(0).max(100) as f32;
    let off = n - history.len().min(n);
    div()
        .h(z(80.))
        .p(z(8.))
        .rounded(z(4.))
        .bg(c(Palette::BG_TERTIARY))
        .flex()
        .items_end()
        .gap(z(2.))
        .children((0..n).map(|i| {
            let p = i.checked_sub(off).and_then(|j| history.get(j)).copied();
            let col = match p {
                Some(p) if p < 150 => Palette::GREEN,
                Some(p) if p < 400 => Palette::YELLOW,
                Some(_) => Palette::RED,
                None => Palette::BG_MOD_HOVER,
            };
            let h = p.map(|p| (p as f32 / max * 64.).max(2.)).unwrap_or(2.);
            div().flex_1().h(px(h)).rounded(z(1.)).bg(c(col))
        }))
}

fn stat(label: SharedString, value: String) -> impl IntoElement {
    div()
        .flex()
        .justify_between()
        .gap(z(8.))
        .text_size(z(14.))
        .line_height(z(20.))
        .child(div().flex_none().text_color(c(Palette::TEXT_MUTED)).child(label))
        .child(div().min_w_0().text_ellipsis().font_weight(FontWeight::SEMIBOLD).text_color(c(Palette::TEXT_NORMAL)).child(value))
}

fn dash<T: ToString>(v: Option<T>) -> String {
    v.map(|v| v.to_string()).unwrap_or_else(|| "—".into())
}

fn privacy_body(v: &crate::voice::VoiceUi) -> AnyElement {
    let groups: Vec<String> = v.privacy_code.as_deref().map(|c| c.split(' ').map(str::to_string).collect()).unwrap_or_default();
    div()
        .flex()
        .flex_col()
        .gap(z(12.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(8.))
                .child(asset_icon(SHIELD, 20., c(if v.e2ee { Palette::GREEN } else { Palette::TEXT_MUTED })))
                .child(
                    div()
                        .text_size(z(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(Palette::TEXT_HEADER))
                        .child(if v.e2ee { crate::t!("voice.e2ee") } else { crate::t!("voice.not_e2ee") }),
                ),
        )
        .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(Palette::TEXT_MUTED)).child(if v.e2ee {
            crate::t!("voice.e2ee_note")
        } else {
            crate::t!("voice.transport_only")
        }))
        .when(!groups.is_empty(), |d| {
            d.child(div().text_size(z(12.)).font_weight(FontWeight::BOLD).text_color(c(Palette::TEXT_MUTED)).child(crate::t!("voice.privacy_code")))
                .child(div().flex().flex_wrap().gap(z(8.)).children(groups.into_iter().map(|g| {
                    div()
                        .px(z(8.))
                        .py(z(4.))
                        .rounded(z(4.))
                        .bg(c(Palette::BG_TERTIARY))
                        .font_family(crate::theme::FONT_MONO())
                        .text_size(z(16.))
                        .text_color(c(Palette::TEXT_NORMAL))
                        .child(g)
                })))
                .child(
                    div()
                        .text_size(z(12.))
                        .text_color(c(Palette::TEXT_MUTED))
                        .child(crate::t!("voice.code_changes")),
                )
        })
        .into_any_element()
}

fn connection_body(v: &crate::voice::VoiceUi) -> AnyElement {
    let history: Vec<u32> = v.ping_history.iter().copied().collect();
    div()
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(ping_graph(&history))
        .child(stat(crate::t!("voice.avg_ping"), dash(v.average_ping().map(|p| format!("{p} ms")))))
        .child(stat(crate::t!("voice.last_ping"), dash(v.ping_ms.map(|p| format!("{p} ms")))))
        .child(stat(crate::t!("voice.out_loss"), dash(v.stats.outbound_loss_pct.map(|l| format!("{l:.1}%")))))
        .child(stat(crate::t!("voice.in_loss"), format!("{:.1}%", v.inbound_loss_pct())))
        .child(stat(crate::t!("voice.packets"), format!("{} / {}", v.stats.packets_sent, v.stats.packets_received)))
        .child(div().h(z(1.)).my(z(4.)).bg(c(Palette::DIVIDER)))
        .child(stat(crate::t!("voice.server"), dash(v.endpoint.clone())))
        .child(stat(crate::t!("voice.encryption"), dash(v.mode.clone())))
        .into_any_element()
}

fn voice_status_has_noise_popout(app: &PhoenixApp) -> bool {
    app.voice_channel.is_some() && app.voice.status == VoiceStatus::Connected && app.settings.is_none()
}

pub(crate) fn noise_popout_open(app: &PhoenixApp) -> bool {
    voice_status_has_noise_popout(app) && NOISE_POPOUT.with(Cell::get) == app.voice_channel
}

fn close_noise_popout() -> bool {
    let was_open = NOISE_POPOUT.with(|open| open.take().is_some());
    if was_open {
        stop_quick_mic_test();
    }
    was_open
}

fn noise_popout(anchor: gpui::Bounds<Pixels>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    use crate::voice::prefs::{PROFILE_CUSTOM, PROFILE_ISOLATION};

    let (mode, profile) = {
        let p = prefs().lock();
        (NoiseMode::effective(&p), p.audio_profile)
    };
    let on = mode == NoiseMode::Rnnoise;
    let editable = profile == PROFILE_CUSTOM;
    let switch_description = match profile {
        PROFILE_ISOLATION => crate::t!("voice.noise_by_iso"),
        PROFILE_STUDIO => crate::t!("voice.noise_by_studio"),
        _ => crate::t!("voice.noise_quick_switch_note"),
    };
    let chrome = div()
        .id("voice-noise-popout")
        .w(z(276.))
        .p(z(16.))
        .rounded(z(8.))
        .bg(c(Palette::BG_FLOATING))
        .shadow_lg()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(8.))
                .child(
                    div()
                        .flex_1()
                        .text_size(z(14.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t(idx::TEXT_STRONG))
                        .when(!editable, |d| d.opacity(0.5))
                        .child(crate::t!("voice.noise")),
                )
                .child(
                    crate::ui::controls::switch("voice-noise-switch", on)
                        .role(gpui::Role::Switch)
                        .aria_label(crate::t!("voice.noise_krisp"))
                        .aria_description(switch_description.clone())
                        .aria_toggled(if on { gpui::Toggled::True } else { gpui::Toggled::False })
                        .tip(switch_description)
                        .when(!editable, |switch| switch.opacity(0.5).cursor(gpui::CursorStyle::OperationNotAllowed))
                        .when(editable, |switch| {
                            crate::ui::focus_ring::ring(switch, crate::ui::states::key("voice-noise-switch"), 16.)
                                .on_click(cx.listener(|_, _, _, cx| {
                                    update(toggle_advanced_noise_suppression, true);
                                    cx.notify();
                                }))
                        }),
                ),
        )
        .child(
            div()
                .text_size(z(14.))
                .line_height(z(18.))
                .child(crate::t!("voice.noise_quick_intro")),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(z(12.))
                .child(div().text_size(z(14.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).child(crate::t!("voice.quick_mic_title")))
                .child(quick_mic_test(cx)),
        )
        .child(
            div()
                .pt(z(4.))
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(div().text_size(z(14.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).child(crate::t!("voice.noise_quick_mode")))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .id("voice-noise-wordmark")
                                .flex()
                                .items_center()
                                .h(z(24.))
                                .aria_label(crate::t!("voice.noise_krisp"))
                                .aria_description(crate::t!("voice.noise_krisp_engine"))
                                .child(svg().path(KRISP_WORDMARK).w(z(42.)).h(z(20.)).text_color(t(idx::TEXT_STRONG)))
                                .tip(crate::t!("voice.noise_krisp_engine")),
                        )
                        .child(
                            crate::ui::focus_ring::ring(div().id("voice-noise-learn-more"), crate::ui::states::key("voice-noise-learn-more"), 4.)
                                .role(gpui::Role::Link)
                                .text_size(z(13.))
                                .line_height(z(18.))
                                .text_color(t(idx::TEXT_LINK))
                                .cursor_pointer()
                                .hover(|d| d.underline())
                                .child(crate::t!("voice.noise_quick_learn"))
                                .on_click(|_, _, cx| cx.open_url("https://krisp.ai/discord/")),
                        ),
                ),
        );
    let close = cx.listener(|_, _, _, cx| {
        close_noise_popout();
        cx.notify();
    });
    gpui::deferred(
        div()
            .id("voice-noise-backdrop")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .on_mouse_down(MouseButton::Left, close)
            .on_mouse_down(MouseButton::Right, cx.listener(|_, _, _, cx| {
                close_noise_popout();
                cx.notify();
            }))
            .child(
                gpui::anchored()
                    .attach(anchor, gpui::Anchor::TopCenter)
                    .anchor(gpui::Anchor::BottomCenter)
                    .offset(gpui::point(px(0.), z(-8.)))
                    .snap_to_window_with_margin(z(8.))
                    .child(chrome),
            ),
    )
    .with_priority(4)
    .into_any_element()
}

fn popout(app: &PhoenixApp, tab: Tab, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let tab_button = |id: &'static str, label: SharedString, t: Tab, cx: &mut Context<PhoenixApp>| {
        let sel = t == tab;
        div()
            .id(id)
            .pb(z(6.))
            .border_b_2()
            .border_color(if sel { c(Palette::BLURPLE) } else { gpui::transparent_black() })
            .cursor_pointer()
            .text_size(z(14.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(if sel { Palette::TEXT_HEADER } else { Palette::TEXT_MUTED }))
            .hover(|s| s.text_color(c(Palette::TEXT_NORMAL)))
            .child(label)
            .on_click(cx.listener(move |_, _, _, cx| {
                POPOUT.with(|p| p.set(Some(t)));
                cx.notify();
            }))
    };
    let body = match tab {
        Tab::Connection => connection_body(&app.voice),
        Tab::Privacy => privacy_body(&app.voice),
    };
    let close = cx.listener(|_, _, _, cx| {
        POPOUT.with(|p| p.set(None));
        cx.notify();
    });
    gpui::deferred(
        div()
            .id("voice-popout-backdrop")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .on_click(close)
            .child(
                div()
                    .id("voice-popout")
                    .absolute()
                    .left(z(80.))
                    .bottom(z(150.))
                    .w(z(320.))
                    .p(z(16.))
                    .rounded(z(8.))
                    .bg(c(Palette::BG_FLOATING))
                    .border_1()
                    .border_color(c(Palette::DIVIDER))
                    .shadow_lg()
                    .occlude()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .flex()
                    .flex_col()
                    .gap(z(12.))
                    .child(
                        div()
                            .flex()
                            .gap(z(16.))
                            .border_b_1()
                            .border_color(c(Palette::DIVIDER))
                            .child(tab_button("vp-tab-conn", crate::t!("voice.connection"), Tab::Connection, cx))
                            .child(tab_button("vp-tab-privacy", crate::t!("voice.privacy"), Tab::Privacy, cx)),
                    )
                    .child(body),
            ),
    )
    .with_priority(3)
    .into_any_element()
}

pub fn overlays(app: &PhoenixApp, _window: &mut Window, cx: &mut Context<PhoenixApp>) -> Vec<AnyElement> {
    let mut out = Vec::new();
    if let Some((kind, pos, devices)) = AUDIO_MENU.with(|m| m.borrow().clone()) {
        out.push(audio_menu(kind, pos, devices, app.deafened, app.voice_channel.is_some(), cx));
    }
    if let Some((pos, cams)) = CAMERA_MENU.with(|m| m.borrow().clone()) {
        out.push(camera_menu(pos, cams, cx));
    }
    if noise_popout_open(app) {
        if let Some(anchor) = NOISE_ANCHOR.with(Cell::get) {
            out.push(noise_popout(anchor, cx));
        }
    } else {
        close_noise_popout();
    }
    if app.voice_channel.is_none() {
        USER_MENU.with(|m| m.set(None));
        POPOUT.with(|p| p.set(None));
        return out;
    }
    if let Some(tab) = POPOUT.with(Cell::get) {
        out.push(popout(app, tab, cx));
    }
    if let Some((user, pos)) = USER_MENU.with(Cell::get) {
        out.push(user_menu(app, user, pos, cx));
    }
    out
}

pub fn close_popout() -> bool {
    close_noise_popout() || POPOUT.with(|p| p.take().is_some()) || USER_MENU.with(|m| m.take().is_some())
}

pub fn open_user_menu(user: Id, pos: Point<Pixels>, cx: &mut App) {
    USER_MENU.with(|m| m.set(Some((user, pos))));
    cx.refresh_windows();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_ring_follows_the_client_inset_spread() {
        assert_eq!(self_speaking_inset(f32::NEG_INFINITY, false), 2.);
        assert_eq!(self_speaking_inset(-100., false), 2.);
        assert_eq!(self_speaking_inset(-20., true), 2.);
        assert!((self_speaking_inset(-45., false) - 2.25).abs() < 1e-4);
        assert!((self_speaking_inset(0., false) - 3.28).abs() < 0.01);
        assert!((self_speaking_inset(-90., false) - 1.22).abs() < 0.01);
    }

    #[test]
    fn connection_icon_follows_the_client_quality_map() {
        assert_eq!(connection_icon(Quality::Good), ("connection-fine", idx::ICON_FEEDBACK_POSITIVE));
        assert_eq!(connection_icon(Quality::Fair), ("connection-average", idx::ICON_FEEDBACK_WARNING));
        assert_eq!(connection_icon(Quality::Bad), ("connection-bad", idx::ICON_FEEDBACK_CRITICAL));
        assert_eq!(connection_icon(Quality::Unknown), ("connection-unknown", idx::INTERACTIVE_ICON_DEFAULT));
    }

    #[test]
    fn enlarged_voice_panel_lands_on_the_client_boxes() {
        let b = panel_box(true);
        let ping = 1. + b.pad_top + b.ping_top;
        let buttons = 1. + b.pad_top + 40. + b.gap;
        let separator = buttons + 32. + b.pad_bottom;
        assert_eq!((ping, buttons, separator), (15., 59., 105.));
        let d = panel_box(false);
        assert_eq!(d.pad_top + 32. + d.gap + 32. + d.pad_bottom, 100.);
    }

    #[test]
    fn target_size_experiment_turns_on_for_both_enabled_buckets() {
        let h = phoenix_api::rest::experiments::murmur3(phoenix_api::rest::experiments::A11Y_TARGET_SIZE);
        for (bucket, on) in [(0, false), (1, true), (2, true)] {
            set_target_size_experiment(&[(h, bucket)]);
            assert_eq!(ENLARGE.with(Cell::get), Some(on));
        }
        set_target_size_experiment(&[]);
        assert_eq!(ENLARGE.with(Cell::get), Some(false));
    }

    #[test]
    fn video_connected_follows_any_camera_in_the_channel() {
        use crate::model::{Id, VoiceFlags};
        let (me, other) = (Id(1), Id(2));
        assert!(!super::channel_has_video(&[me, other]));
        super::set_flags(other, VoiceFlags { video: true, ..Default::default() });
        assert!(super::channel_has_video(&[me, other]));
        assert!(!super::channel_has_video(&[me]));
        assert!(!super::channel_has_video(&[]));
    }

    #[test]
    fn camera_pick_falls_back_to_the_first_device() {
        let cams = vec!["A".to_owned(), "B".to_owned()];
        assert_eq!(picked_camera(&cams, None), Some(0));
        assert_eq!(picked_camera(&cams, Some("B")), Some(1));
        assert_eq!(picked_camera(&cams, Some("gone")), Some(0));
        assert_eq!(picked_camera(&[], None), None);
    }

    #[test]
    fn call_mic_menu_lists_the_client_input_profiles() {
        use crate::i18n::t_in;
        assert_eq!(PROFILES.map(|k| t_in("en-US", k, &[]).to_string()), ["Voice Isolation", "Studio", "Custom"]);
        assert_eq!(t_in("fr", "voice.input_profile_menu", &[]).as_ref(), "Profil d'entrée audio");
        assert_eq!(PROFILE_STUDIO, 1);
    }

    #[test]
    fn push_to_talk_check_flips_the_input_mode() {
        use crate::voice::prefs::InputMode;
        assert_eq!(ptt_toggled(true), InputMode::VoiceActivity);
        assert_eq!(ptt_toggled(false), InputMode::PushToTalk);
    }

    #[test]
    fn noise_switch_toggles_the_advanced_filter_only_in_custom() {
        use crate::voice::prefs::{PROFILE_CUSTOM, PROFILE_ISOLATION, VoicePrefs};
        for mode in [NoiseMode::Standard, NoiseMode::Off] {
            let mut p = VoicePrefs { audio_profile: PROFILE_CUSTOM, ..Default::default() };
            mode.apply(&mut p);
            toggle_advanced_noise_suppression(&mut p);
            assert_eq!(NoiseMode::effective(&p), NoiseMode::Rnnoise);
            toggle_advanced_noise_suppression(&mut p);
            assert_eq!(NoiseMode::effective(&p), NoiseMode::Standard);
        }
        for (profile, expected) in [(PROFILE_ISOLATION, NoiseMode::Rnnoise), (PROFILE_STUDIO, NoiseMode::Off)] {
            let mut p = VoicePrefs { audio_profile: profile, ..Default::default() };
            NoiseMode::Standard.apply(&mut p);
            let before = p.clone();
            toggle_advanced_noise_suppression(&mut p);
            assert_eq!(NoiseMode::effective(&p), expected);
            assert!(p == before);
        }
    }
}
