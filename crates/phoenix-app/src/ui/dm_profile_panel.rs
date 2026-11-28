use std::cell::Cell;
use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, img, prelude::*, svg};

use crate::{
    app::PhoenixApp,
    model::Id,
    presence::{Activity, ActivityKind, ProfileRequestState, format_date},
    theme::{Palette, c, generated::idx, tok, z},
    ui::home::data::{RelAction, RelationshipKind},
    ui::profile::{Info, activity_header, avatar_block, banner, banner_buttons, ensure_avatar_colour, info, mana_button, mutuals_row, name_block_clickable, section_title, theme_backdrop},
};

const PROFILE_RETRY_DELAY: Duration = Duration::from_secs(30);
const HIDDEN_PREVIEW: &str = "https://discord.com/assets/5682f76b7c3741bd.svg";
const ACTIVITY_TAB: u8 = 0;
const PEEK: f32 = 8.;
const LIFT: f32 = 2.;

thread_local! {
    static REVEALED: Cell<Option<Id>> = const { Cell::new(None) };
    static TICK_WANTED: Cell<Option<Instant>> = const { Cell::new(None) };
    static TICKING: Cell<bool> = const { Cell::new(false) };
}

pub fn is_group_dm(app: &PhoenixApp, channel: Id) -> bool {
    app.nav.extras.get(&channel).is_some_and(|e| e.recipients > 1 || e.group_icon.is_some())
}

pub fn friend_nickname(app: &PhoenixApp, ch: &crate::model::Channel) -> Option<gpui::SharedString> {
    if ch.guild.is_some() || is_group_dm(app, ch.id) {
        return None;
    }
    let peer = ch.voice_users.first()?;
    app.nav.relationship(*peer).and_then(|r| r.nickname.clone()).filter(|n| !n.is_empty())
}

pub fn channel_name(app: &PhoenixApp, ch: &crate::model::Channel) -> gpui::SharedString {
    friend_nickname(app, ch).unwrap_or_else(|| ch.name.clone())
}

pub fn peer(app: &PhoenixApp) -> Option<Id> {
    let ch = app.selected_channel.and_then(|c| app.channels.get(&c))?;
    if ch.guild.is_some() || ch.voice_users.len() != 1 || is_group_dm(app, ch.id) {
        return None;
    }
    ch.voice_users.first().copied()
}

pub(crate) fn ensure_profile(app: &PhoenixApp, uid: Id, cx: &mut Context<PhoenixApp>) {
    if app.people.profiles.contains_key(&uid) {
        return;
    }
    {
        let mut requests = app.people.profile_requests.borrow_mut();
        match requests.get(&uid) {
            Some(ProfileRequestState::Loading) => return,
            Some(ProfileRequestState::Failed(at)) if at.elapsed() < PROFILE_RETRY_DELAY => return,
            _ => {}
        }
        requests.insert(uid, ProfileRequestState::Loading);
    }
    let owner = app.me.id;
    let fut = app.backend.user_profile(uid, None);
    cx.spawn(async move |this, cx| {
        let result = fut.await;
        let failed = result.is_err();
        let _ = this.update(cx, |app, cx| {
            if app.me.id != owner || !matches!(app.people.profile_requests.borrow().get(&uid), Some(ProfileRequestState::Loading)) {
                return;
            }
            match result {
                Ok(profile) => {
                    app.people.profile_requests.borrow_mut().remove(&uid);
                    app.people.profiles.entry(uid).or_insert(profile);
                }
                Err(_) => {
                    app.people.profile_requests.borrow_mut().insert(uid, ProfileRequestState::Failed(Instant::now()));
                }
            }
            cx.notify();
        });
        if failed {
            cx.background_executor().timer(PROFILE_RETRY_DELAY).await;
            let _ = this.update(cx, |app, cx| {
                if app.me.id == owner {
                    cx.notify();
                }
            });
        }
    }).detach();
}

pub(crate) fn retry_profile(app: &mut PhoenixApp, uid: Id, cx: &mut Context<PhoenixApp>) {
    if matches!(app.people.profile_requests.borrow().get(&uid), Some(ProfileRequestState::Loading)) {
        return;
    }
    app.people.profile_requests.borrow_mut().remove(&uid);
    ensure_profile(app, uid, cx);
    cx.notify();
}

fn bio_lines(bio: &str) -> gpui::Div {
    crate::ui::profile_bio::bio(bio, true, c(tok(idx::TEXT_STRONG)))
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if !app.show_members || !crate::ui::title_bar::dm_profile_fits(crate::ui::title_bar::viewport_w_css()) {
        return None;
    }
    let uid = peer(app)?;
    ensure_profile(app, uid, cx);
    ensure_avatar_colour(app, uid, cx);
    let theme = app.people.profiles.get(&uid).and_then(|p| p.theme);
    Some(crate::ui::profile_theme::scoped(theme, || panel(app, uid, cx)))
}

pub fn hides_profile(blocked: bool, ignored: bool, revealed: bool) -> bool {
    (blocked || ignored) && !revealed
}

fn is_ignored(app: &PhoenixApp, uid: Id) -> bool {
    app.nav.is_ignored(uid) || crate::actions::user::is_ignored(uid)
}

fn keep_ticking(cx: &mut Context<PhoenixApp>) {
    TICK_WANTED.with(|w| w.set(Some(Instant::now())));
    if TICKING.with(|t| t.replace(true)) {
        return;
    }
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let fresh = TICK_WANTED.with(|w| w.get().is_some_and(|t| t.elapsed() < Duration::from_millis(2500)));
            if !fresh || this.update(cx, |_, cx| cx.notify()).is_err() {
                TICKING.with(|t| t.set(false));
                break;
            }
        }
    })
    .detach();
}

pub fn timer_text(secs: i64) -> String {
    let secs = secs.max(0);
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

fn timer_icon(a: &Activity) -> &'static str {
    if a.is_embedded() {
        return "apps";
    }
    match a.kind {
        ActivityKind::Listening => "music",
        ActivityKind::Watching => "tv",
        _ => "game-controller",
    }
}

pub fn live_activities(acts: &[Activity]) -> Vec<&Activity> {
    let mut out: Vec<&Activity> = Vec::new();
    for a in acts.iter().filter(|a| a.kind != ActivityKind::Custom) {
        let dup = out.iter().any(|b| {
            (a.application_id.is_some() && a.application_id == b.application_id) || (!a.name.is_empty() && a.name == b.name)
        });
        if !dup {
            out.push(a);
        }
    }
    out
}

fn overlay_card(id: impl Into<gpui::ElementId>) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .relative()
        .flex()
        .flex_col()
        .flex_none()
        .rounded(z(8.))
        .overflow_hidden()
        .bg(c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND)))
        .when(crate::ui::profile_theme::is_light(), |d| d.border_1().border_color(c(tok(idx::BORDER_MUTED))))
}

fn activity_card(a: &Activity, n: usize, bot: bool, uid: Id, e: gpui::Entity<PhoenixApp>, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let title = activity_header(a)?;
    let spotify = a.is_spotify();
    let heading = if spotify || a.kind == ActivityKind::Streaming { a.details.clone().or_else(|| Some(a.name.clone())) } else { Some(a.name.clone()) };
    let mut lines: Vec<SharedString> = Vec::new();
    if spotify {
        lines.extend(a.state.as_ref().map(|s| SharedString::from(s.split("; ").collect::<Vec<_>>().join(", "))));
    } else {
        lines.extend(a.details.clone().filter(|_| a.kind != ActivityKind::Streaming));
        lines.extend(a.state.clone());
    }
    let timer = a.start.filter(|_| !bot && !spotify && a.kind != ActivityKind::Streaming).map(|start| {
        keep_ticking(cx);
        let secs = (chrono::Utc::now().timestamp_millis() - start) / 1000;
        let green = c(tok(idx::TEXT_FEEDBACK_POSITIVE));
        div()
            .flex()
            .items_center()
            .gap(z(4.))
            .flex_none()
            .max_w_full()
            .overflow_hidden()
            .child(svg().path(crate::icons::path(timer_icon(a))).size(z(12.)).flex_none().text_color(green))
            .child(div().text_size(z(12.)).line_height(z(16.)).text_color(green).whitespace_nowrap().child(timer_text(secs)))
    });
    let art: Option<AnyElement> = (!bot).then(|| match &a.image {
        Some(u) => img(u.clone()).size(z(60.)).rounded(z(8.)).flex_none().into_any_element(),
        None => div().size(z(60.)).rounded(z(8.)).flex_none().bg(c(tok(idx::BACKGROUND_MOD_NORMAL))).into_any_element(),
    });
    let text_default = c(tok(idx::TEXT_DEFAULT));
    let details = div()
        .self_center()
        .w_full()
        .min_w_0()
        .overflow_hidden()
        .flex()
        .flex_col()
        .gap(z(4.))
        .child(
            div()
                .flex()
                .flex_col()
                .when_some(heading, |d, h| {
                    d.child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(text_default).text_ellipsis().whitespace_nowrap().child(h))
                })
                .children(lines.into_iter().map(|l| div().text_size(z(12.)).line_height(z(16.)).text_color(text_default).text_ellipsis().whitespace_nowrap().child(l))),
        )
        .children(timer.map(|t| div().flex().flex_wrap().items_center().gap_x(z(8.)).child(t)));
    let card = overlay_card(("dm-panel-activity", uid.0.wrapping_mul(16).wrapping_add(n as u64)))
        .gap(z(8.))
        .p(z(8.))
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND_HOVER))))
        .on_click(move |_, _, cx| e.update(cx, |a, cx| a.open_profile_modal_at(uid, ACTIVITY_TAB, cx)))
        .child(
            div()
                .flex()
                .gap(z(4.))
                .overflow_hidden()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .child(div().min_w_0().text_ellipsis().whitespace_nowrap().child(title)),
        )
        .child(div().flex().gap(z(8.)).children(art).child(details));
    Some(card.into_any_element())
}

fn activity_cards(app: &PhoenixApp, i: &Info, uid: Id, cx: &mut Context<PhoenixApp>) -> Option<Div> {
    if matches!(i.presence, crate::model::Presence::Offline) {
        return None;
    }
    let bot = i.user.as_deref().is_some_and(|u| u.bot);
    let e = cx.entity();
    let mut cards: Vec<AnyElement> = Vec::new();
    let stream = crate::ui::voice_row::stream_card_with_header(app, uid, e.clone(), true, cx);
    let streaming = stream.is_some();
    cards.extend(stream);
    let live = live_activities(i.acts);
    let voice_activity = live.iter().any(|a| a.is_embedded());
    for (n, a) in live.into_iter().enumerate() {
        cards.extend(activity_card(a, n, bot, uid, e.clone(), cx));
    }
    if !streaming && !voice_activity {
        let themed = crate::ui::profile_theme::active_scheme().is_some();
        cards.extend(crate::ui::profile_voice::card(app, uid, crate::ui::profile_voice::Colors::sidebar(themed), e.clone()));
    }
    let many = cards.len() > 1;
    let first = cards.into_iter().next()?;
    let group = "dm-panel-activity-stack";
    let back = many.then(|| {
        let e = e.clone();
        div()
            .id(("dm-panel-activity-more", uid.0))
            .group("dm-panel-activity-more")
            .absolute()
            .left_0()
            .right_0()
            .bottom(z(-PEEK))
            .h(z(PEEK))
            .px(z(8.))
            .overflow_hidden()
            .cursor_pointer()
            .group_hover(group, |s| s.h(z(PEEK + 2. * LIFT)).bottom(z(-PEEK - LIFT)))
            .on_click(move |_, _, cx| e.update(cx, |a, cx| a.open_profile_modal_at(uid, ACTIVITY_TAB, cx)))
            .child(
                div()
                    .absolute()
                    .left(z(8.))
                    .right(z(8.))
                    .bottom_0()
                    .h(z(2. * (PEEK + 2. * LIFT)))
                    .rounded(z(8.))
                    .bg(c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND)))
                    .when(crate::ui::profile_theme::is_light(), |d| d.border_1().border_color(c(tok(idx::BORDER_MUTED))))
                    .group_hover("dm-panel-activity-more", |s| s.bg(c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND_HOVER)))),
            )
    });
    Some(
        div().relative().when(many, |d| d.pb(z(PEEK))).child(
            div()
                .group(group)
                .relative()
                .children(back)
                .child(
                    div()
                        .relative()
                        .when(many, |d| d.rounded(z(8.)).shadow(crate::ui::profile_toolbar::shadow_low()).group_hover(group, |s| s.top(z(-LIFT))))
                        .child(first),
                ),
        ),
    )
}

fn widget_card(app: &PhoenixApp, uid: Id, themed: bool, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let board = crate::ui::profile_board::get(app, uid, cx);
    if let Some(w) = board.widgets.first() {
        let col = crate::ui::settings::profile_widgets::WidgetColors {
            strong: c(tok(idx::TEXT_STRONG)),
            muted: c(tok(idx::TEXT_MUTED)),
            card: c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND)),
            more: false,
        };
        return Some(crate::ui::settings::profile_widgets::render_compact(w, &col).into_any_element());
    }
    let e = cx.entity();
    crate::ui::profile_games::breadcrumb(&board.games, themed)
        .map(|b| b.on_click(move |_, _, cx| e.update(cx, |a, cx| a.open_profile_modal(uid, cx))).into_any_element())
}

fn load_failed_notice(app: &PhoenixApp, uid: Id) -> Option<Div> {
    let failed = matches!(app.people.profile_requests.borrow().get(&uid), Some(ProfileRequestState::Failed(_)));
    let related = app.nav.relationship(uid).is_some_and(|r| matches!(r.kind, RelationshipKind::Friend | RelationshipKind::Incoming));
    if !failed || related {
        return None;
    }
    Some(
        div()
            .flex()
            .items_start()
            .gap(z(8.))
            .p(z(12.))
            .rounded(z(8.))
            .bg(c(tok(idx::BACKGROUND_FEEDBACK_MOD_WARNING)))
            .border_1()
            .border_color(c(tok(idx::INLINENOTICE_BORDER_WARNING)))
            .child(div().h(z(18.)).flex().flex_none().items_center().child(svg().path(crate::icons::path("warning")).size(z(16.)).text_color(c(tok(idx::ICON_FEEDBACK_WARNING)))))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                    .child(crate::t!("dm_panel.load_failed")),
            ),
    )
}

fn incoming_request_card(app: &PhoenixApp, uid: Id, i: &Info, cx: &mut Context<PhoenixApp>) -> Option<gpui::Stateful<Div>> {
    if !app.nav.relationship(uid).is_some_and(|r| r.kind == RelationshipKind::Incoming) {
        return None;
    }
    let body = crate::t!("dm_panel.incoming_request", name = i.display.clone());
    let bold = crate::ui::modals::channel_perms::bold_range(&body, &i.display);
    let text = gpui::StyledText::new(body).with_highlights(bold.map(|r| (r, gpui::HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() })));
    let act = |action: RelAction| {
        cx.listener(move |a: &mut PhoenixApp, _: &gpui::ClickEvent, _, cx| {
            cx.stop_propagation();
            match &action {
                RelAction::Accept => a.accept_friend_request(uid, cx),
                _ => a.relationship_action(uid, action.clone(), cx),
            }
        })
    };
    let primary = mana_button(("dm-panel-accept", uid.0), crate::t!("friends.accept"), true, false).line_height(z(18.)).on_click(act(RelAction::Accept));
    let secondary = mana_button(("dm-panel-ignore-request", uid.0), crate::t!("friends.ignore"), false, false).line_height(z(18.)).on_click(act(RelAction::Remove));
    Some(
        overlay_card(("dm-panel-incoming", uid.0))
            .gap(z(8.))
            .px(z(12.))
            .py(z(10.))
            .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(text))
            .child(div().flex().items_center().gap(z(8.)).child(primary).child(secondary)),
    )
}

fn private_notice(i: &Info, uid: Id) -> Option<gpui::Stateful<Div>> {
    if !i.prof.is_some_and(|p| p.private) {
        return None;
    }
    Some(
        overlay_card(("dm-panel-private", uid.0))
            .p(z(12.))
            .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(crate::t!("dm_panel.private", name = i.display.clone()))),
    )
}

fn provisional_notice(i: &Info, uid: Id) -> Option<gpui::Stateful<Div>> {
    if !i.user.as_deref().is_some_and(|u| u.is_provisional()) {
        return None;
    }
    let strong = c(tok(idx::TEXT_STRONG));
    let body = crate::t!("dm_panel.provisional_body");
    let more = crate::t!("dm_panel.provisional_learn_more");
    let link = body.len()..body.len() + more.len();
    let text = SharedString::from(format!("{body}{more}"));
    Some(
        overlay_card(("dm-panel-provisional", uid.0))
            .gap(z(12.))
            .p(z(8.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(z(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(z(4.))
                            .text_color(strong)
                            .child(div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::SEMIBOLD).child(crate::t!("dm_panel.provisional_title")))
                            .child(svg().path(crate::icons::path("settings/circle-error")).size(z(12.)).flex_none().text_color(strong)),
                    )
                    .child(
                        div()
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .text_color(c(tok(idx::TEXT_DEFAULT)))
                            .child(gpui::StyledText::new(text).with_highlights([(link, gpui::HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() })])),
                    ),
            ),
    )
}

fn blocked_notice(app: &PhoenixApp, uid: Id, blocked: bool, ignored: bool, cx: &mut Context<PhoenixApp>) -> Option<gpui::Stateful<Div>> {
    let incoming = app.nav.relationship(uid).is_some_and(|r| r.kind == RelationshipKind::Incoming);
    let shown = blocked || (ignored && !incoming);
    if !shown {
        return None;
    }
    let text = |t: SharedString| div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_DEFAULT))).child(t);
    let e = cx.entity();
    Some(
        overlay_card(("dm-panel-hidden-notice", uid.0))
            .flex_row()
            .items_center()
            .gap(z(8.))
            .p(z(16.))
            .when(blocked, |d| d.child(text(crate::t!("dm_panel.you_blocked"))))
            .when(ignored, |d| {
                d.child(text(crate::t!("dm_panel.you_ignored"))).child(text("\u{2022}".into())).child(
                    div()
                        .id(("dm-panel-unignore", uid.0))
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(tok(idx::TEXT_LINK)))
                        .cursor_pointer()
                        .hover(|s| s.underline())
                        .on_click(move |_, _, cx| e.update(cx, |a, cx| a.toggle_ignore(uid, cx)))
                        .child(crate::t!("menu.unignore")),
                )
            }),
    )
}

fn hidden_panel(app: &PhoenixApp, uid: Id, i: &Info, blocked: bool, cx: &mut Context<PhoenixApp>) -> Div {
    let channel = app.selected_channel;
    let e = cx.entity();
    let body = crate::t!(if blocked { "dm_panel.hidden_blocked" } else { "dm_panel.hidden_ignored" }, name = i.display.clone());
    let bold = crate::ui::modals::channel_perms::bold_range(&body, &i.display);
    let badge = div()
        .absolute()
        .bottom(z(-8.))
        .right(z(-8.))
        .p(z(4.))
        .rounded_full()
        .overflow_hidden()
        .bg(c(tok(idx::BACKGROUND_MOD_NORMAL)))
        .flex()
        .items_center()
        .justify_center()
        .child(svg().path(crate::icons::path(if blocked { "deny" } else { "eye-slash" })).size(z(24.)).text_color(c(tok(idx::INTERACTIVE_ICON_DEFAULT))));
    let reveal = mana_button(("dm-panel-reveal", uid.0), crate::t!("dm_panel.reveal"), true, false).line_height(z(18.)).on_click(move |_, _, cx| {
        REVEALED.with(|r| r.set(channel));
        e.update(cx, |_, cx| cx.notify());
    });
    let e2 = cx.entity();
    let dismiss = (!blocked).then(|| {
        div()
            .id(("dm-panel-hide-again", uid.0))
            .text_size(z(14.))
            .line_height(z(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(tok(idx::TEXT_BRAND)))
            .cursor_pointer()
            .hover(|s| s.underline())
            .on_click(move |_, _, cx| {
                REVEALED.with(|r| r.set(channel));
                e2.update(cx, |_, cx| cx.notify());
            })
            .child(crate::t!("dm_panel.dont_show_again"))
    });
    div()
        .relative()
        .size_full()
        .child(img(HIDDEN_PREVIEW).size_full().object_fit(gpui::ObjectFit::Contain).opacity(0.3))
        .child(
            div().absolute().inset_0().flex().items_center().justify_center().child(
                div()
                    .w(gpui::relative(0.8))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(z(16.))
                    .text_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(z(8.))
                            .child(div().relative().child(crate::ui::avatar(i.user.as_deref(), 56.)).child(badge))
                            .child(div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(crate::t!("dm_panel.hidden_title")))
                            .child(
                                div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(
                                    gpui::StyledText::new(body.clone())
                                        .with_highlights(bold.map(|r| (r, gpui::HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() }))),
                                ),
                            ),
                    )
                    .child(div().flex().flex_col().items_center().gap(z(8.)).child(reveal).children(dismiss)),
            ),
        )
}

fn panel(app: &PhoenixApp, uid: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let i = info(app, uid, None);
    let theme = i.prof.and_then(|p| p.theme);
    let bot = i.user.as_deref().is_some_and(|u| u.bot);
    let card_bg = tok(idx::CHAT_BACKGROUND_DEFAULT);
    let ring_bg = theme.map(|t| crate::ui::profile_theme::composite(t, t.0)).unwrap_or(card_bg);
    let blocked = app.nav.is_blocked(uid);
    let ignored = !blocked && is_ignored(app, uid);
    let revealed = REVEALED.with(|r| r.get()).is_some_and(|ch| Some(ch) == app.selected_channel);
    if !revealed {
        REVEALED.with(|r| r.set(None));
    }
    let hidden = hides_profile(blocked, ignored, revealed);
    let (effect, frame) = if hidden {
        (None, None)
    } else {
        let (fx_sku, frame_sku) = i.prof.map(|p| (p.effect_sku, p.frame_sku)).unwrap_or_default();
        (
            fx_sku.and_then(|s| crate::ui::profile_fx::product(app, s, cx)).and_then(|c| crate::ui::profile_fx::effect(app, &c, 300., cx)),
            frame_sku.and_then(|s| crate::ui::profile_fx::product(app, s, cx)).and_then(|c| c.frame),
        )
    };
    let e = cx.entity();
    let content: Vec<AnyElement> = if hidden {
        vec![hidden_panel(app, uid, &i, blocked, cx).into_any_element()]
    } else {
        let bio = i.prof.map(|p| p.bio.clone()).filter(|b| !b.is_empty());
        let created = i.prof.map(|p| p.created);
        let (avatar_x, avatar_y) = if theme.is_some() { (8., 49.) } else { (10., 51.) };
        let header = div()
            .relative()
            .h(z(144.))
            .mb(z(8.))
            .child(banner(&i, 105., 6.))
            .child(banner_buttons(app, uid, None, 8., cx))
            .child(div().absolute().left(z(avatar_x)).top(z(avatar_y)).child(avatar_block(&i, 80., 6., ring_bg)))
            .children(
                crate::ui::profile_toolbar::status(app, uid, None, crate::ui::profile_toolbar::Layout::Sidebar, theme.is_some(), i.custom.clone(), cx)
                    .map(|s| div().absolute().left(z(109.)).top(z(98.)).child(s)),
            );
        let hide_notes = app.prefs.local.streamer_hides_info();
        let mut body = div()
            .mx(z(16.))
            .flex()
            .flex_col()
            .gap(z(20.))
            .children(load_failed_notice(app, uid))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(z(8.))
                    .child(name_block_clickable(
                        &i, false,
                        (!hide_notes).then(|| crate::ui::profile_note::note_button(uid, cx)),
                        false, Some(e.clone()),
                    ))
                    .children(
                        if crate::ui::profile_note::shown(crate::ui::profile_note::Surface::Sidebar, uid == app.me.id, crate::ui::profile_note::editing(uid).is_some(), hide_notes) {
                            crate::ui::profile_note::note_section(i.prof.map(|p| p.note.clone()), uid, cx)
                        } else {
                            None
                        },
                    )
                    .children(i.prof.and_then(|p| mutuals_row(p, uid, &cx.entity()))),
            )
            .children(incoming_request_card(app, uid, &i, cx))
            .children(blocked_notice(app, uid, blocked, ignored, cx))
            .children(private_notice(&i, uid))
            .children(provisional_notice(&i, uid));
        if let Some(b) = bio {
            let e2 = e.clone();
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .child(bio_lines(&b))
                    .child(
                        div()
                            .id("dm-panel-full-bio")
                            .mt(z(8.))
                            .text_size(z(12.))
                            .line_height(z(16.))
                            .text_color(c(tok(idx::TEXT_STRONG)))
                            .cursor_pointer()
                            .hover(|s| s.underline())
                            .on_click(move |_, _, cx| e2.update(cx, |a, cx| a.open_profile_modal(uid, cx)))
                            .child(crate::t!("profile.view_full_bio")),
                    ),
            );
        }
        if let Some(t) = created {
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(z(8.))
                    .child(section_title(if bot { crate::t!("profile.created_on") } else { crate::t!("profile.member_since") }).mb(z(0.)))
                    .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(format_date(t))),
            );
        }
        let cards: Vec<AnyElement> = widget_card(app, uid, theme.is_some(), cx)
            .into_iter()
            .chain(activity_cards(app, &i, uid, cx).map(IntoElement::into_any_element))
            .collect();
        if !cards.is_empty() {
            body = body.child(div().flex().flex_col().gap(z(8.)).children(cards));
        }
        let e3 = e.clone();
        let footer = div().flex_none().p(z(16.)).child(
            div()
                .id("dm-panel-view-full")
                .w_full()
                .h(z(39.))
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                .border_1()
                .border_color(c(Palette::DIVIDER))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(c(Palette::BG_MOD_SELECTED)))
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(Palette::TEXT_HEADER))
                .on_click(move |_, _, cx| e3.update(cx, |a, cx| a.open_profile_modal(uid, cx)))
                .child(crate::t!("profile.view_full")),
        );
        vec![
            div().id("dm-panel-scroll").flex_1().min_h_0().overflow_y_scroll().child(header).child(body).into_any_element(),
            footer.into_any_element(),
        ]
    };
    let card = div()
        .relative()
        .size_full()
        .p(z(2.))
        .rounded(z(8.))
        .overflow_hidden()
        .when(frame.is_none(), |d| d.shadow(crate::ui::inbox::shadow_border()))
        .when(theme.is_none(), |d| d.bg(c(card_bg)))
        .when_some(theme, |d, t| d.children(theme_backdrop(t, 72.)))
        .child(
            div()
                .relative()
                .size_full()
                .rounded(z(6.))
                .overflow_hidden()
                .when_some(theme, |d, t| d.bg(crate::ui::profile_theme::overlay(t)))
                .flex()
                .flex_col()
                .children(content)
                .children(effect)
                .children(if hidden { None } else { crate::ui::profile_toolbar::scrim(uid, crate::ui::profile_toolbar::Layout::Sidebar) })
                .children(if hidden { None } else { crate::ui::profile_toolbar::toast(uid, 105.) }),
        );
    let card = match frame {
        None => card,
        Some(f) => div()
            .relative()
            .size_full()
            .children(crate::ui::profile_fx::frame(&f, 304., false, cx))
            .child(card)
            .children(crate::ui::profile_fx::frame(&f, 304., true, cx)),
    };
    div().w(z(320.)).flex_none().h_full().p(z(8.)).bg(c(Palette::BG_PRIMARY)).child(card).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn act(kind: ActivityKind, name: &str, app: Option<&str>) -> Activity {
        Activity {
            kind,
            name: name.to_string().into(),
            details: None,
            state: None,
            emoji: None,
            url: None,
            display_type: None,
            image: None,
            start: None,
            application_id: app.map(|a| a.to_string().into()),
            party: None,
            platform: None,
            flags: 0,
        }
    }

    #[test]
    fn the_timer_reads_like_the_client() {
        assert_eq!(timer_text(5), "0:05");
        assert_eq!(timer_text(754), "12:34");
        assert_eq!(timer_text(3600 + 2 * 60 + 3), "1:02:03");
        assert_eq!(timer_text(-4), "0:00");
    }

    #[test]
    fn blocked_and_ignored_profiles_stay_hidden_until_revealed() {
        assert!(hides_profile(true, false, false));
        assert!(hides_profile(false, true, false));
        assert!(!hides_profile(true, false, true));
        assert!(!hides_profile(false, false, false));
    }

    #[test]
    fn live_cards_skip_custom_status_and_duplicates() {
        let acts = vec![
            act(ActivityKind::Custom, "Custom Status", None),
            act(ActivityKind::Playing, "Game", Some("1")),
            act(ActivityKind::Playing, "Game again", Some("1")),
            act(ActivityKind::Listening, "Spotify", None),
            act(ActivityKind::Listening, "Spotify", None),
        ];
        let live = live_activities(&acts);
        assert_eq!(live.iter().map(|a| a.name.as_ref()).collect::<Vec<_>>(), ["Game", "Spotify"]);
    }
}
