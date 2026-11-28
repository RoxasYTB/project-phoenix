use crate::ui::tooltip::Tipped as _;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement,
    SharedString, StyledText, Styled, InteractiveText, Window, div, img, list, prelude::*,
};

use crate::{
    app::PhoenixApp,
    assets::Icon,
    media,
    model::{Attachment, AttachmentKind, ChannelKind, Embed, Id, Message},
    richtext::{ClickTarget, RBlock, RenderedMessage, RichText},
    theme::{FONT_MONO, Palette, c, fz, generated::idx, m, space, tok, z},
    ui::{avatar, icon, members, sidebar::channel_icon},
};

pub mod announce;
pub mod app_launcher;
pub mod attach;
pub mod automod;
pub mod plaintext;
pub mod composer;
pub mod icons;
pub mod invite_cards;
pub mod invite_embed;
pub mod mention;
pub mod forward;
pub mod message_ext;
pub mod mosaic;
pub mod panels;
pub mod page_scroll;
pub mod pending;
pub mod rich_input;
pub mod rows;
pub mod scheduled;
pub mod selection;
pub mod scrollbar;
pub mod alt_text;
pub mod state;
pub mod thread_banner;
pub mod video_player;
pub mod voice_message;
pub(crate) mod welcome;
pub mod blocked;
pub mod burst_tip;
pub mod command_msg;
pub mod slash;
pub mod spoiler;
pub mod system;
pub mod embed_rules;
pub mod skeleton;
pub use state::ChatState;

pub(crate) fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>, in_voice_panel: bool) -> impl IntoElement {
    let entity = cx.entity();
    skeleton::set_window_height(f32::from(window.viewport_size().height));
    if let Some(channel) = app.selected_channel.and_then(|id| app.channels.get(&id))
        && channel.guild.is_none()
        && !crate::ui::dm_profile_panel::is_group_dm(app, channel.id)
        && let Some(&peer) = channel.voice_users.first()
    {
        crate::ui::dm_profile_panel::ensure_profile(app, peer, cx);
    }
    let show_members = app.show_members && app.selected_guild.is_some();
    let show_members = show_members && app.chat.thread.is_none() && app.active_thread_form().is_none();
    let show_members = show_members && !in_voice_panel && !crate::ui::call_view::active(app);
    let search_results = crate::ui::search::results_shown(cx);
    app.backend.set_members_shown(app.show_members && app.chat.thread.is_none() && !search_results);
    let mod_view = crate::ui::mod_view::render(app, window, cx);
    let search_width = crate::ui::search::results_width();
    let (thread_width, thread_floating) = thread_sidebar_layout(in_voice_panel, crate::ui::search::chat_width(window));
    let thread_sidebar = if search_results {
        None
    } else {
        panels::thread_panel(app, thread_width, cx).or_else(|| crate::ui::thread_create::panel(app, thread_width, cx))
    };
    let (thread_sidebar, floating_thread) = match thread_sidebar {
        Some(tp) if thread_floating && mod_view.is_none() => (None, Some(tp)),
        other => (other, None),
    };
    let has_split = mod_view.is_some() || thread_sidebar.is_some();
    let a11y_channel = || app.selected_channel.and_then(|c| app.channels.get(&c)).map(|c| c.name.clone()).unwrap_or_default();
    let main = crate::ui::a11y::named(div().id("chat-main"), gpui::Role::Main, || crate::t!("a11y.channel_main", name = a11y_channel()))
        .flex_1()
        .min_w_0()
        .h_full()
        .relative()
        .flex()
        .flex_col()
        .when(has_split, |d| d.border_r_1().border_color(c(tok(idx::BORDER_SUBTLE))).rounded_tr(z(12.)).overflow_hidden())
        .bg(crate::theme::client_theme::surface(crate::theme::client_theme::Level::Chat, Some(crate::theme::client_theme::Level::AppFrame), Palette::BG_PRIMARY))
        .map(|d| {
            if crate::ui::call_view::private_split(app) {
                d.children(crate::ui::call_view::render_private(app, header(app, cx, false), window, cx))
            } else {
                d.child(header(app, cx, in_voice_panel)).children(crate::ui::dm_call::render(app, cx))
            }
        })
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_row()
                .child(
                    panels::track_drop(div().id("chat-column"))
                        .group("chat-drop")
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .on_drop(cx.listener(|a, paths: &gpui::ExternalPaths, _, cx| {
                            panels::drop_ended();
                            a.add_paths(paths.paths().to_vec(), cx)
                        }))
                        .children(thread_banner::render(app, cx))
                        .children(crate::ui::join_flow::verification_banner(app, cx))
                        .children(discovery_notice(app, cx))
                        .children(crate::ui::join_flow::nsfw_gate(app, cx))
                        .child(
                            crate::ui::a11y::named(div().id("chat-messages"), gpui::Role::List, || crate::t!("a11y.messages_in", name = a11y_channel()))
                                .flex_1()
                                .min_h_0()
                                .relative()
                                .pr(z(16.))
                                .on_mouse_down(gpui::MouseButton::Left, selection::on_down)
                                .child(rows::keep_anchor(entity.clone(), row))
                                .child(rows::measure_ahead(entity.clone(), row))
                                .child(
                                    list(app.message_list.clone(), move |ix, _w, cx| row(&entity, ix, cx))
                                        .size_full(),
                                )
                                .child(rows::load_when_visible(cx.entity()))
                                .child(selection::listener(app.message_list.clone()))
                                .children(scrollbar::auto_scrollbar(&app.message_list))
                                .children(panels::new_messages_bar(app, cx))
                                .children(panels::jump_bar(app, cx)),
                        )
                        .child(composer::render(app, window, cx))
                        .child(panels::drop_overlay(app))
                        .children(crate::ui::forum::render(app, cx)),
                )
                .when(search_results && !has_split, |d| d.child(div().flex_none().w(search_width)))
                .when(show_members && !search_results, |d| d.child(members::render(app, cx)))
                .when(!search_results, |d| {
                    d.children(crate::ui::dm_profile_panel::render(app, cx)).children(crate::ui::group_dm::members_panel(app, cx))
                }),
        )
        .children(panels::pins_popout(app, cx));
    if has_split {
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_row()
            .child(main)
            .when_some(thread_sidebar, |d, tp| d.child(div().w(z(8.)).flex_none().h_full()).child(tp))
            .children(mod_view)
            .when(search_results, |d| d.child(div().flex_none().w(search_width)))
            .into_any_element()
    } else if let Some(tp) = floating_thread {
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .relative()
            .child(main)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .flex()
                    .shadow(vec![gpui::BoxShadow {
                        color: gpui::hsla(0., 0., 0., 0.52),
                        offset: gpui::point(gpui::px(0.), z(8.)),
                        blur_radius: z(40.),
                        spread_radius: gpui::px(0.),
                        inset: false,
                    }])
                    .child(tp),
            )
            .into_any_element()
    } else {
        main.into_any_element()
    }
}

const THREAD_SIDEBAR_MIN_W: f32 = 450.;
const THREAD_SIDEBAR_RESERVED_W: f32 = 315.;

fn thread_sidebar_layout(in_voice_panel: bool, chat_width: gpui::Pixels) -> (gpui::Pixels, bool) {
    if in_voice_panel {
        return (z(480.), false);
    }
    let max = chat_width - z(THREAD_SIDEBAR_RESERVED_W);
    (z(THREAD_SIDEBAR_MIN_W), max <= z(THREAD_SIDEBAR_MIN_W))
}

fn guild_discoverable(app: &PhoenixApp) -> bool {
    app.selected_guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)).is_some_and(|g| g.discoverable)
}

static NOTICE_CLOSED: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());

static LABEL_CX: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn discovery_label_probe() -> impl IntoElement {
    gpui::canvas(
        |b, _, _| {
            let cx = f32::from(b.origin.x) + f32::from(b.size.width) / 2.;
            LABEL_CX.store(cx.to_bits(), std::sync::atomic::Ordering::Relaxed);
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn discovery_arrow() -> impl IntoElement {
    gpui::canvas(
        |_, _, _| {},
        |b, _, window, _| {
            let lcx = f32::from_bits(LABEL_CX.load(std::sync::atomic::Ordering::Relaxed));
            if lcx <= 0. {
                return;
            }
            let k = crate::theme::zoom();
            let (x0, y0) = (lcx - 8. * k, f32::from(b.origin.y) - 9. * k);
            let p = |x: f32, y: f32| gpui::point(gpui::px(x0 + x * k), gpui::px(y0 + y * k));
            let shape = |mut pb: gpui::PathBuilder, close: bool| {
                pb.move_to(p(0., 10.));
                pb.line_to(p(6., 2.9));
                pb.curve_to(p(10., 2.9), p(8., 0.7));
                pb.line_to(p(16., 10.));
                if close {
                    pb.close();
                }
                pb.build().ok()
            };
            for col in [tk(idx::BACKGROUND_BASE_LOWER), tk(idx::BACKGROUND_FEEDBACK_MOD_INFO)] {
                if let Some(path) = shape(gpui::PathBuilder::fill(), true) {
                    window.paint_path(path, col);
                }
            }
            if let Some(path) = shape(gpui::PathBuilder::stroke(gpui::px(k)), false) {
                window.paint_path(path, tk(idx::INLINENOTICE_BORDER_INFO));
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn discovery_notice(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<gpui::AnyElement> {
    let gid = app.selected_guild?;
    let ch = app.selected_channel.and_then(|c| app.channels.get(&c))?;
    let locked = app.nav.extras.get(&ch.id).is_some_and(|e| e.locked);
    let thread = app.nav.thread_parent(ch.id).is_some();
    if ch.guild.is_none() || locked || thread || !guild_discoverable(app) || NOTICE_CLOSED.lock().ok()?.contains(&gid.0) {
        return None;
    }
    Some(
        div()
            .flex_none()
            .mt(z(-1.))
            .pt(z(4.))
            .pb(z(4.))
            .pl(z(16.))
            .pr(z(8.))
            .flex()
            .items_center()
            .gap(z(8.))
            .bg(tk(idx::BACKGROUND_FEEDBACK_MOD_INFO))
            .border_t_1()
            .border_b_1()
            .border_color(tk(idx::INLINENOTICE_BORDER_INFO))
            .relative()
            .child(discovery_arrow())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .child(discovery_notice_text()),
            )
            .child(
                div()
                    .id("discovery-notice-close")
                    .group("discovery-notice-close")
                    .flex_none()
                    .size(z(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(move |_, _, _, cx| {
                        if let Ok(mut v) = NOTICE_CLOSED.lock() {
                            v.push(gid.0);
                        }
                        cx.notify();
                    }))
                    .child(
                        gpui::svg()
                            .path(crate::icons::path("close"))
                            .size(z(18.))
                            .text_color(tk(idx::ICON_MUTED))
                            .group_hover("discovery-notice-close", |s| s.text_color(tk(idx::ICON_DEFAULT))),
                    ),
            )
            .into_any_element(),
    )
}

fn discovery_notice_text() -> StyledText {
    let lead = crate::t!("chat.discovery_notice");
    let more = crate::t!("chat.discovery_more");
    let start = lead.len();
    let text = format!("{lead}{more}");
    let underline = gpui::HighlightStyle {
        underline: Some(gpui::UnderlineStyle { thickness: gpui::px(1.), ..Default::default() }),
        ..Default::default()
    };
    let end = text.len();
    StyledText::new(text).with_highlights([(start..end, underline)])
}

const HDR_ICON: f32 = 20.;

pub(crate) fn msg_left() -> f32 {
    if crate::theme::font_scaled_down() { 72. * crate::theme::font_scale() } else { m::AVATAR + 2. * space::md() }
}

pub(crate) fn msg_avatar_size() -> f32 {
    msg_avatar()
}

fn msg_avatar() -> f32 {
    if crate::theme::font_scaled_down() { 40. * crate::theme::font_scale() } else { m::AVATAR }
}

fn hdr_button(id: &'static str, i: Icon, color: gpui::Hsla, size: f32) -> gpui::Stateful<gpui::Div> {
    let _ = size;
    hdr(id, i.path(), color)
}

fn pin_button(app: &PhoenixApp, color: gpui::Hsla) -> gpui::Stateful<gpui::Div> {
    let b = hdr_button("pin", Icon::Pin, color, HDR_ICON).child(
        gpui::canvas(|b, window, _| crate::ui::inbox::note_pin_button(b, window.viewport_size()), |_, _, _, _| {})
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
    );
    let unread = app.selected_channel.is_some_and(|c| app.backend.has_unread_pins(c)) && app.chat.pins.is_none();
    b.when(unread, |b| {
        b.child(
            div()
                .absolute()
                .left(z(16.))
                .top(z(16.))
                .size(z(10.))
                .rounded_full()
                .bg(c(Palette::BG_PRIMARY))
                .flex()
                .items_center()
                .justify_center()
                .child(div().size(z(8.)).rounded(z(8.)).bg(tk(idx::BACKGROUND_BRAND))),
        )
    })
}

fn hdr_named(id: &'static str, name: &str, color: gpui::Hsla) -> gpui::Stateful<gpui::Div> {
    hdr(id, crate::icons::path(name), color)
}

fn hdr(id: &'static str, path: SharedString, color: gpui::Hsla) -> gpui::Stateful<gpui::Div> {
    let selected = color == tk(idx::ICON_STRONG);
    let label = match id {
        "threads" => crate::t!("threads.title"),
        "bell" => crate::t!("menu.notify_settings"),
        "pin" => crate::t!("pins.title"),
        "members" if selected => crate::t!("header.members_hide"),
        "members" => crate::t!("header.members_show"),
        "dm-call" => crate::t!("header.voice_call"),
        "dm-video" => crate::t!("header.video_call"),
        "dm-add" => crate::t!("header.add_to_dm"),
        "gdm-add" => crate::t!("header.add_to_gdm"),
        "gdm-members" if selected => crate::t!("header.members_hide"),
        "gdm-members" => crate::t!("header.members_show"),
        "dm-profile" if selected => crate::t!("header.profile_hide"),
        "dm-profile" => crate::t!("header.profile_show"),
        _ => crate::t!("header.more"),
    };
    crate::ui::states::header_button(id, path, selected, label).debug_selector(move || format!("hdr-{id}"))
}

thread_local! {
    static HEADER_W: std::cell::Cell<f32> = const { std::cell::Cell::new(0.) };
}

pub(crate) fn width_probe(cell: &'static std::thread::LocalKey<std::cell::Cell<f32>>) -> impl IntoElement {
    gpui::canvas(
        move |b, w, _| {
            let nw = f32::from(b.size.width);
            if (cell.with(|c| c.replace(nw)) - nw).abs() > 0.5 {
                w.on_next_frame(|w, _| w.refresh());
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

fn header_search_w(pad_start: f32) -> f32 {
    let w = HEADER_W.with(std::cell::Cell::get);
    if w <= 0. { 244. } else { (w - pad_start - 8. - 320.).clamp(64., 244.) }
}

fn call_chat_header(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let ch = app.selected_channel.and_then(|c| app.channels.get(&c));
    let name = ch.map(|c| crate::ui::dm_profile_panel::channel_name(app, c)).unwrap_or_default();
    let ic = tk(idx::ICON_MUTED);
    div()
        .h(z(m::HEADER_H + 1.))
        .flex_none()
        .pl(z(space::md()))
        .pr(z(8.))
        .py(z(8.))
        .flex()
        .items_center()
        .gap(z(8.))
        .bg(crate::theme::client_theme::surface(crate::theme::client_theme::Level::Chat, Some(crate::theme::client_theme::Level::Chat), Palette::BG_PRIMARY))
        .border_t_1()
        .border_b_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .child(gpui::svg().path(crate::icons::path("chat")).size(z(24.)).flex_none().text_color(ic))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk(idx::TEXT_STRONG))
                .child(crate::ui::emoji_text::label(name, 16.)),
        )
        .child(
            hdr_named("close-voice-chat", "close", ic)
                .flex_none()
                .tip(crate::t!("call.hide_chat"))
                .on_click(cx.listener(|a, _, _, cx| {
                    if let Some(ch) = a.selected_channel {
                        crate::ui::call_view::set_chat_open(ch, false);
                    }
                    cx.notify();
                })),
        )
}

fn header(app: &PhoenixApp, cx: &mut Context<PhoenixApp>, in_voice_panel: bool) -> gpui::Div {
    if in_voice_panel {
        return call_chat_header(app, cx);
    }
    let ch = app.selected_channel.and_then(|c| app.channels.get(&c));
    let is_dm = ch.is_some_and(|c| c.guild.is_none());
    let name = ch.map(|c| crate::ui::dm_profile_panel::channel_name(app, c)).unwrap_or_default();
    let topic = ch.and_then(|c| c.topic.clone());
    let is_forum = ch.is_some_and(|c| c.kind == ChannelKind::Forum);
    let topic = topic.filter(|_| !is_forum);
    let thread_parent = ch
        .and_then(|c| app.nav.thread_parent(c.id).or_else(|| app.backend.thread_info(c.id).and_then(|i| i.parent)))
        .and_then(|p| app.channels.get(&p));
    let thread_info = thread_parent.and(ch).and_then(|c| app.backend.thread_info(c.id));
    let parent_is_forum = thread_parent.is_some_and(|p| p.kind == ChannelKind::Forum);
    let kind = thread_parent.or(ch).map(|c| c.kind).unwrap_or(ChannelKind::Text);
    let locked = ch.is_some_and(|c| app.nav.extras.get(&c.id).is_some_and(|e| e.locked));
    let discoverable = !is_dm && !locked && thread_parent.is_none() && guild_discoverable(app);
    let ic = tk(idx::ICON_MUTED);
    let on = tk(idx::ICON_STRONG);
    let is_group_dm = ch.is_some_and(|c| crate::ui::dm_profile_panel::is_group_dm(app, c.id));
    let peer = ch.filter(|c| !crate::ui::dm_profile_panel::is_group_dm(app, c.id)).and_then(|c| c.voice_users.first()).and_then(|u| app.users.get(u));
    let scope: SharedString = if is_dm {
        peer.map(|u| u.name.clone()).unwrap_or_else(|| name.clone())
    } else {
        app.selected_guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)).map(|g| g.name.clone()).unwrap_or_default()
    };
    let search_ph = if is_dm && crate::ui::search::cross_dm(app) {
        crate::t!("search.in_dms")
    } else {
        SharedString::from(format!("{} {}", crate::t!("search.placeholder"), scope))
    };
    let pad_start = if is_group_dm { space::md() / 2. } else { space::md() };
    div()
        .relative()
        .child(width_probe(&HEADER_W))
        .h(z(m::HEADER_H + 1.))
        .flex_none()
        .pl(z(pad_start))
        .pr(z(8.))
        .py(z(8.))
        .flex()
        .items_center()
        .bg(crate::theme::client_theme::surface(crate::theme::client_theme::Level::Chat, Some(crate::theme::client_theme::Level::Chat), Palette::BG_PRIMARY))
        .border_t_1()
        .border_b_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .flex()
                .items_center()
                .child(if let Some(gc) = ch.map(|c| c.id).filter(|_| is_group_dm) {
                    let half = space::md() / 2.;
                    div()
                        .id("gdm-name")
                        .group("gdm-name")
                        .flex_shrink_0()
                        .h(z(32.))
                        .px(z(half))
                        .gap(z(half))
                        .rounded(z(8.))
                        .flex()
                        .items_center()
                        .whitespace_nowrap()
                        .cursor_pointer()
                        .hover(|s| s.bg(tk(idx::BACKGROUND_MOD_SUBTLE)))
                        .child(div().mr(z(4.)).child(crate::ui::group_dm::group_avatar(app, gc, 24. * crate::theme::zoom(), c(Palette::BG_PRIMARY))))
                        .child(
                            div()
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tk(idx::TEXT_STRONG))
                                .child(crate::ui::emoji_text::label(name.clone(), 16.)),
                        )
                        .child(
                            div().hidden().group_hover("gdm-name", |s| s.flex()).child(
                                gpui::svg().path(crate::icons::path("pen")).size(z(16.)).flex_none().text_color(tk(idx::TEXT_DEFAULT)),
                            ),
                        )
                        .into_any_element()
                } else if is_dm {
                    div().mr(z(8.)).child(avatar(peer, 20. * crate::theme::zoom())).into_any_element()
                } else {
                    div()
                        .ml(z(-1.))
                        .mr(z(11.))
                        .w(z(20.))
                        .h(z(32.))
                        .flex()
                        .items_center()
                        .map(|d| match crate::ui::sidebar::channel_icon_override(
                            kind,
                            ch.is_some_and(|ch| crate::ui::sidebar::is_rules_channel(app, ch)),
                            locked && thread_parent.is_none(),
                        ) {
                            Some(n) => d.child(gpui::svg().path(crate::icons::path(n)).size(z(20.)).flex_none().text_color(tk(idx::CHANNEL_ICON))),
                            None => d.child(icon(channel_icon(kind), 20. * crate::theme::zoom(), tk(idx::CHANNEL_ICON))),
                        })
                        .into_any_element()
                })
                .when_some(thread_parent.map(|p| (p.id, p.name.clone())), |d, (pid, pname)| {
                    d.child(
                        div()
                            .id("thread-parent")
                            .flex_none()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .mr(z(8.))
                            .h(z(24.))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .text_size(z(16.))
                            .line_height(z(20.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(tk(idx::TEXT_DEFAULT))
                            .on_click(cx.listener(move |a, _, _, cx| a.select_channel(pid, cx)))
                            .child(crate::ui::emoji_text::label(pname, 16.)),
                    )
                    .child(
                        div().flex_none().mr(z(8.)).w(z(10.)).h(z(24.)).flex().items_center().justify_center().child(
                            gpui::svg().path(crate::icons::path("chevron-right")).size(z(10.)).text_color(tk(idx::ICON_MUTED)),
                        ),
                    )
                })
                .when(!is_group_dm, |d| d.child(
                    crate::fonts::styled_name(
                        div(),
                        peer.filter(|_| is_dm),
                        false,
                    )
                        .flex_shrink(if thread_parent.is_some() { 1. } else { 0. })
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .mr(z(8.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(tk(idx::TEXT_STRONG))
                        .child(crate::ui::emoji_text::label(name.clone(), 16.)),
                ))
                .when_some(ch.filter(|_| kind == ChannelKind::Announcement && thread_parent.is_none()).map(|c| c.id), |d, id| d.child(crate::ui::forum::follow_button(id)))
                .when(discoverable, |d| {
                    d.child(div().flex_none().ml(z(4.)).mr(z(12.)).size(z(4.)).rounded_full().bg(tk(idx::BACKGROUND_MOD_STRONG)))
                        .child(
                            div()
                                .flex_none()
                                .text_size(z(14.))
                                .line_height(z(18.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tk(idx::TEXT_MUTED))
                                .relative()
                                .child(crate::t!("chat.discovery_mode"))
                                .child(discovery_label_probe()),
                        )
                })
                .when_some(topic.filter(|_| !is_dm), |d, t| {
                    let topic_click = crate::ui::home::topic::on_click(app, cx);
                    d.child(if discoverable {
                        div().flex_none().ml(z(8.)).w(z(1.)).h(z(16.)).bg(tk(idx::BORDER_SUBTLE))
                    } else {
                        div().flex_none().mx(z(4.)).size(z(4.)).rounded_full().bg(tk(idx::BACKGROUND_MOD_STRONG))
                    })
                        .child(
                            div()
                                .id("channel-topic")
                                .ml(z(8.))
                                .flex_1()
                                .min_w_0()
                                .cursor_pointer()
                                .text_size(z(14.))
                                .line_height(z(18.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tk(idx::TEXT_MUTED))
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .on_click(topic_click)
                                .child(crate::ui::emoji_text::markup_label(reply_preview_text(app, &t), 14.)),
                        )
                }),
        )
        .child(
            div()
                .flex_none()
                .pl(z(space::xs()))
                .flex()
                .items_center()
                .gap(z(space::xs()))
                .when(is_dm, |d| {
                    let cid = ch.map(|c| c.id);
                    let ring = move |a: &mut PhoenixApp, cx: &mut Context<PhoenixApp>| {
                        if let Some(c) = cid {
                            a.blocked_write(crate::actions::guard::GuardedWrite::ring_call(c.0), cx);
                        }
                    };
                    d.child(hdr_named("dm-call", "phone-call", ic).on_click(cx.listener(move |a, _, _, cx| ring(a, cx))))
                        .child(hdr_named("dm-video", "video-call", ic).on_click(cx.listener(move |a, _, _, cx| ring(a, cx))))
                        .child(
                            pin_button(app, if app.chat.pins.is_some() { on } else { ic })
                                .on_click(cx.listener(|a, _, _, cx| a.toggle_pins(cx))),
                        )
                        .child(hdr_named(if is_group_dm { "gdm-add" } else { "dm-add" }, "add-to-dm", ic).on_click(cx.listener(move |a, _, _, cx| {
                            if let Some(c) = cid {
                                a.open_add_to_dm(c, cx);
                            }
                        })))
                        .map(|d| {
                            if !is_group_dm && !crate::ui::title_bar::dm_profile_fits(crate::ui::title_bar::viewport_w_css()) {
                                return d.child(
                                    crate::ui::states::header_button_disabled(
                                        "dm-profile",
                                        crate::icons::path("user-profile"),
                                        crate::t!("header.profile_unavailable"),
                                    )
                                    .debug_selector(|| "hdr-dm-profile".to_owned()),
                                );
                            }
                            d.child(hdr_named(if is_group_dm { "gdm-members" } else { "dm-profile" }, "user-profile", if app.show_members { on } else { ic }).on_click(cx.listener(|a, _, _, cx| {
                                a.show_members = !a.show_members;
                                cx.notify();
                            })))
                        })
                })
                .when(!is_dm, |d| {
                    d.when(!parent_is_forum && !is_forum, |d| d.child(hdr_button("threads", Icon::Threads, ic, HDR_ICON).on_click(cx.listener(|a, _, _, cx| a.toggle_threads_popout(cx)))))
                        .map(|d| {
                            let slashed = ch.is_some_and(|c| {
                                thread_info.as_ref().is_some_and(|i| !i.joined || i.member_muted || i.notify_level() != 2)
                                    || c.muted
                                    || app.backend.notify_prefs().is_some_and(|p| {
                                        p.header_bell_slashed(c.guild.map(|g| g.0), c.id.0, c.parent.map(|p| p.0))
                                    })
                            });
                            let bell_col = if crate::ui::nav_menus::is_bell_open() { on } else { ic };
                            let bell_ch = ch.map(|c| c.id);
                            let btn = if slashed {
                                hdr_named("bell", "bell-slash", bell_col)
                            } else {
                                hdr_button("bell", Icon::Bell, bell_col, HDR_ICON)
                            };
                            d.child(btn.on_click(move |ev, w, _| {
                                if let Some(id) = bell_ch {
                                    crate::ui::nav_menus::toggle_bell(id, ev.position(), w);
                                }
                            }))
                        })
                        .when(!is_forum, |d| {
                            d.child(
                                pin_button(app, if app.chat.pins.is_some() { on } else { ic })
                                    .on_click(cx.listener(|a, _, _, cx| a.toggle_pins(cx))),
                            )
                        })
                        .when(!thread_info.as_ref().is_some_and(|i| i.archived), |d| {
                            d.child(hdr_button("members", Icon::Members, if app.show_members { on } else { ic }, HDR_ICON).on_click(
                                cx.listener(|a, _, _, cx| {
                                    a.show_members = !a.show_members;
                                    cx.notify();
                                }),
                            ))
                        })
                        .when_some(thread_parent.and(ch.map(|c| c.id)), |d, t| {
                            d.child(hdr_named("thread-more", "more-horizontal", ic).on_click(move |ev, w, _| {
                                crate::ui::thread_menu::open(t, ev.position(), w)
                            }))
                        })
                })
                .child(
                    div()
                        .id("hdr-search")
                        .debug_selector(|| "hdr-search".to_owned())
                        .cursor(gpui::CursorStyle::IBeam)
                        .on_click(cx.listener(|a, _, w, cx| crate::ui::search::open(a, w, cx)))
                        .ml(z(4.))
                        .flex_none()
                        .w(z(header_search_w(pad_start)))
                        .h(z(if crate::theme::density() == crate::theme::Density::Compact { 28. } else { 32. }))
                        .rounded(z(8.))
                        .bg(tk(idx::INPUT_BACKGROUND_DEFAULT))
                        .border_1()
                        .border_color(tk(idx::INPUT_BORDER_DEFAULT))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .pl(z(8.))
                                .pr(z(6.))
                                .pb(z(1.))
                                .text_size(z(14.))
                                .line_height(z(18.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tk(idx::INPUT_PLACEHOLDER_TEXT_DEFAULT))
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(search_ph),
                        )
                        .child(div().size(z(24.)).p(z(4.)).flex().items_center().child(icon(Icon::Search, 16. * crate::theme::zoom(), tk(idx::ICON_MUTED)))),
                ),
        )
}

fn row(entity: &Entity<PhoenixApp>, ix: usize, cx: &mut App) -> AnyElement {
    let app = entity.read(cx);
    let Some(ch_id) = app.selected_channel else { return div().into_any_element() };
    let Some(entry) = app.messages.get(&ch_id) else { return div().into_any_element() };
    if ix == 0 {
        return row_wrapper()
            .relative()
            .when(entry.msgs.is_empty() && !entry.reached_start, |d| d.pb(z(20.)))
            .child(intro(app, entity, ch_id, entry.reached_start))
            .child(rows::top_row_probe())
            .into_any_element();
    }
    let i = ix - 1;
    let (Some(msg), Some(r)) = (entry.msgs.get(i), entry.rendered.get(i)) else {
        return div().into_any_element();
    };
    let el = match blocked::row(app, entity, entry, i) {
        blocked::Row::Normal => message(app, entity, msg, r, entry, cx),
        blocked::Row::Hidden => div().into_any_element(),
        blocked::Row::Collapsed(bar) => bar,
        blocked::Row::Expanded(bar) => {
            let m = message(app, entity, msg, r, entry, cx);
            row_wrapper().bg(tk(idx::MESSAGE_BACKGROUND_HOVER)).children(bar).child(m).into_any_element()
        }
    };
    if i + 1 == entry.msgs.len() {
        return row_wrapper().pb(z(24.)).child(el).into_any_element();
    }
    el
}

fn intro(app: &PhoenixApp, entity: &Entity<PhoenixApp>, ch: Id, reached_start: bool) -> gpui::Div {
    let channel = app.channels.get(&ch);
    let name = channel.map(|c| c.name.clone()).unwrap_or_default();
    if !reached_start {
        return skeleton::for_channel(ch.0, app.prefs.synced.compact);
    }
    if let Some(checklist) = welcome::render(app, entity, ch) {
        return checklist;
    }
    if let Some(chan) = channel.filter(|c| c.guild.is_none()) {
        let group = crate::ui::dm_profile_panel::is_group_dm(app, chan.id);
        let peer = chan.voice_users.first().filter(|_| !group).and_then(|u| app.users.get(u));
        let strong = name.clone();
        let desc = if group { crate::t!("chat.group_start", name = strong) } else { crate::t!("chat.dm_start", name = strong) };
        let bold = desc.find(&*name).filter(|_| !name.is_empty()).map(|i| i..i + name.len());
        let desc_el = gpui::StyledText::new(desc.clone()).with_highlights(
            bold.map(|r| (r, gpui::HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() })),
        );
        let e1 = entity.clone();
        let e2 = entity.clone();
        return div()
            .m(z(20.))
            .flex()
            .flex_col()
            .child(if group { crate::ui::group_dm::group_avatar(app, chan.id, 80., c(Palette::BG_PRIMARY)) } else { avatar(peer, 80.) })
            .child(
                div()
                    .my(z(8.))
                    .text_size(z(32.))
                    .line_height(z(40.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(tk(idx::TEXT_STRONG))
                    .child(name.clone()),
            )
            .child(
                div()
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .when_some(peer.map(|u| u.name.clone()), |d, uname| {
                        d.child(
                            div()
                                .mb(z(20.))
                                .text_size(z(24.))
                                .line_height(z(32.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tk(idx::TEXT_STRONG))
                                .child(uname),
                        )
                    })
                    .child(desc_el)
                    .when_some(peer.map(|u| u.id), |d, uid| d.child(dm_intro_actions(app, entity, uid))),
            )
            .when(group, |d| {
                use crate::ui::controls::{BtnKind, button_icon};
                d.child(
                    div()
                        .mt(z(16.))
                        .flex()
                        .gap(z(8.))
                        .child(
                            button_icon("gdm-intro-invite", "invite", crate::t!("header.add_to_gdm"), BtnKind::Primary)
                                .on_click(move |_, _, cx| e1.update(cx, |a, cx| a.open_add_to_dm(ch, cx))),
                        )
                        .child(
                            button_icon("gdm-intro-edit", "pen", crate::t!("menu.edit_group"), BtnKind::Secondary)
                                .on_click(move |_, _, cx| e2.update(cx, |a, cx| a.open_edit_group(ch, cx))),
                        ),
                )
            });
    }
    let is_thread = app.nav.thread_parent(ch).is_some();
    div()
        .m(z(16.))
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(
            div()
                .mt(z(16.))
                .size(z(68.))
                .rounded_full()
                .bg(tk(idx::BACKGROUND_MOD_MUTED))
                .flex()
                .items_center()
                .justify_center()
                .child(icon(
                    if is_thread { Icon::Threads } else { channel.map_or(Icon::Hash, |c| crate::ui::sidebar::channel_icon(c.kind)) },
                    42.,
                    gpui::white(),
                )),
        )
        .child(
            div()
                .text_size(z(32.))
                .line_height(z(40.))
                .font_weight(FontWeight::BOLD)
                .text_color(c(Palette::TEXT_HEADER))
                .child(if is_thread { name.clone() } else { crate::t!("chat.welcome", name = name) }),
        )
        .when(!is_thread, |d| {
            d.child(
                div()
                    .text_size(z(16.))
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .child(crate::t!("chat.channel_start", name = name)),
            )
        })
}

fn dm_intro_actions(app: &PhoenixApp, entity: &Entity<PhoenixApp>, uid: Id) -> gpui::Div {
    use crate::ui::controls::{BtnKind, button_sm};
    let profile = app.people.profiles.get(&uid);
    let mutual = profile.map(|p| p.mutual_guild_ids.as_slice()).unwrap_or_default();
    let is_friend = app
        .nav
        .relationship(uid)
        .is_some_and(|r| r.kind == crate::ui::home::data::RelationshipKind::Friend);
    let mut row = div().mt(z(8.)).flex().flex_wrap().items_center().gap(z(8.));
    if !mutual.is_empty() {
        let label = if mutual.len() == 1 {
            crate::t!("chat.mutual_one")
        } else {
            crate::t!("profile.mutual_servers", n = mutual.len())
        };
        let icons = mutual
            .iter()
            .filter_map(|g| app.guilds.iter().find(|x| x.id == *g))
            .take(3)
            .map(|g| crate::ui::invite::guild_icon(Some(&g.icon), &g.name, 24. * crate::theme::zoom()))
            .collect::<Vec<_>>();
        let e = entity.clone();
        row = row.child(
            div()
                .id("dm-intro-mutual")
                .group("dm-intro-mutual")
                .flex()
                .items_center()
                .gap(z(8.))
                .cursor_pointer()
                .on_click(move |_, _, cx| e.update(cx, |a, cx| a.open_profile_modal_at(uid, crate::ui::profile::TAB_SERVERS, cx)))
                .child(div().flex().children(icons))
                .child(
                    div()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
                        .group_hover("dm-intro-mutual", |s| s.text_color(tk(idx::INTERACTIVE_TEXT_HOVER)))
                        .child(label),
                )
                .child(div().mx(z(4.)).size(z(4.)).rounded_full().bg(tk(idx::BORDER_STRONG))),
        );
    }
    if mutual.is_empty() && profile.is_some() {
        row = row.child(
            div()
                .flex()
                .items_center()
                .child(div().text_sm().text_color(tk(idx::TEXT_SUBTLE)).child(crate::t!("profile.tab_no_mutual_servers")))
                .child(div().mx(z(4.)).size(z(4.)).rounded_full().bg(tk(idx::BORDER_STRONG))),
        );
    }
    let e1 = entity.clone();
    let e2 = entity.clone();
    let friend_label = if is_friend { crate::t!("menu.remove_friend") } else { crate::t!("chat.add_friend") };
    row.child(
        div()
            .flex()
            .gap(z(8.))
            .child(button_sm("dm-intro-friend", friend_label, BtnKind::Secondary).on_click(move |_, _, cx| {
                e1.update(cx, |a, cx| {
                    if is_friend {
                        a.confirm_remove_friend(uid, cx)
                    } else {
                        a.relationship_action(uid, crate::ui::home::data::RelAction::Accept, cx)
                    }
                })
            }))
            .child(
                button_sm("dm-intro-block", crate::t!("menu.block"), BtnKind::Secondary)
                    .on_click(move |_, _, cx| e2.update(cx, |a, cx| a.confirm_block(uid, cx))),
            ),
    )
}

fn day_divider(label: SharedString) -> gpui::Div {
    dated_divider(label, false)
}

fn dated_divider(label: SharedString, unread: bool) -> gpui::Div {
    let line = if unread { idx::BACKGROUND_FEEDBACK_NOTIFICATION } else { idx::BORDER_SUBTLE };
    div()
        .ml(z(16.))
        .mr(z(14.))
        .mt(z(24.))
        .mb(z(8.))
        .h(z(1.))
        .bg(tk(line))
        .relative()
        .flex()
        .justify_center()
        .items_center()
        .when(unread, |d| d.child(message_ext::unread_end_cap()))
        .child(
            div()
                .bg(tk(idx::BACKGROUND_BASE_LOWER))
                .rounded(z(8.))
                .mt(z(-1.))
                .px(z(4.))
                .py(z(2.))
                .text_size(z(12.))
                .line_height(z(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk(if unread { idx::BACKGROUND_FEEDBACK_NOTIFICATION } else { idx::TEXT_MUTED }))
                .child(label),
        )
}

pub fn format_time_tooltip(ts: i64) -> SharedString {
    use chrono::{Local, TimeZone};
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|d| crate::i18n::full_datetime(d).into())
        .unwrap_or_default()
}

fn time_tooltip(ts: i64) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
    crate::ui::tooltip::tip(format_time_tooltip(ts))
}

pub(crate) fn server_tag_chip(tag: &(SharedString, Option<SharedString>), in_reply: bool) -> gpui::Div {
    div()
        .flex_none()
        .ml(z(if in_reply { 2. } else { 6. }))
        .when(in_reply, |d| d.mr(z(4.)))
        .h(z(15.))
        .px(z(4.))
        .rounded(z(4.))
        .bg(tk(idx::BACKGROUND_MOD_STRONG))
        .flex()
        .items_center()
        .gap(z(2.))
        .text_size(fz(11.52))
        .line_height(z(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(tk(idx::TEXT_DEFAULT))
        .when_some(tag.1.clone(), |d, u| d.child(img(u).size(z(if in_reply { 12. } else { 14. })).flex_none()))
        .child(tag.0.clone())
}

pub(crate) fn role_icon((url, name): (SharedString, SharedString)) -> gpui::Div {
    div().flex_none().size(z(20.)).ml(z(4.)).child(img(url).size_full().object_fit(gpui::ObjectFit::Contain)).tip(name)
}

fn new_member_badge(id: Id) -> gpui::Stateful<gpui::Div> {
    div()
        .id(("new-member", id.0))
        .flex_none()
        .size(z(20.))
        .ml(z(4.))
        .mt(z(1.))
        .cursor_pointer()
        .child(gpui::svg().path(crate::icons::path("new-user")).size_full().text_color(tk(idx::TEXT_FEEDBACK_POSITIVE)))
        .tip(crate::t!("chat.new_member"))
}

pub(crate) fn bot_tag(verified: bool) -> gpui::Div {
    bot_tag_label(verified, crate::t!("common.app"))
}

pub(crate) fn bot_tag_label(verified: bool, label: SharedString) -> gpui::Div {
    div()
        .flex_none()
        .ml(z(4.))
        .mt(z(1.6))
        .px(z(4.4))
        .h(z(15.))
        .rounded(z(4.))
        .bg(tk(idx::BACKGROUND_BRAND))
        .flex()
        .items_center()
        .text_size(z(12.8))
        .line_height(z(15.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(gpui::white())
        .when(verified, |d| d.child(crate::ui::members::verified_check().mt(z(-0.32)).ml(z(-3.2))))
        .child(label)
}

pub(crate) fn message(
    app: &PhoenixApp,
    entity: &Entity<PhoenixApp>,
    msg: &Message,
    r: &RenderedMessage,
    entry: &crate::app::ChannelMessages,
    cx: &App,
) -> AnyElement {
    let author_cow = app.member_user(app.selected_guild, msg.author);
    let author = author_cow.as_deref();
    let name = msg.author_nick.clone().or_else(|| app.member_nick(msg.author)).or_else(|| author.map(|u| u.display_name.clone())).unwrap_or_else(|| crate::t!("common.unknown"));
    let (author_role_color, author_role_gradient) = app.author_role_style(msg.author);
    let color = author_role_color.map(c).unwrap_or_else(|| tk(idx::TEXT_STRONG));
    let id = msg.id;
    if let Some(sys) = system::render(app, entity, msg, r) {
        return row_wrapper().child(sys).into_any_element();
    }
    let compact = app.prefs.synced.compact;
    let editing = app.chat.edit.as_ref().is_some_and(|e| e.id == id);
    let replying = app.replying_to == Some(id);
    let reply_pings_me = msg.reply_mentions_author
        && msg.reply_to.and_then(|rid| entry.index_of(rid)).is_some_and(|p| entry.msgs[p].author == app.me.id);
    let mentioned = r.mentions_me || msg.mention_everyone || reply_pings_me;
    let ts = msg.timestamp;
    let edited_at = if msg.edited_at > 0 { msg.edited_at } else { ts };

    let mut content = div().flex().flex_col().min_w_0().text_size(fz(16.)).text_color(c(Palette::TEXT_NORMAL));
    let last_text = r.blocks.iter().rposition(|b| matches!(b, RBlock::Text(_)));
    let n_blocks = r.blocks.len();
    let compact_avatars = compact && app.prefs.synced.flag("avatars");
    let compact_header_left = 16. * crate::theme::font_scale();
    let is_bot = author.is_some_and(|a| a.bot);
    let overflow = if compact {
        compact_header_measured(id, compact_header_left).unwrap_or_else(|| compact_header_overflow(
            &r.time_short,
            &name,
            compact_avatars,
            is_bot,
            author.and_then(|u| u.tag.as_ref()).map(|t| t.0.as_ref()),
            compact_header_left,
            cx,
        ))
    } else {
        0.
    };
    if compact && !matches!(r.blocks.first(), Some(RBlock::Text(_))) {
        content = content.child(div().flex_none().h(fz(22.)));
    }
    let show_embed = |e: &&crate::model::Embed| {
        let sy = &app.prefs.synced;
        let media = matches!(e.kind.as_deref(), Some("image" | "gifv" | "video"));
        sy.flag("embeds") && (!media || sy.flag("media-links"))
    };
    let gif_only = msg.embeds.iter().all(|e| show_embed(&e)) && crate::model::hides_simple_embed_content(&msg.content, &msg.embeds);
    for (bi, b) in r.blocks.iter().enumerate().filter(|_| !gif_only) {
        let edited_here = (msg.edited && last_text == Some(bi) && bi + 1 == n_blocks).then_some(edited_at);
        match b {
            RBlock::Text(t) if compact && bi == 0 => {
                content = content.child(rich(entity, id, bi, &indent_rich(t, overflow, cx), edited_here));
            }
            _ => content = content.child(block(entity, id, bi, b, edited_here)),
        }
    }
    let edited_after_embeds = msg.edited && last_text.is_none();
    if msg.edited && !edited_after_embeds && !last_text.is_some_and(|l| l + 1 == n_blocks) {
        content = content.child(edited_marker(edited_at));
    }
    let has_media = !msg.attachments.is_empty() || !msg.embeds.is_empty();
    if has_media {
        media::set_current_author(Some(media::ViewerAuthor {
            user: author.cloned(),
            name: name.clone(),
            color,
            time: r.time_full.clone(),
        }));
    }
    content = content.children(invite_embed::render(app, entity, msg.author, &msg.content, cx));
    if !msg.attachments.is_empty() {
        let visual: Vec<&Attachment> = msg.attachments.iter().filter(|a| mosaic::is_visual(a)).collect();
        if visual.len() >= 2 && crate::ui::settings::effects::inline_attachments() {
            content = content.child(mosaic::render(&visual, cx));
            for a in msg.attachments.iter().filter(|a| !mosaic::is_visual(a)) {
                content = content.child(div().mt(z(4.)).child(attachment(a, cx)));
            }
        } else {
            for a in &msg.attachments {
                content = content.child(attachment(a, cx));
            }
        }
    }
    for (ei, e) in msg.embeds.iter().filter(show_embed).enumerate() {
        content = content.child(embed_for_msg(app, entity, e, Some((id.0, selection::EMBED_BASE + ei * selection::EMBED_STRIDE)), cx));
    }
    if has_media {
        media::set_current_author(None);
    }
    if let Some(f) = &msg.forward {
        content = div().flex().flex_col().min_w_0().text_size(fz(16.)).text_color(c(Palette::TEXT_NORMAL)).child(forward::wrap(app, f, content));
    }
    content = content.children(crate::ui::components::render(app, entity, msg, cx)).children(crate::ui::components::render_poll(app, entity, msg));
    if edited_after_embeds {
        content = content.child(div().mt(z(4.)).h(z(20.)).flex().items_center().child(edited_marker(edited_at)));
    }
    if !msg.reactions.is_empty() && app.prefs.synced.flag("reactions") {
        let after_accessory = !msg.attachments.is_empty() || msg.embeds.iter().any(|e| show_embed(&e)) || msg.forward.is_some();
        content = content.child(message_ext::reactions(app, entity, msg, after_accessory));
    }
    if let Some(t) = &msg.thread {
        content = content.child(message_ext::thread_chip(app, entity, t, false));
    }
    if let Some(st) = command_msg::status(app, msg) {
        content = content.child(command_msg::status_line(st)).children(command_msg::dismiss_button(entity, msg));
    }
    if msg.flags & crate::model::FLAG_EPHEMERAL != 0 {
        content = content.child(crate::ui::chat::pending::ephemeral_footer(entity, msg));
    }
    let content = match message_ext::edit_box(app, entity, msg, cx) {
        Some(b) => b,
        None => content,
    };
    let content = if msg.send_state == crate::model::SendState::Sending && msg.interaction.is_none() { content.opacity(0.5) } else { content };
    let content = match attach::upload_progress_card(app, entity, msg) {
        Some(card) => div().flex().flex_col().min_w_0().child(content).child(card),
        None => content,
    };

    let reply = msg.reply_to.map(|rid| {
        let found = entry.index_of(rid);
        let (ru, (preview, mranges), rnick) = match found {
            Some(p) => (
                app.member_user(app.selected_guild, entry.msgs[p].author),
                (entry.rendered[p].preview.clone(), entry.rendered[p].preview_mentions.clone()),
                entry.msgs[p].author_nick.clone().or_else(|| app.member_nick(entry.msgs[p].author)).or_else(|| {
                    let a = entry.msgs[p].author;
                    entry.msgs.iter().rev().take(64).filter(|m| m.author == a).find_map(|m| m.author_nick.clone())
                }),
            ),
            None => match &msg.reply_preview {
                Some((a, nick, text)) => (
                    app.member_user(app.selected_guild, *a),
                    {
                        let (t, m) = crate::richtext::preview_with_mentions(&crate::markdown::parse(text, &app.resolver()));
                        (SharedString::from(t), m)
                    },
                    nick.clone().or_else(|| app.member_nick(*a)),
                ),
                None => (None, (crate::t!("chat.reply_deleted"), Vec::new()), None),
            },
        };
        let reply_voice = match found {
            Some(p) => entry.msgs[p].flags & phoenix_voice::audio::voice_message::FLAG_IS_VOICE_MESSAGE != 0,
            None => msg.reply_voice,
        };
        let found = found.or_else(|| msg.reply_preview.as_ref().map(|_| usize::MAX));
        div()
            .relative()
            .h(fz(if compact { 20. } else { 18. }))
            .when(!compact, |d| d.mb(z(4.)))
            .when(compact, |d| d.ml(z(-24.)).pl(z(10.)))
            .flex()
            .items_center()
            .gap(z(4.))
            .text_size(fz(14.))
            .line_height(fz(18.))
            .text_color(tk(idx::TEXT_DEFAULT))
            .child(
                div()
                    .absolute()
                    .left(z(if compact { -23. } else { -41. }))
                    .top(z(9.))
                    .w(z(if compact { 29. } else { 37. }))
                    .h(z(if compact { 11. } else { 13. }))
                    .border_l_2()
                    .border_t_2()
                    .rounded_tl(z(6.))
                    .border_color(tk(idx::SPINE_DEFAULT))
                    .hover(|s| s.border_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))),
            )
            .when(!compact, |d| d.child(avatar(ru.as_deref(), 16.)))
            .when(ru.as_deref().is_some_and(|u| u.bot), |d| d.child(bot_tag(ru.as_deref().is_some_and(|u| u.verified_bot)).ml_0()))
            .child({
                let (reply_color, reply_gradient) = ru.as_deref().map(|u| app.author_role_style(u.id)).unwrap_or((None, None));
                let reply_text = SharedString::from(format!(
                    "{}{}",
                    if msg.reply_mentions_author { "@" } else { "" },
                    rnick.map(|n| n.to_string()).or_else(|| ru.as_deref().map(|u| u.display_name.to_string())).unwrap_or_else(|| crate::t!("common.unknown").to_string())
                ));
                crate::fonts::styled_name(
                    div()
                        .flex_none()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(reply_color.map(c).unwrap_or_else(|| tk(idx::TEXT_STRONG)))
                        .opacity(0.64),
                    ru.as_deref(),
                    false,
                )
                .id(("reply-author", msg.id.0))
                .when_some(ru.as_deref().map(|u| u.id), |d, uid| {
                    d.cursor_pointer()
                        .hover(|s| s.underline())
                        .on_click(message_ext::open_user(entity, uid, msg.channel))
                        .on_mouse_down(gpui::MouseButton::Right, message_ext::open_user_menu(entity, uid, msg.channel))
                })
                .child({
                    let hovered = crate::ui::avatar_deco::hovered(crate::ui::avatar_deco::hover_key(2, msg.id.0));
                    let motion = if hovered { crate::fonts::Motion::Loop } else { crate::fonts::Motion::Still };
                    crate::fonts::role_name_text_moving(reply_text, ru.as_deref(), 14., app.selected_guild.is_some(), reply_gradient, hovered, motion)
                })
            })
            .when_some(ru.and_then(|u| u.tag.clone()), |d, tg| d.child(server_tag_chip(&tg, true).opacity(0.64)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .hover(|s| s.text_color(tk(idx::TEXT_STRONG)))
                    .id(("reply-jump", msg.id.0))
                    .cursor_pointer()
                    .on_click({
                        let (e, ch) = (entity.clone(), msg.channel);
                        move |_, _, cx| e.update(cx, |a, cx| a.jump_to_message(ch, rid, cx))
                    })
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .when(found.is_none() || preview.is_empty(), |d| d.italic())
                    .map(|d| {
                        let text = if found.is_some() && preview.is_empty() {
                            if reply_voice { crate::t!("chat.reply_click_to_see_voice") } else { crate::t!("chat.reply_click_to_see") }
                        } else {
                            preview
                        };
                        let seg = |a: usize, b: usize| -> gpui::AnyElement {
                            let hl: Vec<_> = mranges
                                .iter()
                                .filter(|r| r.start < b && r.end > a)
                                .map(|r| {
                                    (
                                        r.start.max(a) - a..r.end.min(b) - a,
                                        gpui::HighlightStyle {
                                            color: Some(c(Palette::MENTION_FG)),
                                            background_color: Some(crate::theme::ca(Palette::MENTION_BG)),
                                            font_weight: Some(FontWeight::MEDIUM),
                                            ..Default::default()
                                        },
                                    )
                                })
                                .collect();
                            let t = SharedString::from(text[a..b].to_string());
                            if hl.is_empty() { t.into_any_element() } else { gpui::StyledText::new(t).with_highlights(hl).into_any_element() }
                        };
                        let tokens = crate::media::emoji::find_custom(&text);
                        if tokens.is_empty() {
                            return d.child(seg(0, text.len()));
                        }
                        let mut row = d.flex().items_center();
                        let mut at = 0;
                        for (r, _) in &tokens {
                            if r.start > at {
                                row = row.child(div().flex_none().child(seg(at, r.start)));
                            }
                            row = row.child(div().flex_none().size(z(18.)).child(crate::media::emoji::emoji_element(&text[r.clone()], 18. * crate::theme::zoom())));
                            at = r.end;
                        }
                        if at < text.len() {
                            row = row.child(div().flex_none().child(seg(at, text.len())));
                        }
                        row
                    }),
            )
    });

    let reply = reply.or_else(|| msg.interaction.as_ref().filter(|_| msg.reply_to.is_none()).map(|cu| command_msg::header(app, entity, msg, cu, compact, cx)));

    let a11y_name = name.clone();
    let author_name = (compact || r.group_start).then(|| {
        let role_dot = app.author_role_dot(msg.author);
        crate::fonts::styled_name(div(), author, false)
            .flex_none()
            .when(role_dot.is_some(), |d| d.flex().items_center())
            .text_size(fz(16.))
            .line_height(fz(22.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(color)
            .id(("msg-author", id.0))
            .cursor_pointer()
            .hover(|s| s.underline())
            .on_click(message_ext::open_author(entity, msg))
            .on_mouse_down(gpui::MouseButton::Right, message_ext::open_author_menu(entity, msg))
            .children(role_dot.map(|(col, gradient)| {
                let hovered = crate::ui::avatar_deco::hovered(crate::ui::avatar_deco::hover_key(2, msg.id.0));
                div().flex_none().mr(z(4.)).child(crate::ui::role_dot::role_dot(("msg-role-dot", id.0), col, gradient, hovered, fz(20.)))
            }))
            .child({
                let hovered = crate::ui::avatar_deco::hovered(crate::ui::avatar_deco::hover_key(2, msg.id.0));
                let motion = if hovered { crate::fonts::Motion::Loop } else { crate::fonts::Motion::Still };
                let name_len = name.len();
                selection::note(id.0, selection::AUTHOR_BLOCK, name.clone(), &[]);
                if author_role_gradient.is_none() {
                    let run = gpui::TextRun {
                        len: name_len,
                        font: gpui::Font { family: crate::theme::FONT_UI().into(), weight: FontWeight::MEDIUM, ..Default::default() },
                        color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let runs = selection::highlight(id.0, selection::AUTHOR_BLOCK, 0, name_len, vec![run]);
                    let st = StyledText::new(name).with_runs(runs);
                    let probe = selection::probe(st.layout().clone(), id.0, selection::AUTHOR_BLOCK, 0, name_len);
                    div().relative().child(st).child(probe).into_any_element()
                } else {
                    div()
                        .when(selection::covers(id.0, selection::AUTHOR_BLOCK, &(0..name_len)), |d| d.bg(selection::selection_bg()))
                        .child(crate::fonts::role_name_text_moving(name, author, 16., app.selected_guild.is_some(), author_role_gradient, hovered, motion))
                        .into_any_element()
                }
            })
    });

    let show_toolbar = crate::ui::avatar_deco::hovered(crate::ui::avatar_deco::hover_key(2, id.0))
        || app.chat.menu.is_some_and(|m| m.msg == id)
        || crate::shortcuts::message::focused().is_some_and(|f| f.1 == id);
    let base = div()
        .id(("msg", id.0))
        .when(crate::ui::a11y::on(), |d| {
            let replied = msg.reply_to.map(|rid| match entry.index_of(rid) {
                Some(p) => {
                    let a = entry.msgs[p].author;
                    entry.msgs[p].author_nick.clone().or_else(|| app.member_nick(a)).or_else(|| app.member_user(app.selected_guild, a).map(|u| u.display_name.clone()))
                }
                None => msg.reply_preview.as_ref().and_then(|(a, nick, _)| nick.clone().or_else(|| app.member_nick(*a)).or_else(|| app.member_user(app.selected_guild, *a).map(|u| u.display_name.clone()))),
            });
            let replied = replied.flatten();
            let mut accessories: Vec<SharedString> = msg.attachments.iter().map(crate::ui::a11y::attachment).collect();
            accessories.extend(msg.embeds.iter().filter_map(|e| e.title.clone().or_else(|| e.description.clone())));
            let content = crate::richtext::plain_text(&r.parsed);
            let label = crate::ui::a11y::message(&crate::ui::a11y::MessageParts {
                author: &a11y_name,
                replied_author: replied.as_deref(),
                content: &content,
                edited: msg.edited,
                accessories: &accessories,
                time: if r.group_start { &r.time_full } else { &r.time_short },
            });
            let d = d.role(gpui::Role::Article).aria_label(label);
            if crate::shortcuts::message::focused().is_some_and(|f| f.1 == id) { d.aria_active_descendant() } else { d }
        })
        .group("msg")
        .on_hover({
            let e = entity.clone();
            move |h, _, cx| {
                if crate::ui::avatar_deco::set_hover(crate::ui::avatar_deco::hover_key(2, id.0), *h, cx) {
                    e.update(cx, |_, cx| cx.notify());
                }
            }
        })
        .on_mouse_down(gpui::MouseButton::Right, message_ext::on_right_click(entity, msg))
        .relative()
        .pr(z(space::xl()))
        .map(|d| {
            let ephemeral = msg.flags & crate::model::FLAG_EPHEMERAL != 0;
            let (bg, hover, bar) = if ephemeral {
                (idx::BRAND_05A, idx::BRAND_10A, Some(idx::BRAND_500))
            } else if replying {
                (idx::MESSAGE_HIGHLIGHT_BACKGROUND_DEFAULT, idx::BRAND_10A, Some(idx::BRAND_500))
            } else if mentioned {
                (idx::MESSAGE_MENTIONED_BACKGROUND_DEFAULT, idx::MESSAGE_MENTIONED_BACKGROUND_HOVER, Some(idx::ICON_FEEDBACK_WARNING))
            } else {
                (usize::MAX, idx::MESSAGE_BACKGROUND_HOVER, None)
            };
            let selected = editing || app.chat.menu.is_some_and(|m| m.msg == id) || crate::shortcuts::message::focused().is_some_and(|f| f.1 == id);
            let d = match (bar, selected) {
                (_, true) => d.bg(tk(hover)),
                (Some(_), false) => d.bg(tk(bg)),
                (None, false) => d,
            };
            d.hover(move |s| s.bg(tk(hover))).when_some(bar, |d, b| {
                d.child(div().absolute().left_0().top_0().bottom_0().w(z(2.)).bg(tk(b)))
            })
            .when(app.chat.flash == Some(id), |d| d.child(flash_overlay(id)))
        });

    let body = if compact {
        base.pl(fz(80.))
            .py(fz(2.))
            .when_some(reply, |d, reply| d.child(reply))
            .child(
                div()
                    .absolute()
                    .left(z(compact_header_left))
                    .top(fz(if msg.reply_to.is_some() { 24. } else { 2. }))
                    .h(fz(22.))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .id(("msg-time", id.0))
                            .flex_none()
                            .mr(fz(4.))
                            .text_size(fz(11.))
                            .text_color(tk(idx::TEXT_MUTED))
                            .tooltip(time_tooltip(ts))
                            .child(r.time_short.clone()),
                    )
                    .when(compact_avatars, |d| {
                        d.child(
                            div()
                                .id(("msg-avatar-compact", id.0))
                                .flex_none()
                                .ml(fz(1.6))
                                .mr(fz(4.))
                                .cursor_pointer()
                                .rounded_full()
                                .on_click(message_ext::open_author(entity, msg))
            .on_mouse_down(gpui::MouseButton::Right, message_ext::open_author_menu(entity, msg))
                                .child(avatar(author, 16. * crate::theme::font_scale())),
                        )
                    })
                    .children(author_name)
                    .when_some(author.and_then(|u| u.tag.clone()), |d, tg| {
                        d.child(crate::ui::tag_popout::clickable(server_tag_chip(&tg, false), ("msg-tag-c", id.0), author.and_then(|u| u.tag_guild), &tg, entity.clone()))
                    })
                    .when_some(app.author_role_icon(msg.author), |d, u| d.child(role_icon(u)))
                    .when(app.is_new_member(msg.author), |d| d.child(new_member_badge(msg.id)))
                    .when(is_bot, |d| d.child(bot_tag(author.is_some_and(|u| u.verified_bot))))
                    .when(crate::ui::forum::is_op(app, msg.channel, msg.author), |d| d.child(crate::ui::forum::op_tag()))
                    .child(compact_header_probe(id)),
            )
            .child(content)
    } else {
        base.pl(z(msg_left()))
            .py(fz(2.))
            .when_some(reply, |d, reply| d.child(reply))
            .when(r.group_start, |d| {
                d.child(
                    div()
                        .absolute()
                        .left(if crate::theme::font_scaled_down() { fz(16.) } else { z(space::md()) })
                        .top({
                            let pad = if crate::theme::font_scaled_down() { 4. * crate::theme::font_scale() } else { 4. };
                            z(if msg.reply_to.is_some() { 18. * crate::theme::font_scale() + 4. + pad } else { pad })
                        })
                        .id(("msg-avatar", id.0))
                        .cursor_pointer()
                        .rounded_full()
                        .hover(|s| {
                            s.shadow(vec![gpui::BoxShadow {
                                color: gpui::hsla(0., 0., 0., 0.16),
                                offset: gpui::point(z(0.), z(4.)),
                                blur_radius: z(4.),
                                spread_radius: z(0.),
                                inset: false,
                            }])
                        })
                        .active(|s| s.mt(z(1.)))
                        .on_click(message_ext::open_author(entity, msg))
            .on_mouse_down(gpui::MouseButton::Right, message_ext::open_author_menu(entity, msg))
                        .child(crate::ui::avatar_hover(author, msg_avatar(), crate::ui::avatar_deco::plays(crate::ui::avatar_deco::hover_key(2, id.0))))
                        .when_some(author.and_then(|u| u.decoration.clone()), |d, deco| {
                            let s = msg_avatar() * 1.2;
                            let off = (s - msg_avatar()) / 2.;
                            d.child(
                                gpui::img(crate::ui::avatar_deco::src_for(&deco, crate::ui::avatar_deco::hover_key(2, id.0)))
                                    .id("deco-frames")
                                    .absolute()
                                    .size(z(s))
                                    .left(z(-off))
                                    .top(z(-off)),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .min_h(fz(22.))
                        .children(author_name)
                        .when_some(author.and_then(|u| u.tag.clone()), |d, tg| {
                            d.child(crate::ui::tag_popout::clickable(server_tag_chip(&tg, false), ("msg-tag", id.0), author.and_then(|u| u.tag_guild), &tg, entity.clone()))
                        })
                        .when_some(app.author_role_icon(msg.author), |d, u| d.child(role_icon(u)))
                    .when(app.is_new_member(msg.author), |d| d.child(new_member_badge(msg.id)))
                        .when(is_bot, |d| d.child(bot_tag(author.is_some_and(|u| u.verified_bot))))
                        .when(crate::ui::forum::is_op(app, msg.channel, msg.author), |d| d.child(crate::ui::forum::op_tag()))
                        .child(
                            div()
                                .id(("msg-time", id.0))
                                .ml(fz(4.))
                                .h(fz(22.))
                                .flex()
                                .items_center()
                                .flex_none()
                                .text_size(fz(12.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tk(idx::CHAT_TEXT_MUTED))
                                .tooltip(time_tooltip(ts))
                                .child(r.time_full.clone()),
                        ),
                )
            })
            .when(!r.group_start, |d| {
                d.child(
                    div()
                        .id(("msg-time", id.0))
                        .absolute()
                        .left_0()
                        .top(fz(2.))
                        .w(if crate::theme::font_scaled_down() { fz(56.) } else { z(56.) })
                        .h(fz(22.))
                        .flex()
                        .items_center()
                        .justify_end()
                        .text_size(fz(11.))
                        .text_color(gpui::transparent_black())
                        .group_hover("msg", |s| s.text_color(tk(idx::TEXT_MUTED)))
                        .tooltip(time_tooltip(ts))
                        .child(r.time_short.clone()),
                )
            })
            .child(content)
    }
    .when(!compact && msg.thread.is_some(), |d| d.child(message_ext::thread_spine(if msg.reply_to.is_some() { 70. } else { 48. })))
    .when(show_toolbar, |d| d.child(message_ext::toolbar(app, entity, msg, app.chat.shift_held)));
    let new_marker = app.chat.unread_marker.is_some_and(|(_, m)| m == id);
    let gap = if !r.group_start { 0. } else if compact { 4. } else { 17. * crate::theme::font_scale() };
    row_wrapper()
        .when_some(r.day_divider.clone(), |d, label| d.child(dated_divider(label, new_marker)))
        .when(new_marker && r.day_divider.is_none(), |d| d.child(message_ext::new_divider(r.group_start)))
        .child(div().pt(z(gap)).child(body))
        .into_any_element()
}

fn row_wrapper() -> gpui::Div {
    div().w_full().flex().flex_col()
}

fn text_width(cx: &App, font: &gpui::Font, size: gpui::Pixels, s: &str) -> f32 {
    let ts = cx.text_system();
    let id = ts.resolve_font(font);
    s.chars().map(|ch| ts.advance(id, size, ch).map(|a| f32::from(a.width)).unwrap_or(0.)).sum()
}

thread_local! {
    static COMPACT_HEADER_W: std::cell::RefCell<std::collections::HashMap<u64, f32>> = std::cell::RefCell::new(Default::default());
}

fn compact_header_probe(id: Id) -> impl IntoElement {
    gpui::canvas(
        move |b, w, _| {
            let nw = f32::from(b.size.width);
            let old = COMPACT_HEADER_W.with_borrow_mut(|m| m.insert(id.0, nw));
            if old.is_none_or(|o| (o - nw).abs() > 0.5) {
                w.on_next_frame(|w, _| w.refresh());
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn compact_header_measured(id: Id, left: f32) -> Option<f32> {
    let w = COMPACT_HEADER_W.with_borrow(|m| m.get(&id.0).copied())?;
    Some((f32::from(z(left)) + w + f32::from(z(4.)) - f32::from(fz(80.))).max(0.))
}

fn compact_header_overflow(time: &str, name: &str, compact_avatars: bool, bot: bool, tag: Option<&str>, left: f32, cx: &App) -> f32 {
    let mut name_font = crate::richtext::ui_font();
    name_font.weight = FontWeight::MEDIUM;
    let mut w = text_width(cx, &crate::richtext::ui_font(), fz(11.), time) + f32::from(fz(4.));
    if compact_avatars {
        w += f32::from(fz(16. + 1.6 + 4.));
    }
    w += text_width(cx, &name_font, fz(16.), name);
    if bot {
        let mut f = name_font.clone();
        f.weight = FontWeight::SEMIBOLD;
        w += text_width(cx, &f, z(12.8), "APP") + f32::from(z(12.8));
    }
    if let Some(tg) = tag {
        let mut f = name_font.clone();
        f.weight = FontWeight::SEMIBOLD;
        w += text_width(cx, &f, fz(11.52), tg) + f32::from(z(14.));
    }
    w += f32::from(z(4.));
    (f32::from(z(left)) + w - f32::from(fz(80.))).max(0.)
}

fn indent_rich(t: &RichText, width: f32, cx: &App) -> RichText {
    if width <= 0. {
        return t.clone();
    }
    const SP: char = '\u{a0}';
    let font = crate::richtext::ui_font();
    let mut wide = font.clone();
    wide.style = gpui::FontStyle::Italic;
    let a = text_width(cx, &font, fz(16.), "\u{a0}");
    let b = text_width(cx, &wide, fz(16.), "\u{a0}");
    if a <= 0. {
        return t.clone();
    }
    let (regular, italic) = indent_split(width, a, b);
    let prefix: String = std::iter::repeat_n(SP, regular + italic).collect();
    let shift = prefix.len();
    let cut = regular * SP.len_utf8();
    let run = |len: usize, font: gpui::Font| gpui::TextRun { len, font, color: gpui::transparent_black(), background_color: None, underline: None, strikethrough: None };
    let mut out = t.clone();
    out.text = format!("{prefix}{}", t.text).into();
    if italic > 0 {
        out.runs.insert(0, run(shift - cut, wide));
    }
    if regular > 0 {
        out.runs.insert(0, run(cut, font));
    }
    for r in &mut out.click_ranges {
        *r = r.start + shift..r.end + shift;
    }
    for (r, _) in &mut out.emoji {
        *r = r.start + shift..r.end + shift;
    }
    out
}

fn indent_split(width: f32, a: f32, b: f32) -> (usize, usize) {
    let n = (width / a).floor().max(0.);
    let rem = width - n * a;
    let d = b - a;
    let k = if d > 0.01 { (rem / d).round().min(n) } else { 0. };
    let err = (rem - k * d).abs();
    if err > (a - rem).abs() {
        return ((n as usize) + 1, 0);
    }
    ((n - k) as usize, k as usize)
}

fn edited_marker(ts: i64) -> impl IntoElement {
    div()
        .id(("edited", ts as u64))
        .tooltip(time_tooltip(ts))
        .text_size(z(10.))
        .line_height(z(10.))
        .font_weight(FontWeight::NORMAL)
        .text_color(tk(idx::TEXT_MUTED))
        .child(crate::t!("chat.edited"))
}

fn with_edited(t: &RichText) -> (SharedString, Vec<gpui::TextRun>) {
    let mut text = t.text.to_string();
    text.push_str(&crate::t!("chat.edited_suffix"));
    let mut runs = t.runs.clone();
    runs.push(gpui::TextRun {
        len: crate::t!("chat.edited_suffix").len(),
        font: edited_font(),
        color: gpui::transparent_black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    });
    (text.into(), runs)
}

fn edited_font() -> gpui::Font {
    let mut f = crate::richtext::ui_font();
    f.weight = FontWeight::NORMAL;
    f
}

fn edited_gap() -> usize {
    let s = crate::t!("chat.edited_suffix");
    s.len() - s.trim_start().len()
}

fn edited_overlay(layout: gpui::TextLayout, at: usize) -> impl IntoElement {
    gpui::canvas(
        |_, _, _| (),
        move |_, _, window, cx| {
            let (Some(here), Some(next)) = (layout.position_for_index(at), layout.position_for_index(at + 1)) else {
                return;
            };
            let origin = if next.y > here.y { gpui::point(layout.bounds().origin.x, next.y) } else { here };
            let lh = layout.line_height();
            let text_size = window.text_style().font_size.to_pixels(window.rem_size());
            let size = z(10.);
            let font = edited_font();
            let label = crate::t!("chat.edited");
            let run = gpui::TextRun { len: label.len(), font: font.clone(), color: c(Palette::TEXT_MUTED), background_color: None, underline: None, strikethrough: None };
            let ts = window.text_system().clone();
            let id = ts.resolve_font(&font);
            let baseline = |s: gpui::Pixels| {
                let (a, d) = (ts.ascent(id, s).abs(), ts.descent(id, s).abs());
                (lh - a - d) / 2. + a
            };
            let dy = baseline(text_size) - baseline(size);
            let line = ts.shape_line(label, size, &[run], None);
            let _ = line.paint(gpui::point(origin.x, origin.y + dy), lh, gpui::TextAlign::Left, None, window, cx);
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_0()
}

fn rich(entity: &Entity<PhoenixApp>, msg: Id, bi: usize, t: &RichText, edited: Option<i64>) -> AnyElement {
    rich_k(entity, msg, bi, t, edited, 1.)
}

fn rich_k(entity: &Entity<PhoenixApp>, msg: Id, bi: usize, t: &RichText, edited: Option<i64>, k: f32) -> AnyElement {
    if !t.emoji.is_empty() || t.has_mention() || message_ext::has_code(t) {
        let el = message_ext::rich_with_emoji_scaled(entity, msg, bi, t, k);
        if let Some(at) = edited {
            return div()
                .flex()
                .flex_wrap()
                .items_end()
                .child(div().max_w_full().child(el))
                .child(div().ml(z(4.)).mb(z(5.)).child(edited_marker(at)))
                .into_any_element();
        }
        return el;
    }
    let (text, runs) = if edited.is_some() { with_edited(t) } else { (t.text.clone(), t.runs.clone()) };
    let runs = mention::hovered_runs(msg, bi, 0, &t.click_ranges, &t.click_targets, &runs).unwrap_or(runs);
    let runs = mention::anchor_runs(msg, bi, 0, &runs).unwrap_or(runs);
    selection::note(msg.0, bi, t.text.clone(), &[]);
    let runs = selection::highlight(msg.0, bi, 0, t.text.len(), runs);
    let styled = StyledText::new(text).with_runs(runs);
    let probe = selection::probe(styled.layout().clone(), msg.0, bi, 0, t.text.len());
    let overlay = edited.map(|_| edited_overlay(styled.layout().clone(), t.text.len() + edited_gap()));
    let lh = gpui::relative(1.375);
    let edited_tip = edited.map(|at| {
        let from = t.text.len();
        let tip = std::rc::Rc::new(time_tooltip(at));
        move |ix: usize, w: &mut Window, cx: &mut App| (ix >= from).then(|| tip(w, cx))
    });
    if t.click_ranges.is_empty() {
        return match edited_tip {
            Some(tip) => div()
                .relative()
                .line_height(lh)
                .child(InteractiveText::new(("rt", (msg.0 << 8) | bi as u64), styled).tooltip(tip))
                .child(probe)
                .children(overlay)
                .into_any_element(),
            None => div().line_height(lh).child(styled).child(probe).into_any_element(),
        };
    }
    let targets = t.click_targets.clone();
    let shown: Vec<String> = (0..t.click_ranges.len())
        .map(|k| {
            let r = t.click_ranges[k].clone();
            let r = mention::anchor_at(&t.click_ranges, &t.click_targets, r.start).map_or(r, |a| a.0);
            t.text.get(r).unwrap_or_default().to_owned()
        })
        .collect();
    let link_tip = {
        let (hr, ht, text, le) = (t.click_ranges.clone(), t.click_targets.clone(), t.text.clone(), entity.clone());
        move |ix: usize, cx: &gpui::App| mention::anchor_at(&hr, &ht, ix).map(|(r, url)| mention::link_tip(&le, text.get(r).unwrap_or_default(), &url, cx))
    };
    let e = entity.clone();
    let n = t.text.len();
    div()
        .id(("rt-leave", (msg.0 << 8) | bi as u64))
        .on_hover(move |over, w, _| {
            if !*over {
                mention::leave(msg, bi, Some(0), 0..n, w);
            }
        })
        .relative()
        .line_height(lh)
        .on_mouse_down(gpui::MouseButton::Right, {
            let re = entity.clone();
            move |ev, w, cx| {
                if let Some(u) = mention::hovered_user(msg, bi, 0) {
                    cx.stop_propagation();
                    let g = re.read(cx).selected_guild;
                    crate::ui::user_menu::open(Id(u), g, ev.position, w);
                } else if let Some(url) = mention::hovered_link(msg, bi, 0) {
                    panels::set_menu_link(url);
                }
            }
        })
        .child(
            InteractiveText::new(("rt", (msg.0 << 8) | bi as u64), styled)
                .on_hover({
                    let (hr, ht, n) = (t.click_ranges.clone(), t.click_targets.clone(), t.text.len());
                    move |ix, _ev, w, _cx| {
                        mention::on_hover(msg, bi, 0, &hr, &ht, ix, w);
                        mention::set_anchor(msg, bi, 0..n, ix.and_then(|i| mention::anchor_at(&hr, &ht, i)).map(|a| a.0), w);
                    }
                })
                .tooltip(move |ix, w, cx| match link_tip(ix, cx) {
                    Some(title) => Some(crate::ui::tooltip::tip(title)(w, cx)),
                    None => edited_tip.as_ref().and_then(|tip| tip(ix, w, cx)),
                })
                .on_click(
                t.click_ranges.clone(),
                move |ix, w, cx| match &targets[ix] {
                    ClickTarget::Link(url) => e.update(cx, |a, cx| crate::ui::modals::leave_link::click(a, &shown[ix], url, w, cx)),
                    ClickTarget::Spoiler(sp) => {
                        let sp = *sp;
                        e.update(cx, |a, cx| a.reveal_spoiler(msg, sp, cx));
                    }
                    ClickTarget::Mention(m) => mention::click(&e, m, w, cx),
                },
            ),
        )
        .child(probe)
        .children(overlay)
        .into_any_element()
}

fn block(entity: &Entity<PhoenixApp>, msg: Id, bi: usize, b: &RBlock, edited: Option<i64>) -> AnyElement {
    match b {
        RBlock::Text(t) => rich(entity, msg, bi, t, edited),
        RBlock::Quote(t) => div()
            .flex()
            .my(z(4.))
            .max_w(gpui::relative(0.9))
            .child(div().w(z(4.)).rounded(z(4.)).bg(tk(idx::SPINE_DEFAULT)).flex_none())
            .child(div().flex_1().min_w_0().pl(z(12.)).pr(z(8.)).child(rich(entity, msg, bi, t, None)))
            .into_any_element(),
        RBlock::Heading(level, t) => {
            let size = match level {
                1 => 24.,
                2 => 20.,
                _ => 16.,
            };
            let first = if *level <= 2 { 8. } else { 4. };
            div()
                .mt(z(if bi == 0 { first } else { 16. }))
                .mb(z(8.))
                .text_size(fz(size))
                .child(rich_k(entity, msg, bi, t, edited, size / 16.))
                .into_any_element()
        }
        RBlock::Subtext(t) => div()
            .text_size(fz(14.))
            .line_height(fz(19.25))
            .text_color(tk(idx::TEXT_SUBTLE))
            .child(rich_k(entity, msg, bi, t, edited, 14. / 16.))
            .into_any_element(),
        RBlock::ListItem { ordered, depth, text } => {
            let marker: SharedString = match ordered {
                Some(n) => format!("{n}.").into(),
                None if *depth == 0 => "\u{2022}".into(),
                None => "\u{25E6}".into(),
            };
            div()
                .flex()
                .ml(z(16. * (*depth as f32 + 1.) - 16.))
                .mb(z(4.))
                .child(
                    div()
                        .flex_none()
                        .w(z(if ordered.is_some() { 24. } else { 16. }))
                        .pr(z(4.))
                        .flex()
                        .justify_end()
                        .line_height(gpui::relative(1.375))
                        .child(marker),
                )
                .child(div().flex_1().min_w_0().child(rich(entity, msg, bi, text, edited)))
                .into_any_element()
        }
        RBlock::Code { code, runs, .. } => div()
            .mt(z(6.))
            .max_w(gpui::relative(0.9))
            .p(fz(7.))
            .rounded(z(4.))
            .bg(tk(idx::BACKGROUND_BASE_LOWER))
            .border_1()
            .border_color(tk(idx::BORDER_NORMAL))
            .font_family(FONT_MONO())
            .text_size(fz(14.))
            .line_height(fz(18.))
            .text_color(tk(idx::TEXT_CODE))
            .relative()
            .group(crate::ui::code_copy::GROUP)
            .child(StyledText::new(code.clone()).with_runs(runs.clone()))
            .child(crate::ui::code_copy::CodeCopy::new(code.clone()))
            .into_any_element(),
    }
}

fn media_image(url: &str, original: SharedString, filename: SharedString, size_o: (u32, u32), size: (f32, f32)) -> gpui::Stateful<gpui::Img> {
    media_image_with(url, original, filename, size_o, size, false)
}

pub(crate) fn media_image_with(
    url: &str,
    original: SharedString,
    filename: SharedString,
    (ow, oh): (u32, u32),
    (w, h): (f32, f32),
    still: bool,
) -> gpui::Stateful<gpui::Img> {
    let animated = media::is_animated(url) || media::is_animated(&filename) || media::is_animated(&original);
    let (width, height) = ((w * 2.) as u32, (h * 2.) as u32);
    let (src, fallbacks) = if animated {
        let mut sources = media::gif_image_sources(url, &original, width, height, !still).into_iter();
        (sources.next().expect("GIF preview source"), sources.collect())
    } else {
        (media::resized(url, width, height), Vec::new())
    };
    let viewer = media::ViewerImage {
        url: if animated { media::gif_preview(url, ow.min(2048), oh.min(2048), true) } else { media::resized(url, ow.min(2048), oh.min(2048)) },
        original: if original.is_empty() { url.to_owned().into() } else { original },
        filename,
        width: ow,
        height: oh,
        author: media::current_author(),
        video: None,
    };
    let mut im = img(src.clone());
    if let Some(cache) = media::lru::chat() {
        im = im.image_cache(&cache);
    }
    im.id(gpui::ElementId::Name(src))
        .w(z(w))
        .max_w_full()
        .aspect_ratio(w / h.max(1.))
        .rounded(z(8.))
        .object_fit(if ow == 0 || oh == 0 { gpui::ObjectFit::Contain } else { gpui::ObjectFit::Cover })
        .cursor_pointer()
        .with_loading(|| div().size_full().rounded(z(8.)).bg(tk(idx::BACKGROUND_MOD_NORMAL)).into_any_element())
        .with_fallback(move || gif_image_fallback(&fallbacks, animated))
        .on_click(move |_, _, cx| media::open_viewer(viewer.clone(), cx))
}

fn gif_image_fallback(sources: &[SharedString], animated: bool) -> AnyElement {
    let Some((src, remaining)) = sources.split_first() else {
        return div()
            .size_full()
            .rounded(z(8.))
            .bg(tk(idx::BACKGROUND_MOD_NORMAL))
            .when(animated, |d| d.flex().items_center().justify_center().text_size(z(13.)).text_color(tk(idx::TEXT_MUTED)).child(crate::t!("picker.gifs_unavailable")))
            .into_any_element();
    };
    let mut image = img(src.clone());
    if let Some(cache) = media::lru::chat() {
        image = image.image_cache(&cache);
    }
    let remaining = remaining.to_vec();
    image.id(media::anim_id(src))
        .size_full()
        .rounded(z(8.))
        .with_fallback(move || gif_image_fallback(&remaining, animated))
        .into_any_element()
}

pub(crate) fn spoiler_pill(group: SharedString) -> gpui::Div {
    div()
        .px(z(12.))
        .py(z(8.))
        .rounded(z(20.))
        .bg(tk(idx::SPOILER_WARNING_BACKGROUND_COLOR))
        .text_size(z(15.))
        .line_height(z(15.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(tk(idx::SPOILER_WARNING_TEXT_COLOR))
        .group_hover(group, |s| s.bg(tk(idx::SPOILER_WARNING_BACKGROUND_COLOR_HOVER)).text_color(tk(idx::SPOILER_WARNING_TEXT_COLOR_HOVER)))
        .child(letter_spaced(&crate::t!("chat.spoiler").to_uppercase(), 0.5))
}

pub(crate) fn letter_spaced(text: &str, spacing: f32) -> gpui::Div {
    div().flex().children(text.chars().map(|ch| div().flex_none().mr(z(spacing)).child(SharedString::from(ch.to_string()))))
}

pub(crate) fn spoiler_blur(url: &str, video: bool, (w, h): (f32, f32)) -> SharedString {
    let (tw, th) = (((w / 24.) as u32).max(2), ((h / 24.) as u32).max(2));
    if video { media::video_poster(url, tw, th) } else { media::resized(url, tw, th) }
}

pub(crate) fn spoiler_cover(key: SharedString, url: &str, video: bool, (w, h): (f32, f32), cx: &App) -> Option<AnyElement> {
    if media::is_revealed(&key, cx) {
        return None;
    }
    let tiny = spoiler_blur(url, video, (w, h));
    Some(
        div()
            .id(gpui::ElementId::Name(format!("spoiler-{key}").into()))
            .relative()
            .w(z(w))
            .h(z(h))
            .rounded(z(8.))
            .overflow_hidden()
            .cursor_pointer()
            .bg(tk(idx::SPOILER_BACKGROUND_COLOR_HIDDEN))
            .group("spoiler-cover")
            .child(img(tiny).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover))
            .child(div().absolute().inset_0().flex().items_center().justify_center().child(spoiler_pill("spoiler-cover".into())))
            .on_click(move |_, _, cx| media::reveal(key.clone(), cx))
            .into_any_element(),
    )
}

fn still_sticker(u: SharedString, fmt: media::StickerFormat) -> SharedString {
    let animated = matches!(fmt, media::StickerFormat::Apng | media::StickerFormat::Gif);
    if !animated || media::sticker_plays(&u) || !u.contains("media.discordapp.net") {
        return u;
    }
    if u.contains("&animated=true") {
        return u.replace("&animated=true", "").into();
    }
    let sep = if u.contains('?') { '&' } else { '?' };
    format!("{u}{sep}passthrough=false").into()
}

pub(crate) fn attachment(a: &Attachment, cx: &App) -> AnyElement {
    let el = attachment_inner(a, cx);
    match (&a.kind, panels::sticker_id(&a.url)) {
        (AttachmentKind::Sticker(_), Some(id)) => {
            let t = panels::MenuItemTarget { sticker: true, id, image: Some(a.url.clone()), fav: None };
            div().on_mouse_down(gpui::MouseButton::Right, move |_, _, _| panels::set_menu_item(t.clone())).child(el).into_any_element()
        }
        _ => el,
    }
}

fn attachment_inner(a: &Attachment, cx: &App) -> AnyElement {
    let remote = attach::loadable(&a.url);
    let spoiler = a.is_spoiler();
    let inline = crate::ui::settings::effects::inline_attachments();
    match a.kind {
        AttachmentKind::Sticker(media::StickerFormat::Lottie) if !a.url.is_empty() => {
            let name = a.filename.clone();
            let im = media::lottie::image(a.url.clone(), 200, !media::sticker_plays(&a.url))
                .id(gpui::ElementId::Name(a.url.clone()))
                .mt(z(4.))
                .size(z(160.))
                .object_fit(gpui::ObjectFit::Contain)
                .with_fallback(move || sticker_name(name.clone()));
            return media::sticker_hover(a.url.clone(), im).into_any_element();
        }
        AttachmentKind::Sticker(fmt) => {
            let name = a.filename.clone();
            let animated = matches!(fmt, media::StickerFormat::Apng | media::StickerFormat::Gif);
            return match media::sticker_url(0, fmt).and(Some(a.url.clone())).filter(|u| !u.is_empty()) {
                Some(u) => {
                    let im = img(still_sticker(u.clone(), fmt))
                        .id(media::anim_id(&u))
                        .mt(z(4.))
                        .size(z(160.))
                        .object_fit(gpui::ObjectFit::Contain)
                        .with_fallback(move || sticker_name(name.clone()));
                    if animated { media::sticker_hover(u, im).into_any_element() } else { im.into_any_element() }
                }
                None => div().mt(z(4.)).child(sticker_name(name)).into_any_element(),
            };
        }
        AttachmentKind::Image | AttachmentKind::Video if remote && inline => {
            let size = media::fit(a.width, a.height, media::MAX_W, media::MAX_H);
            if spoiler && let Some(cover) = spoiler_cover(a.url.clone(), &a.url, a.kind == AttachmentKind::Video, size, cx) {
                return div().mt(z(4.)).child(cover).into_any_element();
            }
            if a.kind == AttachmentKind::Video {
                let alt = alt_text::description(a, false);
                return div()
                    .mt(z(4.))
                    .w(z(size.0))
                    .max_w_full()
                    .relative()
                    .child(attach::video_card(a, size, cx))
                    .children(alt.map(|t| alt_text::video_badge(a.url.clone(), t, attach::video_group(&a.url))))
                    .into_any_element();
            }
            let link = if a.original.is_empty() { a.url.clone() } else { a.original.clone() };
            let alt = alt_text::description(a, false);
            return div()
                .mt(z(4.))
                .w(z(size.0))
                .max_w_full()
                .relative()
                .on_mouse_down(gpui::MouseButton::Right, move |_, _, _| panels::set_menu_media(link.clone()))
                .child({
                    let gif = media::is_animated(&a.filename) || media::is_animated(&a.url) || media::is_animated(&a.original);
                    let still = gif && !media::gif_plays(&a.url);
                    let im = media_image_with(&a.url, a.original.clone(), a.filename.clone(), (a.width, a.height), size, still);
                    if gif { media::gif_hover(a.url.clone(), im).into_any_element() } else { im.into_any_element() }
                })
                .children(alt.map(|t| alt_text::badge(a.url.clone(), t)))
                .into_any_element();
        }
        AttachmentKind::Audio => {
            if let Some(meta) = a.voice.as_ref() {
                return voice_message::player(&a.url, meta, cx);
            }
            return div().mt(z(4.)).child(attach::audio_card(a, cx)).into_any_element();
        }
        AttachmentKind::File if a.url.starts_with("https://") && inline && plaintext::previewable(&a.filename) => {
            if let Some(el) = plaintext::render(a, cx) {
                return el;
            }
        }
        _ => {}
    }
    if a.is_image && inline {
        let (w, h) = media::fit(a.width, a.height, media::MAX_W, media::MAX_H);
        return div()
            .mt(z(4.))
            .w(z(w))
            .max_w_full()
            .aspect_ratio(w / h.max(1.))
            .rounded(z(8.))
            .bg(c(Palette::BG_SECONDARY))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(z(4.))
            .child(icon(Icon::Image, 48., c(Palette::INTERACTIVE_MUTED)))
            .child(div().text_size(z(12.)).text_color(c(Palette::TEXT_MUTED)).child(a.filename.clone()))
            .into_any_element();
    }
    div().mt(z(4.)).child(attach::file_card(a)).into_any_element()
}

fn sticker_name(name: SharedString) -> AnyElement {
    div()
        .px(z(8.))
        .py(z(4.))
        .rounded(z(4.))
        .bg(c(Palette::BG_SECONDARY))
        .text_size(z(14.))
        .text_color(c(Palette::TEXT_MUTED))
        .child(crate::t!("chat.sticker", name = name))
        .into_any_element()
}

fn embed_blocks(app: &PhoenixApp, src: &str) -> std::rc::Rc<[RBlock]> {
    use std::{cell::RefCell, collections::HashMap, rc::Rc};
    type EmbedCache = (u32, HashMap<SharedString, Rc<[RBlock]>>);
    thread_local! {
        static CACHE: RefCell<EmbedCache> = RefCell::default();
    }
    CACHE.with_borrow_mut(|(generation, map)| {
        let g = crate::theme::style_generation();
        if *generation != g || map.len() >= 512 {
            *generation = g;
            map.clear();
        }
        if let Some(b) = map.get(src) {
            return b.clone();
        }
        let parsed = crate::markdown::parse_emoji_links(src, &app.resolver());
        let blocks: Rc<[RBlock]> = crate::richtext::render_blocks(&parsed, &|_| true, c(Palette::TEXT_NORMAL)).into();
        map.insert(SharedString::from(src.to_owned()), blocks.clone());
        blocks
    })
}

fn embed_markdown(app: &PhoenixApp, entity: &Entity<PhoenixApp>, src: &str, size: f32, sel: Option<(u64, usize)>) -> AnyElement {
    let blocks = embed_blocks(app, src);
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        src.hash(&mut h);
        Id(h.finish() | (1 << 63))
    };
    let mut d = div().flex().flex_col().text_size(fz(size)).line_height(z(18.)).min_w_0();
    for (bi, b) in blocks.iter().enumerate() {
        let block_sel = sel.map(|(m, base)| (m, base + bi));
        d = d.child(match b {
            RBlock::Code { code, runs, .. } => {
                let mut runs = runs.clone();
                if let Some((m, sb)) = block_sel {
                    selection::note(m, sb, code.clone(), &[]);
                    runs = selection::highlight(m, sb, 0, code.len(), runs);
                }
                let st = StyledText::new(code.clone()).with_runs(runs);
                let probe = block_sel.map(|(m, sb)| selection::probe(st.layout().clone(), m, sb, 0, code.len()));
                div()
                    .relative()
                    .my(z(4.))
                    .p(z(8.))
                    .rounded(z(4.))
                    .bg(c(Palette::BG_TERTIARY))
                    .font_family(FONT_MONO())
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .child(st)
                    .children(probe)
                    .into_any_element()
            }
            RBlock::Quote(t) => div()
                .flex()
                .child(div().w(z(4.)).rounded(z(4.)).bg(c(Palette::INTERACTIVE_MUTED)).flex_none())
                .child(div().pl(z(12.)).child(embed_text(entity, key, bi, t, EMBED_EMOJI, block_sel)))
                .into_any_element(),
            b => {
                let empty = RichText {
                    text: "".into(),
                    runs: vec![],
                    click_ranges: vec![],
                    click_targets: vec![],
                    emoji: vec![],
                };
                let t = b.text().unwrap_or(&empty);
                let emoji_px = embed_emoji_px(matches!(b, RBlock::Heading(..)), size);
                let el = embed_text(entity, key, bi, t, emoji_px, block_sel);
                match b {
                    RBlock::Heading(..) => el.font_weight(FontWeight::BOLD).text_size(z(size + 2.)),
                    RBlock::Subtext(_) => el.text_size(z(12.)).text_color(c(Palette::TEXT_MUTED)),
                    RBlock::ListItem { ordered, depth, .. } => {
                        let marker = ordered.map_or_else(|| "•".into(), |n| format!("{n}."));
                        div().flex().pl(z(16. * f32::from(*depth)))
                            .child(div().flex_none().pr(z(6.)).child(marker))
                            .child(el.flex_1().min_w_0())
                    }
                    _ => el,
                }
                .into_any_element()
            }
        });
    }
    d.into_any_element()
}

fn embed_link_text(entity: &Entity<PhoenixApp>, key: Id, bi: usize, t: &RichText, range: std::ops::Range<usize>, sel: Option<(u64, usize)>) -> AnyElement {
    let mut runs = message_ext::slice_runs(&t.runs, range.clone());
    if let Some((m, sb)) = sel {
        runs = selection::highlight(m, sb, range.start, range.len(), runs);
    }
    let text: SharedString = t.text[range.clone()].to_owned().into();
    let off = range.start;
    let mut ranges = Vec::new();
    let mut urls = Vec::new();
    for (cr, tg) in t.click_ranges.iter().zip(&t.click_targets) {
        if let ClickTarget::Link(url) = tg
            && cr.start < range.end
            && range.start < cr.end
        {
            ranges.push(cr.start.max(range.start) - off..cr.end.min(range.end) - off);
            urls.push(url.clone());
        }
    }
    if ranges.is_empty() {
        let st = StyledText::new(text).with_runs(runs);
        let probe = sel.map(|(m, sb)| selection::probe(st.layout().clone(), m, sb, range.start, range.len()));
        return div().relative().child(st).children(probe).into_any_element();
    }
    let runs = mention::anchor_runs(key, bi, off, &runs).unwrap_or(runs);
    let anchor = {
        let (fr, ft) = (t.click_ranges.clone(), t.click_targets.clone());
        move |ix: usize| mention::anchor_at(&fr, &ft, ix + off)
    };
    let full = t.text.clone();
    let shown: Vec<String> = ranges.iter().map(|r| anchor(r.start).map(|(a, _)| full.get(a).unwrap_or_default().to_owned()).unwrap_or_default()).collect();
    let hover = anchor.clone();
    let tip_full = full.clone();
    let te = entity.clone();
    let e = entity.clone();
    let id = key.0.wrapping_mul(31).wrapping_add(((bi as u64) << 32) | off as u64);
    let scope = range.clone();
    let styled = StyledText::new(text).with_runs(runs);
    let probe = sel.map(|(m, sb)| selection::probe(styled.layout().clone(), m, sb, range.start, range.len()));
    let text_el = InteractiveText::new(("embed-rt", id), styled)
        .on_hover(move |ix, _ev, w, _cx| mention::set_anchor(key, bi, range.clone(), ix.and_then(&hover).map(|a| a.0), w))
        .tooltip(move |ix, w, cx| {
            let (r, url) = anchor(ix)?;
            Some(crate::ui::tooltip::tip(mention::link_tip(&te, tip_full.get(r).unwrap_or_default(), &url, cx))(w, cx))
        })
        .on_click(ranges, move |ix, w, cx| {
            let url = urls[ix].clone();
            e.update(cx, |a, cx| crate::ui::modals::leave_link::click(a, &shown[ix], &url, w, cx));
        });
    div().relative().child(mention::leave_guard(("embed-rt-leave", id), key, bi, None, scope, text_el)).children(probe).into_any_element()
}

const EMBED_EMOJI: f32 = 18.;

fn embed_emoji_px(heading: bool, size: f32) -> f32 {
    if heading { (size + 2.) * 1.375 } else { EMBED_EMOJI }
}

fn embed_key(src: &str, salt: u64) -> Id {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut h);
    salt.hash(&mut h);
    Id(h.finish() | (1 << 63))
}

pub(crate) fn embed_title_rich(app: &PhoenixApp, src: &str, links: bool, base: gpui::Hsla) -> RichText {
    crate::ui::emoji_text::inline_rich(app, src, base, links, FontWeight::SEMIBOLD)
}

fn embed_title_el(entity: &Entity<PhoenixApp>, t: RichText, salt: u64, sel: Option<(u64, usize)>) -> AnyElement {
    if sel.is_none() && t.emoji.is_empty() && t.runs.len() <= 1 && t.click_ranges.is_empty() {
        return div().child(t.text).into_any_element();
    }
    embed_text(entity, embed_key(&t.text, salt), 0, &t, EMBED_EMOJI, sel).into_any_element()
}

fn embed_text(entity: &Entity<PhoenixApp>, key: Id, bi: usize, t: &RichText, emoji_px: f32, sel: Option<(u64, usize)>) -> gpui::Div {
    if let Some((m, sb)) = sel {
        selection::note(m, sb, t.text.clone(), &t.emoji);
    }
    if t.emoji.is_empty() {
        return div().child(embed_link_text(entity, key, bi, t, 0..t.text.len(), sel));
    }
    let mut row = div().min_w_0().max_w_full().flex().flex_wrap().items_center();
    let push = |mut row: gpui::Div, range: std::ops::Range<usize>| -> gpui::Div {
        let mut start = range.start;
        for (i, piece) in t.text[range.clone()].split('\n').enumerate() {
            let r = start..start + piece.len();
            if i > 0 {
                row = row.child(div().w_full().h_0());
            }
            for w in message_ext::words(&t.text, r.clone()) {
                row = row.child(div().flex_none().max_w_full().child(embed_link_text(entity, key, bi, t, w, sel)));
            }
            start = r.end + 1;
        }
        row
    };
    let mut last = 0;
    for (r, token) in &t.emoji {
        if r.start < last {
            continue;
        }
        row = push(row, last..r.start);
        row = row.child(
            div()
                .mx(z(1.))
                .when(sel.is_some_and(|(m, sb)| selection::covers(m, sb, r)), |d| d.bg(selection::selection_bg()))
                .child(crate::media::emoji::emoji_element(token, emoji_px)),
        );
        last = r.end;
    }
    push(row, last..t.text.len())
}

pub fn is_direct_video(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url).to_ascii_lowercase();
    url.starts_with("https://") && [".mp4", ".webm", ".mov", ".m4v"].iter().any(|x| path.ends_with(x))
}

fn direct_video(e: &Embed) -> Option<crate::model::EmbedMedia> {
    e.video.clone().filter(|v| e.kind.as_deref() == Some("video") && is_direct_video(&v.url) && video_player::supported())
}

fn embed_player(v: &crate::model::EmbedMedia, poster: SharedString, size: (f32, f32), cx: &App) -> AnyElement {
    div()
        .mt(z(8.))
        .w(z(size.0))
        .max_w_full()
        .aspect_ratio(size.0 / size.1.max(1.))
        .child(video_player::attachment(
            video_player::Spec {
                key: v.url.clone(),
                src: v.url.clone(),
                poster,
                layout: video_player::Layout::Single,
                portrait: v.height > v.width,
                width: size.0,
                radius: 4.,
                viewer: None,
            },
            cx,
        ))
        .into_any_element()
}

pub(crate) fn embed(app: &PhoenixApp, entity: &Entity<PhoenixApp>, e: &Embed, cx: &App) -> AnyElement {
    embed_for_msg(app, entity, e, None, cx)
}

fn embed_for_msg(app: &PhoenixApp, entity: &Entity<PhoenixApp>, e: &Embed, sel: Option<(u64, usize)>, cx: &App) -> AnyElement {
    let has_text = e.title.is_some() || e.description.is_some() || e.author.is_some() || !e.fields.is_empty();
    let media_only = e.kind.as_deref() == Some("gifv") || (e.kind.as_deref() == Some("image") && !has_text);
    if media_only && let Some(m) = e.thumbnail.as_ref().or(e.image.as_ref()) {
        let max_h = if e.kind.as_deref() == Some("gifv") { 300. } else { media::MAX_H };
        let size = media::fit(m.width, m.height, media::MAX_W, max_h);
        let original = e.url.clone().unwrap_or_else(|| m.url.clone());
        let gifv = e.kind.as_deref() == Some("gifv");
        if gifv
            && video_player::supported()
            && let Some(v) = e.video.as_ref().filter(|v| v.url.starts_with("https://"))
        {
            let poster = media::video_poster(&m.url, (size.0 * 2.) as u32, (size.1 * 2.) as u32);
            let plays = media::gif_plays(&original);
            return div()
                .mt(z(4.))
                .child(media::gif_hover(
                    original,
                    div()
                        .w(z(size.0))
                        .max_w_full()
                        .aspect_ratio(size.0 / size.1.max(1.))
                        .rounded(z(8.))
                        .overflow_hidden()
                        .child(video_player::gifv(v.url.clone(), v.url.clone(), poster, plays, cx)),
                ))
                .into_any_element();
        }
        let animated = (gifv && media::gif_plays(&original))
            .then(|| e.video.iter().chain(e.thumbnail.iter()).find_map(|v| media::gifv_animated(&v.url, size.0)))
            .flatten();
        let Some(src) = animated else {
            let still = media_image_with(&m.url, original.clone(), "".into(), (m.width, m.height), size, !media::gif_plays(&original));
            return div().mt(z(4.)).child(media::gif_hover(original, still)).into_any_element();
        };
        let still = media::video_poster(&m.url, (size.0 * 2.) as u32, (size.1 * 2.) as u32);
        let key = original.clone();
        return div()
            .mt(z(4.))
            .child(media::gif_hover(key, media_image(&src, original, "".into(), (m.width, m.height), size).with_loading(move || {
                let mut im = img(still.clone());
                if let Some(cache) = media::lru::chat() {
                    im = im.image_cache(&cache);
                }
                im.size_full()
                    .rounded(z(8.))
                    .object_fit(gpui::ObjectFit::Cover)
                    .with_loading(|| div().size_full().rounded(z(8.)).bg(c(Palette::BG_SECONDARY)).into_any_element())
                    .into_any_element()
            })))
            .into_any_element();
    }
    let side_thumb = e.image.is_none() && !matches!(e.kind.as_deref(), Some("video" | "article"));
    let thumb = e.thumbnail.clone().filter(|_| side_thumb);
    let big = e.image.clone().or_else(|| e.thumbnail.clone().filter(|_| !side_thumb));
    let gallery: Vec<crate::model::Attachment> = e
        .images
        .iter()
        .map(|m| crate::model::Attachment {
            filename: "image.png".into(),
            url: m.url.clone(),
            width: m.width,
            height: m.height,
            is_image: true,
            kind: crate::model::AttachmentKind::Image,
            ..Default::default()
        })
        .collect();
    let big = big.filter(|_| gallery.len() < 2);

    let per_row = if thumb.is_some() { 2 } else { 3 };
    let mut rows: Vec<Vec<&crate::model::EmbedField>> = Vec::new();
    for f in &e.fields {
        match rows.last_mut() {
            Some(r) if f.inline && r.len() < per_row && r.iter().all(|x| x.inline) => r.push(f),
            _ => rows.push(vec![f]),
        }
    }
    let fields = (!rows.is_empty()).then(|| {
        let mut grid = div().flex().flex_col().gap(z(8.));
        let mut fi = 0usize;
        for r in rows {
            let mut row = div().flex().gap(z(8.));
            for f in r {
                let name_sel = sel.map(|(m, b)| (m, b + 200 + fi * 20));
                let val_sel = sel.map(|(m, b)| (m, b + 200 + fi * 20 + 1));
                fi += 1;
                row = row.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .mb(z(2.))
                                .text_size(z(14.))
                                .line_height(z(18.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(tk(idx::TEXT_STRONG))
                                .child(embed_title_el(entity, embed_title_rich(app, &f.name, e.url.is_none(), tk(idx::TEXT_STRONG)), 1, name_sel)),
                        )
                        .child(embed_markdown(app, entity, &f.value, 14., val_sel)),
                );
            }
            grid = grid.child(row);
        }
        grid
    });

    let footer = (e.footer.is_some() || e.timestamp.is_some()).then(|| {
        let mut parts: Vec<String> = Vec::new();
        if let Some(f) = &e.footer
            && !f.text.is_empty()
        {
            parts.push(f.text.to_string());
        }
        if let Some(ts) = e.timestamp {
            parts.push(crate::app::format_time_full(ts).to_string());
        }
        div()
            .flex()
            .items_center()
            .gap(z(8.))
            .when_some(e.footer.as_ref().and_then(|f| f.icon.clone()), |d, u| {
                d.child(img(media::resized(&u, 40, 40)).size(z(20.)).rounded_full().flex_none())
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_wrap()
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .children(parts.into_iter().enumerate().flat_map(|(i, p)| {
                        let sep = (i > 0).then(|| div().mx(z(4.)).child("\u{2022}").into_any_element());
                        sep.into_iter().chain(std::iter::once(div().child(crate::ui::emoji_text::label(SharedString::from(p), 12.)).into_any_element()))
                    })),
            )
    });

    let author = e.author.clone().map(|a| {
        let url = a.url.clone();
        div()
            .id(gpui::ElementId::Name(format!("embed-author-{}", a.name).into()))
            .flex()
            .items_center()
            .gap(z(8.))
            .when_some(a.icon.clone(), |d, u| {
                d.child(img(media::resized(&u, 48, 48)).size(z(24.)).rounded_full().flex_none())
            })
            .child(
                div()
                    .text_size(z(14.))
                    .line_height(z(22.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(tk(idx::TEXT_STRONG))
                    .when(url.is_some(), |d| d.hover(|s| s.underline()))
                    .child(crate::ui::emoji_text::label(a.name.clone(), 14.)),
            )
            .when_some(url, |d, u| d.cursor_pointer().on_click(move |_, _, cx| cx.open_url(&u)))
    });

    let title = e.title.clone().map(|t| {
        let url = e.url.clone();
        let rich = embed_title_rich(app, &t, url.is_none(), if url.is_some() { tk(idx::TEXT_LINK) } else { tk(idx::TEXT_STRONG) });
        div()
            .id(gpui::ElementId::Name(format!("embed-title-{t}").into()))
            .text_size(z(16.))
            .line_height(z(22.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if url.is_some() { tk(idx::TEXT_LINK) } else { tk(idx::TEXT_STRONG) })
            .when(url.is_some(), |d| d.hover(|s| s.underline()))
            .child(embed_title_el(entity, rich, 0, sel.map(|(m, b)| (m, b + 1))))
            .when_some(url, |d, u| d.cursor_pointer().on_click(move |_, _, cx| cx.open_url(&u)))
    });

    div()
        .mt(z(4.))
        .max_w(z(embed_rules::max_width(e).map_or(521., |w| w.min(521.))))
        .flex()
        .rounded(z(4.))
        .overflow_hidden()
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_t_1()
        .border_r_1()
        .border_b_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .child(div().w(z(4.)).flex_none().bg(embed_rules::border_color(e).map(c).unwrap_or_else(|| tk(idx::BORDER_NORMAL))))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .pt(z(2.))
                .pr(z(16.))
                .pb(z(16.))
                .pl(z(12.))
                .flex()
                .gap(z(16.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(z(8.))
                        .pt(z(8.))
                        .when_some(e.provider.clone(), |d, p| {
                            d.child(div().text_size(z(12.)).line_height(z(16.)).text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT)).child(crate::ui::emoji_text::label(p, 12.)))
                        })
                        .children(author)
                        .children(title)
                        .when_some(e.description.clone().filter(|_| !matches!(e.kind.as_deref(), Some("image" | "video" | "gifv"))), |d, t| {
                            d.child(embed_markdown(app, entity, &t, 14., sel.map(|(m, b)| (m, b + 10))))
                        })
                        .children(fields)
                        .when_some(big, |d, m| {
                            let (mw, mh) = embed_rules::media_max(e);
                            let (sw, sh) = match e.video.as_ref().filter(|v| e.kind.as_deref() == Some("video") && v.width > 0 && v.height > 0) {
                                Some(v) => (v.width, v.height),
                                None => (m.width, m.height),
                            };
                            let size = media::fit(sw, sh, mw, mh);
                            let original = m.url.clone();
                            if let Some(v) = direct_video(e) {
                                let poster = media::video_poster(&m.url, (size.0 * 2.) as u32, (size.1 * 2.) as u32);
                                return d.child(embed_player(&v, poster, size, cx));
                            }
                            let video = (e.kind.as_deref() == Some("video")).then(|| e.url.clone().unwrap_or_else(|| m.url.clone()));
                            d.child(
                                div()
                                    .mt(z(8.))
                                    .relative()
                                    .child(media_image(&m.url, original, "".into(), (m.width, m.height), size).rounded(z(4.)))
                                    .children(video.map(attach::video_embed_actions)),
                            )
                        })
                        .when_some(direct_video(e).filter(|_| e.image.is_none() && e.thumbnail.is_none()), |d, v| {
                            let (mw, mh) = embed_rules::media_max(e);
                            let size = media::fit(v.width, v.height, mw, mh);
                            d.child(embed_player(&v, media::video_poster(&v.url, (size.0 * 2.) as u32, (size.1 * 2.) as u32), size, cx))
                        })
                        .when(gallery.len() >= 2, |d| {
                            let refs: Vec<&crate::model::Attachment> = gallery.iter().collect();
                            d.child(div().mt(z(4.)).child(mosaic::render(&refs, cx)))
                        })
                        .children(footer),
                )
                .when_some(thumb, |d, m| {
                    let size = media::fit(m.width, m.height, 80., 80.);
                    let original = m.url.clone();
                    d.child(
                        div()
                            .flex_none()
                            .pt(z(8.))
                            .child(media_image(&m.url, original, "".into(), (m.width, m.height), size).rounded(z(4.))),
                    )
                }),
        )
        .into_any_element()
}

pub(crate) fn reply_preview_text(app: &PhoenixApp, content: &SharedString) -> SharedString {
    let m = crate::model::Message { content: content.clone(), ..Default::default() };
    app.render_msg(&m, None).preview
}

fn flash_overlay(id: Id) -> AnyElement {
    use gpui::AnimationExt;
    const IN: f32 = 600.;
    const OUT_AT: f32 = 2000.;
    const TOTAL: f32 = 2600.;
    let ease = crate::ui::states::cubic_bezier(0.25, 0.1, 0.25, 1.0);
    div()
        .absolute()
        .inset_0()
        .bg(tk(idx::MESSAGE_HIGHLIGHT_BACKGROUND_DEFAULT))
        .child(div().absolute().left_0().top_0().bottom_0().w(z(2.)).bg(tk(idx::TEXT_BRAND)))
        .with_animation(
            ("msg-flash", id.0),
            gpui::Animation::new(std::time::Duration::from_millis(TOTAL as u64)),
            move |d, t| {
                let ms = t * TOTAL;
                let o = if ms < IN {
                    let p = ms / IN;
                    let k = ((p % 0.4) / 0.2).min(2.);
                    let x = if k <= 1. { 1. - k } else { k - 1. };
                    let eio = crate::ui::states::cubic_bezier(0.42, 0., 0.58, 1.);
                    if p >= 0.8 { 1. } else { eio(x) }
                } else if ms < OUT_AT {
                    1.
                } else {
                    1. - ease((ms - OUT_AT) / (TOTAL - OUT_AT))
                };
                d.opacity(o)
            },
        )
        .into_any_element()
}

#[cfg(test)]
mod embed_emoji_tests {
    #[test]
    fn embed_emoji_are_18px_except_in_headings() {
        assert_eq!(super::embed_emoji_px(false, 14.), 18.);
        assert_eq!(super::embed_emoji_px(true, 14.), 16. * 1.375);
    }
}

#[cfg(test)]
mod indent_tests {
    use super::indent_split;

    #[test]
    fn compact_indent_mixes_regular_and_italic_spaces_to_hit_the_width() {
        let (a, b) = (3.52, 3.84);
        for w in [0.5, 2., 10., 40., 57.3, 121.9] {
            let (r, i) = indent_split(w, a, b);
            let got = r as f32 * a + i as f32 * b;
            let bound = if w >= 40. { (b - a) / 2. + 1e-3 } else { a / 2. + 1e-3 };
            assert!((got - w).abs() <= bound, "{w}: {r}+{i} gives {got}");
        }
        assert_eq!(indent_split(0., a, b), (0, 0));
    }
}#[test]
    fn thread_sidebar_floats_when_the_chat_area_is_narrow() {
        let at = |w: f32| super::thread_sidebar_layout(false, gpui::px(w));
        assert_eq!(at(628.), (gpui::px(450.), true));
        assert_eq!(at(765.), (gpui::px(450.), true));
        assert_eq!(at(788.), (gpui::px(450.), false));
        assert_eq!(at(968.), (gpui::px(450.), false));
        assert_eq!(super::thread_sidebar_layout(true, gpui::px(300.)), (gpui::px(480.), false));
    }

    
