use crate::ui::tooltip::Tipped as _;
use gpui::{Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use crate::{
    app::PhoenixApp,
    assets::Icon,
    theme::{c, generated::idx, m, tok, z, zoom},
    ui::{home::data::HomeView, icon},
};

fn title(app: &PhoenixApp) -> Option<(Icon, SharedString)> {
    if crate::ui::discovery::current().is_some() {
        return Some((Icon::Compass, crate::t!("discovery.title")));
    }
    if app.favorites_view {
        return Some((Icon::Hash, crate::t!("fav.title")));
    }
    if let Some(g) = app.selected_guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)) {
        return Some((Icon::Hash, g.name.clone()));
    }
    Some(match app.nav.home {
        HomeView::Dm => (Icon::Clyde, crate::t!("dm.title")),
        HomeView::Nitro => (Icon::Nitro, crate::t!("settings.page.nitro")),
        HomeView::Shop => (Icon::Shop, crate::t!("dm.shop")),
        HomeView::Quests => (Icon::Quests, crate::t!("dm.quests")),
        HomeView::Friends => (Icon::Friends, crate::t!("friends.title")),
        HomeView::MessageRequests => return None,
    })
}

fn bar_button(id: &'static str, i: Icon, active: bool, tip: SharedString) -> gpui::Stateful<gpui::Div> {
    bar_button_tinted(id, i, tok(if active { idx::INTERACTIVE_ICON_ACTIVE } else { idx::ICON_MUTED }), tip)
}

fn bar_button_tinted(id: &'static str, i: Icon, color: u32, tip: SharedString) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .role(gpui::Role::Button)
        .aria_label(tip.clone())
        .size(z(24.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(m::TITLE_BAR_ITEM_RADIUS))
        .occlude()
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
        .child(icon(i, 18. * zoom(), c(color)))
        .tip(tip)
}

fn nav_arrow(id: &'static str, name: &'static str, back: bool, enabled: bool) -> gpui::Stateful<gpui::Div> {
    let d = div()
        .id(id)
        .role(gpui::Role::Button)
        .aria_label(if back { crate::t!("a11y.back") } else { crate::t!("a11y.forward") })
        .w(z(26.))
        .h(z(24.))
        .flex_none()
        .occlude()
        .flex()
        .items_center()
        .when(back, |d| d.pl(z(6.)).pr(z(2.)).rounded_l(z(6.)))
        .when(!back, |d| d.pl(z(2.)).pr(z(6.)).rounded_r(z(6.)))
        .child(
            gpui::svg()
                .path(crate::icons::path(name))
                .size(z(18.))
                .flex_none()
                .text_color(c(tok(idx::INTERACTIVE_ICON_DEFAULT))),
        );
    if enabled {
        d.cursor_pointer().hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
    } else {
        d.opacity(0.5)
    }
}

pub static FORCE_CLIENT_DECORATIONS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn client_decorated(window: &gpui::Window) -> bool {
    cfg!(windows) || FORCE_CLIENT_DECORATIONS.load(std::sync::atomic::Ordering::Relaxed)
        || matches!(window.window_decorations(), gpui::Decorations::Client { .. })
}

static DREW_WIN_BUTTONS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn inbox_right_inset() -> f32 {
    let tail = if DREW_WIN_BUTTONS.load(std::sync::atomic::Ordering::Relaxed) { 117. + 12. } else { 12. };
    tail + 24. + 12.
}

static VIEWPORT_H_CSS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static VIEWPORT_W_CSS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub const DM_PROFILE_MIN_W: f32 = 1132.;
pub const ACTIVE_NOW_MAX_HIDDEN_W: f32 = 1200.;
pub const FRIENDS_INVITE_MAX_HIDDEN_W: f32 = 940.;

pub fn viewport_w_css() -> f32 {
    f32::from_bits(VIEWPORT_W_CSS.load(std::sync::atomic::Ordering::Relaxed))
}

pub fn viewport_h_css() -> f32 {
    f32::from_bits(VIEWPORT_H_CSS.load(std::sync::atomic::Ordering::Relaxed))
}

pub fn dm_profile_fits(vw: f32) -> bool {
    vw <= 0. || vw >= DM_PROFILE_MIN_W
}

pub fn active_now_shown(vw: f32) -> bool {
    vw <= 0. || vw > ACTIVE_NOW_MAX_HIDDEN_W
}

pub fn friends_invite_shown(vw: f32) -> bool {
    vw <= 0. || vw > FRIENDS_INVITE_MAX_HIDDEN_W
}

pub fn active_now_w(body_w: f32) -> f32 {
    (body_w * 0.3).clamp(360., 420.)
}

pub fn title_max_w(vw: f32) -> f32 {
    if vw <= 0. {
        return 480.;
    }
    vw * 0.5
}

pub fn render(app: &PhoenixApp, window: &gpui::Window, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let vh = f32::from(window.viewport_size().height) / zoom();
    VIEWPORT_H_CSS.store(vh.to_bits(), std::sync::atomic::Ordering::Relaxed);
    let vw = f32::from(window.viewport_size().width) / zoom();
    VIEWPORT_W_CSS.store(vw.to_bits(), std::sync::atomic::Ordering::Relaxed);
    DREW_WIN_BUTTONS.store(client_decorated(window), std::sync::atomic::Ordering::Relaxed);
    let heading = title(app);
    let favorites = app.favorites_view && crate::ui::discovery::current().is_none();
    let guild = app.selected_guild.filter(|_| crate::ui::discovery::current().is_none() && !favorites).and_then(|g| app.guilds.iter().find(|x| x.id == g));
    crate::ui::window_chrome::drag_region(div().id("app-title-bar"))
        .h(z(m::TITLE_BAR_H))
        .flex_none()
        .w_full()
        .relative()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .when_some(heading, |d, (glyph, text)| d.child(
                    div()
                        .max_w(z(title_max_w(vw)))
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .map(|d| match guild {
                            _ if favorites => d.child(
                                gpui::svg()
                                    .path(crate::icons::path("star"))
                                    .size(z(18.))
                                    .flex_none()
                                    .text_color(c(tok(idx::INTERACTIVE_ICON_DEFAULT))),
                            ),
                            Some(g) => d.child(
                                div()
                                    .flex_none()
                                    .rounded(z(4.))
                                    .overflow_hidden()
                                    .child(crate::ui::invite::guild_icon(Some(&g.icon), &g.name, 16. * zoom())),
                            ),
                            None => d.child(div().flex_none().child(icon(glyph, 18. * zoom(), c(tok(idx::INTERACTIVE_ICON_DEFAULT))))),
                        })
                        .child(div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(text)),
                )),
        )
        .when(cfg!(windows), |d| d.child(div().absolute().top_0().bottom_0().left(z(80.)).right(z(260.)).window_control_area(gpui::WindowControlArea::Drag)))
        .child(
            div()
                .relative()
                .w(z(80.))
                .h(z(24.))
                .flex()
                .justify_center()
                .gap(z(2.))
                .child(nav_arrow("title-back", "title/back", true, true))
                .child(nav_arrow("title-forward", "title/forward", false, false)),
        )
        .child(
            div()
                .relative()
                .when(!client_decorated(window), |d| d.pr(z(12.)))
                .flex()
                .items_center()
                .gap(z(12.))
                .when(crate::updater::available_release().is_some(), |d| {
                    d.child(
                        bar_button_tinted("title-update", Icon::Download, tok(idx::ICON_FEEDBACK_WARNING), crate::t!("update.titlebar_tip"))
                            .on_click(cx.listener(|_, _, w, cx| {
                                crate::actions::open_modal(crate::ui::modals::Modal::Update, Some(w));
                                cx.notify();
                            })),
                    )
                })
                .child(
                    bar_button("title-inbox", Icon::Inbox, app.parity.inbox.open, crate::t!("inbox.title"))
                        .on_click(cx.listener(|a, _, _, cx| a.toggle_inbox(cx))),
                )
                .child(bar_button("title-help", Icon::Help, false, crate::t!("title.support")).on_click(cx.listener(|a, _, w, cx| a.open_support(w, cx))))
                .when(client_decorated(window), |d| d.child(win_buttons(cx))),
        )
}

pub fn login_bar(window: &gpui::Window, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    crate::ui::window_chrome::drag_region(div().id("login-title-bar"))
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(z(m::TITLE_BAR_H))
        .flex()
        .items_center()
        .justify_end()
        .when(cfg!(windows), |d| d.child(div().absolute().inset_0().right(z(120.)).window_control_area(gpui::WindowControlArea::Drag)))
        .when(client_decorated(window), |d| d.child(win_buttons(cx)))
}

fn win_buttons(cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let btn = |id: &'static str, area: gpui::WindowControlArea| {
        let close = area == gpui::WindowControlArea::Close;
        let label = match area {
            gpui::WindowControlArea::Min => crate::t!("a11y.minimize"),
            gpui::WindowControlArea::Max => crate::t!("a11y.maximize"),
            _ => crate::t!("a11y.close"),
        };
        div()
            .id(id)
            .role(gpui::Role::Button)
            .aria_label(label)
            .group(id)
            .when(cfg!(windows), |d| d.window_control_area(area))
            .size(z(m::TITLE_BAR_H))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .occlude()
            .cursor_pointer()
            .map(|d| {
                if close {
                    d.hover(|s| s.bg(c(tok(idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_DEFAULT))))
                } else {
                    d.hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                        .active(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_ACTIVE))))
                }
            })
    };
    let fg = c(tok(idx::INTERACTIVE_TEXT_DEFAULT));
    let fg_hover = c(tok(idx::INTERACTIVE_TEXT_HOVER));
    let fg_active = c(tok(idx::INTERACTIVE_TEXT_ACTIVE));
    let glyph_box = || div().relative().size(z(12.)).flex_none();
    div()
        .flex()
        .items_center()
        .justify_end()
        .gap(z(4.))
        .child(div().mx(z(4.)).w(z(1.)).h(z(m::TITLE_BAR_H - 12.)).bg(c(tok(idx::BORDER_SUBTLE))))
        .child(
            btn("win-min", gpui::WindowControlArea::Min)
                .child(glyph_box().child(
                    div()
                        .id("win-min-g")
                        .absolute()
                        .left(z(1.))
                        .top(z(6.))
                        .w(z(10.))
                        .h(z(1.))
                        .bg(fg)
                        .group_hover("win-min", |s| s.bg(fg_hover))
                        .group_active("win-min", |s| s.bg(fg_active)),
                ))
                .when(!cfg!(windows), |d| d.on_click(cx.listener(|_, _, w, _| w.minimize_window()))),
        )
        .child(
            btn("win-max", gpui::WindowControlArea::Max)
                .child(glyph_box().child(
                    div()
                        .id("win-max-g")
                        .absolute()
                        .left(z(1.))
                        .top(z(1.))
                        .size(z(10.))
                        .border_1()
                        .border_color(fg)
                        .group_hover("win-max", |s| s.border_color(fg_hover))
                        .group_active("win-max", |s| s.border_color(fg_active)),
                ))
                .when(!cfg!(windows), |d| d.on_click(cx.listener(|_, _, w, _| w.zoom_window()))),
        )
        .child(
            btn("win-close", gpui::WindowControlArea::Close)
                .child(
                    gpui::svg()
                        .path(crate::icons::path("window/close"))
                        .size(z(12.))
                        .flex_none()
                        .text_color(fg)
                        .group_hover("win-close", |s| s.text_color(c(tok(idx::CONTROL_CRITICAL_PRIMARY_ICON_DEFAULT)))),
                )
                .when(!cfg!(windows), |d| {
                    d.on_click(cx.listener(|a, _, w, _| {
                        a.persist_chat_position();
                        if a.prefs.local.minimize_to_tray && crate::tray::running() {
                            w.minimize_window();
                        } else {
                            w.remove_window();
                        }
                    }))
                }),
        )
}

#[cfg(test)]
mod tests {
    use super::{active_now_shown, active_now_w, dm_profile_fits, friends_invite_shown, title_max_w};

    #[test]
    fn dm_profile_panel_needs_1132_css_px() {
        assert!(dm_profile_fits(0.));
        assert!(!dm_profile_fits(1131.));
        assert!(dm_profile_fits(1132.));
        assert!(dm_profile_fits(1700. / 1.5));
        assert!(!dm_profile_fits(1600. / 1.5));
        assert!(dm_profile_fits(1920. / 1.5));
        assert!(!dm_profile_fits(1280. / 1.25));
        assert!(dm_profile_fits(1920. / 1.25));
    }

    #[test]
    fn friends_columns_follow_media_queries() {
        assert!(!active_now_shown(1200.));
        assert!(active_now_shown(1201.));
        assert!(!friends_invite_shown(940.));
        assert!(friends_invite_shown(941.));
        assert!(!active_now_shown(1700. / 1.5));
        assert!(active_now_shown(1920. / 1.5));
        assert!(!friends_invite_shown(1700. / 2.));
        assert!(friends_invite_shown(1920. / 2.));
    }

    #[test]
    fn active_now_width_is_thirty_percent_clamped() {
        assert_eq!(active_now_w(864.), 360.);
        assert!((active_now_w(1300.) - 390.).abs() < 0.01);
        assert_eq!(active_now_w(1504.), 420.);
    }

    #[test]
    fn title_is_capped_at_half_the_viewport() {
        assert_eq!(title_max_w(0.), 480.);
        assert_eq!(title_max_w(1280.), 640.);
        assert_eq!(title_max_w(640.), 320.);
    }
}
