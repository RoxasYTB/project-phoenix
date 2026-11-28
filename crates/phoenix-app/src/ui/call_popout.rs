use std::cell::RefCell;

use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext, Bounds, Context, Entity, FontWeight, Hsla, IntoElement,
    ParentElement, SharedString, Styled, Subscription, TitlebarOptions, Window, WindowBounds, WindowDecorations,
    WindowOptions, div, prelude::*, px, size, svg,
};
use gpui_component::Root;

use crate::app::PhoenixApp;
use crate::assets::Icon;
use crate::model::Id;
use crate::theme::{DEFAULT_AVATAR, c, generated::idx, m, tok, z, zoom};
use crate::ui::tooltip::Tipped as _;

thread_local! {
    static POPOUT_HANDLE: RefCell<Option<AnyWindowHandle>> = const { RefCell::new(None) };
}

fn t(i: usize) -> Hsla {
    c(tok(i))
}

pub fn is_open() -> bool {
    POPOUT_HANDLE.with(|h| h.borrow().is_some())
}

pub fn clear_handle() {
    POPOUT_HANDLE.with(|h| h.borrow_mut().take());
}

pub fn close(cx: &mut App) {
    let handle = POPOUT_HANDLE.with(|h| h.borrow_mut().take());
    if let Some(handle) = handle {
        let _ = handle.update(cx, |_, window, _| {
            window.remove_window();
        });
    }
}

pub fn open(app: Entity<PhoenixApp>, cx: &mut App) {
    if let Some(handle) = POPOUT_HANDLE.with(|h| *h.borrow()) {
        if handle.update(cx, |_, window, _| window.activate_window()).is_ok() {
            return;
        }
        clear_handle();
    }
    let title = format!("Discord Phoenix - {}", crate::t!("call.title"));
    let bounds = Bounds::centered(None, size(px(860.), px(560.)), cx);
    let opts = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(title.clone().into()),
            appears_transparent: true,
            ..Default::default()
        }),
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some(crate::APP_ID.into()),
        window_min_size: Some(size(px(420.), px(320.))),
        ..Default::default()
    };
    let app_for_close = app.downgrade();
    if let Ok(handle) = cx.open_window(opts, move |window, cx| {
        window.set_window_title(&title);
        window.on_window_should_close(cx, move |_, cx| {
            clear_handle();
            if let Some(a) = app_for_close.upgrade() {
                a.update(cx, |_, cx| cx.notify());
            }
            true
        });
        let view = cx.new(|cx| CallPopoutView::new(app, cx));
        cx.new(|cx| Root::new(view, window, cx))
    }) {
        POPOUT_HANDLE.with(|h| *h.borrow_mut() = Some(handle.into()));
    }
}

struct CallPopoutView {
    app: Entity<PhoenixApp>,
    _sub: Subscription,
}

impl CallPopoutView {
    fn new(app: Entity<PhoenixApp>, cx: &mut Context<Self>) -> Self {
        let sub = cx.observe(&app, |_, _, cx| cx.notify());
        Self { app, _sub: sub }
    }

    fn return_to_main(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        clear_handle();
        let app = self.app.clone();
        cx.defer(move |cx| {
            app.update(cx, |a, cx| a.jump_to_call(cx));
        });
        window.remove_window();
    }
}

fn tile_color(app: &PhoenixApp, user: Id) -> Hsla {
    match app.users.get(&user).map(|u| &u.avatar) {
        Some(crate::model::ImageRef::Url(url)) => crate::ui::avatar_color::get(url).map(c).unwrap_or(t(idx::BACKGROUND_BASE_LOWER)),
        Some(crate::model::ImageRef::Default(ix)) => c(DEFAULT_AVATAR[*ix as usize % DEFAULT_AVATAR.len()]),
        None => t(idx::BACKGROUND_BASE_LOWER),
    }
}

fn render_popout_tile(app: &PhoenixApp, user_id: Id, stream: bool, w: f32, h: f32) -> AnyElement {
    let u = app.users.get(&user_id);
    let me = user_id == app.me.id;
    let flags = crate::ui::voice_panel::flags(user_id);
    let speaking = !stream && app.voice.is_speaking(user_id);
    let name = crate::ui::sidebar::voice_user_name(app, user_id);
    let white = t(idx::WHITE);
    let video = if stream {
        let skey = crate::ui::call_view::stream_key(app, user_id);
        let watching = skey.as_ref().is_some_and(|k| app.voice.watching.contains(k));
        (if me || watching { crate::ui::call_view::stream_image(user_id, me) } else { None }).or_else(|| crate::ui::call_view::demo_still(user_id))
    } else if flags.video && !crate::voice::prefs::prefs().lock().user(user_id).video_disabled {
        if me {
            crate::ui::call_view::camera_image()
        } else {
            crate::ui::call_view::remote_image(user_id)
        }
    } else {
        None
    };

    div()
        .relative()
        .w(z(w))
        .h(z(h))
        .flex_none()
        .rounded(z(8.))
        .overflow_hidden()
        .bg(if video.is_some() || stream {
            t(idx::BACKGROUND_BASE_LOWER)
        } else {
            tile_color(app, user_id)
        })
        .flex()
        .items_center()
        .justify_center()
        .map(|d| match video {
            Some(img) => d.child(gpui::img(img).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Contain)),
            None if stream => d.child(svg().path("icons/voice/screen.svg").size(z(40.)).text_color(white)),
            None => d.child(crate::ui::voice_panel::speaking_avatar(u, 72. * zoom(), None, crate::ui::voice_panel::speaking_plays(speaking))),
        })
        .when(stream, |d| {
            d.child(
                div()
                    .absolute()
                    .top(z(8.))
                    .right(z(8.))
                    .px(z(8.))
                    .rounded(z(4.))
                    .bg(t(idx::STATUS_DANGER))
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(white)
                    .child(crate::t!("voice.live")),
            )
        })
        .child(
            div()
                .absolute()
                .left(z(6.))
                .bottom(z(6.))
                .px(z(8.))
                .py(z(4.))
                .rounded(z(6.))
                .bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT))
                .text_color(white)
                .text_size(z(13.))
                .line_height(z(18.))
                .child(name),
        )
        .when(speaking, |d| {
            d.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(z(8.))
                    .border(z(2.))
                    .border_color(t(idx::STATUS_POSITIVE)),
            )
        })
        .into_any_element()
}

fn ctrl_btn(id: &'static str, icon: SharedString, danger: bool, active: bool, tip: SharedString) -> gpui::Stateful<gpui::Div> {
    let bg = if danger {
        t(idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_DEFAULT)
    } else if active {
        t(idx::OPACITY_RED_12)
    } else {
        t(idx::BACKGROUND_SURFACE_HIGH)
    };
    let hover = if danger {
        t(idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_HOVER)
    } else if active {
        t(idx::OPACITY_RED_16)
    } else {
        t(idx::BACKGROUND_MOD_MUTED)
    };
    let fg = if active && !danger { t(idx::RED_400) } else { t(idx::WHITE) };
    div()
        .id(id)
        .w(z(if danger { 56. } else { 40. }))
        .h(z(40.))
        .rounded(z(10.))
        .flex()
        .items_center()
        .justify_center()
        .bg(bg)
        .hover(move |s| s.bg(hover))
        .cursor_pointer()
        .tip(tip)
        .child(svg().path(icon).size(z(20.)).text_color(fg))
}

impl gpui::Render for CallPopoutView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::ui::call_view::release_video_images(window, cx);
        let app = self.app.read(cx);
        let Some(ch_id) = app.voice_channel else {
            clear_handle();
            window.remove_window();
            return div().size_full().bg(t(idx::BLACK)).into_any_element();
        };
        let ch_name = app.channels.get(&ch_id).map(|c| crate::ui::dm_profile_panel::channel_name(app, c)).unwrap_or_else(|| crate::t!("call.title"));
        let mut tiles = Vec::new();
        if let Some(ch) = app.channels.get(&ch_id) {
            for &u in &ch.voice_users {
                if crate::ui::voice_panel::flags(u).stream {
                    tiles.push((u, true));
                }
                tiles.push((u, false));
            }
        }
        if tiles.is_empty() {
            tiles.push((app.me.id, false));
        }

        if crate::voice::call::camera_active() || crate::voice::call::remote_video_active() {
            crate::ui::call_view::schedule_video_refresh(window, cx);
        }

        let k = zoom();
        let vp = window.viewport_size();
        let w = (f32::from(vp.width) / k - 24.).max(160.);
        let h = (f32::from(vp.height) / k - m::TITLE_BAR_H - 72. - 16.).max(100.);
        let (_cols, tw) = crate::ui::call_view::grid(tiles.len(), w, h, 8.);
        let th = tw * 9. / 16.;
        let rendered_tiles: Vec<AnyElement> = tiles.iter().map(|&(u, s)| render_popout_tile(app, u, s, tw, th)).collect();
        let muted = app.mic_muted || app.deafened;
        let deafened = app.deafened;
        let app_ent = self.app.clone();
        let app_ent_deaf = self.app.clone();
        let app_ent_hang = self.app.clone();
        let hangup = crate::ui::go_live::HangupAction::current(app);
        let hangup_icon = hangup.icon();
        let white = t(idx::WHITE);

        crate::fonts::ui_root(div())
            .id("call-popout-root")
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(t(idx::BLACK))
            .text_color(white)
            .on_key_down(cx.listener(|this, ev: &gpui::KeyDownEvent, w, cx| {
                if ev.keystroke.key == "escape" {
                    if w.is_fullscreen() {
                        w.toggle_fullscreen();
                    } else {
                        clear_handle();
                        let app = this.app.clone();
                        cx.defer(move |cx| {
                            app.update(cx, |_, cx| cx.notify());
                        });
                        w.remove_window();
                    }
                }
            }))
            .child(
                crate::ui::window_chrome::drag_region(div().id("popout-title-bar"))
                    .h(z(m::TITLE_BAR_H))
                    .flex_none()
                    .px(z(12.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .bg(t(idx::BACKGROUND_BASE_LOWEST))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(z(8.))
                            .min_w_0()
                            .child(svg().path(crate::icons::path("speaker")).size(z(16.)).text_color(t(idx::CHANNEL_ICON)))
                            .child(
                                div()
                                    .text_size(z(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(t(idx::TEXT_STRONG))
                                    .text_ellipsis()
                                    .child(ch_name),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(z(4.))
                            .occlude()
                            .child(
                                div()
                                    .id("popout-return-main")
                                    .px(z(8.))
                                    .h(z(24.))
                                    .rounded(z(4.))
                                    .flex()
                                    .items_center()
                                    .gap(z(4.))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_HOVER)))
                                    .text_size(z(12.))
                                    .text_color(t(idx::INTERACTIVE_TEXT_DEFAULT))
                                    .child(crate::t!("call.return_main"))
                                    .on_click(cx.listener(|this, _, w, cx| this.return_to_main(w, cx))),
                            )
                            .child(
                                div()
                                    .id("popout-close")
                                    .size(z(28.))
                                    .rounded(z(4.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(t(idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_DEFAULT)))
                                    .child(svg().path(crate::icons::path("window/close")).size(z(12.)).text_color(white))
                                    .on_click(cx.listener(|this, _, w, cx| this.return_to_main(w, cx))),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .p(z(12.))
                    .flex()
                    .flex_wrap()
                    .content_center()
                    .justify_center()
                    .gap(z(8.))
                    .children(rendered_tiles),
            )
            .child(
                div()
                    .h(z(64.))
                    .flex_none()
                    .px(z(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(z(12.))
                    .child(
                        ctrl_btn(
                            "popout-mic",
                            (if muted { Icon::MicOff } else { Icon::Mic }).path(),
                            false,
                            muted,
                            if muted { crate::t!("voice.unmute") } else { crate::t!("voice.mute") },
                        )
                        .on_click(cx.listener(move |_, _, _, cx| {
                            app_ent.update(cx, |a, cx| {
                                a.mic_muted = !a.mic_muted;
                                crate::voice::sync_self_state(a);
                                cx.notify();
                            });
                        })),
                    )
                    .child(
                        ctrl_btn(
                            "popout-deafen",
                            (if deafened { Icon::HeadphonesOff } else { Icon::Headphones }).path(),
                            false,
                            deafened,
                            if deafened { crate::t!("voice.undeafen") } else { crate::t!("voice.deafen") },
                        )
                        .on_click(cx.listener(move |_, _, _, cx| {
                            app_ent_deaf.update(cx, |a, cx| {
                                a.deafened = !a.deafened;
                                crate::voice::sync_self_state(a);
                                cx.notify();
                            });
                        })),
                    )
                    .child(
                        ctrl_btn("popout-hangup", hangup_icon, true, false, hangup.label())
                            .on_click(cx.listener(move |_, _, w, cx| {
                                if hangup.disconnects() {
                                    clear_handle();
                                }
                                app_ent_hang.update(cx, |a, cx| hangup.run(a, cx));
                                if hangup.disconnects() {
                                    w.remove_window();
                                }
                            })),
                    )
                    .child(
                        div()
                            .id("popout-fullscreen")
                            .size(z(40.))
                            .rounded(z(10.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(t(idx::BACKGROUND_SURFACE_HIGH))
                            .hover(|s| s.bg(t(idx::BACKGROUND_MOD_MUTED)))
                            .cursor_pointer()
                            .tip(if window.is_fullscreen() { crate::t!("call.exit_fullscreen") } else { crate::t!("call.fullscreen") })
                            .child(svg().path(if window.is_fullscreen() { "icons/call/fullscreen-exit.svg" } else { "icons/call/fullscreen.svg" }).size(z(20.)).text_color(white))
                            .on_click(cx.listener(|_, _, w, _| w.toggle_fullscreen())),
                    ),
            )
            .children(crate::ui::window_chrome::resize_handles(window))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{clear_handle, is_open};

    #[test]
    fn popout_starts_closed() {
        clear_handle();
        assert!(!is_open());
    }
}
