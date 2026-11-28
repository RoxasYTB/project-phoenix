use gpui::{AnyElement, App, Context, FontWeight, HighlightStyle, IntoElement, InteractiveText, ParentElement, Pixels, SharedString, Size, Styled, StyledText, div, prelude::*, px};
use phoenix_api::rest::reporting::{ReportElement, ReportMenu, ReportNode, ReportStep, ReportTarget};

use super::{Modal, close_action};
use crate::{
    actions::{ActionReply, ActionRequest, close_modal, with_modal},
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button},
};

const SUCCESS_ART: &str = "https://cdn.discordapp.com/assets/content/9d1441338ce9090a7c1e9fab8fe34e58daa2ab7df4ac84fa280c308869d8f27b.svg";

const MORE_ELEMENTS: [&str; 7] = ["ignore_users", "block_users", "mute_users", "delete_message", "leave_guild", "deauthorize_app", "share_with_parents"];

thread_local! {
    static CONTENT_SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
}

pub struct Report {
    pub target: ReportTarget,
    pub menu: Option<Box<ReportMenu>>,
    pub current: u64,
    pub history: Vec<ReportStep>,
    pub picks: Vec<(String, String)>,
    pub error: Option<SharedString>,
    pub load_error: Option<SharedString>,
    pub sending: bool,
    pub auto_sent: Option<u64>,
    pub viewport: Option<Size<Pixels>>,
    pub done: Vec<&'static str>,
}

impl Report {
    pub fn new(target: ReportTarget) -> Self {
        Self { target, menu: None, current: 0, history: Vec::new(), picks: Vec::new(), error: None, load_error: None, sending: false, auto_sent: None, viewport: None, done: Vec::new() }
    }

    pub fn current(&self) -> Option<&ReportNode> {
        self.menu.as_ref()?.node(self.current)
    }

    pub fn set_menu(&mut self, menu: Box<ReportMenu>) {
        self.current = menu.root_node_id;
        self.menu = Some(menu);
        self.history.clear();
        self.picks = self.preselected();
    }

    fn preselected(&self) -> Vec<(String, String)> {
        let Some(cb) = self.current().and_then(|n| n.element("checkbox")) else { return Vec::new() };
        cb.checkbox_options().into_iter().filter(|o| o.preselected).map(|o| (o.key, o.label)).collect()
    }

    pub fn go(&mut self, label: String, target: u64) -> Option<ActionRequest> {
        let menu = self.menu.as_ref()?;
        let target = menu.resolve(target);
        menu.node(target)?;
        let picks = std::mem::take(&mut self.picks);
        self.history.push(ReportStep { node: self.current, label, target, picks });
        self.current = target;
        self.picks = self.preselected();
        self.error = None;
        self.auto_submit()
    }

    pub fn back(&mut self) {
        if let Some(s) = self.history.pop() {
            self.current = s.node;
            self.picks = s.picks;
            self.error = None;
        }
    }

    pub fn toggle(&mut self, key: &str, label: &str) {
        if let Some(i) = self.picks.iter().position(|(k, _)| k == key) {
            self.picks.remove(i);
        } else {
            self.picks.push((key.to_owned(), label.to_owned()));
        }
    }

    fn steps(&self, target: u64) -> Vec<ReportStep> {
        let mut s = self.history.clone();
        s.push(ReportStep { node: self.current, label: String::new(), target, picks: self.picks.clone() });
        s
    }

    pub fn submit(&mut self) -> Option<ActionRequest> {
        if self.sending {
            return None;
        }
        let menu = self.menu.clone()?;
        let steps = self.steps(menu.success_node_id);
        self.sending = true;
        self.error = None;
        Some(ActionRequest::SubmitReport { menu, target: self.target, steps })
    }

    fn auto_submit(&mut self) -> Option<ActionRequest> {
        let node = self.current()?;
        if !node.is_auto_submit || self.auto_sent == Some(node.id) {
            return None;
        }
        let id = node.id;
        self.auto_sent = Some(id);
        let menu = self.menu.clone()?;
        Some(ActionRequest::SubmitReport { menu, target: self.target, steps: self.steps(id) })
    }

    pub fn submitted(&mut self, ok: Result<(), String>) {
        self.sending = false;
        match ok {
            Ok(()) => {
                if let Some(success) = self.menu.as_ref().map(|m| m.success_node_id)
                    && self.menu.as_ref().and_then(|m| m.node(success)).is_some()
                {
                    let picks = std::mem::take(&mut self.picks);
                    self.history.push(ReportStep { node: self.current, label: String::new(), target: success, picks });
                    self.current = success;
                }
            }
            Err(e) => self.error = Some(if e.contains("50035") { crate::t!("report.error_form") } else { crate::t!("report.error") }),
        }
    }

    pub fn breadcrumbs(&self) -> Vec<String> {
        let mut out = Vec::new();
        for s in &self.history {
            if !s.picks.is_empty() {
                out.push(s.picks.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>().join(", "));
            }
            if !s.label.is_empty() {
                out.push(s.label.clone());
            }
        }
        out
    }
}

impl PhoenixApp {
    pub fn open_report(&mut self, target: ReportTarget, cx: &mut gpui::Context<Self>) {
        crate::actions::open_modal(Modal::Report(Report::new(target)), None);
        self.action(ActionRequest::ReportMenu(target), cx, |_, r, _| {
            with_modal(|m| {
                if let Some(Modal::Report(rep)) = m {
                    match r {
                        Ok(ActionReply::ReportMenu(menu)) => rep.set_menu(menu),
                        Err(e) => rep.load_error = Some(e.0.into()),
                        _ => {}
                    }
                }
            });
        });
        cx.notify();
    }

    pub fn report_go(&mut self, label: String, node: u64, cx: &mut gpui::Context<Self>) {
        let req = with_modal(|m| match m {
            Some(Modal::Report(r)) => r.go(label, node),
            _ => None,
        });
        if let Some(req) = req {
            self.report_send(req, false, cx);
        }
        cx.notify();
    }

    pub fn report_submit(&mut self, cx: &mut gpui::Context<Self>) {
        let req = with_modal(|m| match m {
            Some(Modal::Report(r)) => r.submit(),
            _ => None,
        });
        if let Some(req) = req {
            self.report_send(req, true, cx);
        }
        cx.notify();
    }

    fn report_send(&mut self, req: ActionRequest, navigate: bool, cx: &mut gpui::Context<Self>) {
        self.action(req, cx, move |_, res, _| {
            with_modal(|m| {
                if let Some(Modal::Report(r)) = m {
                    let res = res.map(|_| ()).map_err(|e| e.0);
                    if navigate {
                        r.submitted(res);
                    } else if res.is_err() {
                        r.error = Some(crate::t!("report.error"));
                    }
                }
            });
        });
    }
}

fn text_width(cx: &App, size: Pixels, s: &str) -> f32 {
    let mut font = crate::richtext::ui_font();
    font.weight = FontWeight::MEDIUM;
    let ts = cx.text_system();
    let id = ts.resolve_font(&font);
    s.chars().map(|ch| ts.advance(id, size, ch).map(|a| f32::from(a.width)).unwrap_or(0.)).sum()
}

fn section_title(t: SharedString) -> gpui::Div {
    div().mb(z(8.)).text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(t)
}

fn message_preview(app: &PhoenixApp, channel: Id, message: Id) -> Option<gpui::Div> {
    let m = app.messages.get(&channel)?.msgs.iter().find(|m| m.id == message)?;
    let author = app.member_user(app.selected_guild, m.author);
    let u = author.as_deref();
    let name: SharedString = m
        .author_nick
        .clone()
        .or_else(|| app.member_nick(m.author))
        .or_else(|| u.map(|u| u.display_name.clone()))
        .unwrap_or_else(|| crate::t!("common.unknown"));
    let when = chrono::DateTime::from_timestamp(m.timestamp, 0).map(|t| crate::i18n::clock(t.with_timezone(&chrono::Local))).unwrap_or_default();
    let content: SharedString = if m.content.is_empty() && !m.attachments.is_empty() { crate::t!("msg.attachments", n = m.attachments.len()) } else { m.content.clone() };
    let header = div()
        .flex()
        .items_center()
        .gap(z(4.))
        .min_w_0()
        .child({
            let (role_color, role_gradient) = app.author_role_style(m.author);
            crate::fonts::styled_name(div(), u, false)
                .flex_none()
                .text_size(z(16.))
                .line_height(z(22.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(role_color.map(c).unwrap_or_else(|| c(tok(idx::TEXT_STRONG))))
                .child(crate::fonts::role_name_text(name, u, 16., app.selected_guild.is_some(), role_gradient, false))
        })
        .children(u.and_then(|u| u.tag.as_ref()).map(crate::ui::members::server_tag))
        .child(div().ml(z(4.)).text_size(z(12.)).line_height(z(22.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_MUTED))).child(when));
    Some(
        div().flex().flex_col().child(section_title(crate::t!("report.selected_message"))).child(
            div()
                .id("report-msg-preview")
                .max_h(z(100.))
                .overflow_y_scroll()
                .py(z(16.))
                .pr(z(16.))
                .rounded(z(8.))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .child(
                    div()
                        .flex()
                        .gap(z(20.))
                        .pl(z(20.))
                        .pr(z(20.))
                        .py(z(2.))
                        .child(div().flex_none().mt(z(2.)).child(crate::ui::avatar(u, 40.)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(header)
                                .child(
                                    div()
                                        .text_size(z(16.))
                                        .line_height(z(22.))
                                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                                        .child(crate::ui::emoji_text::inline_markdown(app, &content, c(tok(idx::TEXT_DEFAULT)), FontWeight::NORMAL, 16., true)),
                                ),
                        ),
                ),
        ),
    )
}

fn user_preview(app: &PhoenixApp, user: Id) -> gpui::Div {
    let u = app.users.get(&user);
    div().flex().flex_col().child(section_title(crate::t!("report.selected_user"))).child(
        div()
            .p(z(16.))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .flex()
            .items_center()
            .gap(z(12.))
            .child(crate::ui::avatar(u, 40.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .children(u.map(|u| div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(u.display_name.clone())))
                    .children(u.map(|u| div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(u.name.clone()))),
            ),
    )
}

fn guild_preview(app: &PhoenixApp, guild: Id) -> gpui::Div {
    let name = app.guilds.iter().find(|g| g.id == guild).map(|g| g.name.clone()).unwrap_or_default();
    div().flex().flex_col().child(section_title(crate::t!("report.selected_server"))).child(
        div()
            .p(z(16.))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .text_size(z(16.))
            .line_height(z(20.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(c(tok(idx::TEXT_STRONG)))
            .child(name),
    )
}

fn list_row(id: impl Into<gpui::ElementId>, first: bool, last: bool, min_h: f32, label: SharedString, icon: &'static str, icon_size: f32) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .w_full()
        .min_h(z(min_h))
        .flex()
        .items_center()
        .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
        .active(|s| s.bg(c(tok(idx::BACKGROUND_MOD_STRONG))))
        .when(!last, |d| d.border_b_1().border_color(c(tok(idx::BORDER_MUTED))))
        .when(first, |d| d.rounded_t(z(12.)))
        .when(last, |d| d.rounded_b(z(12.)))
        .cursor_pointer()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .my(z(8.))
                .ml(z(16.))
                .py(z(12.))
                .text_size(z(16.))
                .line_height(z(20.))
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .child(label),
        )
        .child(
            div()
                .flex_none()
                .size(z(24.))
                .ml(z(8.))
                .mr(z(16.))
                .flex()
                .items_center()
                .justify_center()
                .child(gpui::svg().path(crate::icons::path(icon)).size(z(icon_size)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
        )
}

fn checkbox_row(id: usize, label: SharedString, description: Option<String>, on: bool) -> gpui::Stateful<gpui::Div> {
    let (bg, border, hbg, hborder) = if on {
        (idx::CHECKBOX_BACKGROUND_SELECTED_DEFAULT, idx::CHECKBOX_BORDER_SELECTED_DEFAULT, idx::CHECKBOX_BACKGROUND_SELECTED_HOVER, idx::CHECKBOX_BORDER_SELECTED_HOVER)
    } else {
        (idx::CHECKBOX_BACKGROUND_DEFAULT, idx::CHECKBOX_BORDER_DEFAULT, idx::CHECKBOX_BACKGROUND_HOVER, idx::CHECKBOX_BORDER_HOVER)
    };
    div()
        .id(("report-check", id))
        .group("report-check")
        .w_full()
        .flex()
        .items_start()
        .gap(z(12.))
        .rounded(z(8.))
        .cursor_pointer()
        .child(
            div()
                .flex_none()
                .size(z(20.))
                .rounded(z(4.))
                .border_1()
                .border_color(c(tok(border)))
                .bg(c(tok(bg)))
                .group_hover("report-check", |s| s.bg(c(tok(hbg))).border_color(c(tok(hborder))))
                .flex()
                .items_center()
                .justify_center()
                .when(on, |d| d.child(gpui::svg().path(crate::icons::path("check-small-bold")).size(z(18.)).text_color(c(tok(idx::CHECKBOX_ICON_ACTIVE))))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_STRONG))).child(label))
                .children(description.map(|d| div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_MUTED))).child(d))),
        )
}

fn notice(text: SharedString, critical: bool) -> gpui::Div {
    let (border, bg, icon, glyph) = if critical {
        (idx::INLINENOTICE_BORDER_CRITICAL, idx::BACKGROUND_FEEDBACK_MOD_CRITICAL, idx::ICON_FEEDBACK_CRITICAL, "warning")
    } else {
        (idx::INLINENOTICE_BORDER_INFO, idx::BACKGROUND_FEEDBACK_MOD_INFO, idx::ICON_FEEDBACK_INFO, "info")
    };
    div()
        .w_full()
        .flex()
        .items_start()
        .gap(z(8.))
        .p(z(12.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(border)))
        .bg(c(tok(bg)))
        .child(gpui::svg().path(crate::icons::path(glyph)).flex_none().size(z(16.)).mt(z(1.)).text_color(c(tok(icon))))
        .child(div().flex_1().min_w_0().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_STRONG))).child(text))
}

fn legal() -> AnyElement {
    let (a, l, b) = (crate::t!("report.legal_before"), crate::t!("report.legal_link"), crate::t!("report.legal_after"));
    let start = a.len();
    let end = start + l.len();
    let text: SharedString = format!("{a}{l}{b}").into();
    let styled = StyledText::new(text).with_highlights([(start..end, HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() })]);
    div()
        .w_full()
        .text_size(z(14.))
        .line_height(z(18.))
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .child(InteractiveText::new("report-legal", styled).on_click(std::iter::once(start..end).collect(), |_, _, cx| cx.open_url("https://discord.com/guidelines")))
        .into_any_element()
}

fn links(node: &ReportNode, first: bool) -> Vec<AnyElement> {
    let ls: Vec<&ReportElement> = node.links().collect();
    if !ls.iter().any(|e| e.link_localized()) {
        return Vec::new();
    }
    let shown: Vec<_> = ls.iter().filter(|e| e.link_localized()).filter_map(|e| e.link()).collect();
    let mut out = Vec::new();
    if !ls.iter().any(|e| e.link_header_hidden()) {
        out.push(section_title(crate::t!("report.learn_more")).into_any_element());
    }
    let n = shown.len();
    for (i, l) in shown.into_iter().enumerate() {
        let url = l.url.clone();
        let label: SharedString = match &l.description {
            Some(d) => format!("{}\n{d}", l.text).into(),
            None => l.text.clone().into(),
        };
        out.push(list_row(("report-link", i), first && i == 0, i + 1 == n, 64., label, "link-external", 18.).on_click(move |_, _, cx| cx.open_url(&url)).into_any_element());
    }
    out
}

fn reported_user(app: &PhoenixApp, target: &ReportTarget) -> Option<(Id, Option<Id>)> {
    match *target {
        ReportTarget::Message { channel, message } => {
            let ch = Id(channel.0);
            let m = app.messages.get(&ch)?.msgs.iter().find(|m| m.id == Id(message.0))?;
            Some((m.author, Some(ch)))
        }
        ReportTarget::User { user, .. } => Some((Id(user.0), None)),
        ReportTarget::Guild { .. } => None,
    }
}

fn user_name(app: &PhoenixApp, user: Id, channel: Option<Id>) -> String {
    let guild = channel.and_then(|c| app.channels.get(&c)).and_then(|c| c.guild).or(app.selected_guild);
    app.member_nick_in(guild, user)
        .or_else(|| app.users.get(&user).map(|u| if u.display_name.is_empty() { u.name.clone() } else { u.display_name.clone() }))
        .map(|s| s.to_string())
        .unwrap_or_default()
}

fn dm_with(app: &PhoenixApp, user: Id) -> Option<Id> {
    app.dms.iter().copied().find(|d| app.channels.get(d).is_some_and(|c| c.guild.is_none() && c.voice_users.len() == 1 && c.voice_users[0] == user))
}

fn mark_done(key: &'static str) {
    with_modal(|m| {
        if let Some(Modal::Report(r)) = m
            && !r.done.contains(&key)
        {
            r.done.push(key);
        }
    });
}

struct MoreRow {
    key: &'static str,
    title: String,
    description: SharedString,
    label: SharedString,
    kind: BtnKind,
    disabled: bool,
}

fn action_row(row: MoreRow, last: bool, cx: &mut Context<PhoenixApp>, on: impl Fn(&mut PhoenixApp, &mut Context<PhoenixApp>) + 'static) -> gpui::Div {
    let key = row.key;
    let b = crate::ui::controls::button_sm(gpui::ElementId::Name(format!("report-more-{key}").into()), row.label, row.kind).flex_none();
    let b = if row.disabled {
        b.opacity(0.5).cursor_default()
    } else {
        b.on_click(cx.listener(move |a, _, _, cx| {
            mark_done(key);
            on(a, cx);
            cx.notify();
        }))
    };
    div()
        .mx(z(16.))
        .py(z(16.))
        .flex()
        .items_center()
        .justify_between()
        .gap(z(16.))
        .when(!last, |d| d.border_b_1().border_color(c(tok(idx::BORDER_MUTED))))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(z(2.))
                .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(row.title))
                .child(div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_SUBTLE))).child(row.description)),
        )
        .child(b)
}

fn more_you_can_do(app: &PhoenixApp, r: &Report, n: &ReportNode, cx: &mut Context<PhoenixApp>) -> Option<gpui::Div> {
    if !MORE_ELEMENTS.iter().any(|k| n.has(k)) {
        return None;
    }
    let (user, channel) = reported_user(app, &r.target)?;
    let msg_target = matches!(r.target, ReportTarget::Message { .. });
    let name = user_name(app, user, channel);
    let blocked = app.nav.is_blocked(user);
    let ignored = app.nav.is_ignored(user) || crate::actions::user::is_ignored(user);
    let done = |k: &'static str| r.done.contains(&k);
    let ignore = n.has("ignore_users");
    let mut rows: Vec<MoreRow> = Vec::new();
    if ignore {
        let off = ignored || blocked || done("ignore");
        rows.push(MoreRow { key: "ignore", title: crate::t!("report.more_ignore", username = name).to_string(), description: crate::t!("report.more_ignore_desc"), label: crate::t!(if off { "report.ignored" } else { "report.ignore" }), kind: BtnKind::Secondary, disabled: off });
    }
    if n.has("block_users") {
        let off = blocked || done("block");
        rows.push(MoreRow { key: "block", title: crate::t!("report.more_block", username = name).to_string(), description: crate::t!("report.more_block_desc"), label: crate::t!(if off { "report.blocked" } else { "report.block" }), kind: BtnKind::Critical, disabled: off });
    }
    let dm = dm_with(app, user);
    if !ignore && n.has("mute_users") {
        let off = done("mute") || dm.and_then(|d| app.channels.get(&d)).is_some_and(|c| c.muted);
        rows.push(MoreRow { key: "mute", title: crate::t!("report.more_mute", username = name).to_string(), description: crate::t!("report.more_mute_desc"), label: crate::t!(if off { "report.muted" } else { "report.mute" }), kind: BtnKind::Secondary, disabled: off });
    }
    if msg_target && n.has("delete_message") {
        let guild = channel.and_then(|c| app.channels.get(&c)).and_then(|c| c.guild);
        let can = guild.is_some_and(|g| crate::ui::inbox::has_perm(app.backend.guild_permissions(g), 1 << 13));
        if can {
            let off = done("delete");
            rows.push(MoreRow { key: "delete", title: crate::t!("report.more_delete").to_string(), description: crate::t!("report.more_delete_desc"), label: crate::t!(if off { "report.deleted" } else { "report.delete" }), kind: BtnKind::Critical, disabled: off });
        }
    }
    if rows.is_empty() {
        return None;
    }
    let target = r.target;
    let k = rows.len();
    let mut list = div().rounded(z(12.)).border_1().border_color(c(tok(idx::BORDER_SUBTLE))).bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)));
    for (i, row) in rows.into_iter().enumerate() {
        let key = row.key;
        list = list.child(action_row(row, i + 1 == k, cx, move |a, cx| match key {
            "ignore" => a.toggle_ignore(user, cx),
            "block" => a.relationship_action(user, crate::ui::home::data::RelAction::Block, cx),
            "mute" => {
                if let Some(d) = dm {
                    a.set_nav_mute(None, Some(d), true, None, cx);
                }
            }
            _ => {
                if let ReportTarget::Message { message, .. } = target {
                    a.delete_message(Id(message.0), cx);
                }
            }
        }));
    }
    Some(
        div()
            .mb(z(8.))
            .flex()
            .flex_col()
            .child(div().mb(z(8.)).text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_DEFAULT))).child(crate::t!("report.more")))
            .child(list),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upsell {
    DmSpam,
    SexualMedia,
    GraphicMedia,
}

pub const SEXUAL_FILTERS: [&str; 3] = ["cf-servers", "cf-friends", "cf-others"];
pub const GRAPHIC_FILTERS: [&str; 3] = ["cfv-servers", "cfv-friends", "cfv-others"];
const SHOW: usize = 0;
const BLUR: usize = 1;
const SPAM_DISABLED: usize = 2;
const SPAM_NON_FRIENDS: usize = 1;

impl Upsell {
    pub fn subtypes(self) -> &'static [&'static str] {
        match self {
            Self::DmSpam => &["sub_spam"],
            Self::SexualMedia => &["sub_csam", "sub_loli", "sub_ncp", "sub_sexually_degrading_content", "sub_unsolicited_porn"],
            Self::GraphicMedia => &["sub_gore", "sub_glorifying_violence"],
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::DmSpam => "upsell-spam",
            Self::SexualMedia => "upsell-sexual",
            Self::GraphicMedia => "upsell-graphic",
        }
    }

    fn texts(self) -> (SharedString, SharedString) {
        match self {
            Self::DmSpam => (crate::t!("report.upsell_spam"), crate::t!("report.upsell_spam_desc")),
            Self::SexualMedia => (crate::t!("report.upsell_sexual"), crate::t!("report.upsell_sexual_desc")),
            Self::GraphicMedia => (crate::t!("report.upsell_graphic"), crate::t!("report.upsell_graphic_desc")),
        }
    }

    pub fn patches(self, proto: &std::collections::HashMap<&'static str, u64>) -> Vec<(&'static str, usize)> {
        use crate::ui::settings::synced_map::get_pick;
        match self {
            Self::DmSpam => vec![("spam", SPAM_NON_FRIENDS)],
            Self::SexualMedia => SEXUAL_FILTERS.iter().filter(|f| get_pick(proto, f) == SHOW).map(|f| (*f, BLUR)).collect(),
            Self::GraphicMedia => GRAPHIC_FILTERS.iter().filter(|f| get_pick(proto, f) == SHOW).map(|f| (*f, BLUR)).collect(),
        }
    }
}

pub fn settings_upsells(subtype: Option<&str>, dm: bool, proto: &std::collections::HashMap<&'static str, u64>) -> Vec<Upsell> {
    use crate::ui::settings::synced_map::get_pick;
    let Some(sub) = subtype else { return Vec::new() };
    [Upsell::DmSpam, Upsell::SexualMedia, Upsell::GraphicMedia]
        .into_iter()
        .filter(|u| u.subtypes().contains(&sub))
        .filter(|u| match u {
            Upsell::DmSpam => dm && get_pick(proto, "spam") == SPAM_DISABLED,
            _ => !u.patches(proto).is_empty(),
        })
        .collect()
}

fn report_subtype(r: &Report) -> Option<String> {
    let menu = r.menu.as_ref()?;
    let step = r.history.last()?;
    menu.node(step.node)?.report_type.clone()
}

fn recommended_settings(app: &PhoenixApp, r: &Report, n: &ReportNode, cx: &mut Context<PhoenixApp>) -> Option<gpui::Div> {
    let ReportTarget::Message { channel, .. } = r.target else { return None };
    if !n.has("settings_upsells") {
        return None;
    }
    let dm = app.channels.get(&Id(channel.0)).is_none_or(|c| c.guild.is_none());
    let proto = &app.prefs.synced.proto;
    let upsells = settings_upsells(report_subtype(r).as_deref(), dm, proto);
    if upsells.is_empty() {
        return None;
    }
    let k = upsells.len();
    let mut list = div().mb(z(8.)).rounded(z(8.)).bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)));
    for (i, u) in upsells.into_iter().enumerate() {
        let (title, description) = u.texts();
        let done = r.done.contains(&u.key());
        let patches = u.patches(proto);
        let row = MoreRow { key: u.key(), title: title.to_string(), description, label: crate::t!(if done { "report.upsell_applied" } else { "report.upsell_apply" }), kind: BtnKind::Secondary, disabled: done };
        list = list.child(action_row(row, i + 1 == k, cx, move |a, cx| {
            for &(id, pick) in &patches {
                crate::ui::settings::save(a, crate::ui::settings::SettingsPatch::Proto(id, crate::ui::settings::synced_map::pick_wire(id, pick)), cx);
            }
        }));
    }
    let link = div()
        .id("report-upsell-settings")
        .cursor_pointer()
        .text_size(z(16.))
        .line_height(z(20.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(idx::TEXT_BRAND)))
        .hover(|s| s.underline())
        .child(crate::t!("report.upsell_settings"))
        .on_click(cx.listener(|a, _, _, cx| {
            close_modal();
            crate::ui::settings::open(a, cx);
            if let Some(s) = a.settings.as_mut() {
                s.page = crate::ui::settings::Page::PrivacySafety;
            }
            cx.notify();
        }));
    Some(
        div()
            .mb(z(16.))
            .flex()
            .flex_col()
            .child(div().mb(z(8.)).text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_DEFAULT))).child(crate::t!("report.upsell_title")))
            .child(list)
            .child(div().flex().child(link)),
    )
}

pub fn render(app: &PhoenixApp, r: &Report, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let vp = r.viewport.unwrap_or(Size { width: px(1536.), height: px(840.) });
    let width = z(440.).min(vp.width);
    let max_h = z(720.).min(vp.height * 0.7);
    let node = r.current();
    let close = close_action(cx);
    let header = node.map(|n| n.header.clone()).filter(|h| !h.is_empty());
    let success = node.is_some_and(|n| n.has("success"));
    let mut top = div().flex_none().flex().flex_col().px(z(24.));
    let mut body = div().flex().flex_col().gap(z(16.));
    let mut footer_btns: Vec<(SharedString, BtnKind, u8)> = Vec::new();
    match (node, &r.load_error) {
        (_, Some(e)) => body = body.child(notice(e.clone(), true)),
        (None, None) => body = body.child(div().text_color(c(tok(idx::TEXT_SUBTLE))).child(crate::t!("common.loading"))),
        (Some(n), None) => {
            let mut sub = div().flex().flex_col();
            if let Some(s) = n.subheader.clone().filter(|s| !s.is_empty() && !n.header.is_empty()) {
                sub = sub.child(div().mt(z(8.)).text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(s));
            }
            if n.button_kind() == Some("submit") {
                sub = sub.child(legal());
            }
            if let Some(info) = n.info.clone().filter(|s| !s.is_empty()) {
                sub = sub.child(div().mt(z(8.)).child(notice(info.into(), false)));
            }
            if success {
                body = body.child(
                    div()
                        .mb(z(8.))
                        .flex()
                        .flex_col()
                        .items_start()
                        .child(div().w_full().pt(z(24.)).mb(z(16.)).flex().justify_center().child(gpui::img(SUCCESS_ART).w(z(288.)).h(z(162.)).flex_none()))
                        .child(sub),
                );
            } else {
                top = top.child(sub);
            }
            match r.target {
                ReportTarget::Message { channel, message } if n.has("message_preview") => {
                    body = body.children(message_preview(app, Id(channel.0), Id(message.0)).map(|d| d.mb(z(8.))));
                }
                ReportTarget::User { user, .. } if n.has("user_preview") => body = body.child(user_preview(app, Id(user.0)).mb(z(8.))),
                ReportTarget::Guild { guild } if n.has("guild_preview") => body = body.child(guild_preview(app, Id(guild.0)).mb(z(8.))),
                _ => {}
            }
            if n.has("breadcrumbs") {
                let crumbs = r.breadcrumbs();
                if !crumbs.is_empty() {
                    let k = crumbs.len();
                    let mut list = div().flex().flex_col().mb(z(8.)).child(
                        div().mb(z(8.)).text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_DEFAULT))).child(crate::t!("report.category")),
                    );
                    for (i, t) in crumbs.into_iter().enumerate() {
                        list = list.child(
                            div()
                                .flex()
                                .when(i + 1 < k, |d| d.mb(z(8.)))
                                .child(div().flex_none().size(z(4.)).mt(z(8.)).mr(z(8.)).rounded(z(2.)).bg(c(tok(idx::BRAND_500))))
                                .child(div().flex_1().min_w_0().line_clamp(2).text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(t)),
                        );
                    }
                    body = body.child(list);
                }
            }
            if let Some(cb) = n.element("checkbox") {
                let opts = cb.checkbox_options();
                let k = opts.len();
                let mut col = div().flex().flex_col();
                for (i, o) in opts.into_iter().enumerate() {
                    let on = r.picks.iter().any(|(key, _)| *key == o.key);
                    let (key, label) = (o.key.clone(), o.label.clone());
                    col = col.child(
                        checkbox_row(i, o.label.into(), o.description, on)
                            .when(i + 1 < k, |d| d.mb(z(16.)))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                with_modal(|m| {
                                    if let Some(Modal::Report(r)) = m {
                                        r.toggle(&key, &label);
                                    }
                                });
                                cx.notify();
                            })),
                    );
                }
                body = body.child(col);
            }
            body = body.children(more_you_can_do(app, r, n, cx));
            body = body.children(recommended_settings(app, r, n, cx));
            let has_links = n.links().any(|e| e.link_localized());
            if !n.children.is_empty() || has_links {
                let k = n.children.len();
                let mut list = div().flex().flex_col();
                for (i, (label, target)) in n.children.iter().enumerate() {
                    let (l, t) = (label.clone(), *target);
                    list = list.child(
                        list_row(("report-opt", i), i == 0, i + 1 == k && !has_links, 60., label.clone().into(), "chevron-right", 24.)
                            .on_click(cx.listener(move |a, _, _, cx| a.report_go(l.clone(), t, cx))),
                    );
                }
                list = list.children(links(n, k == 0));
                body = body.child(list);
            }
            let kind = n.button_kind();
            if !r.history.is_empty() && kind != Some("done") {
                footer_btns.push((crate::t!("common.back"), BtnKind::Secondary, 0));
            }
            match kind {
                Some("submit") => footer_btns.push((crate::t!("report.submit"), BtnKind::Critical, 1)),
                Some("next") => footer_btns.push((crate::t!("common.next"), BtnKind::Primary, 2)),
                Some("done") => footer_btns.push((crate::t!("common.done"), BtnKind::Primary, 3)),
                _ => {}
            }
        }
    }
    if let Some(e) = &r.error {
        body = body.child(notice(e.clone(), true));
    }
    let blocked = node.is_some_and(|n| n.next_blocked(&r.picks));
    let has_footer = !footer_btns.is_empty();
    let footer = has_footer.then(|| {
        let inner = f32::from(width) - f32::from(z(48.));
        let half = (inner - f32::from(z(8.))) / 2.;
        let vertical = footer_btns.iter().any(|(l, _, _)| text_width(cx, z(16.), l) + f32::from(z(32.)) > half);
        let mut row = div().w_full().flex().gap(z(8.)).when(vertical, |d| d.flex_col());
        for (label, kind, act) in footer_btns {
            let disabled = (act == 2 && blocked) || (act == 1 && r.sending) || (act == 0 && r.sending);
            let next_target = node.and_then(|n| n.button.as_ref()).and_then(|b| b.target);
            let b = button(("report-btn", act as usize), label, kind)
                .min_w_0()
                .when(!vertical, |d| d.flex_1())
                .when(vertical, |d| d.w_full())
                .when(disabled, |d| d.opacity(0.5).cursor_default());
            row = row.child(if disabled {
                b
            } else {
                b.on_click(cx.listener(move |a, _, _, cx| match act {
                    0 => {
                        with_modal(|m| {
                            if let Some(Modal::Report(r)) = m {
                                r.back();
                            }
                        });
                        cx.notify();
                    }
                    1 => a.report_submit(cx),
                    2 => {
                        if let Some(t) = next_target {
                            a.report_go(String::new(), t, cx)
                        }
                    }
                    _ => {
                        close_modal();
                        cx.notify();
                    }
                }))
            });
        }
        div().flex_none().p(z(24.)).bg(c(tok(idx::MODAL_FOOTER_BACKGROUND))).rounded_b(z(12.)).child(row)
    });
    let scroll = CONTENT_SCROLL.with(|h| h.clone());
    let content = div()
        .flex_auto()
        .min_h_0()
        .relative()
        .flex()
        .flex_col()
        .child(
            div()
                .id("report-content")
                .track_scroll(&scroll)
                .flex_auto()
                .min_h_0()
                .overflow_y_scroll()
                .when(!success, |d| d.pt(z(24.)))
                .pl(z(24.))
                .pr(z(16.))
                .mr(z(8.))
                .when(!has_footer, |d| d.pb(z(24.)))
                .child(body),
        )
        .children(crate::ui::chat::scrollbar::thin_scrollbar_div(&scroll));
    let el = div()
        .relative()
        .w(width)
        .max_h(max_h)
        .min_h(z(220.).min(max_h))
        .rounded(z(12.))
        .bg(c(tok(idx::MODAL_BACKGROUND)))
        .border_1()
        .border_color(c(tok(idx::BORDER_NORMAL)))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .flex_none()
                .pt(z(24.))
                .px(z(24.))
                .children(header.map(|h| div().w_full().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(h))),
        )
        .child(top)
        .child(content)
        .children(footer)
        .child(
            div()
                .id("report-close")
                .absolute()
                .top(z(8.))
                .right(z(8.))
                .size(z(40.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
                .on_click({
                    let close = close.clone();
                    move |_, w, cx| close(w, cx)
                })
                .child(gpui::svg().path(crate::icons::path("close")).size(z(20.)).text_color(c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT)))),
        );
    crate::ui::modal::frame("report-backdrop", close, el)
}

#[cfg(test)]
mod tests {
    use super::*;
    use phoenix_api::models::Snowflake;

    fn menu() -> Box<ReportMenu> {
        Box::new(
            serde_json::from_str(
                r#"{"name":"message","variant":"8","version":"1.0","root_node_id":7,"success_node_id":72,"fail_node_id":75,"nodes":{
                "7":{"id":7,"key":"W","header":"Report","button":{"type":"cancel","target":null},"elements":[],"children":[["Dislike",99],["Private",81],["Else",82]]},
                "81":{"id":81,"key":"P","header":"What?","button":{"type":"next","target":17},"elements":[{"name":"pii_select","type":"checkbox","data":[["ip","IP"],["mail","Mail"]],"should_submit_data":true}],"children":[]},
                "82":{"id":82,"key":"E","header":"Else","button":null,"elements":[],"children":[["Self harm",162]]},
                "162":{"id":162,"key":"I","header":"Help","button":{"type":"next","target":95},"elements":[{"name":"skip","type":"skip"}],"children":[]},
                "95":{"id":95,"key":"S","header":"More","button":null,"elements":[],"children":[]},
                "17":{"id":17,"key":"GENERIC_SUBMIT","header":"Summary","button":{"type":"submit","target":null},"elements":[{"name":"breadcrumbs","type":"breadcrumbs"}],"children":[]},
                "99":{"id":99,"key":"D","header":"Thanks","button":{"type":"done","target":null},"elements":[],"children":[],"is_auto_submit":true},
                "72":{"id":72,"key":"OK","header":"Sent","button":{"type":"done","target":null},"elements":[{"name":"success","type":"success"}],"children":[]}}}"#,
            )
            .unwrap(),
        )
    }

    fn report() -> Report {
        let mut r = Report::new(ReportTarget::Message { channel: Snowflake(1), message: Snowflake(2) });
        r.set_menu(menu());
        r
    }

    fn steps(req: Option<ActionRequest>) -> Vec<ReportStep> {
        match req {
            Some(ActionRequest::SubmitReport { steps, .. }) => steps,
            _ => panic!("no submit"),
        }
    }

    #[test]
    fn settings_upsells_follow_subtype_channel_and_filters() {
        let mut proto = std::collections::HashMap::new();
        assert!(settings_upsells(None, true, &proto).is_empty());
        assert!(settings_upsells(Some("sub_spam"), true, &proto).is_empty());
        proto.insert("spam", 1);
        assert_eq!(settings_upsells(Some("sub_spam"), true, &proto), vec![Upsell::DmSpam]);
        assert!(settings_upsells(Some("sub_spam"), false, &proto).is_empty());
        assert_eq!(settings_upsells(Some("sub_csam"), false, &proto), vec![Upsell::SexualMedia]);
        assert_eq!(Upsell::SexualMedia.patches(&proto), vec![("cf-servers", 1), ("cf-friends", 1)]);
        assert_eq!(settings_upsells(Some("sub_gore"), false, &proto), vec![Upsell::GraphicMedia]);
        for f in GRAPHIC_FILTERS {
            proto.insert(f, 2);
        }
        assert!(settings_upsells(Some("sub_glorifying_violence"), false, &proto).is_empty());
    }

    #[test]
    fn checkbox_path_submits_like_the_client() {
        let mut r = report();
        assert!(r.go("Private".into(), 81).is_none());
        assert!(r.current().unwrap().next_blocked(&r.picks));
        r.toggle("mail", "Mail");
        r.toggle("ip", "IP");
        r.toggle("mail", "Mail");
        assert!(r.go(String::new(), 17).is_none());
        assert_eq!(r.breadcrumbs(), vec!["Private".to_owned(), "IP".to_owned()]);
        let s = steps(r.submit());
        assert!(r.submit().is_none());
        assert_eq!(s.iter().map(|s| s.node).collect::<Vec<_>>(), vec![7, 81, 17]);
        let v = phoenix_api::rest::reporting::report_body(r.menu.as_ref().unwrap(), r.target, &s);
        assert_eq!(v["breadcrumbs"], serde_json::json!([7, 81, 17]));
        assert_eq!(v["elements"], serde_json::json!({"pii_select": ["ip"]}));
        assert_eq!(v["message_id"], "2");
        r.submitted(Ok(()));
        assert_eq!(r.current, 72);
        assert!(!r.sending);
    }

    #[test]
    fn back_restores_picks() {
        let mut r = report();
        r.go("Private".into(), 81);
        r.toggle("ip", "IP");
        r.go(String::new(), 17);
        r.back();
        assert_eq!(r.current, 81);
        assert_eq!(r.picks, vec![("ip".to_owned(), "IP".to_owned())]);
        r.back();
        assert_eq!(r.current, 7);
        assert!(r.history.is_empty());
    }

    #[test]
    fn auto_submit_fires_once_on_arrival() {
        let mut r = report();
        let s = steps(r.go("Dislike".into(), 99));
        assert_eq!(s.iter().map(|s| s.node).collect::<Vec<_>>(), vec![7, 99]);
        assert_eq!(r.current, 99);
        r.back();
        assert!(r.go("Dislike".into(), 99).is_none());
    }

    #[test]
    fn skip_nodes_and_errors() {
        let mut r = report();
        r.go("Else".into(), 82);
        r.go("Self harm".into(), 162);
        assert_eq!(r.current, 95);
        let mut r = report();
        r.go("Private".into(), 81);
        r.toggle("ip", "IP");
        r.go(String::new(), 17);
        r.submit();
        r.submitted(Err("HTTP 400: {\"code\": 50035}".into()));
        assert_eq!(r.current, 17);
        assert!(r.error.is_some() && !r.sending);
    }
}
