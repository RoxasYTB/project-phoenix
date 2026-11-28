use gpui::{AnyElement, ClickEvent, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Subscription, Window, div, prelude::*};
use gpui_component::input::{Input, InputEvent, InputState};

use crate::{
    actions::{ActionReply, ActionRequest},
    app::PhoenixApp,
    model::{ChannelKind, Id},
    theme::{c, generated::idx, tok, z},
};

pub(crate) struct CreateThread {
    pub channel: Id,
    pub name: Entity<InputState>,
    pub starter: Entity<InputState>,
    pub private: bool,
    pub source: Option<(Id, String)>,
    _subs: Vec<Subscription>,
}

pub fn default_thread_name(content: &str, embed_title: Option<&str>, poll_question: Option<&str>) -> String {
    fn cut(s: &str, n: usize) -> String {
        if s.encode_utf16().count() > n { String::from_utf16_lossy(&s.encode_utf16().take(n).collect::<Vec<_>>()) + "..." } else { s.to_string() }
    }
    let len = |s: &str| s.encode_utf16().count();
    if let Some(t) = embed_title.filter(|t| !t.is_empty()) {
        return cut(t, 40);
    }
    if let Some(q) = poll_question.filter(|q| !q.is_empty()) {
        return cut(q, 80);
    }
    let first = content.split('\n').next().unwrap_or("");
    let line = first.trim_start_matches([' ', '#', '-']);
    let sep = |c: char| c.is_whitespace() || "!@#$%^&*()_-+={}[]:\";'<>?,./".contains(c);
    let mut parts: Vec<&str> = Vec::new();
    let mut start = 0;
    let mut in_sep = None;
    for (i, ch) in line.char_indices() {
        let s = sep(ch);
        if in_sep.is_some_and(|w| w != s) {
            parts.push(&line[start..i]);
            start = i;
        }
        in_sep = Some(s);
    }
    parts.push(&line[start..]);
    if line.chars().next().is_some_and(sep) {
        parts.insert(0, "");
    }
    let mut out = parts[0].to_string();
    for p in &parts[1..] {
        if len(&out) + len(p) > 40 {
            break;
        }
        out.push_str(p);
    }
    cut(&out, 40)
}

fn t(i: usize) -> gpui::Hsla {
    c(tok(i))
}

fn icon(name: &str, size: f32, color: gpui::Hsla) -> gpui::Svg {
    gpui::svg().path(crate::icons::path(name)).size(z(size)).flex_none().text_color(color)
}

impl PhoenixApp {
    pub(crate) fn active_thread_form(&self) -> Option<&CreateThread> {
        self.chat.create_thread.as_ref().filter(|form| self.selected_channel == Some(form.channel))
    }

    pub fn thread_form_open(&self) -> bool {
        self.active_thread_form().is_some()
    }

    pub fn open_create_thread(&mut self, channel: Id, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_channel != Some(channel)
            || !self.channels.get(&channel).is_some_and(|c| c.guild.is_some() && matches!(c.kind, ChannelKind::Text | ChannelKind::Announcement))
        {
            return;
        }
        let name = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("thread.name_placeholder")));
        let starter = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("thread.starter_placeholder")));
        let mut subs = Vec::new();
        for input in [&name, &starter] {
            subs.push(cx.subscribe_in(input, window, |this, _, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { shift: false, .. } = ev {
                    this.submit_create_thread(window, cx);
                }
            }));
        }
        starter.update(cx, |s, cx| s.focus(window, cx));
        self.chat.thread = None;
        self.parity.threads_open = false;
        self.chat.create_thread = Some(CreateThread { channel, name, starter, private: false, source: None, _subs: subs });
        cx.notify();
    }

    pub fn open_create_thread_from(&mut self, channel: Id, msg: Id, window: &mut Window, cx: &mut Context<Self>) {
        self.chat.menu = None;
        let Some(m) = self.messages.get(&channel).and_then(|x| x.msgs.iter().find(|m| m.id == msg)) else { return };
        let poll = crate::commands::store::get(m.id).and_then(|x| x.poll.as_ref().map(|p| p.question.clone()));
        let default = default_thread_name(&m.content, m.embeds.first().and_then(|e| e.title.as_deref()), poll.as_deref());
        self.open_create_thread(channel, window, cx);
        let ph: SharedString = if default.is_empty() { crate::t!("thread.default_name") } else { default.clone().into() };
        let input = self.chat.create_thread.as_mut().filter(|s| s.channel == channel).map(|s| {
            s.source = Some((msg, default));
            s.name.clone()
        });
        if let Some(input) = input {
            input.update(cx, |i, cx| i.set_placeholder(ph, window, cx));
        }
        cx.notify();
    }

    pub fn close_create_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.chat.create_thread = None;
        self.focus_chat_composer(window, cx);
        cx.notify();
    }

    fn submit_create_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.active_thread_form() else { return };
        let form = s.name.clone();
        let (channel, private) = (s.channel, s.private);
        let name = s.name.read(cx).value().trim().to_string();
        let starter = s.starter.read(cx).value().trim().to_string();
        if let Some((msg, default)) = s.source.clone() {
            let handle = window.window_handle();
            let name = if !name.is_empty() { name } else if !default.is_empty() { default } else { crate::t!("thread.default_name").to_string() };
            let thread_name = name.clone();
            self.action(ActionRequest::CreateThread { channel, msg, name }, cx, move |a, r, cx| match r {
                Ok(ActionReply::Channel(id)) => {
                    a.show_created_thread(&form, id, thread_name, handle, cx);
                    if !starter.is_empty() {
                        let fut = a.backend.send_message(id, starter, None);
                        cx.spawn(async move |this, cx| {
                            if let Ok(m) = fut.await {
                                let _ = this.update(cx, |this, cx| this.append_message(m, cx));
                            }
                        })
                        .detach();
                    }
                }
                Ok(_) => {}
                Err(e) => a.parity_toast(e.0, cx),
            });
            return;
        }
        let name = if name.is_empty() { starter.chars().take(40).collect() } else { name };
        if name.is_empty() {
            return;
        }
        let announcement = self.channels.get(&channel).is_some_and(|c| c.kind == ChannelKind::Announcement);
        let handle = window.window_handle();
        let thread_name = name.clone();
        self.action(ActionRequest::CreateStandaloneThread { channel, name, private, announcement }, cx, move |a, r, cx| match r {
            Ok(ActionReply::Channel(id)) => {
                a.show_created_thread(&form, id, thread_name, handle, cx);
                if !starter.is_empty() {
                    let fut = a.backend.send_message(id, starter, None);
                    cx.spawn(async move |this, cx| {
                        if let Ok(m) = fut.await {
                            let _ = this.update(cx, |this, cx| this.append_message(m, cx));
                        }
                    })
                    .detach();
                }
            }
            Ok(_) => {}
            Err(e) => a.parity_toast(e.0, cx),
        });
    }

    fn show_created_thread(&self, form: &Entity<InputState>, id: Id, name: String, handle: gpui::AnyWindowHandle, cx: &mut Context<Self>) {
        let entity = cx.entity();
        let form = form.clone();
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                entity.update(cx, |app, cx| {
                    if app.active_thread_form().is_some_and(|current| current.name == form) {
                        app.open_thread(crate::model::ThreadRef { id, name: name.into(), message_count: 0 }, window, cx);
                    }
                })
            });
        });
    }
}

pub fn panel(app: &PhoenixApp, width: gpui::Pixels, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let s = app.active_thread_form()?;
    let private = s.private;
    let (name, starter) = (s.name.clone(), s.starter.clone());
    let source = s.source.as_ref().map(|(m, _)| *m);
    let preview = source.and_then(|mid| {
        let entry = app.messages.get(&s.channel)?;
        let i = entry.index_of(mid)?;
        let entity = cx.entity();
        Some(div().pb(z(24.)).child(crate::ui::chat::message(app, &entity, &entry.msgs[i], &entry.rendered[i], entry, cx)))
    });
    let header = div()
        .flex_none()
        .h(z(crate::theme::m::HEADER_H))
        .pt(z(8.))
        .pr(z(8.))
        .pb(z(8.))
        .pl(z(20.))
        .border_b_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .flex()
        .items_center()
        .child(div().size(z(32.)).flex().items_center().justify_center().child(icon("chat/thread", 20., t(idx::ICON_MUTED))))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t(idx::TEXT_STRONG))
                .child(crate::t!("thread.name_placeholder")),
        )
        .child(
            div()
                .id("create-thread-close")
                .flex_none()
                .ml(z(10.))
                .size(z(32.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .child(icon("close", 20., t(idx::INTERACTIVE_ICON_DEFAULT)))
                .on_click(cx.listener(|a, _: &ClickEvent, window, cx| a.close_create_thread(window, cx))),
        );
    let label = |text: SharedString| {
        div()
            .mb(z(8.))
            .text_size(z(16.))
            .line_height(z(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t(idx::TEXT_STRONG))
            .child(text)
    };
    let checkbox = div()
        .size(z(20.))
        .flex_none()
        .rounded(z(4.))
        .border_1()
        .border_color(t(if private { idx::CHECKBOX_BORDER_SELECTED_DEFAULT } else { idx::CHECKBOX_BORDER_DEFAULT }))
        .bg(t(if private { idx::CHECKBOX_BACKGROUND_SELECTED_DEFAULT } else { idx::CHECKBOX_BACKGROUND_DEFAULT }))
        .flex()
        .items_center()
        .justify_center()
        .when(private, |d| d.child(icon("check", 14., t(idx::CHECKBOX_ICON_ACTIVE))));
    let form = div()
        .mx(z(20.))
        .mb(z(20.))
        .flex()
        .flex_col()
        .child(
            div()
                .my(z(16.))
                .size(z(64.))
                .rounded_full()
                .bg(t(idx::BACKGROUND_MOD_STRONG))
                .flex()
                .items_center()
                .justify_center()
                .child(icon("chat/thread", 32., t(idx::ICON_SUBTLE))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(z(24.))
                .child(
                    div().flex().flex_col().child(label(if source.is_some() { crate::t!("thread.name_optional") } else { crate::t!("thread.name") })).child(
                        div()
                            .h(z(40.))
                            .rounded(z(8.))
                            .border_1()
                            .border_color(t(idx::INPUT_BORDER_DEFAULT))
                            .bg(t(idx::INPUT_BACKGROUND_DEFAULT))
                            .px(z(12.))
                            .flex()
                            .items_center()
                            .text_size(z(16.))
                            .child(Input::new(&name).appearance(false).bordered(false)),
                    ),
                )
                .when(source.is_none(), |d| d.child(
                    div().flex().flex_col().child(label(crate::t!("thread.private"))).child(
                        div()
                            .id("create-thread-private")
                            .flex()
                            .gap(z(12.))
                            .cursor_pointer()
                            .child(checkbox)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(z(16.))
                                    .line_height(z(20.))
                                    .text_color(t(idx::TEXT_STRONG))
                                    .child(crate::t!("thread.private_hint")),
                            )
                            .on_click(cx.listener(|app, _: &ClickEvent, _, cx| {
                                if let Some(s) = app.chat.create_thread.as_mut() {
                                    s.private = !s.private;
                                }
                                cx.notify();
                            })),
                    ),
                )),
        );
    let composer = div()
        .flex_none()
        .mx(z(10.))
        .mb(z(10.))
        .min_h(z(58.))
        .rounded(z(8.))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .bg(t(idx::CHAT_BACKGROUND_DEFAULT))
        .pl(z(10.))
        .pr(z(12.))
        .flex()
        .items_center()
        .text_size(z(16.))
        .line_height(z(22.))
        .child(div().flex_1().child(Input::new(&starter).appearance(false).bordered(false)));
    Some(
        div()
            .id("create-thread")
            .w(width)
            .flex_none()
            .h_full()
            .border_l_1()
            .border_t_1()
            .border_r_1()
            .rounded_tl(z(12.))
            .overflow_hidden()
            .border_color(t(idx::BORDER_SUBTLE))
            .bg(t(idx::BACKGROUND_BASE_LOWER))
            .flex()
            .flex_col()
            .child(header)
            .child(
                crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::ThinFade)
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id("create-thread-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&crate::ui::chat::scrollbar::handle("create-thread-scroll"))
                            .flex()
                            .flex_col()
                            .justify_end()
                            .child(form)
                            .children(preview),
                    )
                    .children(crate::ui::chat::scrollbar::bar_for("create-thread-scroll", crate::ui::chat::scrollbar::Bar::ThinFade)),
            )
            .child(composer)
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::default_thread_name;

    #[test]
    fn default_name_matches_the_live_client() {
        let c = "[Test Phoenix] envoyé depuis le client officiel — message source pour tester les fils";
        assert_eq!(default_thread_name(c, None, None), "[Test Phoenix] envoyé depuis le client ");
        assert_eq!(default_thread_name("## Titre\nsuite", None, None), "Titre");
        assert_eq!(default_thread_name("x", Some("Embed"), None), "Embed");
        assert_eq!(default_thread_name("", None, Some("Question ?")), "Question ?");
        let long = "a".repeat(50);
        assert_eq!(default_thread_name(&long, None, None), format!("{}...", "a".repeat(40)));
    }
}
