use crate::theme::z;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, ParentElement, SharedString, Styled, Subscription,
    Window, div, prelude::*, px,
};
use gpui_component::input::{Input, InputEvent, InputState};
use chrono::{Datelike, NaiveDate};
use phoenix_api::rest::search::{FilterKey, PendingFilter, complete_filter, parse_search_with, pending_filter};

use crate::ui::search_query::{self as sq, Answer, AnswerLead};

use crate::{
    search_highlight::Highlighter,
    app::{PhoenixApp, format_time_full},
    assets::Icon,
    model::{ChannelKind, Id, Message},
    theme::{Palette, c, generated::idx, m as layout},
    ui::avatar,
};

pub const PAGE: u32 = 25;
const WIDTH: f32 = 420.;

pub(super) fn results_width() -> gpui::Pixels {
    px(WIDTH)
}

pub(super) fn chat_width(window: &Window) -> gpui::Pixels {
    let navigation = z(crate::theme::m::SERVER_BAR_W + crate::ui::sidebar_resize::width());
    (window.viewport_size().width - navigation).max(px(0.))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchRequest {
    pub guild: Option<Id>,
    pub channel: Option<Id>,
    pub all_dms: bool,
    pub content: Option<String>,
    pub authors: Vec<Id>,
    pub mentions: Vec<Id>,
    pub channels: Vec<Id>,
    pub has: Vec<String>,
    pub min_id: Option<Id>,
    pub max_id: Option<Id>,
    pub pinned: Option<bool>,
    pub author_types: Vec<String>,
    pub attachment_extensions: Vec<String>,
    pub attachment_filenames: Vec<String>,
    pub link_hostnames: Vec<String>,
    pub offset: u32,
    pub sort: SortOrder,
    pub report_indexing: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortOrder {
    #[default]
    Newest,
    Oldest,
    Relevance,
}

#[derive(Clone, Debug, Default)]
pub struct SearchPage {
    pub total: u32,
    pub messages: Vec<Message>,
    pub indexing: Option<f64>,
    pub threads: Vec<crate::model::Channel>,
}

pub struct SearchState {
    pub input: Entity<InputState>,
    pub request: Option<SearchRequest>,
    pub page: Option<SearchPage>,
    pub loading: bool,
    pub error: Option<SharedString>,
    sort_menu: bool,
    dm_menu: bool,
    seq: u64,
    idle: bool,
    ac: (String, Option<usize>),
    cal: Option<(usize, NaiveDate)>,
    attempts: u32,
    _sub: Subscription,
}

#[derive(Default)]
pub struct SearchHistory(pub Vec<String>);

impl gpui::Global for SearchHistory {}

#[derive(Default)]
pub struct Search(pub Option<SearchState>);

impl gpui::Global for Search {}

pub fn cross_dm(app: &PhoenixApp) -> bool {
    crate::ui::settings::synced_map::get_pick(&app.prefs.synced.proto, "dm-search") == 1
}

pub fn in_dm(app: &PhoenixApp) -> bool {
    app.selected_guild.is_none() && app.selected_channel.is_some()
}

fn dm_peer(app: &PhoenixApp, ch: Id) -> Option<&crate::model::User> {
    let c = app.channels.get(&ch)?;
    if c.guild.is_some() || crate::ui::dm_profile_panel::is_group_dm(app, ch) {
        return None;
    }
    c.voice_users.first().and_then(|u| app.users.get(u))
}

pub fn dm_tag(app: &PhoenixApp, ch: Id) -> SharedString {
    dm_peer(app, ch).map(|u| u.name.clone()).or_else(|| app.channels.get(&ch).map(|c| c.name.clone())).unwrap_or_default()
}

fn dm_label(app: &PhoenixApp, ch: Id) -> SharedString {
    dm_peer(app, ch).map(|u| u.display_name.clone()).or_else(|| app.channels.get(&ch).map(|c| c.name.clone())).unwrap_or_default()
}

fn quote(v: &str) -> String {
    if v.contains(' ') { format!("\"{v}\"") } else { v.to_owned() }
}

pub fn strip_in(q: &str) -> String {
    let local = sq::token(FilterKey::In);
    let mut out = strip_key(q, "in:");
    if local != "in:" {
        out = strip_key(&out, &local);
    }
    out
}

fn strip_key(q: &str, key: &str) -> String {
    let mut out = String::new();
    let mut rest = q;
    let n = key.len();
    while let Some(i) = rest.find(key) {
        let at_word = i == 0 || rest[..i].ends_with(' ');
        if !at_word {
            out.push_str(&rest[..i + n]);
            rest = &rest[i + n..];
            continue;
        }
        out.push_str(&rest[..i]);
        let v = rest[i + n..].trim_start();
        let end = if let Some(stripped) = v.strip_prefix('"') { stripped.find('"').map_or(v.len(), |e| e + 2) } else { v.find(' ').unwrap_or(v.len()) };
        rest = &v[end..];
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn build_request(app: &PhoenixApp, text: &str) -> Result<SearchRequest, String> {
    let p = parse_search_with(text, &sq::keys());
    let all_dms = in_dm(app) && cross_dm(app);
    let mut req = SearchRequest {
        guild: app.selected_guild,
        channel: if app.selected_guild.is_none() && !all_dms { app.selected_channel } else { None },
        all_dms,
        content: (!p.content.trim().is_empty()).then(|| p.content.trim().to_owned()),
        has: p.has.iter().map(|h| h.as_str().to_owned()).collect(),
        pinned: p.pinned,
        author_types: p.author_types.clone(),
        attachment_extensions: p.file_types.clone(),
        attachment_filenames: p.file_names.clone(),
        link_hostnames: p.link_from.clone(),
        report_indexing: true,
        ..Default::default()
    };
    if req.guild.is_none() && req.channel.is_none() && !req.all_dms {
        return Err(crate::t!("search.need_scope").into());
    }
    let find_user = |name: &str| -> Result<Id, String> {
        let n = name.to_lowercase();
        let mut best: Option<(u8, Id)> = None;
        for u in app.users.values() {
            for cand in [u.name.to_lowercase(), u.display_name.to_lowercase()] {
                let s = if cand == n { 0 } else if cand.starts_with(&n) { 1 } else { continue };
                if best.is_none_or(|(b, _)| s < b) {
                    best = Some((s, u.id));
                }
            }
        }
        best.map(|(_, id)| id).ok_or_else(|| crate::t!("search.no_user", name = name).to_string())
    };
    for f in &p.from {
        req.authors.push(find_user(f)?);
    }
    for f in &p.mentions {
        req.mentions.push(find_user(f)?);
    }
    for name in &p.in_channels {
        let n = name.to_lowercase();
        if req.all_dms {
            let ch = app
                .dms
                .iter()
                .copied()
                .find(|d| dm_tag(app, *d).to_lowercase() == n || dm_label(app, *d).to_lowercase() == n)
                .ok_or_else(|| crate::t!("search.no_channel", name = name).to_string())?;
            req.channels.push(ch);
            continue;
        }
        let ch = app
            .guild_channels
            .get(&req.guild.unwrap_or_default())
            .into_iter()
            .flatten()
            .filter_map(|id| app.channels.get(id))
            .find(|c| c.kind != ChannelKind::Category && c.name.to_lowercase() == n)
            .ok_or_else(|| crate::t!("search.no_channel", name = name).to_string())?;
        req.channels.push(ch.id);
    }
    let words = sq::DateWords::current();
    let today = chrono::Local::now().date_naive();
    let range = |d: &str| sq::date_range(d, today, &words).ok_or_else(|| crate::t!("search.bad_date", d = d).to_string());
    if let Some(d) = &p.before {
        req.max_id = Some(Id(sq::local_snowflake(range(d)?.0)));
    }
    if let Some(d) = &p.after {
        req.min_id = Some(Id(sq::local_snowflake(range(d)?.1)));
    }
    if let Some(d) = &p.during {
        let (a, b) = range(d)?;
        req.min_id = Some(Id(sq::local_snowflake(a)));
        req.max_id = Some(Id(sq::local_snowflake(b)));
    }
    let empty = req.content.is_none()
        && req.authors.is_empty()
        && req.mentions.is_empty()
        && req.channels.is_empty()
        && req.has.is_empty()
        && req.min_id.is_none()
        && req.max_id.is_none()
        && req.pinned.is_none()
        && req.author_types.is_empty()
        && req.attachment_extensions.is_empty()
        && req.attachment_filenames.is_empty()
        && req.link_hostnames.is_empty();
    if empty {
        return Err(String::new());
    }
    Ok(req)
}

pub fn results_shown(cx: &App) -> bool {
    cx.try_global::<Search>().and_then(|s| s.0.as_ref()).is_some_and(|s| s.page.is_some() && s.request.is_some())
}

pub fn is_open(cx: &App) -> bool {
    cx.try_global::<Search>().is_some_and(|s| s.0.is_some())
}

pub fn is_focused(window: &Window, cx: &App) -> bool {
    cx.try_global::<Search>()
        .and_then(|s| s.0.as_ref())
        .is_some_and(|s| gpui::Focusable::focus_handle(s.input.read(cx), cx).is_focused(window))
}

pub fn open(_app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if let Some(s) = cx.try_global::<Search>().and_then(|s| s.0.as_ref()) {
        let input = s.input.clone();
        if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
            s.idle = false;
        }
        input.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
        return;
    }
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("search.placeholder")));
    let sub = cx.subscribe_in(&input, window, |app, _, ev: &InputEvent, _, cx| {
        if let InputEvent::PressEnter { .. } = ev {
            submit(app, cx);
        }
    });
    input.update(cx, |s, cx| s.focus(window, cx));
    cx.set_global(Search(Some(SearchState {
        input,
        request: None,
        page: None,
        loading: false,
        error: None,
        sort_menu: false,
        dm_menu: false,
        seq: 0,
        idle: false,
        ac: (String::new(), None),
        cal: None,
        attempts: 0,
        _sub: sub,
    })));
    cx.notify();
}

pub fn close(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if cx.has_global::<Search>() {
        cx.global_mut::<Search>().0 = None;
    }
    app.focus_chat_composer(window, cx);
    cx.notify();
}

fn set_idle(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let empty = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).is_none_or(|s| s.input.read(cx).value().trim().is_empty());
    if empty {
        close(app, window, cx);
        return;
    }
    if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
        s.idle = true;
    }
    app.focus_chat_composer(window, cx);
    cx.notify();
}

pub fn on_key(app: &mut PhoenixApp, key: &str, window: &mut Window, cx: &mut Context<PhoenixApp>) -> bool {
    if key == "escape" && crate::ui::search_filters::is_open(cx) {
        if !crate::ui::search_filters::close_popover(cx) {
            crate::ui::search_filters::close(cx);
        }
        return true;
    }
    if key == "escape" && (close_sort_menu(cx) | close_dm_menu(cx)) {
        return true;
    }
    if matches!(key, "up" | "down" | "enter") && !results_shown(cx) {
        let Some(text) = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).map(|s| s.input.read(cx).value().to_string()) else { return false };
        let shape = popout_layout(app, &text, cx);
        let rows = shape.rows();
        if !rows.is_empty() {
            let first = shape.first_selected();
            let Some(s) = cx.global_mut::<Search>().0.as_mut() else { return false };
            if s.ac.0 != text {
                s.ac = (text, first);
            }
            let n = rows.len();
            let cur = s.ac.1.map(|c| c.min(n - 1));
            match key {
                "up" => s.ac.1 = Some(cur.map_or(n - 1, |c| (c + n - 1) % n)),
                "down" => s.ac.1 = Some(cur.map_or(0, |c| (c + 1) % n)),
                _ => {
                    let Some(c) = cur else { return false };
                    activate(app, rows[c].clone(), window, cx);
                    return true;
                }
            }
            cx.notify();
            return true;
        }
    }
    if key == "escape" {
        let input = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).map(|s| s.input.clone());
        if let Some(input) = input.filter(|i| !i.read(cx).value().is_empty() && !results_shown(cx)) {
            input.update(cx, |s, cx| s.set_value("", window, cx));
            cx.notify();
            return true;
        }
        close(app, window, cx);
        return true;
    }
    false
}

fn pick_answer(app: &mut PhoenixApp, value: &str, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some(input) = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).map(|s| s.input.clone()) else { return };
    let text = input.read(cx).value().to_string();
    let Some(p) = pending_filter(&text, &sq::keys()) else { return };
    let next = complete_filter(&text, &p, value);
    input.update(cx, |s, cx| s.set_value(next, window, cx));
    submit(app, cx);
}

fn submit(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let Some(s) = cx.global::<Search>().0.as_ref() else { return };
    let text = s.input.read(cx).value().to_string();
    match build_request(app, &text) {
        Ok(req) => {
            let t = text.trim().to_owned();
            let h = &mut cx.default_global::<SearchHistory>().0;
            h.retain(|x| *x != t);
            h.insert(0, t);
            h.truncate(5);
            run(app, req, cx)
        }
        Err(e) => {
            if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
                s.error = (!e.is_empty()).then(|| e.into());
                s.page = None;
                s.request = None;
            }
            cx.notify();
        }
    }
}

pub fn run(app: &mut PhoenixApp, req: SearchRequest, cx: &mut Context<PhoenixApp>) {
    run_attempt(app, req, 0, cx);
}

pub const INDEX_RETRIES: u32 = 5;

fn run_attempt(app: &mut PhoenixApp, req: SearchRequest, attempts: u32, cx: &mut Context<PhoenixApp>) {
    let Some(s) = cx.global_mut::<Search>().0.as_mut() else { return };
    s.seq += 1;
    let seq = s.seq;
    s.loading = attempts == 0;
    s.error = None;
    s.attempts = attempts;
    s.request = Some(req.clone());
    let fut = app.backend.search_messages(req.clone());
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let retry = this
            .update(cx, |app, cx| {
                if let Ok(p) = &res {
                    adopt_threads(app, p, cx);
                }
                let s = cx.global_mut::<Search>().0.as_mut()?;
                if s.seq != seq {
                    return None;
                }
                s.loading = false;
                let mut retry = None;
                match res {
                    Ok(p) => {
                        if let Some(wait) = p.indexing {
                            s.attempts += 1;
                            retry = (s.attempts <= INDEX_RETRIES).then_some((wait, s.attempts));
                        }
                        s.page = Some(p);
                    }
                    Err(e) => {
                        s.page = None;
                        s.error = Some(e.0.into());
                    }
                }
                cx.notify();
                retry
            })
            .ok()
            .flatten();
        let Some((wait, attempts)) = retry else { return };
        cx.background_executor().timer(std::time::Duration::from_secs_f64(wait)).await;
        let _ = this.update(cx, |app, cx| {
            if cx.try_global::<Search>().and_then(|s| s.0.as_ref()).is_some_and(|s| s.seq == seq) {
                run_attempt(app, req, attempts, cx);
            }
        });
    })
    .detach();
    cx.notify();
}

pub fn indexing_text(req: &SearchRequest) -> SharedString {
    if req.all_dms {
        crate::t!("search.indexing_dms")
    } else if req.guild.is_some() {
        crate::t!("search.indexing_guild")
    } else {
        crate::t!("search.indexing_dm")
    }
}

fn rerun(app: &mut PhoenixApp, f: impl FnOnce(&mut SearchRequest), cx: &mut Context<PhoenixApp>) {
    let Some(mut req) = cx.global::<Search>().0.as_ref().and_then(|s| s.request.clone()) else { return };
    f(&mut req);
    run(app, req, cx);
}

pub fn set_query(q: &str, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some(input) = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).map(|s| s.input.clone()) else { return };
    let q = q.to_string();
    input.update(cx, |s, cx| s.set_value(q, window, cx));
}

pub fn append_and_submit(app: &mut PhoenixApp, tokens: &str, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some(input) = cx.global::<Search>().0.as_ref().map(|s| s.input.clone()) else { return };
    if !tokens.is_empty() {
        input.update(cx, |s, cx| {
            let v = s.value().trim_end().to_string();
            let sep = if v.is_empty() { "" } else { " " };
            s.set_value(format!("{v}{sep}{tokens}"), window, cx);
        });
    }
    submit(app, cx);
}

fn add_filter(key: FilterKey, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some(s) = cx.global::<Search>().0.as_ref() else { return };
    let input = s.input.clone();
    let prefix = sq::token(key);
    input.update(cx, |s, cx| {
        let v = s.value().to_string();
        let sep = if v.is_empty() || v.ends_with(' ') { "" } else { " " };
        s.set_value(format!("{v}{sep}{prefix}"), window, cx);
        s.focus(window, cx);
    });
}

const FILTERS: [(FilterKey, &str, &str, &str); 4] = [
    (FilterKey::From, "search/user", "search.f_from", "search.v_user"),
    (FilterKey::In, "hash", "search.f_in", "search.v_channel"),
    (FilterKey::Has, "search/attachment", "search.f_has", "search.v_has"),
    (FilterKey::Mentions, "search/at", "search.f_mentions", "search.v_user"),
];


fn result_card(app: &PhoenixApp, ix: usize, m: &Message, hl: &Highlighter, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let guild = app.channel_guild(m.channel).or(app.selected_guild);
    let author_cow = app.member_user(guild, m.author);
    let author = author_cow.as_deref();
    let channel = m.channel;
    let id = m.id;
    let content: String = m.content.chars().take(400).collect();
    let rich = !content.is_empty();
    let acx: &App = cx;
    let extras: Vec<AnyElement> = m
        .attachments
        .iter()
        .map(|a| crate::ui::chat::attachment(a, acx))
        .chain(m.embeds.iter().map(|e| crate::ui::chat::embed(app, &cx.entity(), e, acx)))
        .chain(crate::ui::chat::invite_embed::render(app, &cx.entity(), m.author, &m.content, acx))
        .chain(crate::ui::components::render_static(app, &cx.entity(), m, acx))
        .collect();
    let bot = author.is_some_and(|u| u.bot);
    let shown: SharedString = m
        .author_nick
        .clone()
        .or_else(|| app.member_nick_in(guild, m.author))
        .or_else(|| author.map(|u| u.display_name.clone()))
        .unwrap_or_else(|| crate::t!("common.unknown"));
    let (role_color, role_gradient) = app.author_role_style_in(guild, m.author);
    let reply = m.reply_to.is_some().then(|| reply_line(app, m, &cx.entity()));
    let text: Option<AnyElement> = rich.then(|| crate::ui::components::markdown_highlighted(app, &content, &cx.entity(), m.id, hl));
    div()
        .id(("search-hit", ix))
        .mb(z(8.))
        .rounded(z(8.))
        .bg(tk(idx::BACKGROUND_BASE_LOWEST))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .relative()
        .group("search-hit")
        .pt(z(6.))
        .pb(z(5.))
        .pl(z(80.))
        .pr(z(20.))
        .cursor_pointer()
        .on_click(cx.listener(move |a, _, _, cx| a.jump_to_message(channel, id, cx)))
        .on_mouse_down(gpui::MouseButton::Right, {
            let (author, pinned, kind) = (m.author, m.pinned, crate::ui::chat::panels::wire_type(m));
            let (e, hit) = (cx.entity(), m.clone());
            move |ev, w, cx| {
                cx.stop_propagation();
                e.update(cx, |a, _| a.chat.search_menu_msg = Some(hit.clone()));
                let link = crate::ui::chat::mention::any_hovered_link(id);
                crate::ui::extra_menus::open(crate::ui::extra_menus::Target::SearchResult { channel, msg: id, author, pinned, kind, link }, ev.position, w)
            }
        })
        .children(reply)
        .child(
            div()
                .absolute()
                .left(z(20.))
                .top(z(if m.reply_to.is_some() { 30. } else { 8. }))
                .child(avatar(author, 40.))
                .when_some(crate::ui::avatar_deco::source(author, None), |d, src| d.child(crate::ui::avatar_deco::layer(src, 40. * crate::theme::zoom()))),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .min_h(z(22.))
                .min_w_0()
                .child(
                    crate::fonts::styled_name(
                        div()
                            .max_w_full()
                            .text_size(z(16.))
                            .line_height(z(22.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(role_color.map(c).unwrap_or_else(|| tk(idx::TEXT_STRONG))),
                        author,
                        false,
                    )
                    .id(("search-author", ix))
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .on_click(crate::ui::chat::message_ext::open_user(&cx.entity(), m.author, m.channel))
                    .child(crate::fonts::role_name_text(shown, author, 16., guild.is_some(), role_gradient, false)),
                )
                .when_some(author.and_then(|u| u.tag.clone()), |d, tg| d.child(crate::ui::chat::server_tag_chip(&tg, false)))
                .when_some(app.author_role_icon_in(guild, m.author), |d, u| d.child(crate::ui::chat::role_icon(u)))
                .when(bot, |d| d.child(crate::ui::chat::bot_tag(author.is_some_and(|u| u.verified_bot))))
                .child(
                    div()
                        .flex_none()
                        .ml(z(4.))
                        .text_size(z(12.))
                        .line_height(z(22.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(tk(idx::TEXT_MUTED))
                        .child(format_time_full(m.timestamp)),
                ),
        )
        .when_some(text, |d, t| d.child(div().text_size(z(16.)).line_height(z(22.)).text_color(tk(idx::TEXT_DEFAULT)).child(t)))
        .children(extras)
        .child(
            div().absolute().top(z(8.)).right(z(8.)).invisible().group_hover("search-hit", |s| s.visible()).child(
                div()
                    .id(("search-jump", ix))
                    .h(z(24.))
                    .p(z(4.))
                    .rounded(z(3.))
                    .flex()
                    .items_center()
                    .bg(tk(idx::BACKGROUND_BASE_LOWEST))
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .hover(|s| s.text_color(tk(idx::INTERACTIVE_TEXT_HOVER)))
                    .active(|s| s.text_color(tk(idx::INTERACTIVE_TEXT_ACTIVE)).mt(z(1.)))
                    .on_click(cx.listener(move |a, _, _, cx| {
                        cx.stop_propagation();
                        a.jump_to_message(channel, id, cx)
                    }))
                    .child(crate::t!("pins.jump")),
            ),
        )
        .into_any_element()
}

fn highlight_plain(text: SharedString, hl: &Highlighter) -> gpui::StyledText {
    let style = gpui::HighlightStyle { background_color: Some(gpui::rgba(crate::search_highlight::HIGHLIGHT_RGBA).into()), ..Default::default() };
    let ranges = hl.ranges(&text);
    gpui::StyledText::new(text).with_highlights(ranges.into_iter().map(|r| (r, style)))
}

fn reply_line(app: &PhoenixApp, m: &Message, entity: &gpui::Entity<PhoenixApp>) -> gpui::Div {
    let spine = div()
        .absolute()
        .left(z(-41.))
        .top(z(9.))
        .w(z(37.))
        .h(z(13.))
        .border_l_2()
        .border_t_2()
        .rounded_tl(z(6.))
        .border_color(tk(idx::SPINE_DEFAULT));
    let row = div().relative().h(z(18.)).mb(z(4.)).flex().items_center().gap(z(4.)).text_size(z(14.)).line_height(z(18.)).text_color(tk(idx::TEXT_DEFAULT)).child(spine);
    match &m.reply_preview {
        Some((a, nick, text)) => {
            let guild = app.channel_guild(m.channel).or(app.selected_guild);
            let ru_cow = app.member_user(guild, *a);
            let ru = ru_cow.as_deref();
            let name: SharedString = nick.clone().or_else(|| app.member_nick_in(guild, *a)).or_else(|| ru.map(|u| u.display_name.clone())).unwrap_or_else(|| crate::t!("common.unknown"));
            let (reply_color, reply_gradient) = app.author_role_style_in(guild, *a);
            let preview: String = text.chars().take(200).collect::<String>().replace('\n', " ");
            row.child(avatar(ru, 16.))
                .child(
                    crate::fonts::styled_name(
                        div().flex_none().font_weight(FontWeight::MEDIUM).opacity(0.64).text_color(reply_color.map(c).unwrap_or_else(|| tk(idx::TEXT_STRONG))),
                        ru,
                        false,
                    )
                    .id(("search-reply-author", m.id.0))
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .on_click(crate::ui::chat::message_ext::open_user(entity, *a, m.channel))
                    .child(crate::fonts::role_name_text(name, ru, 14., guild.is_some(), reply_gradient, false)),
                )
                .map(|r| {
                    if preview.trim().is_empty() && m.reply_voice {
                        r.child(div().flex_1().min_w_0().truncate().italic().child(crate::t!("chat.reply_click_to_see_voice")))
                    } else {
                        r.child(div().flex_1().min_w_0().truncate().child(preview))
                    }
                })
        }
        None => row
            .child(div().size(z(16.)).flex().items_center().justify_center().child(named("reply", 12., tk(idx::TEXT_DEFAULT))))
            .child(div().italic().truncate().child(crate::t!("search.reply_unloaded"))),
    }
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let s = cx.try_global::<Search>()?.0.as_ref()?;
    let input = s.input.clone();
    let loading = s.loading;
    let error = s.error.clone();
    let page = s.page.clone();
    let req = s.request.clone();
    let sort_menu = s.sort_menu;
    if s.idle && (page.is_none() || req.is_none()) && !gpui::Focusable::focus_handle(input.read(cx), cx).is_focused(window) {
        return None;
    }
    if (page.is_none() || req.is_none())
        && let Some(modal) = crate::ui::search_filters::render(app, window, cx)
    {
        return Some(modal);
    }
    let (Some(page), Some(req)) = (page, req) else {
        return Some(options_popout(app, input, error, loading, cx));
    };
    Some(results_panel(app, input, page, req, error, loading, sort_menu, window, cx))
}

fn close_dm_menu(cx: &mut App) -> bool {
    let Some(s) = cx.try_global::<Search>().and_then(|s| s.0.as_ref()) else { return false };
    if !s.dm_menu {
        return false;
    }
    if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
        s.dm_menu = false;
    }
    cx.refresh_windows();
    true
}

fn close_sort_menu(cx: &mut App) -> bool {
    let open = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).is_some_and(|s| s.sort_menu);
    if open {
        if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
            s.sort_menu = false;
        }
        cx.refresh_windows();
    }
    open
}

pub(crate) fn group_digits(n: u32) -> String {
    let sep = if crate::i18n::locale() == "en" { ',' } else { '\u{202f}' };
    let d = n.to_string();
    let mut out = String::new();
    for (i, ch) in d.chars().enumerate() {
        if i > 0 && (d.len() - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(ch);
    }
    out
}

fn page_items(cur: u32, last: u32) -> Vec<Option<u32>> {
    let mut want: Vec<u32> = vec![1, last, cur.saturating_sub(1).max(1), cur, (cur + 1).min(last)];
    if cur <= 2 {
        want.push(3.min(last));
    }
    if cur + 1 >= last {
        want.push(last.saturating_sub(2).max(1));
    }
    want.sort_unstable();
    want.dedup();
    let mut out = Vec::new();
    let mut prev = 0;
    for p in want {
        if p > prev + 1 {
            out.push(None);
        }
        out.push(Some(p));
        prev = p;
    }
    out
}

fn live_bar(input: &Entity<InputState>, active: bool, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    div()
        .id("search-bar-live")
        .absolute()
        .top(z(layout::TITLE_BAR_H + 8.))
        .right(z(8.))
        .w(z(244.))
        .h(z(32.))
        .rounded(z(8.))
        .bg(c(Palette::BG_PRIMARY))
        .border_1()
        .border_color(tk(if active { idx::INPUT_BORDER_ACTIVE } else { idx::INPUT_BORDER_DEFAULT }))
        .occlude()
        .flex()
        .items_center()
        .child(div().flex_1().min_w_0().px(z(6.)).text_size(z(14.)).font_weight(FontWeight::MEDIUM).child(Input::new(input).appearance(false).bordered(false)))
        .child(
            div()
                .id("search-bar-close")
                .size(z(24.))
                .p(z(4.))
                .flex()
                .items_center()
                .cursor_pointer()
                .on_click(cx.listener(|a, _, w, cx| close(a, w, cx)))
                .child(named("close", 16., tk(idx::ICON_DEFAULT))),
        )
}

fn sm_secondary(id: &'static str, glyph: &str, label: SharedString) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(z(32.))
        .px(z(12.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
        .hover(|s| s.bg(tk(idx::BACKGROUND_MOD_NORMAL)))
        .flex()
        .items_center()
        .gap(z(4.))
        .cursor_pointer()
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(tk(idx::TEXT_STRONG))
        .child(named(glyph, 16., tk(idx::TEXT_STRONG)))
        .child(label)
}

#[allow(clippy::too_many_arguments)]
fn results_panel(
    app: &PhoenixApp,
    input: Entity<InputState>,
    page: SearchPage,
    req: SearchRequest,
    error: Option<SharedString>,
    loading: bool,
    sort_menu: bool,
    window: &mut Window,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let total = page.total;
    let dm_scope = req.all_dms || (req.guild.is_none() && req.channel.is_some());
    let dm_menu = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).is_some_and(|s| s.dm_menu);
    let title: SharedString = if loading || page.indexing.is_some() {
        crate::t!("search.searching")
    } else if total == 0 {
        crate::t!("search.no_result_title")
    } else if total == 1 {
        crate::t!("search.result_one")
    } else {
        crate::t!("search.results", n = group_digits(total))
    };
    let header = div()
        .flex_none()
        .h(z(48.))
        .px(z(16.))
        .py(z(8.))
        .flex()
        .items_center()
        .justify_between()
        .gap(z(8.))
        .child(div().min_w_0().truncate().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_STRONG)).child(title))
        .child(
            div()
                .flex_none()
                .flex()
                .gap(z(8.))
                .child(sm_secondary("search-results-filters", "inbox/filter", crate::t!("search.filters")).on_click(cx.listener(|a, _, w, cx| {
                    crate::ui::search_filters::open(a, w, cx);
                })))
                .child(sm_secondary("search-results-sort", "sort", crate::t!("search.sort")).on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|_, _, _, cx| {
                        cx.stop_propagation();
                        if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
                            s.sort_menu = !s.sort_menu;
                            s.dm_menu = false;
                        }
                        cx.notify();
                    }),
                ))
                .when(dm_scope, |d| {
                    d.child(icon_secondary("search-results-dm-settings", "gear").on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|_, _, _, cx| {
                            cx.stop_propagation();
                            if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
                                s.dm_menu = !s.dm_menu;
                                s.sort_menu = false;
                            }
                            cx.notify();
                        }),
                    ))
                }),
        );
    let results_scroll = crate::ui::chat::scrollbar::handle("search-results");
    let gutter = if f32::from(results_scroll.max_offset().y) > 0. { 8. } else { 0. };
    let mut body = div().flex().flex_col().pt(z(16.)).pl(z(16.)).pr(z(16. + gutter));
    if let Some(e) = error.filter(|_| !loading) {
        body = body.child(empty_state(e, true));
    } else if page.indexing.is_some() && page.messages.is_empty() && !loading {
        body = body.child(empty_state(indexing_text(&req), false));
    } else if page.messages.is_empty() && !loading {
        body = body.child(empty_state(crate::t!("search.no_results"), false));
    } else {
        let hl = Highlighter::new(req.content.as_deref().unwrap_or_default(), false);
        let mut i = 0;
        while i < page.messages.len() {
            let ch = page.messages[i].channel;
            let mut group = div().mb(z(24.)).flex().flex_col().child(channel_group_header(app, ch, &hl));
            while i < page.messages.len() && page.messages[i].channel == ch {
                group = group.child(result_card(app, i, &page.messages[i], &hl, cx));
                i += 1;
            }
            body = body.child(group);
        }
    }
    let pages = total.div_ceil(PAGE).max(1);
    let cur = req.offset / PAGE + 1;
    let pager = (pages > 1).then(|| pager(cur, pages, cx));
    let mut panel = div()
        .id("search-popout")
        .absolute()
        .top(z(layout::TITLE_BAR_H + layout::HEADER_H))
        .right_0()
        .bottom_0()
        .w(results_width())
        .bg(tk(idx::BACKGROUND_BASE_LOWEST))
        .border_l_1()
        .border_color(tk(idx::APP_FRAME_BORDER))
        .occlude()
        .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| {
            close_sort_menu(cx);
            close_dm_menu(cx);
        }))
        .flex()
        .flex_col()
        .child(header)
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(div().id("search-scroll").flex_1().min_h_0().overflow_y_scroll().track_scroll(&results_scroll).child(body))
                .children(crate::ui::chat::scrollbar::thin_scrollbar_div(&results_scroll)),
        )
        .children(pager);
    if sort_menu {
        panel = panel.child(sort_popover(req.sort, cx));
    }
    if dm_menu && dm_scope {
        panel = panel.child(dm_popover(req.all_dms, cx));
    }
    let modal = crate::ui::search_filters::render(app, window, cx);
    div().absolute().top_0().left_0().right_0().bottom_0().child(live_bar(&input, false, cx)).child(panel).children(modal).into_any_element()
}

pub(crate) fn adopt_threads(app: &mut PhoenixApp, page: &SearchPage, cx: &mut Context<PhoenixApp>) {
    for t in &page.threads {
        app.channels.entry(t.id).or_insert_with(|| t.clone());
    }
    let mut missing: Vec<Id> = page.messages.iter().map(|m| m.channel).filter(|c| !app.channels.contains_key(c) && app.nav.find_thread(*c).is_none()).collect();
    missing.sort_unstable();
    missing.dedup();
    missing.truncate(5);
    for id in missing {
        app.parity_request(crate::commands::ParityRequest::FetchThread { id }, cx, move |a, r, cx| {
            if let Ok(crate::commands::ParityReply::Thread(ch)) = r {
                a.channels.entry(id).or_insert(ch);
                cx.notify();
            }
        });
    }
}

pub(crate) fn grouped_results(app: &PhoenixApp, messages: &[Message], cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let hl = Highlighter::new("", false);
    let mut out = div().flex().flex_col();
    let mut i = 0;
    while i < messages.len() {
        let ch = messages[i].channel;
        let mut group = div().mb(z(24.)).flex().flex_col().child(channel_group_header(app, ch, &hl));
        while i < messages.len() && messages[i].channel == ch {
            group = group.child(result_card(app, i, &messages[i], &hl, cx));
            i += 1;
        }
        out = out.child(group);
    }
    out
}

fn channel_group_header(app: &PhoenixApp, ch: Id, hl: &Highlighter) -> gpui::Div {
    let channel = app.channels.get(&ch).or_else(|| app.nav.find_thread(ch));
    let private = channel.is_some_and(|c| c.guild.is_none());
    let name: SharedString = match channel {
        Some(_) if private => dm_label(app, ch),
        Some(c) => c.name.clone(),
        None => crate::t!("common.unknown_lower"),
    };
    let glyph = match channel {
        Some(_) if private => crate::icons::path("search/at"),
        Some(c) => crate::ui::sidebar::channel_icon(c.kind).path(),
        None => Icon::Hash.path(),
    };
    let parent = channel.and_then(|c| c.parent).and_then(|p| app.channels.get(&p)).map(|p| p.name.clone());
    div()
        .h(z(22.))
        .mb(z(8.))
        .flex()
        .items_center()
        .child(div().w(z(20.)).flex_none().pr(z(4.)).child(gpui::svg().path(glyph).size(z(16.)).text_color(tk(idx::TEXT_STRONG))))
        .child(div().flex_1().min_w_0().truncate().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_STRONG)).child(header_name(name, hl)))
        .when_some(parent, |d, p| {
            d.child(
                div()
                    .max_w(z(190.))
                    .min_w_0()
                    .flex()
                    .items_center()
                    .child(named("folder", 14., tk(idx::TEXT_DEFAULT)).ml(z(4.)).mr(z(4.)))
                    .child(div().min_w_0().truncate().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_DEFAULT)).child(p)),
            )
        })
}

fn header_name(name: SharedString, hl: &Highlighter) -> AnyElement {
    let emoji: Vec<_> = crate::richtext::emoji_ranges(&name).into_iter().map(|(r, _)| r).filter(|r| !name[r.clone()].starts_with('<')).collect();
    if emoji.is_empty() {
        return highlight_plain(name, hl).into_any_element();
    }
    let size = 22. * crate::theme::zoom();
    let mut row = div().flex().items_center().min_w_0().overflow_hidden().whitespace_nowrap();
    let mut at = 0;
    for r in emoji {
        if r.start > at {
            row = row.child(div().flex_none().child(highlight_plain(name[at..r.start].to_string().into(), hl)));
        }
        row = row.child(div().flex_none().mx(px(1.)).child(crate::media::emoji::emoji_element(&name[r.clone()], size)));
        at = r.end;
    }
    if at < name.len() {
        row = row.child(div().min_w_0().truncate().child(highlight_plain(name[at..].to_string().into(), hl)));
    }
    row.into_any_element()
}

fn empty_state(text: SharedString, error: bool) -> gpui::Div {
    div().min_h(z(600.)).p(z(20.)).flex().items_center().justify_center().child(
        div()
            .w(z(280.))
            .pb(z(40.))
            .flex()
            .flex_col()
            .items_center()
            .gap(z(40.))
            .child(
                div()
                    .size(z(160.))
                    .rounded_full()
                    .border_1()
                    .border_color(tk(idx::BORDER_SUBTLE))
                    .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(named(if error { "warning" } else { "search" }, 72., tk(if error { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_MUTED }))),
            )
            .child(
                div()
                    .text_center()
                    .text_size(z(16.))
                    .line_height(z(24.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(tk(if error { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_DEFAULT }))
                    .child(text),
            ),
    )
}

type GoPage = std::rc::Rc<dyn Fn(&mut PhoenixApp, u32, &mut Context<PhoenixApp>)>;

fn pager(cur: u32, pages: u32, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    pager_with(cur, pages, std::rc::Rc::new(go_page), cx)
}

pub(crate) fn pager_with(cur: u32, pages: u32, go: GoPage, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let caret = |next: bool| {
        let g = named("chevron-right", 16., tk(idx::TEXT_STRONG));
        if next { g } else { g.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI))) }
    };
    let end = |id: &'static str, label: SharedString, next: bool, enabled: bool| {
        div()
            .id(id)
            .h(z(40.))
            .when(next, |d| d.pl(z(12.)).pr(z(8.)))
            .when(!next, |d| d.pl(z(8.)).pr(z(12.)))
            .rounded(z(8.))
            .flex()
            .items_center()
            .gap(z(4.))
            .text_size(z(16.))
            .line_height(z(18.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(tk(idx::TEXT_STRONG))
            .when(!enabled, |d| d.opacity(0.5))
            .when(enabled, |d| d.cursor_pointer().hover(|s| s.bg(tk(idx::BACKGROUND_MOD_SUBTLE))))
            .when(!next, |d| d.child(caret(false)))
            .child(label)
            .when(next, |d| d.child(caret(true)))
    };
    let mut nav = div().flex().items_center().gap(z(8.));
    let mut prev = end("search-prev", crate::t!("search.prev_page"), false, cur > 1);
    if cur > 1 {
        let go = go.clone();
        prev = prev.on_click(cx.listener(move |a, _, _, cx| go(a, cur - 1, cx)));
    }
    nav = nav.child(prev);
    for (k, item) in page_items(cur, pages).into_iter().enumerate() {
        nav = nav.child(match item {
            None => div()
                .id(("search-page-gap", k))
                .w(z(28.))
                .flex()
                .justify_center()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk(idx::TEXT_DEFAULT))
                .child("…"),
            Some(p) => {
                let on = p == cur;
                div()
                    .id(("search-page", p as usize))
                    .h(z(28.))
                    .min_w(z(28.))
                    .px(z(6.))
                    .rounded(z(14.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(z(16.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .when(on, |d| d.bg(tk(idx::BRAND_500)).text_color(tk(idx::WHITE)))
                    .when(!on, |d| {
                        let go = go.clone();
                        d.text_color(tk(idx::TEXT_STRONG)).cursor_pointer().hover(|s| s.bg(tk(idx::BACKGROUND_MOD_SUBTLE))).on_click(cx.listener(move |a, _, _, cx| go(a, p, cx)))
                    })
                    .child(p.to_string())
            }
        });
    }
    let mut next = end("search-next", crate::t!("common.next"), true, cur < pages);
    if cur < pages {
        next = next.on_click(cx.listener(move |a, _, _, cx| go(a, cur + 1, cx)));
    }
    nav = nav.child(next);
    div().flex_none().h(z(56.)).px(z(16.)).flex().items_center().justify_center().child(nav)
}

fn go_page(app: &mut PhoenixApp, p: u32, cx: &mut Context<PhoenixApp>) {
    crate::ui::chat::scrollbar::handle("search-results").set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
    rerun(app, |r| r.offset = p.saturating_sub(1) * PAGE, cx);
}

fn icon_secondary(id: &'static str, glyph: &str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(z(32.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
        .hover(|s| s.bg(tk(idx::BACKGROUND_MOD_NORMAL)))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(named(glyph, 16., tk(idx::TEXT_STRONG)))
}

pub fn set_cross_dm(app: &mut PhoenixApp, on: bool, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    close_dm_menu(cx);
    if cross_dm(app) == on {
        return;
    }
    crate::ui::settings::save(app, crate::ui::settings::SettingsPatch::Proto("dm-search", u64::from(on)), cx);
    let Some(input) = cx.global::<Search>().0.as_ref().map(|s| s.input.clone()) else { return };
    if !on {
        input.update(cx, |s, cx| {
            let v = strip_in(&s.value());
            s.set_value(v, window, cx);
        });
    }
    submit(app, cx);
}

fn dm_popover(all: bool, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let row = |key: &'static str, on: bool, value: bool, cx: &mut Context<PhoenixApp>| {
        div()
            .id(key)
            .h(z(36.))
            .p(z(8.))
            .rounded(z(2.))
            .flex()
            .items_center()
            .justify_between()
            .cursor_pointer()
            .hover(|s| s.bg(tk(idx::BACKGROUND_MOD_SUBTLE)))
            .on_click(cx.listener(move |a, _, w, cx| set_cross_dm(a, value, w, cx)))
            .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_STRONG)).child(crate::t!(key)))
            .child(crate::ui::controls::radio_indicator(on))
    };
    div()
        .id("search-dm-menu")
        .absolute()
        .top(z(47.))
        .right(z(16.))
        .w(z(237.))
        .p(z(8.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHER))
        .shadow(crate::ui::inbox::popout_shadow())
        .occlude()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .h(z(36.))
                .p(z(8.))
                .flex()
                .items_center()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(tk(idx::TEXT_MUTED))
                .child(crate::t!("search.dm_default")),
        )
        .child(row("search.dm_this", !all, false, cx))
        .child(row("search.dm_all", all, true, cx))
}

fn sort_popover(cur: SortOrder, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let items = [(SortOrder::Newest, "search.newest"), (SortOrder::Oldest, "search.sort_date"), (SortOrder::Relevance, "search.relevance")];
    div()
        .id("search-sort-menu")
        .absolute()
        .top(z(47.))
        .right(z(16.))
        .w(z(144.))
        .py(z(8.))
        .pl(z(8.))
        .pr(z(8.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHER))
        .shadow(crate::ui::inbox::popout_shadow())
        .occlude()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .children(items.into_iter().map(|(o, key)| {
            div()
                .id(key)
                .h(z(36.))
                .p(z(8.))
                .rounded(z(2.))
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .hover(|s| s.bg(tk(idx::BACKGROUND_MOD_SUBTLE)))
                .on_click(cx.listener(move |a, _, _, cx| {
                    close_sort_menu(cx);
                    rerun(
                        a,
                        |r| {
                            r.sort = o;
                            r.offset = 0
                        },
                        cx,
                    )
                }))
                .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_STRONG)).child(crate::t!(key)))
                .child(crate::ui::controls::radio_indicator(o == cur))
        }))
}

fn tk(i: usize) -> gpui::Hsla {
    c(crate::theme::tok(i))
}

fn named(name: &str, size: f32, color: gpui::Hsla) -> gpui::Svg {
    gpui::svg().path(crate::icons::path(name)).size(z(size)).flex_none().text_color(color)
}

#[derive(Debug, PartialEq)]
enum HistorySeg {
    Text(String),
    Filter { key: FilterKey, value: String },
}

fn history_segments(q: &str) -> Vec<HistorySeg> {
    let keys = sq::keys();
    let mut out: Vec<HistorySeg> = Vec::new();
    for word in q.split_whitespace() {
        let filter = word.split_once(':').filter(|(_, v)| !v.is_empty()).and_then(|(k, v)| keys.key(k).map(|k| (k, v)));
        if let Some((key, value)) = filter {
            out.push(HistorySeg::Filter { key, value: value.to_owned() });
        } else if let Some(HistorySeg::Text(t)) = out.last_mut() {
            t.push(' ');
            t.push_str(word);
        } else {
            out.push(HistorySeg::Text(word.to_owned()));
        }
    }
    out
}

fn history_pills(app: &PhoenixApp, q: &str) -> gpui::Div {
    let mut row = div().flex().items_center().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_STRONG)).whitespace_nowrap().overflow_hidden();
    let pill = || div().py(z(3.)).bg(tk(idx::BACKGROUND_MOD_STRONG));
    for seg in history_segments(q) {
        row = match seg {
            HistorySeg::Text(t) => row.child(div().mr(z(8.)).child(t)),
            HistorySeg::Filter { key, value } => {
                let label: SharedString = sq::token(key).into();
                let user = matches!(key, FilterKey::From | FilterKey::Mentions)
                    .then(|| {
                        let n = value.to_lowercase();
                        app.users.values().find(|u| u.name.to_lowercase() == n || u.display_name.to_lowercase() == n)
                    })
                    .flatten();
                let mut answer = pill().pl(z(2.)).pr(z(6.)).mr(z(8.)).rounded_r(z(4.)).flex().items_center().gap(z(4.));
                if let Some(u) = user {
                    answer = answer.child(crate::ui::avatar(Some(u), 16.));
                }
                row.child(pill().pl(z(6.)).pr(z(2.)).rounded_l(z(4.)).child(label)).child(answer.child(value))
            }
        };
    }
    row
}

fn option_row(id: (&'static str, usize)) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px(z(8.))
        .py(z(4.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .cursor_pointer()
        .hover(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
}

fn group_header(label: SharedString) -> gpui::Div {
    div()
        .px(z(8.))
        .py(z(4.))
        .flex()
        .items_center()
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(tk(idx::TEXT_SUBTLE))
        .child(div().flex_1().child(label))
}

fn answer_lead(app: &PhoenixApp, lead: &AnswerLead) -> Option<AnyElement> {
    let user = |id: Id| if id == app.me.id { Some(&app.me) } else { app.users.get(&id) };
    Some(match lead {
        AnswerLead::User(id) => avatar(user(*id), 20.),
        AnswerLead::Dm(ch) => avatar(dm_peer(app, *ch), 20.),
        AnswerLead::Channel(kind) => {
            gpui::svg().path(crate::ui::sidebar::channel_icon(*kind).path()).flex_none().size(z(20.)).text_color(tk(idx::TEXT_MUTED)).into_any_element()
        }
        AnswerLead::Glyph(g) => named(g, 20., tk(idx::TEXT_MUTED)).into_any_element(),
        AnswerLead::None => return None,
    })
}

fn answer_label(a: &Answer) -> AnyElement {
    let base = || div().text_size(z(14.)).line_height(z(18.));
    match (&a.lead, &a.sub) {
        (AnswerLead::User(_), Some(sub)) => div()
            .flex()
            .gap(z(3.))
            .min_w_0()
            .child(base().font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_DEFAULT)).child(a.label.clone()))
            .child(base().font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_SUBTLE)).child(sub.clone()))
            .into_any_element(),
        (AnswerLead::Channel(_) | AnswerLead::Dm(_), _) => base().font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_STRONG)).child(a.label.clone()).into_any_element(),
        _ => base().font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_STRONG)).child(a.label.clone()).into_any_element(),
    }
}

fn answers_body(app: &PhoenixApp, key: FilterKey, list: &[Answer], selected: usize, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let title = sq::group_title(key);
    let header = if title.is_empty() { div().h(z(8.)) } else { group_header(title) };
    let mut group = div().flex().flex_col().gap(z(4.)).child(header);
    for (i, a) in list.iter().enumerate() {
        let value = a.value.clone();
        group = group.child(
            option_row(("search-answer", i))
                .when(i == selected, |d| d.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
                .on_click(cx.listener(move |a, _, w, cx| pick_answer(a, &value, w, cx)))
                .children(answer_lead(app, &a.lead).map(|l| div().flex_none().mr(z(8.)).child(l)))
                .child(answer_label(a)),
        );
    }
    group
}

#[derive(Clone, Debug, PartialEq)]
enum Row {
    Answer(String),
    SearchFor,
    Filter(FilterKey),
    MoreFilters(bool),
    Cross(FilterKey, Answer),
    FindIn(String, SharedString),
    History(String),
}

enum Item {
    Row(Row),
    Group { title: SharedString, clear: bool, rows: Vec<Row> },
}

enum Layout {
    Answers(FilterKey, Vec<Answer>),
    Calendar(PendingFilter),
    Hidden,
    Items(Vec<Item>),
}

impl Layout {
    fn rows(&self) -> Vec<Row> {
        match self {
            Self::Answers(_, list) => list.iter().map(|a| Row::Answer(a.value.clone())).collect(),
            Self::Items(items) => items
                .iter()
                .flat_map(|i| match i {
                    Item::Row(r) => vec![r.clone()],
                    Item::Group { rows, .. } => rows.clone(),
                })
                .collect(),
            Self::Calendar(_) | Self::Hidden => Vec::new(),
        }
    }

    fn first_selected(&self) -> Option<usize> {
        matches!(self, Self::Answers(..)).then_some(0)
    }
}

fn filter_rows(skip_in: bool) -> Vec<Row> {
    FILTERS.iter().filter(|(k, ..)| !(skip_in && *k == FilterKey::In)).map(|(k, ..)| Row::Filter(*k)).chain([Row::MoreFilters(true)]).collect()
}

fn popout_layout(app: &PhoenixApp, raw: &str, cx: &App) -> Layout {
    let keys = sq::keys();
    if let Some(p) = pending_filter(raw, &keys) {
        if p.key.is_date() {
            return Layout::Calendar(p);
        }
        let list = sq::answers(app, p.key, &p.partial);
        return if list.is_empty() { Layout::Hidden } else { Layout::Answers(p.key, list) };
    }
    let dm = in_dm(app);
    let all_dms = dm && cross_dm(app);
    let skip_in = dm && !all_dms;
    let filters = || Item::Group { title: crate::t!("search.filters"), clear: false, rows: filter_rows(skip_in) };
    let mut items = Vec::new();
    if raw.trim().is_empty() {
        if all_dms && let Some(cur) = app.selected_channel {
            let tag = dm_tag(app, cur);
            items.push(Item::Row(Row::FindIn(format!("{}{}", sq::token(FilterKey::In), quote(&tag)), tag)));
        }
        items.push(filters());
        let history = cx.try_global::<SearchHistory>().map(|h| h.0.clone()).unwrap_or_default();
        if !history.is_empty() {
            items.push(Item::Group { title: crate::t!("search.history"), clear: true, rows: history.into_iter().map(Row::History).collect() });
        }
        return Layout::Items(items);
    }
    items.push(Item::Row(Row::SearchFor));
    let cross = sq::free_word(raw, &keys).map(|(_, w)| sq::cross_answers(app, &w, skip_in)).unwrap_or_default();
    if cross.is_empty() {
        items.push(filters());
    } else {
        items.push(Item::Row(Row::MoreFilters(false)));
        for (k, list) in cross {
            items.push(Item::Group { title: sq::group_title(k), clear: false, rows: list.into_iter().map(|a| Row::Cross(k, a)).collect() });
        }
    }
    Layout::Items(items)
}

fn set_and_submit(app: &mut PhoenixApp, q: String, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some(inp) = cx.global::<Search>().0.as_ref().map(|s| s.input.clone()) else { return };
    inp.update(cx, |s, cx| s.set_value(q, window, cx));
    submit(app, cx);
}

fn activate(app: &mut PhoenixApp, row: Row, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    match row {
        Row::Answer(v) => pick_answer(app, &v, window, cx),
        Row::SearchFor => submit(app, cx),
        Row::Filter(k) => add_filter(k, window, cx),
        Row::MoreFilters(_) => crate::ui::search_filters::open(app, window, cx),
        Row::Cross(k, a) => pick_cross(k, &a.value, window, cx),
        Row::FindIn(q, _) | Row::History(q) => set_and_submit(app, q, window, cx),
    }
}

fn pick_cross(key: FilterKey, value: &str, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some(input) = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).map(|s| s.input.clone()) else { return };
    let text = input.read(cx).value().to_string();
    let Some((start, _)) = sq::free_word(&text, &sq::keys()) else { return };
    let next = sq::replace_word(&text, start, key, value);
    input.update(cx, |s, cx| {
        s.set_value(next, window, cx);
        s.focus(window, cx);
    });
    cx.notify();
}

fn pick_date(app: &mut PhoenixApp, d: NaiveDate, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some(input) = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).map(|s| s.input.clone()) else { return };
    let text = input.read(cx).value().to_string();
    let Some(p) = pending_filter(&text, &sq::keys()) else { return };
    let next = complete_filter(&text, &p, &d.format("%Y-%m-%d").to_string());
    if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
        s.cal = None;
    }
    input.update(cx, |s, cx| s.set_value(next, window, cx));
    submit(app, cx);
}

pub const MIN_SEARCH_DATE: (i32, u32, u32) = (2015, 5, 15);

fn date_popout(p: &PendingFilter, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let today = chrono::Local::now().date_naive();
    let first = |d: NaiveDate| d.with_day(1).unwrap_or(d);
    let start = p.start;
    let month = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).and_then(|s| s.cal).filter(|(at, _)| *at == start).map_or(first(today), |(_, m)| m);
    let (y, m, d) = MIN_SEARCH_DATE;
    let spec = crate::ui::search_filters::CalSpec { key: "popout".into(), month, selected: today, min: NaiveDate::from_ymd_opt(y, m, d), six: true, width: 356. };
    let on_month: crate::ui::search_filters::OnMonth = std::rc::Rc::new(move |to: NaiveDate, cx: &mut App| {
        if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
            s.cal = Some((start, to));
        }
    });
    let on_day: crate::ui::search_filters::OnDay = std::rc::Rc::new(|d: NaiveDate, a: &mut PhoenixApp, w: &mut Window, cx: &mut Context<PhoenixApp>| pick_date(a, d, w, cx));
    crate::ui::search_filters::calendar_box(spec, on_month, on_day, cx)
}

fn row_lead(glyph: &str) -> gpui::Svg {
    named(glyph, 20., tk(idx::TEXT_MUTED)).mr(z(8.))
}

fn two_line(label: AnyElement, sub: AnyElement) -> gpui::Div {
    div().flex().flex_col().min_w_0().child(label).child(sub)
}

fn text_14(weight: FontWeight, color: usize, s: impl Into<SharedString>) -> gpui::Div {
    div().text_size(z(14.)).line_height(z(18.)).font_weight(weight).text_color(tk(color)).child(s.into())
}

fn cross_row(app: &PhoenixApp, key: FilterKey, a: &Answer) -> Vec<AnyElement> {
    let glyph = match key {
        FilterKey::Mentions => "search/at",
        FilterKey::In => "hash",
        _ => "search/user",
    };
    let token: SharedString = sq::token(key).into();
    let body = match &a.lead {
        AnswerLead::User(id) => {
            let user = if *id == app.me.id { Some(&app.me) } else { app.users.get(id) };
            two_line(
            div()
                .flex()
                .items_center()
                .gap(z(4.))
                .child(avatar(user, 16.))
                .child(text_14(FontWeight::SEMIBOLD, idx::TEXT_DEFAULT, a.label.clone()))
                .into_any_element(),
            div()
                .flex()
                .items_center()
                .gap(z(4.))
                .child(text_14(FontWeight::SEMIBOLD, idx::TEXT_SUBTLE, token))
                .child(text_14(FontWeight::MEDIUM, idx::TEXT_MUTED, a.sub.clone().unwrap_or_default()))
                .into_any_element(),
            )
        }
        lead => {
            let icon = match lead {
                AnswerLead::Channel(kind) => {
                    gpui::svg().path(crate::ui::sidebar::channel_icon(*kind).path()).flex_none().size(z(14.)).mr(z(4.)).text_color(tk(idx::TEXT_DEFAULT)).into_any_element()
                }
                AnswerLead::Dm(ch) => div().flex_none().mr(z(4.)).child(avatar(dm_peer(app, *ch), 16.)).into_any_element(),
                _ => div().into_any_element(),
            };
            div()
                .flex()
                .items_center()
                .gap(z(4.))
                .min_w_0()
                .child(text_14(FontWeight::MEDIUM, idx::TEXT_STRONG, token))
                .child(div().flex().items_center().min_w_0().child(icon).child(text_14(FontWeight::SEMIBOLD, idx::TEXT_STRONG, a.label.clone()).truncate()))
        }
    };
    vec![row_lead(glyph).into_any_element(), body.into_any_element()]
}

fn row_el(app: &PhoenixApp, row: &Row, ix: usize, selected: bool, query: &str, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let base = option_row(("search-row", ix)).when(selected, |d| d.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)));
    let picked = row.clone();
    let base = base.on_click(cx.listener(move |a, _, w, cx| activate(a, picked.clone(), w, cx)));
    let children: Vec<AnyElement> = match row {
        Row::Answer(_) => Vec::new(),
        Row::SearchFor => vec![
            row_lead("search").into_any_element(),
            div()
                .flex()
                .gap(z(4.))
                .child(text_14(FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("search.search_for")))
                .child(text_14(FontWeight::SEMIBOLD, idx::TEXT_STRONG, query.to_owned()).mr(z(8.)))
                .into_any_element(),
        ],
        Row::Filter(k) => {
            let (glyph, label, value) = FILTERS.iter().find(|(f, ..)| f == k).map_or(("search", "", ""), |(_, g, l, v)| (*g, *l, *v));
            vec![
                row_lead(glyph).into_any_element(),
                two_line(
                    text_14(FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!(label)).into_any_element(),
                    div()
                        .flex()
                        .gap(z(4.))
                        .child(text_14(FontWeight::SEMIBOLD, idx::TEXT_SUBTLE, sq::token(*k)))
                        .child(text_14(FontWeight::MEDIUM, idx::TEXT_MUTED, crate::t!(value)))
                        .into_any_element(),
                )
                .into_any_element(),
            ]
        }
        Row::MoreFilters(true) => vec![
            row_lead("inbox/filter").into_any_element(),
            two_line(
                text_14(FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("search.f_more")).into_any_element(),
                text_14(FontWeight::MEDIUM, idx::TEXT_MUTED, crate::t!("search.v_more")).into_any_element(),
            )
            .into_any_element(),
        ],
        Row::MoreFilters(false) => {
            vec![row_lead("inbox/filter").into_any_element(), text_14(FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("search.f_add")).into_any_element()]
        }
        Row::Cross(k, a) => cross_row(app, *k, a),
        Row::FindIn(_, tag) => vec![
            row_lead("search").into_any_element(),
            text_14(FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("search.find_in", name = tag.clone())).into_any_element(),
        ],
        Row::History(q) => vec![row_lead("search").into_any_element(), history_pills(app, q).into_any_element()],
    };
    base.children(children).into_any_element()
}

fn divider() -> gpui::Div {
    div().my(z(8.)).mx(z(-8.)).h(z(1.)).bg(tk(idx::BORDER_SUBTLE))
}

fn options_popout(app: &PhoenixApp, input: Entity<InputState>, error: Option<SharedString>, loading: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let raw = input.read(cx).value().to_string();
    let shape = popout_layout(app, &raw, cx);
    let stored = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).filter(|s| s.ac.0 == raw).map(|s| s.ac.1);
    let selected = stored.unwrap_or_else(|| shape.first_selected());
    let mut body = div().flex().flex_col();
    match shape {
        Layout::Calendar(p) => {
            let cal = date_popout(&p, cx);
            return popout_frame(input, div().child(cal), false, cx);
        }
        Layout::Answers(key, list) => {
            let sel = selected.unwrap_or(0).min(list.len() - 1);
            body = body.child(answers_body(app, key, &list, sel, cx));
        }
        Layout::Hidden => {
            if error.is_none() && !loading {
                return div().absolute().top_0().left_0().right_0().bottom_0().child(live_bar(&input, true, cx)).into_any_element();
            }
        }
        Layout::Items(items) => {
            let query = raw.trim().to_owned();
            let mut n = 0;
            for item in items {
                match item {
                    Item::Row(r) => {
                        body = body.child(row_el(app, &r, n, selected == Some(n), &query, cx));
                        n += 1;
                    }
                    Item::Group { title, clear, rows } => {
                        if n > 0 {
                            body = body.child(divider());
                        }
                        let mut header = group_header(title);
                        if clear {
                            header = header.child(
                                div()
                                    .id("search-clear-history")
                                    .cursor_pointer()
                                    .child(named("chat/trash", 18., tk(idx::TEXT_SUBTLE)))
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.default_global::<SearchHistory>().0.clear();
                                        cx.notify();
                                    })),
                            );
                        }
                        let mut group = div().flex().flex_col().gap(z(4.)).child(header);
                        for r in rows {
                            group = group.child(row_el(app, &r, n, selected == Some(n), &query, cx));
                            n += 1;
                        }
                        body = body.child(group);
                    }
                }
            }
        }
    }
    if let Some(e) = error {
        body = body.child(div().px(z(8.)).pt(z(8.)).text_size(z(14.)).text_color(tk(idx::TEXT_FEEDBACK_CRITICAL)).child(e));
    }
    if loading {
        body = body.child(div().px(z(8.)).pt(z(8.)).text_size(z(14.)).text_color(tk(idx::TEXT_MUTED)).child(crate::t!("search.searching")));
    }
    popout_frame(input, body, true, cx)
}

fn popout_frame(input: Entity<InputState>, body: gpui::Div, pad: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let bar = live_bar(&input, true, cx);
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
        .child(bar)
        .child(
            div()
                .id("search-popout")
                .debug_selector(|| "search-options-popout".into())
                .on_mouse_down_out(cx.listener(|a, ev: &gpui::MouseDownEvent, w, cx| {
                    let bar = gpui::Bounds::new(
                        gpui::point(w.viewport_size().width - z(8. + 244.), z(layout::TITLE_BAR_H + 8.)),
                        gpui::size(z(244.), z(32.)),
                    );
                    if !bar.contains(&ev.position) && !crate::ui::search_filters::is_open(cx) {
                        set_idle(a, w, cx);
                    }
                }))
                .absolute()
                .top(z(layout::TITLE_BAR_H + layout::HEADER_H - 3.))
                .right(z(12.))
                .w(z(356.))
                .when(pad, |d| d.px(z(8.)).py(z(12.)))
                .when(!pad, |d| d.overflow_hidden())
                .rounded(z(8.))
                .border_1()
                .border_color(tk(idx::BORDER_SUBTLE))
                .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
                .shadow(crate::ui::inbox::popout_shadow())
                .occlude()
                .child(body),
        )
        .into_any_element()
}

#[cfg(test)]
mod history_tests {
    use super::*;


    #[test]
    fn pager_pages() {
        assert_eq!(page_items(1, 164), vec![Some(1), Some(2), Some(3), None, Some(164)]);
        assert_eq!(page_items(2, 164), vec![Some(1), Some(2), Some(3), None, Some(164)]);
        assert_eq!(page_items(10, 164), vec![Some(1), None, Some(9), Some(10), Some(11), None, Some(164)]);
        assert_eq!(page_items(164, 164), vec![Some(1), None, Some(162), Some(163), Some(164)]);
        assert_eq!(page_items(1, 2), vec![Some(1), Some(2)]);
    }

    #[test]
    fn splits_filters_and_text() {
        assert_eq!(
            history_segments("hello world from:kasiopy has:image"),
            vec![
                HistorySeg::Text("hello world".into()),
                HistorySeg::Filter { key: FilterKey::From, value: "kasiopy".into() },
                HistorySeg::Filter { key: FilterKey::Has, value: "image".into() },
            ]
        );
        assert_eq!(history_segments("from:"), vec![HistorySeg::Text("from:".into())]);
    }

    #[test]
    fn strip_in_drops_channel_filters_only() {
        assert_eq!(strip_in("salut in:staar from:bob"), "salut from:bob");
        assert_eq!(strip_in("in:\"HG Team\" merci"), "merci");
        assert_eq!(strip_in("in: staar"), "");
        assert_eq!(strip_in("main:x"), "main:x");
        assert_eq!(quote("a b"), "\"a b\"");
    }
}
