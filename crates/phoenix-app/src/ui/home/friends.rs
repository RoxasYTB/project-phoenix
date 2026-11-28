use crate::ui::tooltip::Tipped as _;
use crate::theme::z;
use gpui::{
    AnyElement, Context, Focusable, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div,
    prelude::*, px,
};
use gpui_component::input::{Input, InputEvent, InputState};
use std::collections::HashMap;

use crate::{
    app::PhoenixApp,
    assets::Icon,
    model::{Id, Presence},
    theme::{Palette, c, generated::idx, m, tok},
    ui::{
        badge, icon,
        home::data::{FriendsTab, HomeView, NavRequest, RelAction, Relationship, RelationshipKind},
    },
};

pub fn render(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    if app.nav.home == HomeView::MessageRequests {
        return crate::ui::home::requests::render(app, cx);
    }
    if app.nav.friends_tab == FriendsTab::AddFriend && app.nav.add_friend_input.is_none() {
        ensure_add_input(app, window, cx);
    }
    if app.nav.friends_search.is_none() {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("search.placeholder")));
        let sub = cx.subscribe_in(&input, window, |_: &mut PhoenixApp, _, ev: &InputEvent, _w, cx| {
            if let InputEvent::Change = ev {
                cx.notify();
            }
        });
        app.nav.subs.push(sub);
        app.nav.friends_search = Some(input);
    }
    if let Some(i) = app.nav.friends_search.clone() {
        crate::ui::locale_inputs::keep_placeholder(&i, "search.placeholder", window, cx);
    }
    if let Some(i) = app.nav.add_friend_input.clone() {
        crate::ui::locale_inputs::keep_placeholder(&i, "friends.add_placeholder", window, cx);
        if app.nav.add_friend_reset {
            i.update(cx, |s, cx| {
                s.set_value("", window, cx);
                s.focus(window, cx);
            });
        }
    }
    if let Some(n) = app.nav.add_friend_note.clone()
        && std::mem::take(&mut app.nav.add_friend_reset)
    {
        n.update(cx, |s, cx| s.set_value("", window, cx));
    }
    let body: AnyElement = match app.nav.friends_tab {
        FriendsTab::AddFriend => add_friend(app, window, cx).into_any_element(),
        tab => list(app, tab, cx).into_any_element(),
    };
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .bg(c(Palette::BG_PRIMARY))
        .child(header(app, cx))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_row()
                .child(div().flex_1().min_w_0().h_full().child(body))
                .when(crate::ui::title_bar::active_now_shown(crate::ui::title_bar::viewport_w_css()), |d| d.child(active_now(app, cx))),
        )
        .into_any_element()
}

fn ensure_add_input(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    use crate::ui::home::add_friend::{NOTE_MAX, TAG_MAX};
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("friends.add_placeholder")));
    let last = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let sub = cx.subscribe_in(&input, window, move |this: &mut PhoenixApp, st, ev: &InputEvent, window, cx| match ev {
        InputEvent::PressEnter { .. } => {
            if this.nav.add_friend.can_send {
                send_request(this, cx);
            }
        }
        InputEvent::Change => {
            let now = st.read(cx).value().to_string();
            let now = match crate::ui::modals::clamp_insert(&last.borrow(), &now, TAG_MAX) {
                Some((text, cursor)) => {
                    st.update(cx, |s, cx| {
                        s.set_value(text.clone(), window, cx);
                        s.set_selected_range(cursor..cursor, cx);
                    });
                    text
                }
                None => now,
            };
            this.nav.add_friend.on_change(&now);
            this.nav.add_friend_success_tag = None;
            *last.borrow_mut() = now;
            cx.notify();
        }
        _ => {}
    });
    app.nav.subs.push(sub);
    app.nav.add_friend_input = Some(input);
    let note = cx.new(|cx| {
        gpui_component::input::TextareaState::new(window, cx).auto_grow(2, 2).placeholder(crate::t!("friends.add_note_placeholder"))
    });
    let last = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let sub = cx.subscribe_in(&note, window, move |_: &mut PhoenixApp, st, ev: &InputEvent, window, cx| {
        if !matches!(ev, InputEvent::Change) {
            return;
        }
        let raw = st.read(cx).value().to_string();
        let flat = raw.replace('\n', "");
        let next = crate::ui::modals::clamp_insert(&last.borrow(), &flat, NOTE_MAX).map(|(t, _)| t).unwrap_or(flat);
        if next != raw {
            let cursor = next.len();
            st.update(cx, |s, cx| {
                s.set_value(next.clone(), window, cx);
                s.set_selected_range(cursor..cursor, cx);
            });
        }
        *last.borrow_mut() = next;
        cx.notify();
    });
    app.nav.subs.push(sub);
    app.nav.add_friend_note = Some(note);
}

fn send_request(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    use crate::ui::home::add_friend as af;
    let Some(input) = app.nav.add_friend_input.clone() else { return };
    let tag = af::normalize(&input.read(cx).value());
    let raw_note = app.nav.add_friend_note.as_ref().map(|n| n.read(cx).value().to_string()).unwrap_or_default();
    if !af::valid(&tag) {
        app.nav.add_friend.on_error(crate::t!("friends.request_failed"));
        cx.notify();
        return;
    }
    if af::note_too_long(&raw_note) {
        app.nav.add_friend.on_error(crate::t!("friends.err_note"));
        cx.notify();
        return;
    }
    let note = af::clean_note(&raw_note);
    app.nav_request(NavRequest::Relationship { user: Id(0), action: RelAction::AddByTag { tag: tag.clone(), note } }, cx, move |this, res, _cx| {
        match res {
            Ok(_) => {
                this.nav.add_friend.on_success(&tag);
                this.nav.add_friend_success_tag = Some(tag.clone().into());
                this.nav.add_friend_reset = true;
            }
            Err(e) => {
                let f = af::Failure::decode(&e.0).unwrap_or(af::Failure { status: 0, code: None, note_rejected: false });
                if let Some(d) = af::dialog(&f, &tag) {
                    open_failure_dialog(d);
                }
                this.nav.add_friend.on_error(af::inline_error(&f, &tag));
            }
        }
    });
}

fn open_failure_dialog(d: crate::ui::home::add_friend::Dialog) {
    use crate::ui::home::add_friend::Dialog;
    let (title, body, ok) = match d {
        Dialog::TooFast => (crate::t!("friends.too_fast_title"), crate::t!("friends.too_fast_body"), crate::t!("friends.too_fast_ok")),
        Dialog::VerifyEmail => (crate::t!("friends.verify_title"), crate::t!("friends.verify_body"), crate::t!("friends.verify_ok")),
        Dialog::Failed(body) => (crate::t!("friends.fail_title"), body, crate::t!("friends.fail_ok")),
    };
    crate::actions::open_modal(
        crate::ui::modals::Modal::Form(crate::ui::modals::Form {
            title,
            subtitle: Some(body),
            fields: Vec::new(),
            choices: Vec::new(),
            choice: None,
            check: None,
            submit: ok,
            danger: false,
            on_submit: std::rc::Rc::new(|_, _, _, _| {}),
            preview: None,
            width: 440.,
        }),
        None,
    );
}

pub fn pending_badge(rels: &[Relationship]) -> usize {
    rels.iter().filter(|r| r.kind == RelationshipKind::Incoming && !r.spam && !r.ignored).count()
}

pub fn initial_tab(rels: &[Relationship]) -> FriendsTab {
    let n = |k| rels.iter().filter(|r| r.kind == k).count();
    if n(RelationshipKind::Friend) > 0 {
        FriendsTab::Online
    } else if n(RelationshipKind::Incoming) > 0 {
        FriendsTab::Pending
    } else {
        FriendsTab::AddFriend
    }
}

fn header(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let pending = app.nav.pending_requests();
    let friends = app.nav.count(RelationshipKind::Friend);
    let online = app.nav.relationships.iter().any(|r| r.kind == RelationshipKind::Friend && app.presence_of(r.user) != Presence::Offline);
    let has_pending = app.nav.count(RelationshipKind::Incoming) + app.nav.count(RelationshipKind::Outgoing) > 0;
    let pill = || {
        div()
            .mx(z(8.))
            .px(z(12.))
            .py(z(4.))
            .h(z(32.))
            .rounded(z(8.))
            .flex()
            .items_center()
            .gap(z(6.))
            .cursor_pointer()
            .text_size(z(16.))
            .line_height(z(20.))
    };
    let tab = |id: &'static str, label: SharedString, t: FriendsTab, cx: &mut Context<PhoenixApp>| {
        let sel = app.nav.friends_tab == t;
        pill()
            .id(id)
            .text_color(c(tok(if sel { idx::TEXT_STRONG } else { idx::TEXT_SUBTLE })))
            .when(sel, |d| d.bg(c(tok(idx::BACKGROUND_MOD_STRONG))))
            .when(!sel, |d| {
                d.hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))).text_color(c(tok(idx::TEXT_STRONG))))
                    .active(|s| s.bg(c(tok(idx::BACKGROUND_MOD_STRONG))).text_color(c(tok(idx::TEXT_STRONG))))
            })
            .on_click(cx.listener(move |a, _, _, cx| {
                a.nav.friends_tab = t;
                cx.notify();
            }))
            .child(label)
            .when(t == FriendsTab::Pending && pending > 0, |d| d.child(badge(pending)))
    };
    let add_sel = app.nav.friends_tab == FriendsTab::AddFriend;
    div()
        .h(px(m::HEADER_H))
        .flex_none()
        .pl(z(20.))
        .pr(z(8.))
        .py(z(8.))
        .flex()
        .items_center()
        .border_b_1()
        .border_color(c(Palette::HEADER_SHADOW))
        .child(div().size(z(32.)).flex().items_center().justify_center().child(icon(Icon::Friends, 20. * crate::theme::zoom(), c(Palette::CHANNEL_ICON))))
        .child(
            div()
                .mr(z(8.))
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(Palette::TEXT_HEADER))
                .child(crate::t!("friends.title")),
        )
        .child(div().mx(z(4.)).size(z(4.)).rounded_full().bg(c(Palette::BG_MOD_SELECTED)))
        .when(online, |d| d.child(tab("tab-online", crate::t!("status.online"), FriendsTab::Online, cx)))
        .when(friends > 0, |d| d.child(tab("tab-all", crate::t!("friends.all"), FriendsTab::All, cx)))
        .when(has_pending, |d| d.child(tab("tab-pending", crate::t!("friends.pending"), FriendsTab::Pending, cx)))
        .child(
            pill()
                .id("tab-add")
                .when(add_sel, |d| d.bg(c(tok(idx::OPACITY_BLURPLE_16))).text_color(c(tok(idx::TEXT_BRAND))))
                .when(!add_sel, |d| {
                    d.bg(c(tok(idx::CONTROL_PRIMARY_BACKGROUND_DEFAULT)))
                        .text_color(gpui::white())
                        .hover(|s| s.bg(c(tok(idx::CONTROL_PRIMARY_BACKGROUND_HOVER))))
                        .active(|s| s.bg(c(tok(idx::CONTROL_PRIMARY_BACKGROUND_ACTIVE))))
                })
                .on_click(cx.listener(|a, _, _, cx| {
                    a.nav.friends_tab = FriendsTab::AddFriend;
                    cx.notify();
                }))
                .child(crate::t!("friends.add_tab")),
        )
        .child(div().flex_1())
        .when(crate::ui::title_bar::friends_invite_shown(crate::ui::title_bar::viewport_w_css()), |d| {
            d.child(
                div()
                    .pl(z(10.))
                    .pr(z(8.))
                    .child(
                        div()
                            .id("fr-new-group-dm")
                            .group("fr-new-group-dm")
                            .size(z(32.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .tip(crate::t!("friends.new_group_dm"))
                            .on_click(cx.listener(|a, _, _, cx| a.open_group_dm_picker(cx)))
                            .child(
                                gpui::svg()
                                    .path(crate::icons::path("new-group-dm"))
                                    .size(z(20.))
                                    .text_color(c(tok(idx::ICON_MUTED)))
                                    .group_hover("fr-new-group-dm", |s| s.text_color(c(tok(idx::ICON_SUBTLE)))),
                            ),
                    ),
            )
        })
}

pub fn in_tab(r: &Relationship, tab: FriendsTab, online: bool) -> bool {
    match tab {
        FriendsTab::Online => r.kind == RelationshipKind::Friend && online,
        FriendsTab::All => r.kind == RelationshipKind::Friend,
        FriendsTab::Pending => (r.kind == RelationshipKind::Incoming && !r.spam && !r.ignored) || r.kind == RelationshipKind::Outgoing,
        FriendsTab::Blocked => r.kind == RelationshipKind::Blocked,
        FriendsTab::AddFriend => false,
    }
}

pub fn order(rows: &mut [Relationship], key: impl Fn(&Relationship) -> String, tab: FriendsTab) {
    let rank = |k: RelationshipKind| match k {
        RelationshipKind::None => 0,
        RelationshipKind::Friend => 1,
        RelationshipKind::Blocked => 2,
        RelationshipKind::Incoming => 3,
        RelationshipKind::Outgoing => 4,
    };
    rows.sort_by_cached_key(|r| (rank(r.kind), key(r)));
    if tab == FriendsTab::Pending {
        rows.sort_by(|a, b| {
            rank(a.kind).cmp(&rank(b.kind)).then_with(|| if a.kind == RelationshipKind::Incoming { b.since.cmp(&a.since) } else { std::cmp::Ordering::Equal })
        });
    }
}

pub fn matches_query(r: &Relationship, username: &str, global: Option<&str>, q: &str) -> bool {
    q.is_empty()
        || [Some(username), r.nickname.as_deref(), global].into_iter().flatten().any(|v| v.to_lowercase().contains(q))
}

fn visible(app: &PhoenixApp, tab: FriendsTab) -> Vec<Relationship> {
    let mut v: Vec<Relationship> = app
        .nav
        .relationships
        .iter()
        .filter(|r| app.users.contains_key(&r.user) && in_tab(r, tab, app.presence_of(r.user) != Presence::Offline))
        .cloned()
        .collect();
    order(
        &mut v,
        |r| {
            let u = app.users.get(&r.user);
            r.nickname
                .as_ref()
                .map(|n| n.to_lowercase())
                .unwrap_or_else(|| u.map(|u| u.display_name.to_lowercase()).unwrap_or_default())
        },
        tab,
    );
    v
}

fn list(app: &PhoenixApp, tab: FriendsTab, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let q = app.nav.friends_search.as_ref().map(|i| i.read(cx).value().to_lowercase()).unwrap_or_default();
    let rows: Vec<Relationship> = visible(app, tab)
        .into_iter()
        .filter(|r| app.users.get(&r.user).is_some_and(|u| matches_query(r, &u.name, Some(u.display_name.as_ref()), &q)))
        .collect();
    let others = tab == FriendsTab::Pending
        && app.nav.relationships.iter().any(|r| r.kind == RelationshipKind::Incoming && (r.spam || r.ignored));
    let search = div()
        .flex_none()
        .mt(z(12.))
        .mr(z(16.))
        .mb(z(8.))
        .ml(z(24.))
        .h(z(40.))
        .pl(z(4.))
        .gap(z(4.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(Palette::BG_MOD_SELECTED))
        .bg(c(gpui_input_bg()))
        .flex()
        .items_center()
        .child(div().size(z(30.)).flex().items_center().justify_center().child(icon(Icon::Search, 16. * crate::theme::zoom(), c(Palette::TEXT_MUTED))))
        .when_some(app.nav.friends_search.clone(), |d, i| {
            d.child(div().flex_1().child(Input::new(&i).appearance(false).bordered(false).pl(z(0.)).text_size(z(16.)).line_height(z(20.))))
        });
    if rows.is_empty() && q.is_empty() {
        return empty_state(tab, others, cx).into_any_element();
    }
    let scroll = FRIENDS_SCROLL.with(|h| h.clone());
    let mut col =
        div().id("friends-list").size_full().overflow_y_scroll().track_scroll(&scroll).pb(z(8.)).pr(z(16.)).flex().flex_col();
    if rows.is_empty() {
        col = col.child(
            div().flex_1().flex().items_center().justify_center().child(
                div()
                    .w_full()
                    .max_w(z(440.))
                    .mt(z(8.))
                    .text_center()
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .text_color(c(tok(idx::TEXT_MUTED)))
                    .child(crate::t!("friends.empty_search")),
            ),
        );
    } else {
        let incoming = rows.iter().filter(|r| r.kind == RelationshipKind::Incoming).count();
        let mut sections: Vec<(SharedString, Vec<Relationship>)> = Vec::new();
        match tab {
            FriendsTab::Pending => {
                let (inc, out): (Vec<_>, Vec<_>) = rows.into_iter().partition(|r| r.kind == RelationshipKind::Incoming);
                if !inc.is_empty() {
                    sections.push((crate::t!("friends.sec_received", n = inc.len()), inc));
                }
                if !out.is_empty() {
                    sections.push((crate::t!("friends.sec_sent", n = out.len()), out));
                }
            }
            FriendsTab::Online => sections.push((crate::t!("friends.sec_online", n = rows.len()), rows)),
            FriendsTab::All => sections.push((crate::t!("friends.sec_all", n = rows.len()), rows)),
            _ => sections.push((crate::t!("friends.sec_blocked", n = rows.len()), rows)),
        }
        for (n, (title, rows)) in sections.into_iter().enumerate() {
            let clear = tab == FriendsTab::Pending && n == 0 && incoming >= 5;
            col = col.child(
                div()
                    .h(z(50.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .ml(z(24.))
                            .mr(z(20.))
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(Palette::TEXT_NORMAL))
                            .child(title),
                    )
                    .when(clear, |d| {
                        d.child(
                            text_button("fr-clear-all", crate::t!("friends.clear_all"), 14., 18., idx::TEXT_BRAND)
                                .mr(z(24.))
                                .on_click(cx.listener(move |a, _, _, cx| a.confirm_clear_pending(incoming, cx))),
                        )
                    }),
            );
            for r in rows {
                col = col.child(div().flex_none().h(z(1.)).ml(z(30.)).mr(z(20.)).bg(c(Palette::DIVIDER)));
                col = col.child(friend_row(app, &r, cx));
            }
        }
        if others {
            col = col.child(
                text_button("fr-other-requests", crate::t!("friends.view_other_requests"), 12., 16., idx::TEXT_STRONG)
                    .mx(z(24.))
                    .py(z(4.))
                    .pr(z(8.))
                    .on_click(cx.listener(|a, _, _, cx| a.open_spam_requests(cx))),
            );
        }
    }
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(search)
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(col)
                .children(crate::ui::chat::scrollbar::auto_scrollbar_div(&scroll)),
        )
        .into_any_element()
}

fn text_button(id: &'static str, label: SharedString, size: f32, line: f32, color: usize) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .self_start()
        .cursor_pointer()
        .text_size(z(size))
        .line_height(z(line))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(color)))
        .hover(|s| s.underline())
        .child(label)
}

fn empty_state(tab: FriendsTab, others: bool, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let msg = match tab {
        FriendsTab::Online => crate::t!("friends.empty_online"),
        FriendsTab::Pending => crate::t!("friends.empty_pending"),
        FriendsTab::Blocked => crate::t!("friends.empty_blocked"),
        _ => crate::t!("friends.empty_all"),
    };
    let button: Option<AnyElement> = match tab {
        FriendsTab::All => Some(
            crate::ui::controls::button("friends-empty-add", crate::t!("friends.add_tab"), crate::ui::controls::BtnKind::Primary)
                .mt(z(20.))
                .on_click(cx.listener(|a, _, _, cx| {
                    a.nav.friends_tab = FriendsTab::AddFriend;
                    cx.notify();
                }))
                .into_any_element(),
        ),
        FriendsTab::Pending if others => Some(
            crate::ui::controls::button("friends-empty-spam", crate::t!("friends.view_spam_requests"), crate::ui::controls::BtnKind::Secondary)
                .mt(z(20.))
                .h(z(32.))
                .min_w(z(60.))
                .px(z(12.))
                .text_size(z(14.))
                .line_height(z(18.))
                .on_click(cx.listener(|a, _, _, cx| a.open_spam_requests(cx)))
                .into_any_element(),
        ),
        _ => None,
    };
    div().size_full().flex().items_center().justify_center().child(
        div()
            .w_full()
            .max_w(z(440.))
            .flex()
            .flex_col()
            .items_center()
            .child(div().mt(z(8.)).text_center().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_MUTED))).child(msg))
            .children(button),
    )
}

thread_local! {
    static DISCOVERY_ARROW_W: std::cell::Cell<f32> = const { std::cell::Cell::new(20.) };
}

thread_local! {
    static FRIENDS_SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
    static ACTIVE_NOW_SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
}

fn gpui_input_bg() -> u32 {
    crate::theme::tok(crate::theme::generated::idx::INPUT_BACKGROUND_DEFAULT)
}

pub fn action_button(id: (&'static str, u64), name: &str, accept: bool, tip: SharedString) -> gpui::Stateful<gpui::Div> {
    round_button(id, name, c(tok(if accept { idx::ICON_FEEDBACK_POSITIVE } else { idx::ICON_FEEDBACK_CRITICAL })), tip)
}

fn round_button(id: (&'static str, u64), name: &str, hover: Hsla, tip: SharedString) -> gpui::Stateful<gpui::Div> {
    let g = SharedString::from(format!("{}-{}", id.0, id.1));
    div()
        .id(id)
        .group(g.clone())
        .size(z(36.))
        .flex_none()
        .rounded_full()
        .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .tip(tip)
        .child(
            gpui::svg()
                .path(crate::icons::path(name))
                .size(z(20.))
                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                .group_hover(g, move |s| s.text_color(hover)),
        )
}

fn friend_row(app: &PhoenixApp, r: &Relationship, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let user = r.user;
    let u = app.users.get(&user);
    let p = app.presence_of(user);
    let name = r
        .nickname
        .clone()
        .or_else(|| u.map(|u| u.display_name.clone()))
        .unwrap_or_else(|| SharedString::from(user.0.to_string()));
    let username = u.map(|u| u.name.clone()).unwrap_or_default();
    let sub: SharedString = match r.kind {
        RelationshipKind::None => SharedString::default(),
        RelationshipKind::Incoming | RelationshipKind::Outgoing => username.clone(),
        RelationshipKind::Blocked => crate::t!("friends.blocked"),
        RelationshipKind::Friend if r.ignored => crate::t!("friends.ignored"),
        RelationshipKind::Friend => app.nav.activities.get(&user).cloned().unwrap_or_else(|| {
            match p {
                Presence::Online => crate::t!("status.online"),
                Presence::Idle => crate::t!("status.idle"),
                Presence::Dnd => crate::t!("status.dnd"),
                Presence::Offline => crate::t!("status.offline"),
            }
        }),
    };
    let pending = matches!(r.kind, RelationshipKind::Incoming | RelationshipKind::Outgoing);
    let sub = friend_activity(app, user, r.kind).unwrap_or_else(|| sub.into_any_element());
    let mut actions = div().flex().gap(z(10.)).when(pending, |d| d.ml(z(8.)));
    let act = |action: RelAction| {
        cx.listener(move |a: &mut PhoenixApp, _: &gpui::ClickEvent, _, cx| {
            cx.stop_propagation();
            a.relationship_action(user, action.clone(), cx);
        })
    };
    let hover = c(tok(idx::INTERACTIVE_TEXT_HOVER));
    match r.kind {
        RelationshipKind::None => {}
        RelationshipKind::Friend => {
            actions = actions
                .child(round_button(("fr-msg", user.0), "chat", hover, crate::t!("profile.message")).on_click(cx.listener(move |a, _, _, cx| {
                    cx.stop_propagation();
                    a.open_dm_with(user, cx)
                })))
                .child({
                    round_button(("fr-more", user.0), "more-vertical", hover, crate::t!("header.more")).on_click(move |ev, window, cx| {
                        cx.stop_propagation();
                        crate::ui::extra_menus::open(crate::ui::extra_menus::Target::FriendRow(user), ev.position(), window);
                    })
                });
        }
        RelationshipKind::Incoming => {
            actions = actions
                .child(action_button(("fr-ok", user.0), "check", true, crate::t!("friends.accept")).on_click(cx.listener(move |a, _, _, cx| {
                    cx.stop_propagation();
                    a.accept_friend_request(user, cx);
                })))
                .child(action_button(("fr-no", user.0), "close", false, crate::t!("friends.ignore")).on_click(act(RelAction::Remove)));
        }
        RelationshipKind::Outgoing => {
            actions = actions.child(action_button(("fr-cancel", user.0), "close", false, crate::t!("common.cancel")).on_click(act(RelAction::Remove)));
        }
        RelationshipKind::Blocked => {
            actions = actions.child(
                round_button(("fr-unblock", user.0), "members", c(tok(idx::ICON_FEEDBACK_CRITICAL)), crate::t!("friends.unblock"))
                    .on_click(act(RelAction::Remove)),
            );
        }
    }
    let revealed = app.nav.revealed_notes.contains(&user);
    let note_line: Option<AnyElement> = r.note.clone().filter(|_| pending).map(|note| {
        if r.kind == RelationshipKind::Incoming && !revealed {
            div()
                .id(("fr-note", user.0))
                .flex()
                .items_center()
                .gap(z(4.))
                .cursor_pointer()
                .text_size(z(14.))
                .line_height(z(17.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_BRAND)))
                .hover(|s| s.underline())
                .on_click(cx.listener(move |a, _, _, cx| {
                    cx.stop_propagation();
                    a.nav.revealed_notes.insert(user);
                    cx.notify();
                }))
                .child(gpui::svg().path(crate::icons::path("chat/eye")).size(z(16.)).text_color(c(tok(idx::TEXT_BRAND))))
                .child(crate::t!("friends.view_request"))
                .into_any_element()
        } else {
            div()
                .flex()
                .items_start()
                .gap(z(4.))
                .italic()
                .text_size(z(14.))
                .line_height(z(17.5))
                .text_color(c(tok(idx::TEXT_SUBTLE)))
                .child(div().pt(z(3.)).child(gpui::svg().path(crate::icons::path("settings/quote")).size(z(12.)).text_color(c(tok(idx::ICON_SUBTLE)))))
                .child(div().min_w_0().text_ellipsis().child(note))
                .into_any_element()
        }
    });
    let tag = u.and_then(|u| u.tag.clone());
    div()
        .id(("friend", user.0))
        .group("friend-row")
        .h(z(61.))
        .flex_none()
        .ml(z(14.))
        .mr(z(10.))
        .px(z(10.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
        .on_click(cx.listener(move |a, _, _, cx| if pending { a.open_profile_modal(user, cx) } else { a.open_dm_with(user, cx) }))
        .on_mouse_down(gpui::MouseButton::Right, move |ev, w, cx| {
            cx.stop_propagation();
            crate::ui::user_menu::open(user, None, ev.position, w);
        })
        .child(div().mr(z(12.)).child(if pending && p == Presence::Offline {
            div()
                .relative()
                .size(z(32.))
                .child(crate::ui::avatar(u, 32.))
                .when_some(crate::ui::avatar_deco::source(u, None), |d, src| d.child(crate::ui::avatar_deco::layer(src, 32.)))
        } else {
            crate::ui::avatar_with_status_deco(
                u,
                32.,
                p,
                c(Palette::BG_PRIMARY),
                |d| d.group_hover("friend-row", |s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))),
                crate::ui::avatar_deco::source(u, None),
            )
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(z(20.))
                        .min_w_0()
                        .flex()
                        .items_center()
                        .child(
                            crate::fonts::styled_name(div(), u, false)
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c(Palette::TEXT_HEADER))
                                .text_ellipsis()
                                .child(name),
                        )
                        .when_some(tag, |d, t| d.child(crate::ui::members::server_tag(&t).ml(z(4.))))
                        .when(!pending, |d| d.child(
                            div()
                                .ml(z(5.))
                                .text_size(z(14.))
                                .line_height(z(16.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(c(Palette::TEXT_NORMAL))
                                .opacity(0.)
                                .group_hover("friend-row", |s| s.opacity(1.))
                                .child(username),
                        )),
                )
                .map(|d| match note_line {
                    Some(n) => d.child(n),
                    None => d.child(
                        div()
                            .text_size(z(14.))
                            .line_height(z(if pending { 18. } else { 17.5 }))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(if pending { tok(idx::TEXT_SUBTLE) } else { Palette::TEXT_MUTED }))
                            .text_ellipsis()
                            .child(sub),
                    ),
                }),
        )
        .child(actions)
}

fn friend_activity(app: &PhoenixApp, user: Id, kind: RelationshipKind) -> Option<gpui::AnyElement> {
    use crate::presence::ActivityKind as K;
    if kind != RelationshipKind::Friend || app.presence_of(user) == Presence::Offline {
        return None;
    }
    let acts = app.people.activities.get(&user)?;
    let main = acts.iter().find(|a| a.kind != K::Custom);
    let custom = acts
        .iter()
        .find(|a| a.kind == K::Custom)
        .map(|a| (a.emoji.clone(), a.state.clone().filter(|t| !t.trim().is_empty())))
        .filter(|(e, t)| e.is_some() || t.is_some());
    if main.is_none() && custom.is_none() {
        return None;
    }
    let label = |t: SharedString| div().min_w_0().text_ellipsis().whitespace_nowrap().overflow_hidden().child(crate::ui::emoji_text::status_label(t, 16. * crate::theme::zoom() / 1.375));
    let mut row = div().min_w_0().flex().items_center().gap(z(4.)).overflow_hidden();
    if let Some(a) = main {
        if let Some(icon) = crate::ui::members::activity_icon(a) {
            row = row.child(gpui::svg().path(crate::icons::path(icon)).size(z(14.)).flex_none().text_color(c(tok(idx::STATUS_ONLINE))));
        }
        if let Some(t) = crate::ui::members::activity_text(a) {
            row = row.child(label(t));
        }
    }
    if let Some((emoji, state)) = custom {
        if main.is_some() {
            row = row.child(div().flex_none().child("•"));
        }
        if let Some(e) = emoji {
            row = row.child(crate::ui::members::status_emoji(&e, 16.));
        }
        if let Some(t) = state {
            row = row.child(label(t));
        }
    }
    Some(row.into_any_element())
}

pub const WUMPUS_WAVING: &str = "https://discord.com/assets/25c27c1328c986f6.svg";

fn add_friend(app: &PhoenixApp, window: &Window, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    use crate::ui::home::add_friend::NOTE_MAX;
    let input = app.nav.add_friend_input.clone();
    let note = app.nav.add_friend_note.clone();
    let form = app.nav.add_friend.clone();
    let note_len: usize = note.as_ref().map(|n| n.read(cx).value().chars().map(char::len_utf16).sum()).unwrap_or(0);
    let focused = input.as_ref().is_some_and(|i| i.read(cx).focus_handle(cx).contains_focused(window, cx))
        || note.as_ref().is_some_and(|n| n.read(cx).focus_handle(cx).contains_focused(window, cx));
    let (bg, border) = if form.error.is_some() {
        (tok(idx::INPUT_BACKGROUND_ERROR_DEFAULT), tok(idx::INPUT_BORDER_ERROR_DEFAULT))
    } else if form.success.is_some() {
        (tok(idx::INPUT_BACKGROUND_DEFAULT), tok(idx::GREEN_360))
    } else if focused {
        (tok(idx::INPUT_BACKGROUND_DEFAULT), tok(idx::TEXT_LINK))
    } else {
        (tok(idx::INPUT_BACKGROUND_DEFAULT), tok(idx::INPUT_BORDER_DEFAULT))
    };
    let can_send = form.can_send;
    let send = crate::ui::controls::button("send-fr", crate::t!("friends.send_request"), crate::ui::controls::BtnKind::Primary)
        .h(z(32.))
        .min_w(z(60.))
        .px(z(12.))
        .text_size(z(14.))
        .line_height(z(18.))
        .when(!can_send, |b| b.opacity(0.5).cursor_default())
        .on_click(cx.listener(move |a, _, _, cx| {
            if a.nav.add_friend.can_send {
                send_request(a, cx);
            }
        }));
    let status_line = |text: AnyElement| div().mt(z(8.)).text_size(z(14.)).line_height(z(18.)).child(text);
    let message: AnyElement = match (&form.error, &form.success) {
        (Some(e), _) => status_line(div().text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL))).child(e.clone()).into_any_element()).into_any_element(),
        (None, Some(ok)) => {
            let tag = app.nav.add_friend_success_tag.clone().unwrap_or_default();
            let text = gpui::StyledText::new(ok.clone()).with_highlights(
                ok.find(tag.as_ref())
                    .filter(|_| !tag.is_empty())
                    .map(|i| (i..i + tag.len(), gpui::HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() })),
            );
            status_line(div().text_color(c(tok(idx::TEXT_FEEDBACK_POSITIVE))).child(text).into_any_element()).into_any_element()
        }
        (None, None) => status_line(div().text_color(c(tok(idx::TEXT_SUBTLE))).child(crate::t!("friends.add_note_hint")).into_any_element())
            .into_any_element(),
    };
    let heading = |t: SharedString| {
        div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(t)
    };
    let lead = |t: SharedString| {
        div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(t)
    };
    let form_box = div()
        .mt(z(16.))
        .px(z(12.))
        .rounded(z(16.))
        .border_1()
        .border_color(c(border))
        .bg(c(bg))
        .flex()
        .flex_col()
        .child(
            div()
                .py(z(2.))
                .flex()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .mr(z(16.))
                        .h(z(52.))
                        .py(z(4.))
                        .flex()
                        .items_center()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .when_some(input, |d, i| d.child(Input::new(&i).appearance(false).bordered(false).pl(z(0.)).text_size(z(16.)).line_height(z(20.)))),
                )
                .child(send),
        )
        .child(
            div()
                .border_t_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .py(z(12.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(z(41.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::TEXT_SUBTLE)))
                        .when_some(note, |d, n| {
                            d.child(gpui_component::input::Textarea::new(&n).appearance(false).bordered(false).p(z(0.)).text_size(z(16.)).line_height(z(20.)))
                        }),
                )
                .child(
                    div().pt(z(4.)).flex().justify_end().child(
                        div()
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .text_color(c(tok(if note_len >= NOTE_MAX { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_SUBTLE })))
                            .child(SharedString::from(NOTE_MAX.saturating_sub(note_len).to_string())),
                    ),
                ),
        );
    let explore = div()
        .id("add-friend-discovery")
        .group("add-friend-discovery")
        .w_full()
        .min_w_0()
        .px(z(6.))
        .py(z(1.))
        .flex()
        .items_center()
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
        .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
        .active(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_ACTIVE))))
        .cursor_pointer()
        .on_click(cx.listener(|_, _, _, cx| {
            crate::ui::discovery::open(crate::ui::discovery::Tab::Servers);
            cx.notify();
        }))
        .child(
            div()
                .m(z(8.))
                .size(z(36.))
                .flex_none()
                .rounded(z(8.))
                .bg(c(tok(idx::GREEN_360)))
                .flex()
                .items_center()
                .justify_center()
                .child(gpui::svg().path(crate::icons::path("compass")).size(z(24.)).text_color(gpui::white())),
        )
        .child(
            div()
                .flex_initial()
                .min_w_0()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .child(crate::t!("friends.explore_discovery")),
        )
        .child({
            let w = DISCOVERY_ARROW_W.with(std::cell::Cell::get);
            div()
                .relative()
                .flex_initial()
                .flex_basis(z(20.))
                .min_w_0()
                .h(z(20.))
                .ml_auto()
                .mr(z(16.))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    gpui::svg()
                        .path(crate::icons::path("friends/discovery-arrow"))
                        .flex_none()
                        .size(gpui::px(w))
                        .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                        .group_hover("add-friend-discovery", |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER)))),
                )
                .child(
                    gpui::canvas(
                        |b, window, _| {
                            let side = f32::from(b.size.width).min(f32::from(b.size.height));
                            if (DISCOVERY_ARROW_W.with(std::cell::Cell::get) - side).abs() > 0.1 {
                                DISCOVERY_ARROW_W.with(|c| c.set(side));
                                window.refresh();
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                )
        });
    div()
        .id("add-friend-page")
        .size_full()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .child(
            div()
                .flex_none()
                .px(z(30.))
                .py(z(20.))
                .border_b_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .flex()
                .flex_col()
                .child(heading(crate::t!("friends.h_add")).mb(z(8.)))
                .child(
                    div()
                        .relative()
                        .pr(z(128.))
                        .flex()
                        .items_start()
                        .child(lead(crate::t!("friends.add_hint")).flex_1().min_w_0())
                        .child(div().absolute().right(z(16.)).bottom(z(-16.)).w(z(84.)).h(z(72.)).child(gpui::img(WUMPUS_WAVING).size_full())),
                )
                .child(form_box)
                .child(message),
        )
        .child(
            div()
                .flex_none()
                .px(z(30.))
                .py(z(20.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(heading(crate::t!("friends.other_places")))
                .child(lead(crate::t!("friends.other_places_body"))),
        )
        .child(div().flex_none().px(z(20.)).flex().gap(z(32.)).child(div().flex_1().min_w_0().child(explore)).child(div().flex_1()))
}

fn active_now_width() -> f32 {
    let vw = crate::ui::title_bar::viewport_w_css();
    if vw <= 0. {
        return 360.;
    }
    crate::ui::title_bar::active_now_w(vw - crate::theme::m::SERVER_BAR_W - crate::ui::sidebar_resize::width())
}

fn active_now(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let scroll = ACTIVE_NOW_SCROLL.with(|h| h.clone());
    div()
        .relative()
        .flex_none()
        .h_full()
        .child(active_now_column(app, cx).track_scroll(&scroll))
        .children(crate::ui::chat::scrollbar::thin_scrollbar_div(&scroll))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Affinity {
    pub user: Id,
    pub communication_rank: u32,
    pub communication_probability: f64,
    pub vc_probability: f64,
}

pub const AFFINITY_RANK_MAX: u32 = 15;
pub const VOICE_AFFINITY_EXPERIMENT: &str = "2025-08-voice-user-affinity";
const AFFINITY_TTL: std::time::Duration = std::time::Duration::from_secs(8 * 3600);

pub fn personalization_consented(v: &serde_json::Value) -> bool {
    v.get("personalization").and_then(|p| p.get("consented")).and_then(serde_json::Value::as_bool).unwrap_or(false)
}

pub fn parse_affinities(v: &serde_json::Value) -> Vec<Affinity> {
    let num = |e: &serde_json::Value, k: &str| e.get(k).and_then(serde_json::Value::as_f64).unwrap_or(0.);
    v.get("user_affinities")
        .and_then(serde_json::Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|e| {
                    let user = e.get("other_user_id")?.as_str()?.parse().ok().map(Id)?;
                    Some(Affinity {
                        user,
                        communication_rank: num(e, "communication_rank") as u32,
                        communication_probability: num(e, "communication_probability"),
                        vc_probability: num(e, "vc_probability"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn now_playing_ids(affinities: &[Affinity], friends: &[Id]) -> Vec<Id> {
    let mut out: Vec<Id> = Vec::new();
    for u in affinities.iter().filter(|a| a.communication_rank <= AFFINITY_RANK_MAX).map(|a| a.user).chain(friends.iter().copied()) {
        if !out.contains(&u) {
            out.push(u);
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AffinitySort {
    Voice,
    Communication,
}

pub fn affinity_sort(bucket: i64) -> Option<AffinitySort> {
    match bucket {
        1 => Some(AffinitySort::Voice),
        2 => Some(AffinitySort::Communication),
        _ => None,
    }
}

#[derive(Default)]
struct AffinityState {
    list: Vec<Affinity>,
    fetched: Option<std::time::Instant>,
    fetching: bool,
    sort: Option<Option<AffinitySort>>,
    sort_fetching: bool,
}

thread_local! {
    static AFFINITIES: std::cell::RefCell<AffinityState> = std::cell::RefCell::new(AffinityState::default());
}

fn load_affinities(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let (fetch, fetch_sort) = AFFINITIES.with(|a| {
        let mut a = a.borrow_mut();
        let fetch = !a.fetching && a.fetched.is_none_or(|t| t.elapsed() > AFFINITY_TTL);
        let fetch_sort = a.sort.is_none() && !a.sort_fetching;
        a.fetching |= fetch;
        a.sort_fetching |= fetch_sort;
        (fetch, fetch_sort)
    });
    if fetch {
        let fut = app.backend.user_affinities();
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |_, cx| {
                AFFINITIES.with(|a| {
                    let mut a = a.borrow_mut();
                    a.fetching = false;
                    a.fetched = Some(std::time::Instant::now());
                    if let Ok(list) = res {
                        a.list = list;
                    }
                });
                cx.notify();
            });
        })
        .detach();
    }
    if fetch_sort {
        let fut = app.backend.user_experiments();
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |_, cx| {
                let sort = res.ok().and_then(|l| affinity_sort(phoenix_api::rest::experiments::bucket(&l, VOICE_AFFINITY_EXPERIMENT)));
                AFFINITIES.with(|a| {
                    let mut a = a.borrow_mut();
                    a.sort = Some(sort);
                    a.sort_fetching = false;
                });
                cx.notify();
            });
        })
        .detach();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NowGame {
    pub key: SharedString,
    pub name: SharedString,
    pub kind: crate::presence::ActivityKind,
    pub party: Option<SharedString>,
    pub start: Option<i64>,
    pub spotify: bool,
    pub xbox: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NowFriend {
    pub user: Id,
    pub name: SharedString,
    pub voice: Option<Id>,
    pub game: Option<NowGame>,
    pub stream: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CardGame {
    pub user: Id,
    pub game: NowGame,
    pub playing: Vec<Id>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NowCard {
    pub voice: Option<Id>,
    pub priority: Vec<Id>,
    pub partied: Vec<Id>,
    pub games: Vec<CardGame>,
    pub streams: Vec<Id>,
}

pub struct NowCtx<'a> {
    pub voice_members: &'a dyn Fn(Id) -> Vec<Id>,
    pub party_members: &'a dyn Fn(&str) -> Vec<Id>,
    pub is_friend: &'a dyn Fn(Id) -> bool,
    pub blocked: &'a dyn Fn(Id) -> bool,
    pub undiscoverable: &'a dyn Fn(Id) -> bool,
    pub weight: Option<&'a dyn Fn(Id) -> f64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum GroupKey {
    Voice(Id),
    Party(SharedString),
    User(Id),
}

fn by_weight(ids: &mut [Id], ctx: &NowCtx) {
    match ctx.weight {
        Some(w) => ids.sort_by(|a, b| w(*b).total_cmp(&w(*a))),
        None => ids.sort_by_key(|u| !(ctx.is_friend)(*u)),
    }
}

fn card_of(voice: Option<Id>, mut members: Vec<&NowFriend>, ctx: &NowCtx) -> (NowCard, bool, String) {
    if let Some(w) = ctx.weight {
        members.sort_by(|a, b| w(b.user).total_cmp(&w(a.user)));
    }
    let priority: Vec<Id> = members.iter().map(|f| f.user).collect();
    let streams: Vec<Id> = members.iter().filter(|f| f.stream).map(|f| f.user).collect();
    let mut spotify = false;
    let mut games: Vec<CardGame> = Vec::new();
    for m in &members {
        let Some(g) = &m.game else { continue };
        spotify = g.spotify;
        if games.iter().any(|x| x.game.key == g.key) {
            continue;
        }
        let mut playing: Vec<Id> = match &g.party {
            Some(p) => (ctx.party_members)(p),
            None => members.iter().filter(|f| f.game.as_ref().is_some_and(|x| x.key == g.key)).map(|f| f.user).collect(),
        };
        playing.sort_by_key(|u| !(ctx.is_friend)(*u));
        games.push(CardGame { user: m.user, game: g.clone(), playing });
    }
    games.sort_by(|a, b| b.game.start.unwrap_or(0).cmp(&a.game.start.unwrap_or(0)).then_with(|| a.game.name.cmp(&b.game.name)));
    let mut partied = priority.clone();
    if let Some(v) = voice {
        let mut here = (ctx.voice_members)(v);
        by_weight(&mut here, ctx);
        partied.extend(here.into_iter().filter(|u| !priority.contains(u)));
    }
    let names = members.iter().map(|f| f.name.to_lowercase()).collect::<Vec<_>>().join(" ");
    (NowCard { voice, priority, partied, games, streams }, spotify, names)
}

pub fn now_cards(friends: &[NowFriend], ctx: &NowCtx) -> Vec<NowCard> {
    let mut groups: Vec<(GroupKey, Vec<&NowFriend>)> = Vec::new();
    for f in friends {
        let key = match (f.voice, f.game.as_ref().and_then(|g| g.party.clone())) {
            (Some(v), _) => GroupKey::Voice(v),
            (None, Some(p)) => GroupKey::Party(p),
            (None, None) => GroupKey::User(f.user),
        };
        match groups.iter().position(|(g, _)| *g == key) {
            Some(i) => groups[i].1.push(f),
            None => groups.push((key, vec![f])),
        }
    }
    let hidden = |c: &NowCard| -> (bool, bool) {
        let Some(v) = c.voice else { return (false, false) };
        let states = (ctx.voice_members)(v);
        let n = states.iter().filter(|u| (ctx.undiscoverable)(**u)).count();
        (n > 0, n > 0 && n == states.len())
    };
    let mut keyed: Vec<(NowCard, bool, String, bool)> = groups
        .into_iter()
        .map(|(key, members)| card_of(if let GroupKey::Voice(v) = key { Some(v) } else { None }, members, ctx))
        .filter_map(|(c, sp, names)| {
            let (some_hidden, all_hidden) = hidden(&c);
            let shown = (c.voice.is_some() || !c.games.is_empty() || !c.streams.is_empty()) && !c.partied.iter().any(|u| (ctx.blocked)(*u)) && !all_hidden;
            shown.then_some((c, sp, names, some_hidden))
        })
        .collect();
    keyed.sort_by(|(a, asp, an, ah), (b, bsp, bn, bh)| {
        let k = |c: &NowCard, sp: bool, h: bool| (h, c.partied.len() <= 1, c.streams.is_empty(), c.voice.is_none(), c.games.is_empty(), sp);
        k(a, *asp, *ah).cmp(&k(b, *bsp, *bh)).then_with(|| an.cmp(bn))
    });
    keyed.into_iter().map(|(c, ..)| c).collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HeaderLine {
    Timer,
    Xbox,
    Twitch,
    Listening,
    Watching,
    InVoice,
    Multiple,
    Nothing,
}

pub fn header_line(card: &NowCard) -> HeaderLine {
    use crate::presence::ActivityKind as K;
    if card.partied.len() == 1
        && let Some(g) = card.games.first()
    {
        return match g.game.kind {
            K::Playing if g.game.xbox => HeaderLine::Xbox,
            K::Playing => HeaderLine::Timer,
            K::Streaming => HeaderLine::Twitch,
            K::Listening => HeaderLine::Listening,
            K::Watching => HeaderLine::Watching,
            _ => HeaderLine::InVoice,
        };
    }
    match card.games.len() {
        0 if card.voice.is_some() => HeaderLine::InVoice,
        0 => HeaderLine::Nothing,
        1 => HeaderLine::Timer,
        _ => HeaderLine::Multiple,
    }
}

pub fn card_title(names: &[SharedString]) -> String {
    let a = names.first().map(|s| s.to_string()).unwrap_or_default();
    let b = names.get(1).map(|s| s.to_string()).unwrap_or_default();
    match names.len() {
        0 | 1 => a,
        2 => crate::t!("friends.now_two", user1 = a, user2 = b).to_string(),
        3 => crate::t!("friends.now_more_one", user1 = a, user2 = b, extras = 1).to_string(),
        n => crate::t!("friends.now_more", user1 = a, user2 = b, extras = n - 2).to_string(),
    }
}

pub fn facepile(total: usize) -> (usize, Option<usize>) {
    let shown = total.min(2);
    match total - shown {
        0 => (shown, None),
        1 => (shown + 1, None),
        rest => (shown, Some(rest.min(99))),
    }
}

fn game_key(a: &crate::presence::Activity) -> SharedString {
    a.application_id.clone().unwrap_or_else(|| format!("name:{}", a.name).into())
}

fn first_game(app: &PhoenixApp, u: Id) -> Option<&crate::presence::Activity> {
    app.people.activities.get(&u).and_then(|v| v.iter().find(|a| a.kind != crate::presence::ActivityKind::Custom))
}

fn now_friends(app: &PhoenixApp) -> Vec<NowFriend> {
    let voice_of = |u: Id| {
        app.channels
            .values()
            .find(|c| {
                matches!(c.kind, crate::model::ChannelKind::Voice | crate::model::ChannelKind::Stage)
                    && c.voice_users.contains(&u)
                    && c.guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)).is_some_and(|g| g.afk_channel != Some(c.id))
            })
            .map(|c| c.id)
    };
    let friends: Vec<Id> = app.nav.relationships.iter().filter(|r| r.kind == RelationshipKind::Friend).map(|r| r.user).collect();
    let ids = AFFINITIES.with(|a| now_playing_ids(&a.borrow().list, &friends));
    ids.into_iter()
        .filter(|u| *u != app.me.id)
        .filter_map(|id| {
            let u = app.users.get(&id)?;
            if u.bot {
                return None;
            }
            let game = first_game(app, id).map(|a| NowGame {
                key: game_key(a),
                name: a.name.clone(),
                kind: a.kind,
                party: a.party.clone(),
                start: a.start,
                spotify: a.is_spotify(),
                xbox: a.is_xbox(),
            });
            Some(NowFriend { user: id, name: u.name.clone(), voice: voice_of(id), game, stream: crate::ui::voice_panel::flags(id).stream })
        })
        .collect()
}

fn cards_for(app: &PhoenixApp) -> Vec<NowCard> {
    let friends = now_friends(app);
    let (sort, weights) = AFFINITIES.with(|a| {
        let a = a.borrow();
        (a.sort.flatten(), a.list.iter().map(|x| (x.user, (x.vc_probability, x.communication_probability))).collect::<HashMap<_, _>>())
    });
    let voice_members = |v: Id| app.channels.get(&v).map(|c| c.voice_users.clone()).unwrap_or_default();
    let party_members = |p: &str| {
        let mut out: Vec<Id> = app
            .people
            .activities
            .iter()
            .filter(|(_, acts)| acts.iter().any(|a| a.party.as_deref() == Some(p)))
            .map(|(u, _)| *u)
            .filter(|u| app.users.contains_key(u))
            .collect();
        out.sort_by_key(|u| u.0);
        out
    };
    let is_friend = |u: Id| app.nav.relationships.iter().any(|r| r.user == u && r.kind == RelationshipKind::Friend);
    let blocked = |u: Id| app.nav.is_blocked(u) || app.nav.relationships.iter().any(|r| r.user == u && r.ignored);
    let weight = |u: Id| {
        let (vc, comm) = weights.get(&u).copied().unwrap_or_default();
        if sort == Some(AffinitySort::Voice) { vc } else { comm }
    };
    let undiscoverable = |u: Id| crate::ui::voice_panel::flags(u).undiscoverable;
    let ctx = NowCtx {
        voice_members: &voice_members,
        party_members: &party_members,
        is_friend: &is_friend,
        blocked: &blocked,
        undiscoverable: &undiscoverable,
        weight: sort.map(|_| &weight as &dyn Fn(Id) -> f64),
    };
    now_cards(&friends, &ctx)
}

fn active_now_column(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    load_affinities(app, cx);
    let cards = cards_for(app);
    let mut col = div()
        .id("active-now")
        .w(z(active_now_width()))
        .flex_none()
        .h_full()
        .border_l_1()
        .border_color(c(Palette::DIVIDER))
        .pt(z(14.))
        .pl(z(17.))
        .pr(z(16.))
        .pb(z(16.))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(z(20.))
                .line_height(z(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(Palette::TEXT_HEADER))
                .child(crate::t!("friends.active_now_title")),
        );
    if cards.is_empty() {
        return col.child(
            div()
                .mt(z(12.))
                .p(z(16.))
                .rounded(z(8.))
                .bg(c(crate::theme::tok(crate::theme::generated::idx::BACKGROUND_BASE_LOW)))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .mb(z(4.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(crate::theme::tok(crate::theme::generated::idx::TEXT_STRONG)))
                        .child(crate::t!("friends.quiet")),
                )
                .child(
                    div()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .text_color(c(crate::theme::tok(crate::theme::generated::idx::INTERACTIVE_TEXT_DEFAULT)))
                        .text_center()
                        .child(crate::t!("friends.quiet_sub")),
                ),
        );
    }
    for card_info in cards {
        col = col.child(now_card(app, card_info, cx));
    }
    col
}

fn game_thumb(img: Option<SharedString>, size: f32) -> gpui::Div {
    div().size(z(size)).flex_none().rounded(z(4.)).overflow_hidden().when_some(img, |d, url| d.child(gpui::img(url).size_full()))
}

fn activity_of<'a>(app: &'a PhoenixApp, g: &CardGame) -> Option<&'a crate::presence::Activity> {
    app.people.activities.get(&g.user).and_then(|v| v.iter().find(|a| a.kind != crate::presence::ActivityKind::Custom && game_key(a) == g.game.key))
}

fn multiple_icon() -> AnyElement {
    div()
        .size(z(24.))
        .flex_none()
        .rounded_full()
        .bg(c(tok(idx::BACKGROUND_MOD_NORMAL)))
        .flex()
        .items_center()
        .justify_center()
        .child(gpui::svg().path(crate::icons::path("game-controller")).size(z(18.)).text_color(c(tok(idx::PRIMARY_300))))
        .into_any_element()
}

fn now_card(app: &PhoenixApp, card_info: NowCard, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let user = card_info.priority[0];
    let u = app.users.get(&user);
    let now = crate::mock::now();
    let first = card_info.games.first();
    let first_act = first.and_then(|g| activity_of(app, g));
    let line = header_line(&card_info);
    let timer = || match (first, first_act.and_then(|a| a.start)) {
        (Some(g), Some(st)) => format!("{} - {}", g.game.name, crate::ui::members::since_short(st, now)).into(),
        (Some(g), None) => g.game.name.clone(),
        _ => SharedString::default(),
    };
    let game_name = first.map(|g| g.game.name.to_string()).unwrap_or_default();
    let sub: Option<SharedString> = match line {
        HeaderLine::Timer => Some(timer()),
        HeaderLine::Xbox => Some(crate::t!("friends.now_xbox")),
        HeaderLine::Twitch => Some(crate::t!("friends.now_twitch")),
        HeaderLine::Listening => Some(crate::t!("friends.now_listening", name = game_name)),
        HeaderLine::Watching => Some(crate::t!("friends.now_watching", name = game_name)),
        HeaderLine::InVoice => Some(crate::t!("friends.now_in_voice")),
        HeaderLine::Multiple => Some(crate::t!("friends.now_multiple")),
        HeaderLine::Nothing => None,
    };
    let icon: Option<AnyElement> = match line {
        HeaderLine::Multiple => Some(multiple_icon()),
        HeaderLine::Xbox => platform_icon("xbox", 24.),
        HeaderLine::Listening if first.is_some_and(|g| g.game.spotify) => platform_icon("spotify", 24.),
        HeaderLine::Timer if first.is_some_and(|g| g.game.xbox) => platform_icon("xbox", 24.),
        HeaderLine::Timer | HeaderLine::Twitch | HeaderLine::Listening | HeaderLine::Watching => {
            first_act.and_then(|a| a.image.clone()).map(|i| game_thumb(Some(i), 24.).into_any_element())
        }
        _ => None,
    };
    let names: Vec<SharedString> =
        card_info.partied.iter().map(|id| app.users.get(id).map(|u| u.display_name.clone()).unwrap_or_default()).collect();
    let title: SharedString = card_title(&names).into();
    let mut sections: Vec<AnyElement> = Vec::new();
    if let Some(v) = card_info.voice {
        sections.push(voice_section(app, v, cx));
    }
    for owner in &card_info.streams {
        sections.push(go_live_section(app, *owner, cx));
    }
    for g in &card_info.games {
        if let Some(s) = game_section(app, g, card_info.games.len(), card_info.partied.len() == 1, card_info.voice, cx) {
            sections.push(s);
        }
    }
    div()
        .id(("active", user.0))
        .flex_none()
        .mt(z(12.))
        .rounded(z(16.))
        .border_1()
        .border_color(c(Palette::DIVIDER))
        .shadow(crate::ui::inbox::low_shadow())
        .overflow_hidden()
        .bg(c(Palette::BG_PRIMARY))
        .cursor_pointer()
        .on_click(cx.listener(move |a, _, _, cx| a.open_dm_with(user, cx)))
        .child(
            div()
                .p(z(16.))
                .flex()
                .items_center()
                .gap(z(12.))
                .group("active-card-head")
                .bg(c(Palette::BG_MSG_HOVER))
                .hover(|s| s.bg(c(Palette::BG_MOD_HOVER)))
                .child(crate::ui::avatar_with_status_deco(
                    u,
                    32.,
                    app.presence_of(user),
                    c(Palette::BG_PRIMARY),
                    |d| d.bg(c(Palette::BG_MSG_HOVER)).group_hover("active-card-head", |s| s.bg(c(Palette::BG_MOD_HOVER))),
                    crate::ui::avatar_deco::source(u, None),
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c(Palette::TEXT_NORMAL))
                                .text_ellipsis()
                                .child(title),
                        )
                        .when_some(sub, |d, sub| {
                            d.child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(Palette::TEXT_MUTED)).text_ellipsis().child(sub))
                        }),
                )
                .children(icon),
        )
        .children(sections.into_iter().enumerate().map(|(i, s)| {
            div().when(i > 0, |d| d.border_t_1().border_color(c(tok(idx::BORDER_SUBTLE)))).child(s).into_any_element()
        }))
        .into_any_element()
}

fn party_pile(app: &PhoenixApp, members: &[Id]) -> gpui::Div {
    let (shown, plus) = facepile(members.len());
    let mut pile = div().flex().items_center().flex_none();
    for (i, m) in members.iter().take(shown).enumerate() {
        pile = pile.child(div().when(i > 0, |d| d.ml(z(-2.))).child(crate::ui::avatar(app.users.get(m), 24.)));
    }
    if let Some(n) = plus {
        pile = pile.child(
            div()
                .ml(z(-2.))
                .h(z(24.))
                .min_w(z(24.))
                .px(z(4.))
                .rounded(z(12.))
                .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
                .flex()
                .items_center()
                .justify_center()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .child(format!("+{n}")),
        );
    }
    pile
}

fn platform_icon(kind: &str, size: f32) -> Option<AnyElement> {
    let (url, _) = crate::ui::settings::art::connection_icon(kind)?;
    Some(gpui::img(SharedString::from(url)).size(z(size)).flex_none().object_fit(gpui::ObjectFit::Contain).into_any_element())
}

fn game_section(app: &PhoenixApp, g: &CardGame, games: usize, solo: bool, voice: Option<Id>, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    use crate::presence::ActivityKind as K;
    let a = activity_of(app, g)?;
    let lead = app.users.get(&g.user);
    let rich = a.details.is_some() || a.state.is_some();
    if a.kind == K::Streaming {
        return Some(twitch_section(app, g.user, a));
    }
    if a.kind == K::Listening && a.is_spotify() {
        return Some(spotify_section(app, g, a, solo));
    }
    if a.kind == K::Playing
        && a.is_embedded()
        && !(games > 1 && !rich)
        && let Some(v) = voice
    {
        return Some(embedded_section(app, g, a, v, cx));
    }
    if a.kind == K::Playing && (!rich || games > 1) {
        let people = if g.playing.len() == 1 {
            crate::t!("friends.now_people_one", count = g.playing.len())
        } else {
            crate::t!("friends.now_people", count = g.playing.len())
        };
        return Some(
            div()
                .p(z(16.))
                .flex()
                .items_center()
                .gap(z(12.))
                .child(game_thumb(a.image.clone(), 32.))
                .child(div().flex_1().min_w_0().child(section_text(a.name.clone(), true)).child(section_text(people, false)))
                .child(party_pile(app, &g.playing))
                .into_any_element(),
        );
    }
    if !rich && a.image.is_none() {
        return None;
    }
    let title = a.details.clone().filter(|d| !d.is_empty()).unwrap_or_else(|| a.name.clone());
    Some(
        div()
            .p(z(16.))
            .flex()
            .items_center()
            .gap(z(12.))
            .child(game_thumb(a.image.clone(), 32.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(section_text(title, true))
                    .when_some(a.state.clone(), |d, t| d.child(section_text(t, false))),
            )
            .child(crate::ui::avatar(lead, 24.))
            .into_any_element(),
    )
}

pub fn spotify_artists(state: &str) -> String {
    state.replace("; ", ", ")
}

fn spotify_section(app: &PhoenixApp, g: &CardGame, a: &crate::presence::Activity, solo: bool) -> AnyElement {
    let art = div()
        .relative()
        .size(z(32.))
        .flex_none()
        .when_some(a.image.clone(), |d, url| d.child(gpui::img(url).size_full().object_fit(gpui::ObjectFit::Cover)))
        .when(!solo, |d| d.children(platform_icon("spotify", 16.).map(|i| div().absolute().right(z(-4.)).bottom(z(-4.)).size(z(16.)).rounded_full().child(i))));
    div()
        .p(z(16.))
        .flex()
        .items_start()
        .gap(z(12.))
        .child(art)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .when_some(a.details.clone(), |d, t| d.child(section_text(t, true)))
                .when_some(a.state.clone(), |d, t| d.child(section_text(spotify_artists(&t).into(), false))),
        )
        .child(party_pile(app, &g.playing))
        .into_any_element()
}

fn embedded_section(app: &PhoenixApp, g: &CardGame, a: &crate::presence::Activity, voice: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let now = crate::mock::now();
    let others: Vec<Id> = g.playing.iter().copied().filter(|u| *u != app.me.id).collect();
    div()
        .p(z(16.))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .mb(z(8.))
                .child(div().mr(z(8.)).child(game_thumb(a.image.clone(), 24.)))
                .child(div().flex_1().min_w_0().child(section_text(a.name.clone(), true)))
                .when_some(a.start, |d, st| {
                    d.child(
                        div()
                            .flex_none()
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .text_color(c(Palette::TEXT_MUTED))
                            .child(crate::ui::members::elapsed(st, now)),
                    )
                }),
        )
        .child(
            div()
                .relative()
                .w_full()
                .h(z(165.))
                .rounded(z(12.))
                .bg(c(tok(idx::BLACK)))
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(z(12.))
                        .bg(c(tok(idx::OPACITY_BLACK_60)))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .child(party_pile(app, &others))
                        .child(
                            div().mt(z(8.)).child(
                                crate::ui::profile::mana_button(("now-embedded-join", voice.0), crate::t!("profile.voice_join"), true, false).on_click(
                                    cx.listener(move |a, _, _, cx| {
                                        cx.stop_propagation();
                                        a.select_channel(voice, cx);
                                    }),
                                ),
                            ),
                        ),
                ),
        )
        .into_any_element()
}

fn twitch_section(app: &PhoenixApp, user: Id, s: &crate::presence::Activity) -> AnyElement {
    let u = app.users.get(&user);
    div()
        .p(z(16.))
        .flex()
        .flex_col()
        .gap(z(12.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(12.))
                .child(crate::ui::avatar(u, 32.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(section_text(u.map(|u| u.display_name.clone()).unwrap_or_default(), true))
                        .child(section_text(s.details.clone().unwrap_or(s.name.clone()), false)),
                )
                .child(
                    div()
                        .h(z(16.))
                        .px(z(8.))
                        .rounded_full()
                        .bg(c(tok(idx::STATUS_DANGER)))
                        .flex()
                        .items_center()
                        .text_size(z(12.))
                        .line_height(z(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(gpui::white())
                        .child(crate::t!("voice.live")),
                ),
        )
        .child(
            div()
                .w_full()
                .h(z(165.))
                .rounded(z(8.))
                .overflow_hidden()
                .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
                .flex()
                .items_center()
                .justify_center()
                .when_some(s.image.clone(), |d, url| d.child(gpui::img(url).size_full().object_fit(gpui::ObjectFit::Cover))),
        )
        .into_any_element()
}

fn section_text(t: SharedString, strong: bool) -> gpui::Div {
    let d = div().text_ellipsis();
    if strong {
        d.text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(c(Palette::TEXT_NORMAL)).child(t)
    } else {
        d.text_size(z(12.)).line_height(z(16.)).text_color(c(Palette::TEXT_MUTED)).child(t)
    }
}

fn voice_section(app: &PhoenixApp, voice: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let ch = app.channels.get(&voice);
    let guild = ch.and_then(|c| c.guild).and_then(|g| app.guilds.iter().find(|x| x.id == g));
    let guild_id = guild.map(|g| g.id);
    let mut members: Vec<Id> = ch.map(|c| c.voice_users.clone()).unwrap_or_default();
    members.sort_by_key(|u| !app.nav.relationships.iter().any(|r| r.user == *u && r.kind == RelationshipKind::Friend));
    let icon_box = match guild.map(|g| &g.icon) {
        Some(crate::model::ImageRef::Url(url)) => gpui::img(url.clone()).size(z(32.)).rounded_full().into_any_element(),
        _ => {
            let acr = guild.map(|g| g.acronym.clone()).unwrap_or_default();
            let size = [14., 14., 12., 12., 10., 8., 6.].get(acr.chars().count()).copied().unwrap_or(6.);
            div()
                .size(z(32.))
                .rounded_full()
                .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .text_size(z(size))
                .text_color(gpui::white())
                .child(acr)
                .into_any_element()
        }
    };
    let assets = div()
        .id(("active-voice-guild", voice.0))
        .size(z(32.))
        .flex_none()
        .relative()
        .cursor_pointer()
        .on_click(cx.listener(move |a, _, _, cx| {
            cx.stop_propagation();
            a.select_guild(guild_id, cx);
        }))
        .child(icon_box)
        .child(
            div()
                .absolute()
                .bottom(z(-2.))
                .right(z(-2.))
                .size(z(16.))
                .rounded_full()
                .bg(c(Palette::BG_TERTIARY))
                .flex()
                .items_center()
                .justify_center()
                .child(gpui::svg().path(crate::icons::path("speaker")).size(z(10.)).text_color(c(tok(idx::TEXT_DEFAULT)))),
        );
    let group: SharedString = format!("active-voice-{}", voice.0).into();
    div()
        .p(z(16.))
        .flex()
        .items_center()
        .gap(z(12.))
        .child(assets)
        .child(
            div()
                .id(("active-voice-join", voice.0))
                .group(group.clone())
                .flex_1()
                .min_w_0()
                .cursor_pointer()
                .on_click(cx.listener(move |a, _, _, cx| {
                    cx.stop_propagation();
                    a.select_channel(voice, cx);
                }))
                .child(section_text(guild.map(|g| g.name.clone()).unwrap_or_default(), true).group_hover(group.clone(), |s| s.underline()))
                .child(section_text(ch.map(|c| c.name.clone()).unwrap_or_default(), false).group_hover(group, |s| s.underline())),
        )
        .child(party_pile(app, &members))
        .into_any_element()
}

pub fn stream_action(own: bool, watching: bool) -> &'static str {
    match (own, watching) {
        (true, _) => "friends.now_own_stream",
        (false, true) => "friends.now_watching_stream",
        (false, false) => "friends.now_watch_stream",
    }
}

pub const HOVER_TEXT_MS: u64 = 170;

fn hover_text_fade(el: gpui::Div, key: u64) -> AnyElement {
    use gpui::PresentTransitionExt;
    let state = crate::ui::states::state(key);
    let on = state.is_some_and(|s| s.0);
    let (from, to) = if on { (0., 1.) } else { (1., 0.) };
    let ms = crate::ui::settings::effects::motion(HOVER_TEXT_MS);
    let t = state
        .filter(|(_, at)| at.elapsed() < ms)
        .map(|(_, at)| gpui::Transition::ease(ms, gpui::CubicBezier(0.25, 0.1, 0.25, 1.)).start(at).opacity(from, to));
    el.opacity(to).present_transition(gpui::ElementId::NamedInteger("active-stream-hover-pt".into(), key), t).into_any_element()
}

fn go_live_section(app: &PhoenixApp, owner: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let u = app.users.get(&owner);
    let key = crate::ui::call_view::stream_key_in(app, owner);
    if let Some(k) = key.clone() {
        crate::ui::voice_row::fetch_preview(app, k, cx);
    }
    let url = key.as_deref().and_then(crate::ui::voice_row::preview_url);
    let loading = key.as_deref().is_some_and(crate::ui::voice_row::preview_loading);
    let own = owner == app.me.id;
    let watching = key.as_deref().is_some_and(crate::voice::call::watching);
    let action: SharedString = crate::t!(stream_action(own, watching));
    let allowed = key.as_deref().is_some_and(|k| crate::ui::voice_row::preview_allowed(app, k));
    let hover_key = crate::ui::states::key(("active-stream-hover", owner.0));
    let empty = if loading {
        div().absolute().inset_0().flex().items_center().justify_center().child(crate::ui::spinner::spinner()).into_any_element()
    } else {
        div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(z(10.))
            .child(
                div()
                    .w(gpui::relative(0.8))
                    .h(gpui::relative(0.6))
                    .flex()
                    .justify_center()
                    .child(gpui::img(crate::ui::call_view::stream_empty_preview(crate::theme::is_light())).h_full().object_fit(gpui::ObjectFit::Contain)),
            )
            .child(
                div()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                    .text_center()
                    .child(if allowed { crate::t!("friends.now_stream_started") } else { crate::t!("friends.now_no_permission") }),
            )
            .into_any_element()
    };
    let watch_key = key.clone();
    div()
        .p(z(16.))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(12.))
                .child(crate::ui::avatar(u, 32.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(section_text(u.map(|u| u.display_name.clone()).unwrap_or_default(), true))
                        .child(section_text(crate::t!("friends.now_sharing_screen"), false)),
                )
                .child(
                    div()
                        .h(z(16.))
                        .px(z(8.))
                        .rounded_full()
                        .bg(c(tok(idx::BACKGROUND_FEEDBACK_NOTIFICATION)))
                        .flex()
                        .items_center()
                        .text_size(z(12.))
                        .line_height(z(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(gpui::white())
                        .child(crate::t!("voice.live")),
                ),
        )
        .child(
            crate::ui::states::track(div().id(("active-stream-preview", owner.0)), hover_key)
                .relative()
                .mt(z(12.))
                .w_full()
                .h(z(165.))
                .rounded(z(8.))
                .overflow_hidden()
                .bg(c(tok(idx::BACKGROUND_MOD_NORMAL)))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .on_click(cx.listener(move |a, _, _, cx| {
                    cx.stop_propagation();
                    if !own && let Some(k) = watch_key.clone() {
                        crate::ui::voice_row::watch_from_sidebar(a, k, cx);
                    }
                }))
                .child(match url {
                    Some(url) => gpui::img(url).size_full().object_fit(gpui::ObjectFit::Contain).into_any_element(),
                    None => empty,
                })
                .child(
                    div()
                        .absolute()
                        .px(z(16.))
                        .py(z(7.))
                        .rounded(z(24.))
                        .bg(c(tok(idx::OPACITY_BLACK_60)))
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::TEXT_OVERLAY_LIGHT)))
                        .child(action)
                        .map(|d| hover_text_fade(d, hover_key)),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(user: u64, kind: RelationshipKind, since: i64) -> Relationship {
        Relationship { user: Id(user), kind, nickname: None, since: Some(since), ignored: false, spam: false, note: None }
    }

    fn game(key: &str, party: Option<&str>, start: i64) -> NowGame {
        NowGame {
            key: key.into(),
            name: key.into(),
            kind: crate::presence::ActivityKind::Playing,
            party: party.map(Into::into),
            start: Some(start),
            spotify: false,
            xbox: false,
        }
    }

    fn nf(user: u64, name: &str, voice: Option<u64>, activity: bool) -> NowFriend {
        NowFriend { user: Id(user), name: name.into(), voice: voice.map(Id), game: activity.then(|| game(name, None, 0)), stream: false }
    }

    fn cards(friends: &[NowFriend], voice: &dyn Fn(Id) -> Vec<Id>, party: &dyn Fn(&str) -> Vec<Id>) -> Vec<NowCard> {
        let ctx = NowCtx {
            voice_members: voice,
            party_members: party,
            is_friend: &|u: Id| u.0 < 100,
            blocked: &|u: Id| u.0 == 666,
            undiscoverable: &|u: Id| u.0 >= 800 && u.0 < 900,
            weight: None,
        };
        now_cards(friends, &ctx)
    }

    #[test]
    fn friends_in_one_voice_channel_share_a_card_with_the_other_members() {
        let mut friends = vec![nf(1, "alchi", Some(50), false), nf(2, "fox", Some(50), false), nf(3, "solo", None, true)];
        friends[0].stream = true;
        let cards = cards(&friends, &|v| if v == Id(50) { vec![Id(900), Id(2), Id(1)] } else { vec![] }, &|_| vec![]);
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].voice, Some(Id(50)));
        assert_eq!(cards[0].priority, vec![Id(1), Id(2)]);
        assert_eq!(cards[0].partied, vec![Id(1), Id(2), Id(900)]);
        assert_eq!(cards[0].streams, vec![Id(1)]);
        assert_eq!(cards[1].priority, vec![Id(3)]);
    }

    #[test]
    fn the_current_user_in_the_voice_channel_counts_in_the_title_after_the_friends() {
        let friends = [nf(1, "alchi", Some(50), false)];
        let cards = cards(&friends, &|_| vec![Id(500), Id(1)], &|_| vec![]);
        assert_eq!(cards[0].partied, vec![Id(1), Id(500)]);
        assert_eq!(header_line(&cards[0]), HeaderLine::InVoice);
    }

    #[test]
    fn idle_friends_without_voice_or_activity_get_no_card() {
        let cards = cards(&[nf(1, "a", None, false)], &|_| vec![], &|_| vec![]);
        assert!(cards.is_empty());
    }

    #[test]
    fn cards_follow_the_client_order() {
        let friends = [
            nf(1, "zed", None, true),
            nf(2, "amy", None, true),
            nf(3, "bob", Some(70), false),
            nf(4, "cat", Some(80), false),
            nf(5, "dan", Some(80), false),
        ];
        let cards = cards(&friends, &|_| vec![], &|_| vec![]);
        let first: Vec<u64> = cards.iter().map(|c| c.priority[0].0).collect();
        assert_eq!(first, vec![4, 3, 2, 1]);
    }

    #[test]
    fn friends_in_one_game_party_share_a_card_with_the_party_members() {
        let mut a = nf(1, "amy", None, false);
        a.game = Some(game("123", Some("p1"), 10));
        let mut b = nf(2, "bob", None, false);
        b.game = Some(game("123", Some("p1"), 20));
        let solo = nf(3, "cat", None, true);
        let cards = cards(&[a, b, solo], &|_| vec![], &|p| if p == "p1" { vec![Id(200), Id(2), Id(1)] } else { vec![] });
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].priority, vec![Id(1), Id(2)]);
        assert_eq!(cards[0].games.len(), 1);
        assert_eq!(cards[0].games[0].playing, vec![Id(2), Id(1), Id(200)]);
        assert_eq!(header_line(&cards[0]), HeaderLine::Timer);
    }

    #[test]
    fn different_games_in_one_voice_channel_read_playing_multiple_games() {
        let mut a = nf(1, "amy", Some(50), false);
        a.game = Some(game("lol", None, 10));
        let mut b = nf(2, "bob", Some(50), false);
        b.game = Some(game("osu", None, 30));
        let cards = cards(&[a, b], &|_| vec![Id(1), Id(2)], &|_| vec![]);
        assert_eq!(header_line(&cards[0]), HeaderLine::Multiple);
        let order: Vec<&str> = cards[0].games.iter().map(|g| g.game.key.as_ref()).collect();
        assert_eq!(order, vec!["osu", "lol"]);
    }

    #[test]
    fn a_single_member_header_follows_the_activity_type() {
        let mut a = nf(1, "amy", None, false);
        a.game = Some(NowGame { kind: crate::presence::ActivityKind::Listening, ..game("spotify", None, 0) });
        assert_eq!(header_line(&cards(&[a], &|_| vec![], &|_| vec![])[0]), HeaderLine::Listening);
    }

    #[test]
    fn undiscoverable_voice_states_sink_the_card_or_drop_it() {
        let friends = [nf(1, "amy", Some(50), false), nf(2, "bob", Some(60), false), nf(3, "cat", Some(70), false)];
        let voice = |v: Id| match v.0 {
            50 => vec![Id(1), Id(801)],
            60 => vec![Id(2)],
            _ => vec![Id(801), Id(802)],
        };
        let cards = cards(&friends, &voice, &|_| vec![]);
        let voices: Vec<Option<Id>> = cards.iter().map(|c| c.voice).collect();
        assert_eq!(voices, vec![Some(Id(60)), Some(Id(50))]);
    }

    #[test]
    fn a_single_xbox_player_reads_playing_on_xbox() {
        let mut a = nf(1, "amy", None, false);
        a.game = Some(NowGame { xbox: true, ..game("xbox", None, 0) });
        assert_eq!(header_line(&cards(&[a], &|_| vec![], &|_| vec![])[0]), HeaderLine::Xbox);
        assert_eq!(spotify_artists("A; B; C"), "A, B, C");
    }

    #[test]
    fn a_card_with_a_blocked_member_is_dropped() {
        let friends = [nf(1, "amy", Some(50), false)];
        assert!(cards(&friends, &|_| vec![Id(1), Id(666)], &|_| vec![]).is_empty());
    }

    #[test]
    fn affinity_users_come_first_then_friends_without_duplicates() {
        let aff = |u: u64, rank: u32| Affinity { user: Id(u), communication_rank: rank, communication_probability: 0., vc_probability: 0. };
        let ids = now_playing_ids(&[aff(7, 3), aff(8, 16), aff(2, 1)], &[Id(1), Id(2)]);
        assert_eq!(ids, vec![Id(7), Id(2), Id(1)]);
    }

    #[test]
    fn affinities_and_consent_parse_like_the_client() {
        let v = serde_json::json!({"user_affinities": [{"other_user_id": "42", "communication_rank": 3, "communication_probability": 0.5, "vc_probability": 0.25}, {"communication_rank": 1}]});
        assert_eq!(
            parse_affinities(&v),
            vec![Affinity { user: Id(42), communication_rank: 3, communication_probability: 0.5, vc_probability: 0.25 }]
        );
        assert!(personalization_consented(&serde_json::json!({"personalization": {"consented": true}})));
        assert!(!personalization_consented(&serde_json::json!({"usage_statistics": {"consented": true}})));
        assert_eq!(affinity_sort(1), Some(AffinitySort::Voice));
        assert_eq!(affinity_sort(0), None);
    }

    #[test]
    fn the_stream_hover_text_follows_the_client() {
        assert_eq!(stream_action(true, false), "friends.now_own_stream");
        assert_eq!(stream_action(false, true), "friends.now_watching_stream");
        assert_eq!(stream_action(false, false), "friends.now_watch_stream");
    }

    #[test]
    fn facepile_shows_two_then_a_third_or_a_count() {
        assert_eq!(facepile(1), (1, None));
        assert_eq!(facepile(2), (2, None));
        assert_eq!(facepile(3), (3, None));
        assert_eq!(facepile(4), (2, Some(2)));
        assert_eq!(facepile(300), (2, Some(99)));
    }

    #[test]
    fn single_member_cards_use_the_plain_name() {
        assert_eq!(card_title(&["Alchi".into()]), "Alchi");
        assert_eq!(card_title(&[]), "");
    }

    #[test]
    fn three_member_cards_count_the_third() {
        let t = card_title(&["A".into(), "B".into(), "C".into()]);
        assert!(!t.contains('{'), "{t}");
        assert!(t.contains('1'), "{t}");
        assert!(!card_title(&["A".into(), "B".into(), "C".into(), "D".into()]).contains('{'));
    }

    #[test]
    fn pending_hides_spam_and_ignored_incoming() {
        let mut spam = rel(1, RelationshipKind::Incoming, 0);
        spam.spam = true;
        let mut ignored = rel(2, RelationshipKind::Incoming, 0);
        ignored.ignored = true;
        let plain = rel(3, RelationshipKind::Incoming, 0);
        let out = rel(4, RelationshipKind::Outgoing, 0);
        let shown: Vec<u64> =
            [&spam, &ignored, &plain, &out].iter().filter(|r| in_tab(r, FriendsTab::Pending, false)).map(|r| r.user.0).collect();
        assert_eq!(shown, vec![3, 4]);
        assert_eq!(pending_badge(&[spam.clone(), ignored.clone(), plain.clone(), out.clone()]), 1);
        let mut nav = crate::ui::home::data::NavState { relationships: vec![spam, ignored, plain, out], ..Default::default() };
        nav.apply(crate::ui::home::data::NavEvent::GameRequests(vec![(Id(1), Id(50)), (Id(8), Id(50))]));
        nav.apply(crate::ui::home::data::NavEvent::GameRequest { user: Id(9), app: Id(51), incoming: true });
        assert_eq!(nav.pending_requests(), 3, "game requests count too, except from spam or ignored users");
        nav.apply(crate::ui::home::data::NavEvent::GameRequest { user: Id(9), app: Id(51), incoming: false });
        assert_eq!(nav.pending_requests(), 2);
    }

    #[test]
    fn pending_incoming_newest_first_outgoing_by_name() {
        let mut v = vec![
            rel(1, RelationshipKind::Outgoing, 50),
            rel(2, RelationshipKind::Incoming, 10),
            rel(3, RelationshipKind::Incoming, 30),
            rel(4, RelationshipKind::Outgoing, 1),
        ];
        let names = |r: &Relationship| match r.user.0 {
            1 => "zed".to_owned(),
            4 => "amy".to_owned(),
            _ => "x".to_owned(),
        };
        order(&mut v, names, FriendsTab::Pending);
        assert_eq!(v.iter().map(|r| r.user.0).collect::<Vec<_>>(), vec![3, 2, 4, 1]);
    }

    #[test]
    fn first_section_follows_the_client() {
        assert_eq!(initial_tab(&[rel(1, RelationshipKind::Friend, 0)]), FriendsTab::Online);
        assert_eq!(initial_tab(&[rel(1, RelationshipKind::Incoming, 0)]), FriendsTab::Pending);
        assert_eq!(initial_tab(&[rel(1, RelationshipKind::Outgoing, 0)]), FriendsTab::AddFriend);
    }

    #[test]
    fn search_reads_username_nickname_and_global_name() {
        let mut r = rel(1, RelationshipKind::Friend, 0);
        r.nickname = Some("Bestie".into());
        assert!(matches_query(&r, "wumpus", Some("Wump"), "best"));
        assert!(matches_query(&r, "wumpus", None, "mpu"));
        assert!(!matches_query(&r, "wumpus", Some("Wump"), "zzz"));
    }
}
