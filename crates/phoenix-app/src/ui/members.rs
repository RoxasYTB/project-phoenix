use crate::theme::z;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, div,
    list, prelude::*, px,
};

use crate::{
    app::{MemberRow, PhoenixApp},
    backend::AppEvent,
    model::{Id, Member, Presence},
    presence::{ListEntry, ranges_for},
    theme::{Palette, c, generated::idx, m, role_color, space, tok},
};

pub fn on_event(app: &mut PhoenixApp, ev: AppEvent, cx: &mut Context<PhoenixApp>) {
    match ev {
        AppEvent::MemberListRows { guild, rows } => {
            app.people.lists.insert(guild, rows);
            if app.selected_guild == Some(guild) {
                sync_server_list(app);
            }
        }
        AppEvent::Activities { user, mut activities } => {
            use crate::presence::ActivityKind as K;
            if user == app.me.id
                && app.people.custom_status.is_some()
                && !activities.iter().any(|a| a.kind == K::Custom)
                && let Some(c) = app.people.activities.get(&user).and_then(|a| a.iter().find(|a| a.kind == K::Custom)).cloned()
            {
                activities.insert(0, c);
            }
            app.people.on_activities(user, activities)
        }
        _ => return,
    }
    cx.notify();
}

pub fn sync_server_list(app: &mut PhoenixApp) -> bool {
    let Some(len) = app.selected_guild.and_then(|g| rows_for(app, g)).map(|l| l.len() + 1) else {
        return false;
    };
    app.member_rows.clear();
    let old = app.member_list.item_count();
    if old == len {
        app.member_list.remeasure();
    } else if old < len {
        app.member_list.splice(old..old, len - old);
    } else {
        app.member_list.splice(len..old, 0);
    }
    true
}

fn request_ranges(app: &PhoenixApp, g: Id, cx: &mut Context<PhoenixApp>) {
    let (Some(ch), true) = (app.selected_channel, app.people.lists.contains_key(&g)) else { return };
    let top = app.member_list.logical_scroll_top().item_ix.saturating_sub(1);
    let key = (g, top / 100);
    if app.people.requested.get() == key {
        return;
    }
    app.people.requested.set(key);
    if key.1 == 0 {
        return;
    }
    let fut = app.backend.subscribe_member_ranges(g, ch, ranges_for(top));
    cx.background_spawn(fut).detach();
}

fn open_thread(app: &PhoenixApp) -> Option<Id> {
    app.selected_channel.filter(|c| app.nav.thread_parent(*c).is_some())
}

static NO_ROWS: Vec<ListEntry> = Vec::new();

fn rows_for(app: &PhoenixApp, g: Id) -> Option<&Vec<ListEntry>> {
    match open_thread(app) {
        Some(t) => Some(app.people.thread_lists.get(&t).unwrap_or(&NO_ROWS)),
        None => app.people.lists.get(&g),
    }
}

fn thread_rows(app: &PhoenixApp, g: Id, mut members: Vec<Member>) -> Vec<ListEntry> {
    let roles = app.roles.get(&g).cloned().unwrap_or_default();
    let mut hoisted: Vec<_> = roles.iter().filter(|r| r.hoist).collect();
    hoisted.sort_by_key(|r| std::cmp::Reverse(r.position));
    for m in &mut members {
        if m.presence == Presence::Offline {
            m.presence = app.presence_of(m.user);
        }
    }
    let name = |m: &Member| {
        m.nick.clone().or_else(|| app.users.get(&m.user).map(|u| u.display_name.clone())).unwrap_or_default().to_lowercase()
    };
    members.sort_by_key(name);
    let group_of = |m: &Member| -> String {
        if m.presence == Presence::Offline {
            return "offline".into();
        }
        hoisted.iter().find(|r| m.roles.contains(&r.id)).map(|r| r.id.0.to_string()).unwrap_or_else(|| "online".into())
    };
    let mut order: Vec<String> = hoisted.iter().map(|r| r.id.0.to_string()).collect();
    order.push("online".into());
    order.push("offline".into());
    let mut out = Vec::new();
    for gid in order {
        let ms: Vec<&Member> = members.iter().filter(|m| group_of(m) == gid).collect();
        if ms.is_empty() {
            continue;
        }
        out.push(ListEntry::Group { id: gid.into(), count: ms.len() as u32 });
        out.extend(ms.into_iter().map(|m| ListEntry::Member(m.clone(), m.nick.clone())));
    }
    out
}

fn ensure_thread_list(app: &PhoenixApp, g: Id, cx: &mut Context<PhoenixApp>) {
    let want = rows_for(app, g).map(|l| l.len() + 1);
    if want.is_some_and(|n| n != app.member_list.item_count()) {
        let e = cx.entity();
        cx.defer(move |cx| {
            e.update(cx, |a, cx| {
                sync_server_list(a);
                cx.notify();
            })
        });
    }
    let Some(t) = open_thread(app) else { return };
    if !app.people.thread_requested.borrow_mut().insert(t) {
        return;
    }
    let fut = app.backend.thread_members(t);
    cx.spawn(async move |this, cx| {
        let members = match fut.await {
            Ok(m) => m,
            Err(e) => {
                if std::env::var_os("PHX_DEBUG").is_some() {
                    tracing::warn!("thread members: {}", e.0);
                }
                return;
            }
        };
        let _ = this.update(cx, |a, cx| {
            let rows = thread_rows(a, g, members);
            a.people.thread_lists.insert(t, rows);
            sync_server_list(a);
            cx.notify();
        });
    })
    .detach();
}

fn empty_thread() -> gpui::Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .pt(z(16.))
        .px(z(16.))
        .flex()
        .flex_col()
        .items_center()
        .child(div().w_full().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("members.members")))
        .child(div().mt(z(48.)).child(crate::ui::icon(crate::assets::Icon::Members, 40., c(tok(idx::ICON_MUTED)))))
        .child(div().mt(z(40.)).text_center().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(crate::t!("thread.nobody")))
        .child(div().mt(z(8.)).text_center().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(crate::t!("thread.nobody_sub")))
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let entity = cx.entity();
    if let Some(g) = app.selected_guild {
        request_ranges(app, g, cx);
        ensure_thread_list(app, g, cx);
    }
    let nobody = open_thread(app).and_then(|t| app.people.thread_lists.get(&t)).is_some_and(|v| v.is_empty());
    div()
        .flex_none()
        .h_full()
        .relative()
        .flex()
        .flex_col()
        .bg(crate::theme::client_theme::surface(crate::theme::client_theme::Level::Chat, Some(crate::theme::client_theme::Level::AppFrame), Palette::BG_SECONDARY))
        .border_l_1()
        .border_color(c(Palette::DIVIDER))
        .child(
            crate::ui::a11y::named(div().id("member-list"), gpui::Role::List, || crate::t!("a11y.members"))
                .flex_1()
                .min_h_0()
                .w(z(m::member_list_w()))
                .relative()
                .group(crate::ui::chat::scrollbar::THIN_FADE_GROUP)
                .child(
                    list(app.member_list.clone(), move |ix, _w, cx| row(&entity, ix, cx))
                        .size_full()
                        .pr(z(8.))
                        .pb(z(10.)),
                )
                .children(crate::ui::chat::scrollbar::thin_fade_scrollbar(&app.member_list)),
        )
        .when(nobody, |d| d.child(empty_thread()))
}

pub fn elapsed(start_ms: i64, now_s: i64) -> String {
    let s = (now_s - start_ms / 1000).max(0);
    match s {
        0..60 => crate::t!("time.now").to_string(),
        60..3600 => crate::t!("time.minutes_ago_short", n = s / 60).to_string(),
        3600..86_400 => crate::t!("time.hours_ago_short", n = s / 3600).to_string(),
        _ => crate::t!("time.days_ago_short", n = s / 86_400).to_string(),
    }
}

pub fn since_short(start_ms: i64, now_s: i64) -> String {
    let s = (now_s - start_ms / 1000).max(0);
    match s {
        0..60 => crate::t!("time.since_m", n = 1).to_string(),
        60..3600 => crate::t!("time.since_m", n = s / 60).to_string(),
        3600..86_400 => crate::t!("time.since_h", n = s / 3600).to_string(),
        86_400..2_592_000 => crate::t!("time.since_d", n = s / 86_400).to_string(),
        2_592_000..31_104_000 => crate::t!("time.since_mo", n = s / 2_592_000).to_string(),
        _ => crate::t!("time.since_y", n = s / 31_104_000).to_string(),
    }
}

fn feed_cards(app: &PhoenixApp, g: Id) -> Vec<crate::commands::InventoryEntry> {
    let is_member = |u: Id| {
        app.members.get(&g).is_some_and(|ms| ms.iter().any(|m| m.user == u))
            || rows_for(app, g).is_some_and(|rows| rows.iter().any(|r| matches!(r, ListEntry::Member(m, _) if m.user == u)))
    };
    let entries = crate::ui::activity_traits::entries();
    let mut unknown: Vec<Id> = Vec::new();
    let mut out = Vec::new();
    for e in entries {
        if !matches!(e.content_type, 1 | 8) || e.game.is_none() {
            continue;
        }
        if !is_member(e.author) {
            if !unknown.contains(&e.author) && app.people.feed_requested.borrow_mut().insert((g, e.author)) {
                unknown.push(e.author);
            }
            continue;
        }
        if app.selected_channel.is_some_and(|ch| !app.backend.member_can_view(ch, e.author)) {
            continue;
        }
        out.push(e);
    }
    if !unknown.is_empty() {
        app.backend.request_feed_members(g, unknown);
    }
    out
}

static ACTIVITY_EXPANDED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn activity_section(app: &PhoenixApp, entity: &Entity<PhoenixApp>, g: Id) -> AnyElement {
    let acts = feed_cards(app, g);
    let count = acts.len();
    let expanded = ACTIVITY_EXPANDED.load(std::sync::atomic::Ordering::Relaxed);
    let forum = app
        .selected_channel
        .and_then(|id| app.channels.get(&id))
        .is_some_and(|ch| ch.kind == crate::model::ChannelKind::Forum && !app.backend.is_media_channel(ch.id));
    if acts.is_empty() || forum || !app.backend.activity_feed_enabled(g) {
        return div().into_any_element();
    }
    let now = crate::mock::now();
    let muted = c(Palette::TEXT_MUTED);
    let mut col = div().flex_none().flex().flex_col().child(
        div()
            .pt(z(space::lg()))
            .pb(z(space::xxs()))
            .pl(z(space::md()))
            .pr(z(space::xxs()))
            .child(
                div()
                    .h(z(24.))
                    .my(z(-3.))
                    .flex()
                    .items_center()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(muted)
                    .child(crate::t!("members.activity", n = count))
                    .child(
                        div()
                            .size(z(24.))
                            .p(z(4.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(gpui::svg().path(crate::icons::path("gear")).size(z(12.)).text_color(muted)),
                    )
                    .child(div().flex_1())
                    .when(acts.len() > 3, |d| {
                        let ent = entity.clone();
                        d.child(
                            div()
                                .id("activity-expand")
                                .cursor_pointer()
                                .on_click(move |_, _, cx| {
                                    ACTIVITY_EXPANDED.fetch_xor(true, std::sync::atomic::Ordering::Relaxed);
                                    ent.update(cx, |a, cx| {
                                        a.member_list.remeasure();
                                        cx.notify();
                                    });
                                })
                                .w(z(24.))
                                .h(z(18.))
                                .flex()
                                .justify_center()
                                .child(
                                    gpui::svg()
                                        .path(crate::icons::path(if expanded { "chevron" } else { "chevron-right" }))
                                        .size(z(18.))
                                        .text_color(c(Palette::INTERACTIVE)),
                                ),
                        )
                    }),
            ),
    );
    for e in acts.into_iter().take(if expanded { usize::MAX } else { 3 }) {
        let uid = e.author;
        let user_cow = app.member_user(Some(g), uid);
        let user = user_cow.as_deref();
        let mem = app.members.get(&g).and_then(|ms| crate::member_index::find(ms, g, uid));
        let (role_col, role_grad) = mem.map_or((None, None), |m| app.member_role_style(g, m));
        let name = mem
            .and_then(|m| m.nick.clone())
            .or_else(|| user.map(|u| u.display_name.clone()))
            .unwrap_or_default();
        let ent = entity.clone();
        let game: SharedString = e.game.clone().unwrap_or_default().into();
        let art = match e.icon.clone() {
            Some(url) => gpui::img(url).size_full().rounded(z(4.)).object_fit(gpui::ObjectFit::Cover).into_any_element(),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(z(16.))
                .font_weight(FontWeight::BOLD)
                .text_color(muted)
                .child(SharedString::from(game.chars().next().map(String::from).unwrap_or_default()))
                .into_any_element(),
        };
        let badge_in = |ic: &'static str, t: SharedString, col: gpui::Hsla| {
            div()
                .flex()
                .items_center()
                .min_w_0()
                .overflow_hidden()
                .gap(z(4.))
                .child(gpui::svg().path(crate::icons::path(ic)).size(z(12.)).flex_none().text_color(col))
                .child(div().min_w_0().text_size(z(12.)).line_height(z(16.)).text_color(col).text_ellipsis().child(t))
        };
        let badge = |ic: &'static str, t: SharedString| badge_in(ic, t, c(Palette::TEXT_MUTED));
        let entry_live = e.live && e.expires_ms.is_none_or(|x| x > now * 1000);
        let live_start = e.started_ms.filter(|_| entry_live).or_else(|| {
            app.people.activities.get(&uid).and_then(|acts| {
                acts.iter()
                    .find(|a| e.game.as_deref().is_some_and(|g| a.name.eq_ignore_ascii_case(g)))
                    .and_then(|a| a.start)
            })
        });
        let time_icon = if e.content_type == 8 { "apps" } else { "game-controller" };
        let time_badge = match live_start {
            Some(start) => badge_in(time_icon, live_timer(start, now), c(tok(idx::TEXT_FEEDBACK_POSITIVE))),
            None => badge(time_icon, crate::ui::settings::profile_activity::ago(crate::presence::snowflake_time(e.id.0), now)),
        };
        col = col.child(
            div()
                .id(("activity-card", e.id.0))
                .mt(z(8.))
                .ml(z(8.))
                .w(z(m::member_list_w() - 16.))
                .h(z(52. + 16. + 2. * 0.667))
                .overflow_hidden()
                .p(z(8.))
                .rounded(z(12.))
                .border_1()
                .border_color(c(tok(idx::BORDER_MUTED)))
                .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                .cursor_pointer()
                .flex()
                .items_center()
                .gap(z(8.))
                .on_click(move |ev, window, cx| {
                    let at = ev.position();
                    ent.update(cx, |a, cx| a.open_profile(uid, Some(g), at, window, cx));
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .mb(z(-2.))
                        .child(
                            div()
                                .h(z(18.))
                                .mb(z(2.))
                                .min_w_0()
                                .overflow_hidden()
                                .flex()
                                .items_center()
                                .child(div().mr(z(6.)).child(crate::ui::avatar(user, 16.)))
                                .child(
                                    crate::fonts::styled_name(
                                        div()
                                            .min_w_0()
                                            .max_w(gpui::relative(0.85))
                                            .overflow_hidden()
                                            .text_size(z(14.))
                                            .line_height(z(18.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(
                                                role_col
                                                    .map(c)
                                                    .unwrap_or_else(|| c(crate::theme::tok(crate::theme::generated::idx::CHANNELS_DEFAULT))),
                                            )
                                            .text_ellipsis(),
                                        user,
                                        false,
                                    )
                                    .child(crate::fonts::name_text_with(name, user, 14., role_grad)),
                                ),
                        )
                        .child(
                            div()
                                .text_size(z(14.))
                                .line_height(z(18.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(c(Palette::TEXT_NORMAL))
                                .text_ellipsis()
                                .child(game.clone()),
                        )
                        .child(
                            div()
                                .h(z(16.))
                                .flex()
                                .gap(z(8.))
                                .overflow_hidden()
                                .child(time_badge.flex_none())
                                .when(e.first_time, |d| d.child(badge("medal", crate::t!("members.new_player"))))
                                .when(e.resurrected, |d| d.child(badge("refresh", crate::t!("members.resurrection"))))
                                .when(e.trending, |d| d.child(badge("fire", crate::t!("members.trending"))))
                                .when_some(e.streak_days.filter(|n| *n >= 3), |d, n| d.child(badge("streak", crate::t!("pa.streak", n = n)))),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .size(z(48.))
                        .rounded(z(4.))
                        .overflow_hidden()
                        .when(e.icon.is_none(), |d| d.bg(c(Palette::BG_MOD_SELECTED)))
                        .child(art),
                ),
        );
    }
    col.into_any_element()
}

fn live_timer(start_ms: i64, now_s: i64) -> SharedString {
    let s = (now_s - start_ms / 1000).max(0);
    let (h, m, sec) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{sec:02}") } else { format!("{m}:{sec:02}") }.into()
}

pub fn server_tag(tag: &(SharedString, Option<SharedString>)) -> gpui::Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .px(z(4.))
        .rounded(z(4.))
        .bg(c(tok(idx::BACKGROUND_MOD_STRONG)))
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(Palette::TEXT_NORMAL))
        .when_some(tag.1.clone(), |d, url| d.child(gpui::img(url).size(z(14.)).mr(z(2.)).flex_none()))
        .child(tag.0.clone())
}

pub fn header(t: SharedString) -> AnyElement {
    div()
        .pt(z(space::lg()))
        .pb(z(space::xxs()))
        .pl(z(space::md()))
        .pr(z(space::xxs()))
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(Palette::TEXT_MUTED))
        .text_ellipsis()
        .child(t)
        .into_any_element()
}

fn group_header(app: &PhoenixApp, g: Id, id: &str, count: u32) -> AnyElement {
    let name = group_name(app, g, id);
    let spoken = crate::ui::a11y::member_group(name.trim(), count);
    let icon = id
        .parse::<u64>()
        .ok()
        .and_then(|r| app.roles.get(&g)?.iter().find(|x| x.id == Id(r))?.icon.clone());
    div()
        .pt(z(space::lg()))
        .pb(z(space::xxs()))
        .pl(z(space::md()))
        .pr(z(space::xxs()))
        .child(
            div()
                .h(z(18.))
                .flex()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(Palette::TEXT_MUTED))
                .when_some(icon, |d, url| d.child(gpui::img(url).flex_none().size(z(16.)).mr(z(4.))))
                .child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(SharedString::from(name.trim().to_string())))
                .child(div().flex_none().whitespace_nowrap().child(SharedString::from(format!("\u{a0}— {count}")))),
        )
        .id(SharedString::from(format!("member-group-{id}")))
        .map(|d| crate::ui::a11y::named(d, gpui::Role::Heading, move || spoken))
        .into_any_element()
}

fn group_name(app: &PhoenixApp, g: Id, id: &str) -> String {
    match id {
        "online" => crate::t!("status.online").to_string(),
        "offline" => crate::t!("status.offline").to_string(),
        _ => id
            .parse::<u64>()
            .ok()
            .and_then(|r| app.roles.get(&g)?.iter().find(|x| x.id == Id(r)).map(|x| x.name.to_string()))
            .unwrap_or_else(|| crate::t!("members.members").to_string()),
    }
}

fn placeholder(ix: usize) -> AnyElement {
    let w = 60. + ((ix * 37) % 80) as f32;
    div()
        .ml(z(16.))
        .h(px(m::MEMBER_ROW_H))
        .flex()
        .items_center()
        .gap(z(12.))
        .child(div().size(z(32.)).rounded_full().bg(c(Palette::BG_MOD_HOVER)))
        .child(div().w(px(w)).h(z(12.)).rounded(z(6.)).bg(c(Palette::BG_MOD_HOVER)))
        .into_any_element()
}

fn row(entity: &Entity<PhoenixApp>, ix: usize, cx: &mut App) -> AnyElement {
    let app = entity.read(cx);
    let Some(g) = app.selected_guild else { return div().into_any_element() };
    let Some(ix) = ix.checked_sub(1) else {
        let entity = entity.clone();
        let el = div().flex().flex_col().child(activity_section(app, &entity, g)).into_any_element();
        if crate::ui::activity_traits::stale() {
            cx.defer(move |cx| entity.update(cx, crate::ui::activity_traits::ensure));
        }
        return el;
    };
    if let Some(rows) = rows_for(app, g) {
        return match rows.get(ix) {
            Some(ListEntry::Group { id, count }) => {
                let n = if *count > 0 {
                    *count
                } else {
                    rows[ix + 1..].iter().take_while(|r| !matches!(r, ListEntry::Group { .. })).count() as u32
                };
                group_header(app, g, id, n)
            }
            Some(ListEntry::Member(mem, nick)) => member_row(app, entity, g, mem, nick.as_ref()),
            Some(ListEntry::Loading) => placeholder(ix),
            None => div().into_any_element(),
        };
    }
    match app.member_rows.get(ix) {
        Some(MemberRow::Header(t)) => header(t.clone()),
        Some(MemberRow::Member(i)) => match app.members.get(&g).and_then(|ms| ms.get(*i)) {
            Some(mem) => member_row(app, entity, g, mem, None),
            None => div().into_any_element(),
        },
        None => div().into_any_element(),
    }
}

fn member_row(
    app: &PhoenixApp,
    entity: &Entity<PhoenixApp>,
    g: Id,
    mem: &Member,
    nick: Option<&SharedString>,
) -> AnyElement {
    let hoisted = mem.presence != Presence::Offline
        && app.roles.get(&g).is_some_and(|rs| rs.iter().any(|r| r.hoist && mem.roles.contains(&r.id)));
    let owner = !hoisted && app.guilds.iter().find(|x| x.id == g).and_then(|x| x.owner) == Some(mem.user);
    member_row_in(app, entity, Some(g), mem, nick, owner)
}

pub fn member_row_in(
    app: &PhoenixApp,
    entity: &Entity<PhoenixApp>,
    g: Option<Id>,
    mem: &Member,
    nick: Option<&SharedString>,
    owner: bool,
) -> AnyElement {
    let user_cow = app.member_user(g, mem.user);
    let user = user_cow.as_deref();
    let offline = mem.presence == Presence::Offline;
    let name = nick.cloned().or_else(|| user.map(|u| u.display_name.clone())).unwrap_or_default();
    let uid = mem.user;
    let (role_col, role_grad) = g.map_or((None, None), |g| app.member_role_style(g, mem));
    let e = entity.clone();
    let tag = user.and_then(|u| u.tag.clone());
    let acts = app.people.activities.get(&uid).map(Vec::as_slice).unwrap_or(&[]);
    let streaming = !offline && acts.iter().any(|a| a.kind == crate::presence::ActivityKind::Streaming);
    let in_voice = match g {
        Some(g) => app
            .guild_channels
            .get(&g)
            .is_some_and(|ids| ids.iter().filter_map(|i| app.channels.get(i)).any(|ch| ch.voice_users.contains(&uid))),
        None => app.channels.values().any(|ch| ch.guild.is_some() && ch.voice_users.contains(&uid)),
    } && !crate::ui::voice_panel::flags(uid).undiscoverable;
    let go_live = in_voice && crate::ui::voice_panel::flags(uid).stream;
    let sub = SubText::of(acts, go_live);
    let voice_icon = if go_live { "golive/screen" } else { "speaker" };
    let two_lines = !offline && (in_voice || sub.is_some() || (acts.is_empty() && mem.activity.is_some()));
    let has_plate = user.is_some_and(|u| u.nameplate.is_some());
    div().py(z(1.)).child(div()
        .id(("member", mem.user.0))
        .child(
            gpui::canvas(move |b, _, _| crate::ui::profile::note_row_anchor(uid, b), |_, _, _, _| {})
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
        )
        .when(crate::ui::a11y::on(), |d| {
            let status = if streaming {
                crate::t!("activity.live")
            } else {
                match mem.presence {
                    Presence::Online => crate::t!("status.online"),
                    Presence::Idle => crate::t!("status.idle"),
                    Presence::Dnd => crate::t!("status.dnd"),
                    Presence::Offline => crate::t!("status.offline"),
                }
            };
            let activity = sub.as_ref().and_then(|s| s.main.as_ref().map(|m| m.0.clone()).or_else(|| s.custom.as_ref().and_then(|c| c.1.clone())));
            d.role(gpui::Role::ListItem).aria_label(crate::ui::a11y::member(&name, &status, activity.as_deref()))
        })
        .child(crate::ui::avatar_deco::guard(crate::ui::avatar_deco::hover_key(1, uid.0)))
        .on_hover({
            let e = e.clone();
            move |h, _, cx| {
                if crate::ui::avatar_deco::set_hover(crate::ui::avatar_deco::hover_key(1, uid.0), *h, cx) {
                    e.update(cx, |_, cx| cx.notify());
                }
            }
        })
        .group("member-row")
        .ml(z(8.))
        .h(z(if two_lines { m::MEMBER_ROW_H } else { m::MEMBER_ROW_H - 2. }))
        .pl(z(space::xs()))
        .pr(z(if has_plate { 32. } else { 8. }))
        .py(z(4.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .cursor_pointer()
        .when(offline, |d| d.opacity(0.3))
        .hover(|s| s.bg(c(crate::theme::tok(crate::theme::generated::idx::INTERACTIVE_BACKGROUND_HOVER))).opacity(1.))
        .active(|s| s.bg(c(crate::theme::tok(crate::theme::generated::idx::INTERACTIVE_BACKGROUND_ACTIVE))))
        .relative()
        .map(|d| crate::ui::focus_ring::ring_inset(d, crate::ui::states::key(("member", uid.0)), 8., 4., 3.))
        .when_some(
            user.and_then(|u| {
                let sel = app.people.popout.as_ref().is_some_and(|p| p.user == uid);
                let name_w = name.chars().count() as f32 * 9.0
                    + if tag.is_some() { 56.0 } else { 0.0 }
                    + if u.bot { 50.0 } else { 0.0 }
                    + if owner || mem.premium_since.is_some() { 18.0 } else { 0.0 };
                let sub_chars = sub
                    .as_ref()
                    .map(|s| {
                        s.custom.as_ref().and_then(|c| c.1.as_ref()).map_or(0, |t| t.chars().count())
                            + s.main.as_ref().map_or(0, |(t, _)| t.chars().count())
                    })
                    .unwrap_or(0);
                let sub_w = if two_lines { sub_chars as f32 * 6.5 + 16.0 } else { 0.0 };
                let fallback = (26.0 + name_w.max(sub_w) + 30.0).clamp(120.0, 220.0);
                let row_h = if two_lines { m::MEMBER_ROW_H } else { m::MEMBER_ROW_H - 2. };
                let fade = crate::ui::nameplate::Fade::Measured {
                    place: crate::ui::nameplate::Place::Member,
                    id: uid.0,
                    offset: 10.,
                    fallback,
                };
                crate::ui::nameplate::layer_with_fade(u, row_h, crate::ui::nameplate::Place::Member, false, sel, fade)
            }),
            |d, l| d.child(l),
        )
        .when(app.people.popout.as_ref().is_some_and(|p| p.user == uid), |d| {
            d.bg(c(tok(idx::INTERACTIVE_BACKGROUND_SELECTED))).opacity(1.)
        })
        .on_click(move |ev, window, cx| {
            let at = ev.position();
            e.update(cx, |a, cx| a.open_profile(uid, g, at, window, cx));
        })
        .on_mouse_down(gpui::MouseButton::Right, move |ev, window, cx| {
            cx.stop_propagation();
            crate::ui::user_menu::open(uid, g, ev.position, window);
        })
        .child(
            div()
                .relative()
                .min_w_0()
                .max_w_full()
                .flex()
                .items_center()
                .gap(z(12.))
                .child(
                    div()
                        .relative()
                        .flex_none()
                        .child({
                            let sel = app.people.popout.as_ref().is_some_and(|p| p.user == uid);
                            let cut = move |d: gpui::Div| {
                                d.group_hover("member-row", |s| s.bg(c(crate::theme::tok(crate::theme::generated::idx::INTERACTIVE_BACKGROUND_HOVER))))
                                    .when(sel, |d| d.bg(c(tok(idx::INTERACTIVE_BACKGROUND_SELECTED))))
                            };
                            let deco = crate::ui::avatar_deco::source(user, Some(crate::ui::avatar_deco::hover_key(1, uid.0)));
                            let play = sel || crate::ui::avatar_deco::plays(crate::ui::avatar_deco::hover_key(1, uid.0));
                            if mem.presence == crate::model::Presence::Online && crate::ui::mobile_status::is_mobile(uid.0) {
                                mobile_avatar(user, cut, deco, play)
                            } else {
                                crate::ui::avatar_with_status_deco_hover(user, 32., mem.presence, c(Palette::BG_SECONDARY), cut, deco, play)
                            }
                        })
                        .when(streaming, |d| d.child(streaming_dot())),
                )
                .child(
                    div()
                        .flex_grow_1()
                        .flex_shrink_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(z(4.))
                                .child(
                                    crate::fonts::styled_name(div(), user, false)
                                        .text_size(z(16.))
                                        .line_height(z(20.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .map(|d| match role_col {
                                            Some(col) => d.text_color(role_color(Some(col))),
                                            None if app.people.popout.as_ref().is_some_and(|p| p.user == uid) => {
                                                d.text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE)))
                                            }
                                            None => d
                                                .text_color(c(tok(idx::CHANNELS_DEFAULT)))
                                                .group_hover("member-row", |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER)))),
                                        })
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child(crate::fonts::role_name_text_moving(
                                            name,
                                            user,
                                            16.,
                                            g.is_some(),
                                            role_grad,
                                            true,
                                            if crate::ui::avatar_deco::hovered(crate::ui::avatar_deco::hover_key(1, uid.0)) {
                                                crate::fonts::Motion::Loop
                                            } else {
                                                crate::fonts::Motion::Still
                                            },
                                        )),
                                )
                                .when_some(tag, |d, t| {
                                    d.child(crate::ui::tag_popout::clickable(
                                        server_tag(&t).my(z(1.)),
                                        ("member-tag", uid.0),
                                        user.and_then(|u| u.tag_guild),
                                        &t,
                                        entity.clone(),
                                    ))
                                })
                                .when(user.is_some_and(|u| u.bot), |d| d.child(bot_tag_px(user.is_some_and(|u| u.verified_bot))))
                                .when(owner, |d| {
                                    use crate::ui::tooltip::Tipped as _;
                                    d.child(
                                        div()
                                            .flex_none()
                                            .size(z(14.))
                                            .child(gpui::svg().path(crate::icons::path("crown")).size_full().text_color(c(tok(idx::TEXT_FEEDBACK_WARNING))))
                                            .tip(crate::t!("members.owner_tip")),
                                    )
                                })
                                .when_some(mem.premium_since, |d, since| {
                                    use crate::ui::tooltip::Tipped as _;
                                    d.child(
                                        div()
                                            .flex_none()
                                            .size(z(14.))
                                            .ml(z(3.))
                                            .relative()
                                            .top(z(1.))
                                            .child(gpui::svg().path(crate::icons::path("chat/boost")).size_full().text_color(c(tok(idx::GUILD_BOOSTING_PINK))))
                                            .tip(crate::t!("members.boosting_since", date = crate::presence::format_date(since))),
                                    )
                                }),
                        )
                        .when(two_lines, |d| d.child(sub_text(in_voice, voice_icon, user.is_some_and(|u| u.bot), sub, mem.activity.clone().filter(|_| acts.is_empty())))),
                )
                .when(has_plate, |d| d.child(crate::ui::nameplate::width_probe(crate::ui::nameplate::Place::Member, uid.0))),
        ))
        .into_any_element()
}

fn streaming_dot() -> gpui::Div {
    let ring = c(Palette::BG_SECONDARY);
    div()
        .absolute()
        .right(px(-4.))
        .bottom(px(-4.))
        .size(px(16.))
        .rounded_full()
        .bg(ring)
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size(px(10.))
                .rounded_full()
                .bg(c(tok(idx::PLATFORM_TWITCH)))
                .flex()
                .items_center()
                .justify_center()
                .child(gpui::svg().path(crate::icons::path("play")).size(px(6.)).text_color(ring)),
        )
}

pub fn bot_tag_px(verified: bool) -> gpui::Div {
    div()
        .flex_none()
        .h(z(16.))
        .px(z(4.))
        .rounded(z(4.))
        .bg(c(tok(idx::BACKGROUND_BRAND)))
        .flex()
        .items_center()
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(gpui::white())
        .when(verified, |d| d.child(verified_check().ml(z(-2.))))
        .child(crate::t!("common.app").to_uppercase())
}

pub fn verified_check() -> gpui::Div {
    use crate::ui::tooltip::Tipped;
    div()
        .relative()
        .flex_none()
        .size(z(16.))
        .child(gpui::svg().path(crate::icons::path("check-small-bold")).size_full().text_color(gpui::white()))
        .tip(crate::t!("common.verified_app"))
}


fn mobile_avatar(user: Option<&crate::model::User>, _cut: impl FnOnce(gpui::Div) -> gpui::Div, deco: Option<SharedString>, play: bool) -> gpui::Div {
    mobile_avatar_at(
        div().relative().size(px(32.)).child(crate::ui::avatar_hover(user, 32., play)).when_some(deco, |d, src| d.child(crate::ui::avatar_deco::layer(src, 32.))),
        32.,
    )
}

pub(crate) fn mobile_avatar_at(content: impl IntoElement, size: f32) -> gpui::Div {
    let (_, _, st) = crate::ui::status_svg::geometry(size);
    let pad = st * 0.3;
    let hole = crate::ui::blob_mask::Hole { x: size - st - pad, y: size - 1.5 * st - pad, w: st + 2. * pad, h: 1.5 * st + 2. * pad, r: st * 0.42 };
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(crate::ui::blob_mask::cut(div().relative().size(px(size)).child(content), [hole]))
        .child(
            gpui::svg()
                .path(crate::ui::status_svg::MOBILE)
                .absolute()
                .left(px(size - st))
                .top(px(size - 1.5 * st))
                .w(px(st))
                .h(px(1.5 * st))
                .text_color(crate::theme::presence_color(crate::model::Presence::Online)),
        )
}

pub(crate) fn dm_sub_text(app: &PhoenixApp, uid: Id) -> Option<gpui::Div> {
    if app.presence_of(uid) == Presence::Offline {
        return None;
    }
    let acts = app.people.activities.get(&uid).map(Vec::as_slice).unwrap_or(&[]);
    let in_voice = app.channels.values().any(|ch| ch.guild.is_some() && ch.voice_users.contains(&uid))
        && !crate::ui::voice_panel::flags(uid).undiscoverable;
    let go_live = in_voice && crate::ui::voice_panel::flags(uid).stream;
    let sub = SubText::of(acts, go_live);
    let fallback = app.nav.activities.get(&uid).cloned().filter(|_| acts.is_empty());
    if !in_voice && sub.is_none() && fallback.is_none() {
        return None;
    }
    let bot = app.users.get(&uid).is_some_and(|u| u.bot);
    Some(sub_text(in_voice, if go_live { "golive/screen" } else { "speaker" }, bot, sub, fallback))
}

pub(crate) fn self_sub_text(app: &PhoenixApp) -> Option<gpui::Div> {
    let uid = app.me.id;
    let acts = app.people.activities.get(&uid).map(Vec::as_slice).unwrap_or(&[]);
    let in_voice = app.voice_channel.is_some();
    let go_live = in_voice && crate::ui::voice_panel::flags(uid).stream;
    let sub = SubText::of(acts, go_live);
    if !in_voice && sub.as_ref().is_none_or(|s| s.main.is_none()) {
        return None;
    }
    Some(
        sub_text(in_voice, if go_live { "golive/screen" } else { "speaker" }, false, sub, None)
            .mt(z(0.))
            .h(z(13.))
            .line_height(z(13.))
            .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))),
    )
}

struct SubText {
    custom: Option<(Option<SharedString>, Option<SharedString>)>,
    main: Option<(SharedString, Option<&'static str>)>,
    count: usize,
}

pub(crate) fn activity_text(a: &crate::presence::Activity) -> Option<SharedString> {
    use crate::presence::ActivityKind as K;
    let nz = |v: &Option<SharedString>| v.clone().filter(|t| !t.is_empty());
    let name = Some(a.name.clone()).filter(|t| !t.is_empty());
    let (details, state) = (nz(&a.details), nz(&a.state));
    let mut m = if a.kind == K::Streaming { details.clone().or(name.clone()) } else { name.clone() };
    match a.display_type {
        Some(0) if name.is_some() => m = name.clone(),
        Some(1) if state.is_some() => m = state.clone(),
        Some(2) if details.is_some() => m = details.clone(),
        _ => {}
    }
    if a.is_spotify()
        && let Some(st) = state
    {
        return Some(st.replace("; ", ", ").into());
    }
    m
}

pub(crate) fn activity_icon(a: &crate::presence::Activity) -> Option<&'static str> {
    use crate::presence::ActivityKind as K;
    match a.kind {
        K::Playing | K::Competing => Some("game-controller"),
        K::Listening => Some("music"),
        K::Watching | K::Streaming => Some("tv"),
        K::Custom => None,
    }
}

impl SubText {
    fn of(acts: &[crate::presence::Activity], go_live: bool) -> Option<Self> {
        use crate::presence::ActivityKind as K;
        let custom = acts
            .iter()
            .find(|a| a.kind == K::Custom)
            .map(|a| (a.emoji.clone(), a.state.as_ref().map(|t| SharedString::from(t.trim().to_string())).filter(|t| !t.is_empty())))
            .filter(|(e, t)| e.is_some() || t.is_some());
        let stream_game = acts.iter().find(|a| go_live && a.kind == K::Playing);
        let mut seen: Vec<&crate::presence::Activity> = Vec::new();
        for a in acts.iter().filter(|a| a.kind != K::Custom && stream_game.is_none_or(|g| a.name != g.name)) {
            let same = |b: &&crate::presence::Activity| {
                (a.application_id.is_some() && b.application_id.is_some() && a.application_id == b.application_id) || a.name == b.name
            };
            if !seen.iter().any(same) {
                seen.push(a);
            }
        }
        let main = if go_live {
            stream_game.map(|g| (g.name.clone(), Some("tv")))
        } else {
            seen.first().map(|a| (activity_text(a).unwrap_or_default(), activity_icon(a)))
        };
        let count = seen.len();
        (custom.is_some() || main.is_some()).then_some(Self { custom, main, count })
    }
}

fn sub_text(in_voice: bool, voice_icon: &'static str, bot: bool, sub: Option<SubText>, fallback: Option<SharedString>) -> gpui::Div {
    let green = c(tok(idx::STATUS_ONLINE));
    let (custom, main, count) = sub.map(|s| (s.custom, s.main, s.count)).unwrap_or((None, None, 0));
    let total = count + usize::from(in_voice);
    let custom_text = custom.as_ref().is_some_and(|c| c.1.is_some());
    let text_el = |t: SharedString| div().min_w_0().font_weight(FontWeight::MEDIUM).text_ellipsis().child(crate::ui::emoji_text::status_label(t, 14. * crate::theme::zoom() / 1.375));
    let mut row = div()
        .mt(z(-2.))
        .h(z(16.))
        .flex()
        .items_center()
        .gap(z(4.))
        .text_size(z(12.))
        .line_height(z(16.))
        .text_color(c(Palette::TEXT_MUTED))
        .overflow_hidden();
    let piece = match main {
        Some((t, icon)) => Some((Some(t), icon)),
        None if in_voice => Some((Some(crate::t!("members.in_voice")), Some(voice_icon))),
        None => None,
    };
    if let Some((t, icon)) = piece {
        if let Some(i) = icon.filter(|_| !bot) {
            row = row.child(gpui::svg().path(crate::icons::path(i)).size(z(12.)).flex_none().text_color(green));
        }
        if let Some(t) = t.filter(|t| !custom_text && !t.is_empty()) {
            row = row.child(text_el(t));
        }
        if total > 1 && !bot {
            row = row.child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .when(custom_text, |d| d.text_color(c(tok(idx::STATUS_POSITIVE))))
                    .child(SharedString::from(format!("+{}", total - 1))),
            );
        }
    } else if custom.is_none()
        && let Some(t) = fallback.filter(|_| !in_voice)
    {
        row = row.child(text_el(t));
    }
    if let Some((emoji, state)) = custom {
        if total > 0 {
            row = row.child(div().flex_none().font_weight(FontWeight::NORMAL).text_color(c(Palette::TEXT_NORMAL)).child("•"));
        }
        if let Some(e) = emoji {
            use crate::ui::tooltip::Tipped as _;
            let el = status_emoji(&e, 14.).debug_selector(|| "status-emoji".to_owned());
            row = row.child(if state.is_none() {
                el.tip_delayed(crate::ui::tooltip::Body::Plain(crate::ui::emoji_text::emoji_tip(&e)), crate::ui::tooltip::Side::Top, 150)
            } else {
                el
            });
        }
        if let Some(t) = state {
            row = row.child(text_el(t).debug_selector(|| "status-text".to_owned()));
        }
    }
    row
}

pub(crate) fn status_emoji(token: &str, size: f32) -> gpui::Div {
    div().flex_none().size(z(size)).flex().items_center().overflow_hidden().child(crate::media::emoji::emoji_element(token, size * crate::theme::zoom()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presence::{Activity, ActivityKind};

    struct StatusRow(f32);

    impl gpui::Render for StatusRow {
        fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let custom = Some((Some(SharedString::from("<:rip:123>")), Some(SharedString::from("Repose en paix Happy 03/11/2026"))));
            div().w(gpui::px(self.0)).child(sub_text(false, "voice", false, Some(SubText { custom, main: None, count: 0 }), None))
        }
    }

    #[gpui::test]
    fn custom_status_text_starts_after_the_emoji_and_the_gap(cx: &mut gpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| StatusRow(166.));
        let emoji = cx.debug_bounds("status-emoji").unwrap();
        let text = cx.debug_bounds("status-text").unwrap();
        assert_eq!(emoji.size.width, gpui::px(14.));
        assert_eq!(text.origin.x - emoji.origin.x, gpui::px(18.));
        assert_eq!(text.size.width, gpui::px(148.));
    }

    fn act(kind: ActivityKind, name: &str, app_id: Option<&str>, state: Option<&str>) -> Activity {
        Activity {
            kind,
            name: name.to_owned().into(),
            details: None,
            state: state.map(|s| s.to_owned().into()),
            emoji: None,
            url: None,
            display_type: None,
            image: None,
            start: None,
            application_id: app_id.map(|s| s.to_owned().into()),
            party: None,
            platform: None,
            flags: 0,
        }
    }

    fn total(acts: &[Activity], go_live: bool, in_voice: bool) -> usize {
        SubText::of(acts, go_live).map_or(0, |s| s.count) + usize::from(in_voice)
    }

    #[test]
    fn game_and_custom_status_has_no_counter() {
        let custom = act(ActivityKind::Custom, "Custom Status", None, Some("Intensely doing nothing"));
        let game = act(ActivityKind::Playing, "VALORANT", Some("700136079562375258"), None);
        let s = SubText::of(&[custom.clone(), game.clone()], false).unwrap();
        assert_eq!(s.count, 1);
        assert!(s.custom.is_some());
        assert_eq!(total(&[custom.clone(), game.clone()], false, false), 1);
        assert_eq!(total(&[game, custom], false, false), 1);
    }

    #[test]
    fn same_application_or_same_name_counts_once() {
        let game = act(ActivityKind::Playing, "VALORANT", Some("1"), None);
        let same_app = act(ActivityKind::Playing, "VALORANT RPC", Some("1"), None);
        let same_name = act(ActivityKind::Playing, "VALORANT", None, None);
        let other_case = act(ActivityKind::Playing, "valorant", None, None);
        let spotify = act(ActivityKind::Listening, "Spotify", None, Some("Track"));
        assert_eq!(total(&[game.clone(), same_app, same_name], false, false), 1);
        assert_eq!(total(&[game.clone(), other_case], false, false), 2);
        assert_eq!(total(&[game, spotify], false, false), 2);
    }

    #[test]
    fn missing_application_ids_do_not_merge_different_names() {
        let a = act(ActivityKind::Playing, "Minecraft", None, None);
        let b = act(ActivityKind::Playing, "Terraria", None, None);
        assert_eq!(total(&[a, b], false, false), 2);
    }

    #[test]
    fn voice_channel_adds_one() {
        let game = act(ActivityKind::Playing, "VALORANT", Some("1"), None);
        assert_eq!(total(std::slice::from_ref(&game), false, true), 2);
        assert_eq!(total(&[], false, true), 1);
    }

    #[test]
    fn go_live_replaces_the_streamed_game() {
        let custom = act(ActivityKind::Custom, "Custom Status", None, Some("hi"));
        let game = act(ActivityKind::Playing, "VALORANT", Some("1"), None);
        let spotify = act(ActivityKind::Listening, "Spotify", None, Some("Track"));
        let s = SubText::of(&[custom.clone(), game.clone()], true).unwrap();
        assert_eq!(s.count, 0);
        assert_eq!(s.main.map(|m| m.0.to_string()).as_deref(), Some("VALORANT"));
        assert_eq!(total(&[custom, game.clone()], true, true), 1);
        assert_eq!(total(&[game, spotify], true, true), 2);
    }
}
