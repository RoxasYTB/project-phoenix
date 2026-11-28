use std::cell::RefCell;
use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, StyledText, Window, div, prelude::*};

use crate::app::PhoenixApp;
use crate::model::{Id, Message};
use crate::markdown::{Block, Inline, Span, Style};
use crate::richtext::RichText;
use crate::theme::{generated::idx, tok, z};

pub const MAX: usize = 3;
pub const LINGER: Duration = Duration::from_secs(10);
const STAGE_TYPES: [u8; 5] = [27, 28, 29, 30, 31];

#[derive(Default)]
struct Toasts {
    channel: Option<Id>,
    ids: Vec<Id>,
    msgs: Vec<Message>,
    born: Vec<(Id, Instant)>,
    leaving: Vec<(Message, Instant)>,
    fading: Option<(Vec<Message>, Instant)>,
    hide_at: Option<Instant>,
    frozen: bool,
}

thread_local! {
    static TOASTS: RefCell<Toasts> = RefCell::new(Toasts::default());
}

pub const SPRING_DONE: Duration = Duration::from_millis(600);

pub fn spring(t: Duration) -> f32 {
    let w = 170f32.sqrt();
    let x = t.as_secs_f32() * w;
    if t >= SPRING_DONE { 1. } else { (1. - (1. + x) * (-x).exp()).clamp(0., 1.) }
}

fn d(i: usize) -> gpui::Hsla {
    crate::theme::c_for(tok(i), crate::ui::settings::AppTheme::Dark)
}

pub fn accepts(m: &Message, voice: Option<Id>) -> bool {
    voice == Some(m.channel) && !STAGE_TYPES.contains(&m.type_id) && m.send_state != crate::model::SendState::Sending
}

pub fn push(channel: Id, id: Id, now: Instant) {
    TOASTS.with_borrow_mut(|t| {
        if t.channel != Some(channel) {
            *t = Toasts { channel: Some(channel), ..Default::default() };
        }
        t.fading = None;
        t.ids.push(id);
        t.born.push((id, now));
        let extra = t.ids.len().saturating_sub(MAX);
        let gone: Vec<Id> = t.ids.drain(..extra).collect();
        for g in gone {
            if let Some(m) = t.msgs.iter().find(|m| m.id == g) {
                t.leaving.push((m.clone(), now));
            }
        }
        let keep = t.ids.clone();
        t.msgs.retain(|m| keep.contains(&m.id));
        t.born.retain(|(i, _)| keep.contains(i));
        t.hide_at = Some(now + LINGER);
    });
}

pub fn shown(channel: Id, now: Instant) -> Vec<Id> {
    TOASTS.with_borrow_mut(|t| {
        if t.channel != Some(channel) {
            return Vec::new();
        }
        if !t.frozen && t.hide_at.is_some_and(|h| now >= h) {
            let msgs = std::mem::take(&mut t.msgs);
            if !msgs.is_empty() {
                t.fading = Some((msgs, now));
            }
            t.ids.clear();
            t.born.clear();
            t.hide_at = None;
        }
        t.leaving.retain(|(_, at)| now.duration_since(*at) < SPRING_DONE);
        if t.fading.as_ref().is_some_and(|(_, at)| now.duration_since(*at) >= SPRING_DONE) {
            t.fading = None;
        }
        t.ids.clone()
    })
}

fn set_frozen(on: bool) {
    TOASTS.with_borrow_mut(|t| t.frozen = on);
}

pub fn on_message(app: &PhoenixApp, m: &Message, cx: &mut Context<PhoenixApp>) {
    if !accepts(m, app.voice_channel) {
        return;
    }
    push(m.channel, m.id, Instant::now());
    TOASTS.with_borrow_mut(|t| t.msgs.push(m.clone()));
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(LINGER + Duration::from_millis(50)).await;
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

pub fn joined_inline(blocks: &[Block]) -> Inline {
    let mut out = Inline::default();
    for b in blocks {
        let piece = match b {
            Block::Code { code, .. } => Inline {
                text: code.clone(),
                spans: vec![Span { range: 0..code.len(), style: Style::CODE, link: None, mention: None, spoiler: None }],
                channel_icons: Vec::new(),
            },
            other => other.inline().cloned().unwrap_or_default(),
        };
        if piece.text.is_empty() {
            continue;
        }
        if !out.text.is_empty() {
            let at = out.text.len();
            out.text.push(' ');
            out.spans.push(Span { range: at..at + 1, style: Style::PLAIN, link: None, mention: None, spoiler: None });
        }
        let off = out.text.len();
        out.channel_icons.extend(piece.channel_icons.iter().map(|(at, n)| (at + off, *n)));
        out.text.push_str(&piece.text.replace(['\n', '\r'], " "));
        out.spans.extend(piece.spans.into_iter().map(|mut sp| {
            sp.range = sp.range.start + off..sp.range.end + off;
            sp
        }));
    }
    out
}

fn mentioned(app: &PhoenixApp, m: &Message) -> bool {
    m.mention_everyone || m.content.contains(&format!("<@{}>", app.me.id.0)) || m.content.contains(&format!("<@!{}>", app.me.id.0))
}

fn stickers(m: &Message) -> Vec<&crate::model::Attachment> {
    m.attachments.iter().filter(|a| matches!(a.kind, crate::model::AttachmentKind::Sticker(_)) && !a.url.is_empty()).collect()
}

fn est_height(m: &Message) -> f32 {
    if m.content.is_empty() && !stickers(m).is_empty() { 136. } else { 32. }
}

fn line(rt: &RichText) -> gpui::Div {
    let emoji = 22.;
    let mut row = div().flex().items_center().min_w_0().overflow_hidden().whitespace_nowrap();
    let mut last = 0;
    let seg = |r: std::ops::Range<usize>| StyledText::new(SharedString::from(rt.text[r.clone()].to_owned())).with_runs(crate::ui::profile_bio::slice_runs(&rt.runs, r));
    for (r, token) in &rt.emoji {
        if last < r.start {
            row = row.child(div().flex_none().child(seg(last..r.start)));
        }
        row = row.child(div().flex_none().mx(z(1.)).size(z(emoji)).overflow_hidden().child(crate::media::emoji::emoji_element(token, emoji * crate::theme::zoom())));
        last = r.end;
    }
    if last < rt.text.len() {
        row = row.child(div().min_w_0().text_ellipsis().child(seg(last..rt.text.len())));
    }
    row
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trailing {
    Image,
    Sticker,
    Command,
    Voice,
}

const FLAG_COMPONENTS_V2: u64 = 1 << 15;

pub fn placeholder(m: &Message, blocked: bool, ignored: bool) -> (Option<&'static str>, Option<Trailing>) {
    let empty = m.content.trim().is_empty();
    let has_sticker = !stickers(m).is_empty() || m.attachments.iter().any(|a| matches!(a.kind, crate::model::AttachmentKind::Sticker(_)));
    let command = m.interaction.is_some();
    let voice = m.flags & phoenix_voice::audio::voice_message::FLAG_IS_VOICE_MESSAGE != 0;
    let poll_result = matches!(m.kind, crate::model::MessageKind::PollResult(_));
    let mut trailing = None;
    let key = if blocked {
        Some("call_toast.blocked")
    } else if ignored {
        Some("call_toast.ignored")
    } else if m.kind == crate::model::MessageKind::Pin {
        Some("call_toast.pinned")
    } else if empty && !poll_result {
        Some(if has_sticker {
            "chat.reply_click_to_see_sticker"
        } else if command {
            "chat.reply_click_to_see_command"
        } else if voice {
            "chat.reply_click_to_see_voice"
        } else if m.flags & FLAG_COMPONENTS_V2 != 0 {
            "chat.reply_click_to_see_message"
        } else {
            trailing = Some(Trailing::Image);
            "chat.reply_click_to_see"
        })
    } else {
        None
    };
    if has_sticker {
        trailing = Some(Trailing::Sticker);
    } else if command {
        trailing = Some(Trailing::Command);
    } else if voice {
        trailing = Some(Trailing::Voice);
    } else if (!m.attachments.is_empty() || !m.embeds.is_empty()) && !poll_result {
        trailing = Some(Trailing::Image);
    }
    (key, trailing)
}

fn trailing_icon(t: Trailing) -> AnyElement {
    let size = 20. * crate::theme::zoom();
    let color = d(idx::TEXT_DEFAULT);
    let (path, w) = match t {
        Trailing::Image => (crate::assets::Icon::Image.path(), size),
        Trailing::Sticker => (crate::icons::path("chat/sticker-small"), size),
        Trailing::Command => (crate::icons::path("chat/slash-box"), size),
        Trailing::Voice => (crate::assets::Icon::Mic.path(), size * 19. / 24.),
    };
    div().flex_none().ml(z(4.)).child(gpui::svg().path(path).w(gpui::px(w)).h(gpui::px(size)).text_color(color)).into_any_element()
}

fn body(app: &PhoenixApp, m: &Message) -> AnyElement {
    let (key, trailing) = placeholder(m, app.nav.is_blocked(m.author), app.nav.is_ignored(m.author));
    let has_stickers = !stickers(m).is_empty();
    let main = match key {
        Some(k) if !has_stickers => div().min_w_0().truncate().child(crate::i18n::t(k, &[])).into_any_element(),
        Some(_) => sticker_row(m),
        None => content(app, m),
    };
    let size = 20. * crate::theme::zoom();
    div()
        .flex()
        .items_center()
        .min_w_0()
        .when(m.forward.is_some() && !has_stickers, |r| {
            r.child(div().flex_none().mr(z(4.)).child(gpui::svg().path(crate::icons::path("chat/forwarded")).size(gpui::px(size)).text_color(d(idx::TEXT_DEFAULT))))
        })
        .child(main)
        .when_some(trailing.filter(|_| !has_stickers), |r, tr| r.child(trailing_icon(tr)))
        .into_any_element()
}

fn sticker_row(m: &Message) -> AnyElement {
    div()
        .flex()
        .gap(z(4.))
        .children(stickers(m).into_iter().map(|a| {
            let im = match a.kind {
                crate::model::AttachmentKind::Sticker(crate::media::StickerFormat::Lottie) => crate::media::lottie::image(a.url.clone(), 128, true),
                _ => gpui::img(a.url.clone()),
            };
            im.size(z(128.)).object_fit(gpui::ObjectFit::Contain).into_any_element()
        }))
        .into_any_element()
}

fn content(app: &PhoenixApp, m: &Message) -> AnyElement {
    let text = m.content.as_ref();
    if text.is_empty() {
        return if stickers(m).is_empty() { div().into_any_element() } else { sticker_row(m) };
    }
    let blocks = crate::markdown::parse(text, &app.resolver());
    let rt = crate::richtext::rich_inline(&joined_inline(&blocks), &|_| false, d(idx::TEXT_DEFAULT));
    line(&rt).into_any_element()
}

fn toast(app: &PhoenixApp, m: &Message, opacity: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let shadow = vec![gpui::BoxShadow { color: d(idx::OPACITY_BLACK_20), offset: gpui::point(z(0.), z(2.)), blur_radius: z(10.), spread_radius: z(0.), inset: false }];
    let group: SharedString = format!("call-toast-{}", m.id.0).into();
    let (channel, id) = (m.channel, m.id);
    div()
        .id(("call-toast", m.id.0))
        .w_full()
        .flex()
        .justify_end()
        .opacity(opacity)
        .child(
            div()
                .group(group.clone())
                .max_w_full()
                .flex()
                .flex_row()
                .overflow_hidden()
                .cursor_pointer()
                .child(
                    div()
                        .relative()
                        .min_w_0()
                        .min_h(z(32.))
                        .px(z(12.))
                        .py(z(4.))
                        .rounded(z(16.))
                        .overflow_hidden()
                        .flex()
                        .items_center()
                        .bg(d(idx::BACKGROUND_BASE_LOWEST))
                        .group_hover(group, |s| s.bg(d(idx::BACKGROUND_BASE_LOWER)))
                        .shadow(shadow.clone())
                        .text_size(z(16.))
                        .line_height(z(22.))
                        .text_color(d(idx::TEXT_DEFAULT))
                        .when(mentioned(app, m), |el| {
                            el.child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .rounded(z(16.))
                                    .bg(d(idx::YELLOW_300).opacity(0.15))
                                    .border(z(2.))
                                    .border_color(d(idx::YELLOW_300).opacity(0.3)),
                            )
                        })
                        .child(body(app, m)),
                )
                .child(div().flex_none().ml(z(8.)).size(z(32.)).rounded_full().shadow(shadow).child(crate::ui::avatar(app.users.get(&m.author), 32. * crate::theme::zoom()))),
        )
        .on_click(cx.listener(move |a, _, _, cx| {
            crate::ui::call_view::set_chat_open(channel, true);
            a.jump_to_message(channel, id, cx);
        }))
        .into_any_element()
}

pub fn render(app: &PhoenixApp, channel: Id, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if crate::ui::call_view::chat_open(channel) || !crate::ui::call_menus::chat_toasts(channel) {
        return None;
    }
    let now = Instant::now();
    let ids = shown(channel, now);
    let (kept, born, leaving, fading) = TOASTS.with_borrow(|t| (t.msgs.clone(), t.born.clone(), t.leaving.clone(), t.fading.clone()));
    let stored = app.messages.get(&channel);
    let list: Vec<Message> = ids
        .iter()
        .filter_map(|id| stored.and_then(|e| e.msgs.iter().rev().find(|m| m.id == *id)).or_else(|| kept.iter().find(|m| m.id == *id)).cloned())
        .collect();
    let fading_all = fading.filter(|_| list.is_empty());
    if list.is_empty() && leaving.is_empty() && fading_all.is_none() {
        return None;
    }
    let mut animating = false;
    let mut shift = 0.;
    let mut out_items = Vec::new();
    for (m, at) in &leaving {
        let s = spring(now.duration_since(*at));
        animating |= s < 1.;
        let h = est_height(m) + 8.;
        out_items.push((m.clone(), -h * s + shift, 1. - s));
        shift += h * (1. - s);
    }
    let fade_out = fading_all.as_ref().map(|(_, at)| 1. - spring(now.duration_since(*at)));
    let shown_list = match &fading_all {
        Some((msgs, _)) => msgs.clone(),
        None => list,
    };
    let opacity_of = |m: &Message| -> f32 {
        if let Some(o) = fade_out {
            return o;
        }
        born.iter().find(|(i, _)| *i == m.id).map_or(1., |(_, at)| spring(now.duration_since(*at)))
    };
    let items: Vec<(Message, f32)> = shown_list.iter().map(|m| (m.clone(), opacity_of(m))).collect();
    animating |= fade_out.is_some_and(|o| o > 0.) || items.iter().any(|(_, o)| *o < 1.);
    if animating {
        window.request_animation_frame();
    }
    Some(
        div()
            .id("call-chat-toasts")
            .absolute()
            .top(z(48.))
            .right(z(12.))
            .w(z(304.))
            .h(gpui::relative(0.5))
            .overflow_hidden()
            .on_hover(|on, _, _| set_frozen(*on))
            .child(
                div()
                    .relative()
                    .top(z(shift))
                    .flex()
                    .flex_col()
                    .gap(z(8.))
                    .children(items.iter().map(|(m, o)| toast(app, m, *o, cx))),
            )
            .children(out_items.iter().map(|(m, y, o)| div().absolute().left_0().right_0().top(z(*y)).child(toast(app, m, *o, cx))))
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_last_three_and_hides_after_ten_seconds() {
        let (ch, t0) = (Id(1), Instant::now());
        for i in 0..5 {
            push(ch, Id(10 + i), t0);
        }
        assert_eq!(shown(ch, t0), vec![Id(12), Id(13), Id(14)]);
        push(ch, Id(20), t0 + Duration::from_secs(9));
        assert_eq!(shown(ch, t0 + Duration::from_secs(18)).len(), 3);
        assert!(shown(ch, t0 + Duration::from_secs(19)).is_empty());
        assert!(shown(Id(2), t0).is_empty());
    }

    #[test]
    fn previews_enter_and_leave_with_the_default_spring() {
        assert_eq!(spring(Duration::ZERO), 0.);
        assert!(spring(Duration::from_millis(100)) > 0.3 && spring(Duration::from_millis(100)) < 0.5);
        assert!(spring(Duration::from_millis(400)) > 0.95);
        assert_eq!(spring(SPRING_DONE), 1.);
        let (ch, t0) = (Id(40), Instant::now());
        for i in 0..3 {
            push(ch, Id(50 + i), t0);
            TOASTS.with_borrow_mut(|t| t.msgs.push(Message { id: Id(50 + i), channel: ch, ..Default::default() }));
        }
        push(ch, Id(60), t0);
        assert_eq!(TOASTS.with_borrow(|t| t.leaving.iter().map(|(m, _)| m.id).collect::<Vec<_>>()), vec![Id(50)]);
        assert_eq!(shown(ch, t0 + Duration::from_millis(700)), vec![Id(51), Id(52), Id(60)]);
        assert!(TOASTS.with_borrow(|t| t.leaving.is_empty()));
        assert!(shown(ch, t0 + LINGER).is_empty());
        assert!(TOASTS.with_borrow(|t| t.fading.as_ref().is_some_and(|(m, _)| m.len() == 2)));
        let _ = shown(ch, t0 + LINGER + SPRING_DONE);
        assert!(TOASTS.with_borrow(|t| t.fading.is_none()));
    }

    #[test]
    fn preview_joins_blocks_on_one_line_with_their_styles() {
        let blocks = crate::markdown::parse("# Titre\n_italique_ et **gras**\n```\ncode\n```", &crate::markdown::NoResolve);
        let i = joined_inline(&blocks);
        assert_eq!(i.text, "Titre italique et gras code");
        let covered: usize = i.spans.iter().map(|s| s.range.len()).sum();
        assert_eq!(covered, i.text.len());
        let italic = i.spans.iter().find(|s| s.style.has(Style::ITALIC)).unwrap();
        assert_eq!(&i.text[italic.range.clone()], "italique");
        assert!(i.spans.iter().any(|s| s.style.has(Style::CODE) && &i.text[s.range.clone()] == "code"));
    }

    #[test]
    fn attachment_only_previews_use_the_client_placeholder() {
        use crate::model::{Attachment, AttachmentKind, MessageKind};
        let file = Attachment { filename: "a.png".into(), kind: AttachmentKind::Image, ..Default::default() };
        let m = Message { attachments: vec![file.clone()], ..Default::default() };
        assert_eq!(placeholder(&m, false, false), (Some("chat.reply_click_to_see"), Some(Trailing::Image)));
        let text = Message { content: "salut".into(), ..m.clone() };
        assert_eq!(placeholder(&text, false, false), (None, Some(Trailing::Image)));
        assert_eq!(placeholder(&text, true, false).0, Some("call_toast.blocked"));
        assert_eq!(placeholder(&text, false, true).0, Some("call_toast.ignored"));
        assert_eq!(placeholder(&Message { kind: MessageKind::Pin, ..Default::default() }, false, false), (Some("call_toast.pinned"), None));
        let voice = Message { flags: phoenix_voice::audio::voice_message::FLAG_IS_VOICE_MESSAGE, attachments: vec![file], ..Default::default() };
        assert_eq!(placeholder(&voice, false, false), (Some("chat.reply_click_to_see_voice"), Some(Trailing::Voice)));
        let v2 = Message { flags: FLAG_COMPONENTS_V2, ..Default::default() };
        assert_eq!(placeholder(&v2, false, false), (Some("chat.reply_click_to_see_message"), None));
        assert_eq!(placeholder(&Message { content: "x".into(), ..Default::default() }, false, false), (None, None));
    }

    #[test]
    fn only_messages_of_the_call_channel() {
        let m = Message { channel: Id(5), ..Default::default() };
        assert!(accepts(&m, Some(Id(5))));
        assert!(!accepts(&m, Some(Id(6))));
        assert!(!accepts(&Message { type_id: 27, ..m.clone() }, Some(Id(5))));
    }
}
