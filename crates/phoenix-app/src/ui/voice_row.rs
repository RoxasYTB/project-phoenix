use std::cell::RefCell;
use std::collections::HashMap;

use gpui::prelude::{FluentBuilder as _, StyledImage as _};
use gpui::{AnyElement, Bounds, InteractiveElement, IntoElement, ParentElement, Pixels, Point, SharedString, StatefulInteractiveElement, Styled, div, point, px, svg};

use crate::theme::{c, generated::idx, tok, z};

use crate::model::Id;

thread_local! {
    static ROWS: RefCell<HashMap<u64, Bounds<Pixels>>> = RefCell::new(HashMap::new());
}

thread_local! {
    static HIDDEN_NAMES: std::cell::RefCell<Option<std::collections::BTreeSet<u64>>> = const { std::cell::RefCell::new(None) };
}

fn hidden_path() -> Option<std::path::PathBuf> {
    crate::paths::config_file("voice-hidden-names.txt")
}

fn with_hidden<R>(f: impl FnOnce(&mut std::collections::BTreeSet<u64>) -> R) -> R {
    HIDDEN_NAMES.with(|h| {
        let mut h = h.borrow_mut();
        let set = h.get_or_insert_with(|| {
            hidden_path()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|s| s.split_whitespace().filter_map(|x| x.parse().ok()).collect())
                .unwrap_or_default()
        });
        f(set)
    })
}

pub fn names_hidden(channel: Id) -> bool {
    with_hidden(|h| h.contains(&channel.0))
}

pub fn set_names_hidden(channel: Id, hidden: bool) {
    let text = with_hidden(|h| {
        if hidden {
            h.insert(channel.0);
        } else {
            h.remove(&channel.0);
        }
        h.iter().map(u64::to_string).collect::<Vec<_>>().join(" ")
    });
    if let Some(p) = hidden_path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, text);
    }
}

pub fn collapsed_list(app: &crate::app::PhoenixApp, users: &[Id]) -> AnyElement {
    div()
        .w_full()
        .pl(z(38.))
        .pb(z(10.))
        .flex()
        .flex_row()
        .flex_wrap()
        .items_start()
        .children(users.iter().map(|u| {
            let speaking = app.voice.is_speaking(*u);
            div()
                .h(z(32.))
                .ml(z(8.))
                .flex()
                .items_center()
                .child(crate::ui::voice_panel::speaking_ring(crate::ui::voice_panel::speaking_avatar(app.users.get(u), 24., None, crate::ui::voice_panel::speaking_plays(speaking)), 24., speaking))
        }))
        .into_any_element()
}

pub fn record(user: Id, b: Bounds<Pixels>) {
    ROWS.with(|r| r.borrow_mut().insert(user.0, b));
}

pub fn popout_anchor(user: Id, fallback: Point<Pixels>) -> Point<Pixels> {
    let b = ROWS.with(|r| r.borrow().get(&user.0).copied());
    match b {
        Some(b) => point(b.right() + px(8. + 300. + 16.), b.top() + px(60.)),
        None => point(fallback.x + px(300. + 16. + 8.), fallback.y + px(60.)),
    }
}

pub fn options_button(user: Id, guild: Option<Id>, _in_call: bool) -> AnyElement {
    div()
        .id(("voice-user-options", user.0))
        .size(z(24.))
        .flex_none()
        .rounded(z(4.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_ACTIVE))))
        .child(
            svg()
                .path(crate::icons::path("more-horizontal"))
                .size(z(16.))
                .text_color(c(tok(idx::CHANNELS_DEFAULT))),
        )
        .on_click(move |ev, window, cx| {
            cx.stop_propagation();
            crate::ui::user_menu::open(user, guild, ev.position(), window);
        })
        .into_any_element()
}

pub fn voice_activity_card(app: &crate::app::PhoenixApp, user: Id, entity: gpui::Entity<crate::app::PhoenixApp>) -> Option<AnyElement> {
    let themed = crate::ui::profile_theme::active_scheme().is_some();
    let card = crate::ui::profile_voice::card(app, user, crate::ui::profile_voice::Colors::popout(themed), entity)?;
    Some(div().id(("profile-voice-card", user.0)).pb(z(8.)).child(card).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    #[test]
    fn anchor_is_right_of_row() {
        record(Id(5), Bounds::new(point(px(118.), px(290.)), size(px(289.), px(32.))));
        let a = popout_anchor(Id(5), point(px(0.), px(0.)));
        assert_eq!(a.x - px(316.), px(415.));
        assert_eq!(a.y - px(60.), px(290.));
    }

    #[test]
    fn preview_needs_connect_or_presence() {
        let connect = phoenix_api::models::Permissions::CONNECT.bits();
        let view = phoenix_api::models::Permissions::VIEW_CHANNEL.bits();
        assert!(can_fetch_preview(false, Some(connect | view)));
        assert!(!can_fetch_preview(false, Some(view)));
        assert!(!can_fetch_preview(false, None));
        assert!(can_fetch_preview(true, None));
        assert!(can_fetch_preview(true, Some(0)));
    }
}

pub fn stage_audience(app: &crate::app::PhoenixApp, channel: Id) -> AnyElement {
    use crate::theme::generated::global::SPACE_XS;
    let audience: Vec<Id> = app
        .channels
        .get(&channel)
        .map(|c| c.voice_users.iter().copied().filter(|u| crate::ui::voice_panel::flags(*u).suppress).collect())
        .unwrap_or_default();
    let shown = audience.len().min(3);
    let more = audience.len() - shown;
    let ring = |child: AnyElement| {
        div()
            .ml(z(-8.))
            .rounded_full()
            .border(z(4.))
            .border_color(c(tok(idx::BACKGROUND_BASE_LOWEST)))
            .child(child)
    };
    div()
        .w_full()
        .pl(z(38.))
        .pb(z(10.))
        .child(
            div()
                .mt(z(4.))
                .h(z(24.))
                .pl(z(SPACE_XS))
                .flex()
                .flex_row()
                .items_center()
                .gap(z(8.))
                .child(
                    div()
                        .size(z(24.))
                        .flex_none()
                        .rounded_full()
                        .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(crate::ui::icon(crate::assets::Icon::Headphones, 16. * crate::theme::zoom(), c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .pl(z(8.))
                        .children(audience.iter().take(shown).map(|u| ring(crate::ui::avatar(app.users.get(u), 24.))))
                        .children((more > 0).then(|| {
                            div()
                                .ml(z(-8.))
                                .rounded(z(16.))
                                .border(z(4.))
                                .border_color(c(tok(idx::BACKGROUND_BASE_LOWEST)))
                                .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
                                .px(z(8.))
                                .py(z(3.))
                                .text_size(z(12.))
                                .line_height(z(16.))
                                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                                .child(format!("+{more}"))
                        })),
                ),
        )
        .into_any_element()
}

thread_local! {
    static STREAM_HOVER: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

pub fn set_stream_hover(user: Id, hovered: bool) -> bool {
    STREAM_HOVER.with(|h| {
        let before = h.get();
        if hovered {
            h.set(Some(user.0));
        } else if before == Some(user.0) {
            h.set(None);
        }
        before != h.get()
    })
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreviewEntry {
    pub url: Option<SharedString>,
    pub expires: Option<std::time::Instant>,
    pub tries: u32,
    pub loading: bool,
}

impl PreviewEntry {
    pub fn should_fetch(&self, now: std::time::Instant) -> bool {
        !self.loading && self.expires.is_none_or(|e| now > e) && (self.expires.is_some() || self.tries < 5)
    }

    pub fn start(&mut self) {
        self.tries += 1;
        self.loading = true;
    }

    pub fn finish(&mut self, url: Result<Option<SharedString>, ()>, now: std::time::Instant) {
        self.loading = false;
        match url {
            Ok(u) => {
                self.url = u;
                self.expires = Some(now + PREVIEW_TTL);
                self.tries = 0;
            }
            Err(()) => self.expires = Some(now + PREVIEW_RETRY * self.tries),
        }
    }
}

thread_local! {
    static PREVIEWS: RefCell<HashMap<String, PreviewEntry>> = RefCell::new(HashMap::new());
}

pub const PREVIEW_TTL: std::time::Duration = std::time::Duration::from_secs(120);
pub const PREVIEW_RETRY: std::time::Duration = std::time::Duration::from_secs(10);

pub fn can_fetch_preview(in_channel: bool, perms: Option<u64>) -> bool {
    in_channel || perms.is_some_and(|p| p & phoenix_api::models::Permissions::CONNECT.bits() != 0)
}

pub fn preview_allowed(app: &crate::app::PhoenixApp, key: &str) -> bool {
    let Some(k) = crate::voice::stream::parse_key(key) else { return false };
    let ch = Id(k.channel);
    can_fetch_preview(app.voice_channel == Some(ch), app.backend.channel_permissions(ch))
}

pub fn fetch_preview(app: &crate::app::PhoenixApp, key: String, cx: &mut gpui::App) {
    if !preview_allowed(app, &key) {
        return;
    }
    let now = std::time::Instant::now();
    let go = PREVIEWS.with(|p| {
        let mut p = p.borrow_mut();
        let e = p.entry(key.clone()).or_default();
        let go = e.should_fetch(now);
        if go {
            e.start();
        }
        go
    });
    if !go {
        return;
    }
    let fut = app.backend.stream_preview(key.clone());
    cx.spawn(async move |cx| {
        let res = fut.await.map(|u| u.map(SharedString::from)).map_err(|_| ());
        cx.update(|cx| {
            PREVIEWS.with(|p| p.borrow_mut().entry(key).or_default().finish(res, std::time::Instant::now()));
            cx.refresh_windows();
        });
    })
    .detach();
}

pub fn preview_url(key: &str) -> Option<SharedString> {
    PREVIEWS.with(|p| p.borrow().get(key).and_then(|e| e.url.clone()))
}

pub fn preview_loading(key: &str) -> bool {
    PREVIEWS.with(|p| p.borrow().get(key).is_some_and(|e| e.loading))
}

pub fn stream_preview(app: &crate::app::PhoenixApp, user: Id, entity: gpui::Entity<crate::app::PhoenixApp>) -> Option<AnyElement> {
    use gpui::{AnimationExt, FontWeight};
    if STREAM_HOVER.with(|h| h.get()) != Some(user.0) || !crate::ui::voice_panel::flags(user).stream {
        return None;
    }
    let b = ROWS.with(|r| r.borrow().get(&user.0).copied())?;
    let key = crate::ui::call_view::stream_key_in(app, user)?;
    let url = preview_url(&key);
    let badge = div()
        .h(z(16.))
        .px(z(8.))
        .rounded_full()
        .bg(c(tok(idx::BACKGROUND_FEEDBACK_NOTIFICATION)))
        .flex()
        .items_center()
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(tok(idx::WHITE)))
        .child(crate::t!("voice.live"));
    let watch_key = key.clone();
    let card = div()
        .id(("stream-preview", user.0))
        .occlude()
        .w(z(280.))
        .pt(z(16.))
        .pr(z(8.))
        .pb(z(16.))
        .pl(z(16.))
        .rounded(z(12.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .shadow(crate::ui::inbox::popout_shadow())
        .flex()
        .flex_col()
        .child(
            div()
                .w(z(248.))
                .mb(z(10.))
                .flex()
                .justify_between()
                .items_center()
                .child(div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("voice.streaming")))
                .child(badge),
        )
        .child(
            div()
                .id(("stream-preview-img", user.0))
                .group("stream-preview-img")
                .relative()
                .w(z(248.))
                .h(z(142.))
                .rounded(z(8.))
                .overflow_hidden()
                .bg(c(tok(idx::BACKGROUND_SECONDARY_ALT)))
                .shadow(crate::ui::inbox::popout_shadow())
                .map(|d| match url {
                    Some(u) => d.child(gpui::img(u).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover)),
                    None => d.child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_center()
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .text_color(c(tok(idx::TEXT_DEFAULT)))
                            .child(crate::t!("voice.stream_just_started")),
                    ),
                })
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .bg(c(tok(idx::OPACITY_BLACK_60)))
                        .opacity(0.)
                        .group_hover("stream-preview-img", |s| s.opacity(1.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(z(18.))
                        .text_size(z(14.))
                        .text_color(c(tok(idx::WHITE)))
                        .child(crate::t!("voice.watch_stream")),
                ),
        )
        .child(
            div().pt(z(10.)).child(
                div()
                    .id(("stream-preview-watch", user.0))
                    .w(z(248.))
                    .h(z(32.))
                    .px(z(12.))
                    .rounded(z(8.))
                    .border_1()
                    .border_color(c(tok(idx::CONTROL_CONNECTED_BORDER_DEFAULT)))
                    .bg(c(tok(idx::CONTROL_CONNECTED_BACKGROUND_DEFAULT)))
                    .hover(|s| s.bg(c(tok(idx::CONTROL_CONNECTED_BACKGROUND_HOVER))))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(z(4.))
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(idx::CONTROL_CONNECTED_TEXT_DEFAULT)))
                    .child(svg().path(crate::icons::path("voice/screen")).size(z(16.)).text_color(c(tok(idx::CONTROL_CONNECTED_TEXT_DEFAULT))))
                    .child(crate::t!("voice.watch_stream"))
                    .on_click(move |_, _, cx| {
                        let k = watch_key.clone();
                        entity.update(cx, |a, cx| watch_from_sidebar(a, k, cx));
                    }),
            ),
        )
        .with_animation(
            ("stream-preview-in", user.0),
            gpui::Animation::new(std::time::Duration::from_millis(150)),
            |d, t| d.opacity(t).ml(z(10. * t)),
        );
    Some(
        crate::media::lru::deferred(gpui::anchored().position(point(b.right() + z(17.), b.top() - z(12.))).child(card))
            .with_priority(1)
            .into_any_element(),
    )
}

pub fn watch_from_sidebar(app: &mut crate::app::PhoenixApp, key: String, cx: &mut gpui::Context<crate::app::PhoenixApp>) {
    let Some(k) = crate::voice::stream::parse_key(&key) else { return };
    set_stream_hover(Id(k.owner), false);
    crate::ui::call_view::set_focus(crate::ui::call_view::Focus::Tile(Id(k.owner), true));
    if app.voice_channel != Some(Id(k.channel)) {
        drop(app.backend.voice_control(crate::voice::VoiceControl::WatchAfterJoin(key)));
        app.join_voice(Some(Id(k.channel)), cx);
    } else {
        drop(app.backend.voice_control(crate::voice::VoiceControl::WatchStream(key)));
    }
    cx.notify();
}

thread_local! {
    static STREAM_MENU: std::cell::Cell<Option<(gpui::Point<gpui::Pixels>, Id)>> = const { std::cell::Cell::new(None) };
}

pub fn open_activity_menu(pos: gpui::Point<gpui::Pixels>, user: Id, window: &mut gpui::Window) {
    STREAM_MENU.with(|m| m.set(Some((pos, user))));
    window.refresh();
}

pub fn stream_menu(entity: gpui::Entity<crate::app::PhoenixApp>) -> Option<AnyElement> {
    let (pos, user) = STREAM_MENU.with(|m| m.get())?;
    let close: crate::ui::context_menu::Dismiss = std::rc::Rc::new(|w: &mut gpui::Window, _: &mut gpui::App| {
        STREAM_MENU.with(|m| m.set(None));
        w.refresh();
    });
    let c2 = close.clone();
    let items = vec![crate::ui::context_menu::MenuItem::action(crate::t!("profile.view_all_activity"), None, move |_, w, cx| {
        c2(w, cx);
        entity.update(cx, |a, cx| a.open_profile_modal_at(user, 0, cx));
    })];
    Some(crate::ui::context_menu::render(pos, items, close))
}

pub fn stream_activity_card(
    app: &crate::app::PhoenixApp,
    user: Id,
    entity: gpui::Entity<crate::app::PhoenixApp>,
    cx: &mut gpui::App,
) -> Option<AnyElement> {
    stream_card_with_header(app, user, entity, false, cx)
}

pub fn stream_card_with_header(
    app: &crate::app::PhoenixApp,
    user: Id,
    entity: gpui::Entity<crate::app::PhoenixApp>,
    sidebar: bool,
    cx: &mut gpui::App,
) -> Option<AnyElement> {
    use gpui::FontWeight;
    let (header_px, header_fg) = if sidebar { (14., idx::TEXT_STRONG) } else { (12., idx::TEXT_DEFAULT) };
    if !crate::ui::voice_panel::flags(user).stream {
        return None;
    }
    let ch = app.channels.values().find(|c| c.voice_users.contains(&user))?;
    let (cid, cname) = (ch.id, ch.name.clone());
    let key = crate::ui::call_view::stream_key_in(app, user)?;
    fetch_preview(app, key.clone(), cx);
    let url = preview_url(&key);
    let g = ch.guild.and_then(|g| app.guilds.iter().find(|x| x.id == g));
    let acronym = g.map(|g| g.acronym.clone()).unwrap_or_default();
    let icon = g.and_then(|g| match &g.icon {
        crate::model::ImageRef::Url(u) => Some(u.clone()),
        _ => None,
    });
    let locked = app.nav.extras.get(&cid).is_some_and(|e| e.locked);
    let voice_icon = if locked { "voice-lock" } else { "speaker" };
    let people: Vec<Id> = ch.voice_users.iter().copied().take(3).collect();
    let shown = people.len();
    let subtle = c(tok(idx::TEXT_SUBTLE));
    let badge = div()
        .flex()
        .items_center()
        .h(z(16.))
        .px(z(8.))
        .rounded_full()
        .bg(c(tok(idx::BACKGROUND_FEEDBACK_NOTIFICATION)))
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(tok(idx::WHITE)))
        .child(crate::t!("voice.live"));
    let header = div()
        .flex()
        .justify_between()
        .gap(z(4.))
        .h(z(header_px + 4.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(4.))
                .min_w_0()
                .overflow_hidden()
                .text_size(z(header_px))
                .line_height(z(header_px + 4.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(header_fg)))
                .child(div().min_w_0().text_ellipsis().child(crate::t!("profile.live_card")))
                .child(badge),
        )
        .child(
            div()
                .id(("profile-stream-more", user.0))
                .flex()
                .mr(z(4.))
                .cursor_pointer()
                .tooltip(crate::ui::tooltip::tip(crate::t!("profile.more")))
                .on_click(move |ev: &gpui::ClickEvent, window, cx| {
                    cx.stop_propagation();
                    STREAM_MENU.with(|m| m.set(Some((ev.position(), user))));
                    window.refresh();
                })
                .child(svg().path(crate::icons::path("more-horizontal")).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
        );
    let watch_key = key.clone();
    let watch_entity = entity.clone();
    let preview = div()
        .id(("profile-stream-preview", user.0))
        .group("profile-stream-preview")
        .relative()
        .w_full()
        .h(z(252. * 9. / 16.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .rounded(z(8.))
        .overflow_hidden()
        .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
        .map(|d| match url {
            Some(u) => d.child(gpui::img(u).absolute().inset_0().size_full().rounded(z(8.)).object_fit(gpui::ObjectFit::Contain)),
            None => d.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_center()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                    .child(crate::t!("voice.stream_just_started")),
            ),
        })
        .child(
            div()
                .relative()
                .px(z(16.))
                .py(z(7.))
                .rounded(z(8.))
                .bg(c(tok(idx::OPACITY_BLACK_60)))
                .opacity(0.)
                .group_hover("profile-stream-preview", |s| s.opacity(1.))
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::WHITE)))
                .child(crate::t!("voice.watch_stream")),
        )
        .on_click(move |_, _, cx| {
            let k = watch_key.clone();
            watch_entity.update(cx, |a, cx| {
                a.people.popout = None;
                watch_from_sidebar(a, k, cx);
            });
        });
    let guild_icon = div()
        .relative()
        .size(z(16.))
        .flex_none()
        .rounded(z(4.))
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_center()
        .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
        .text_size(z(8.))
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .map(|d| match icon {
            Some(u) => d.child(gpui::img(u).absolute().inset_0().size_full()),
            None => d.child(acronym),
        });
    let open_entity = entity.clone();
    let channel_row = div()
        .flex()
        .items_center()
        .gap(z(4.))
        .h(z(16.))
        .child(guild_icon)
        .child(svg().path(crate::icons::path("chevron-right")).size(z(12.)).flex_none().text_color(subtle))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(z(2.))
                .child(svg().path(crate::icons::path(voice_icon)).size(z(12.)).flex_none().text_color(subtle))
                .child(
                    div()
                        .id(("profile-stream-channel", user.0))
                        .min_w_0()
                        .cursor_pointer()
                        .text_size(z(12.))
                        .line_height(z(16.))
                        .text_color(subtle)
                        .text_ellipsis()
                        .hover(|s| s.underline())
                        .child(cname)
                        .on_click(move |_, _, cx| {
                            open_entity.update(cx, |a, cx| {
                                a.people.popout = None;
                                a.open_channel_chat(cid, cx);
                            })
                        }),
                ),
        )
        .child(div().flex().flex_none().items_center().children(people.iter().enumerate().map(|(n, u)| {
            div().when(n + 1 < shown, |d| d.mr(z(-3.))).child(crate::ui::avatar(app.users.get(u), 16.))
        })));
    let join = crate::ui::profile::mana_button(("profile-stream-join", user.0), crate::t!("profile.join_voice"), false, false)
        .w_full()
        .line_height(z(18.))
        .on_click(move |_, _, cx| {
            entity.update(cx, |a, cx| {
                a.people.popout = None;
                a.join_voice(Some(cid), cx);
            })
        });
    Some(
        div()
            .pb(z(8.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(z(8.))
                    .p(z(8.))
                    .rounded(z(8.))
                    .bg(c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND_HOVER)))
                    .child(header)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(z(8.))
                            .child(preview)
                            .child(div().h(z(1.)).bg(c(tok(idx::USER_PROFILE_BORDER))))
                            .child(channel_row),
                    )
                    .child(div().mt(z(4.)).flex().items_center().child(join)),
            )
            .into_any_element(),
    )
}

#[cfg(test)]
mod preview_cache_tests {
    use super::{PREVIEW_RETRY, PREVIEW_TTL, PreviewEntry};

    #[test]
    fn a_preview_is_kept_two_minutes_then_fetched_again() {
        let t = std::time::Instant::now();
        let mut e = PreviewEntry::default();
        assert!(e.should_fetch(t));
        e.start();
        assert!(!e.should_fetch(t));
        e.finish(Ok(Some("https://x/p.jpeg".into())), t);
        assert_eq!(e.tries, 0);
        assert!(!e.should_fetch(t + PREVIEW_TTL));
        assert!(e.should_fetch(t + PREVIEW_TTL + std::time::Duration::from_millis(1)));
    }

    #[test]
    fn a_failed_preview_retries_after_ten_seconds_per_attempt_and_keeps_the_old_image() {
        let t = std::time::Instant::now();
        let mut e = PreviewEntry { url: Some("old".into()), ..Default::default() };
        e.start();
        e.finish(Err(()), t);
        assert_eq!(e.url.as_deref(), Some("old"));
        assert!(!e.should_fetch(t + PREVIEW_RETRY));
        assert!(e.should_fetch(t + PREVIEW_RETRY + std::time::Duration::from_millis(1)));
        e.start();
        e.finish(Err(()), t);
        assert!(!e.should_fetch(t + PREVIEW_RETRY * 2));
    }
}
