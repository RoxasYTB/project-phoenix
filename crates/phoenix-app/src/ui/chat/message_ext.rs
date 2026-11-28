use gpui::{
    AnyElement, App, Div, Entity, FontWeight, IntoElement, MouseDownEvent, ParentElement,
    SharedString, Styled, Window, div, prelude::*,
};
use gpui_component::{Sizable as _, input::Input};

use super::{
    icons::{ChatIcon, icon},
    state::EmojiTarget,
};
use crate::{
    app::PhoenixApp,
    assets::Icon,
    model::{Message, ThreadRef},
    theme::{Palette, c, generated::idx, z},
    ui,
};

use super::tk;

fn bar_button(id: impl Into<gpui::ElementId>) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .size(z(28.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(6.))
        .cursor_pointer()
        .group("bar-btn")
        .hover(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
        .active(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_ACTIVE)))
}

const ICON_EASE_MS: u64 = 400;

fn tool_button(id: &'static str, msg: u64, i: SharedString, _size: f32) -> gpui::Stateful<Div> {
    use gpui::AnimationExt;
    let k = ui::states::key(("tb-icon", id, msg));
    let glyph = gpui::svg()
        .path(i)
        .flex_none()
        .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
        .group_hover("bar-btn", |s| s.text_color(tk(idx::INTERACTIVE_TEXT_HOVER)))
        .id("glyph")
        .group_active("bar-btn", |s| s.text_color(tk(idx::INTERACTIVE_TEXT_ACTIVE)).size(z(20.)));
    let sized = match ui::states::state(k) {
        Some((on, at)) if at.elapsed() < std::time::Duration::from_millis(ICON_EASE_MS) => glyph
            .with_animation(
                gpui::ElementId::Name(format!("tb-icon-{id}-{on}-{at:?}").into()),
                gpui::Animation::new(std::time::Duration::from_millis(ICON_EASE_MS)).with_easing(ui::states::cubic_bezier(0.16, 1., 0.3, 1.)),
                move |g, t| g.size(z(if on { 20. + 2. * t } else { 22. - 2. * t })),
            )
            .into_any_element(),
        Some((true, _)) => glyph.size(z(22.)).into_any_element(),
        _ => glyph.size(z(20.)).into_any_element(),
    };
    ui::states::track(bar_button(id), k).child(sized)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolbarButton {
    CopyId,
    CopyLink,
    Configure,
    MarkUnread,
    Bookmark,
    Pin,
    Thread,
    ReplySelf,
    Quick,
    AddReaction,
    Reply,
    Edit,
    Forward,
    ViewThread,
    Publish,
    Delete,
    More,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ToolbarCaps {
    pub expanded: bool,
    pub dev: bool,
    pub own: bool,
    pub can_edit: bool,
    pub can_pin: bool,
    pub can_start_thread: bool,
    pub can_view_thread: bool,
    pub can_delete: bool,
    pub can_publish: bool,
    pub can_configure_join: bool,
}

pub fn toolbar_layout(k: ToolbarCaps) -> Vec<ToolbarButton> {
    use ToolbarButton::*;
    let mut v = Vec::new();
    if k.expanded {
        if k.dev {
            v.push(CopyId);
        }
        v.push(CopyLink);
        if k.can_configure_join {
            v.push(Configure);
        }
        v.extend([MarkUnread, Bookmark]);
        if k.can_pin {
            v.push(Pin);
        }
        if k.can_start_thread {
            v.push(Thread);
        }
        if k.own {
            v.push(ReplySelf);
        }
    } else {
        v.push(Quick);
    }
    v.push(AddReaction);
    if !k.own {
        v.push(Reply);
    }
    if k.can_edit {
        v.push(Edit);
    }
    v.push(Forward);
    if !k.can_start_thread && k.can_view_thread {
        v.push(ViewThread);
    }
    if k.can_publish {
        v.push(Publish);
    }
    if k.can_delete && k.expanded {
        v.push(Delete);
    } else {
        v.push(More);
    }
    v
}

pub const CROSSPOSTED: u64 = 1;

pub fn toolbar(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, shift: bool) -> gpui::AnyElement {
    use crate::ui::tooltip::Tipped as _;
    let (ch, id) = (msg.channel, msg.id);
    let own = msg.author == app.me.id;
    let e = entity.clone();
    let local = super::pending::is_local(msg);
    let mut bar = div()
        .absolute()
        .right(z(14.))
        .top(z(-16.))
        .invisible()
        .group_hover("msg", |s| s.visible())
        .when(app.chat.menu.is_some_and(|m| m.msg == id), |d| d.visible())
        .flex()
        .items_center()
        .p(z(2.))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(tk(idx::BORDER_MUTED))
        .rounded(z(8.))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.14),
            offset: gpui::point(z(0.), z(1.)),
            blur_radius: z(4.),
            spread_radius: z(0.),
            inset: false,
        }]);
    if local {
        if msg.send_state != crate::model::SendState::Failed {
            return div().into_any_element();
        }
        let e2 = e.clone();
        return bar
            .child(tool_button("tb-retry", id.0, ChatIcon::Retry.path(), 20.).tip(crate::t!("tb.retry")).on_click(move |_, _, cx| e2.update(cx, |a, cx| a.retry_send(ch, id, cx))))
            .child(tool_button("tb-discard", id.0, ChatIcon::Trash.path(), 20.).tip(crate::t!("tb.delete")).on_click(move |_, _, cx| e.update(cx, |a, cx| a.discard_local(ch, id, cx))))
            .into_any_element();
    }
    let channel = app.channels.get(&ch);
    let guild = channel.and_then(|c| c.guild);
    let manage = crate::actions::has(app, guild, crate::actions::MANAGE_MESSAGES);
    let announcement = channel.is_some_and(|c| c.kind == crate::model::ChannelKind::Announcement);
    let crossposted = msg.flags & CROSSPOSTED != 0;
    let caps = ToolbarCaps {
        expanded: shift,
        dev: app.prefs.synced.developer_mode,
        own,
        can_edit: app.can_edit(msg),
        can_pin: guild.is_none() || crate::actions::has(app, guild, crate::actions::MANAGE_MESSAGES | (1 << 51)),
        can_start_thread: guild.is_some() && msg.thread.is_none(),
        can_view_thread: msg.thread.is_some(),
        can_delete: own || manage,
        can_publish: announcement && (own || manage) && guild.is_some_and(|g| app.backend.guild_features(g).iter().any(|f| f == "NEWS")),
        can_configure_join: guild.is_some() && msg.kind == crate::model::MessageKind::Join && crate::actions::has(app, guild, crate::actions::MANAGE_GUILD),
    };
    let bookmarked = app.parity.inbox.saved.iter().any(|m| m.id == id);
    for b in toolbar_layout(caps) {
        use ToolbarButton as B;
        let ee = e.clone();
        let el: AnyElement = match b {
            B::Quick => {
                for (i, q) in app.chat.quick_reactions_for(guild, app.backend.has_premium(), 3).into_iter().enumerate() {
                    let e = e.clone();
                    bar = bar.child(
                        bar_button(("quick", i))
                            .child(crate::media::emoji::emoji_element(&q.emoji, 20.))
                            .on_click(move |_, _, cx| e.update(cx, |a, cx| a.react_with(ch, id, q.emoji.clone(), q.id, cx))),
                    );
                }
                div().w(z(1.)).h(z(24.)).my(z(2.)).mx(z(4.)).rounded_full().bg(tk(idx::BORDER_SUBTLE)).into_any_element()
            }
            B::CopyId => tool_button("tb-copy-id", id.0, ChatIcon::IdBadge.path(), 20.)
                .tip(crate::t!("msg.copy_id"))
                .on_click(move |_, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(id.0.to_string())))
                .into_any_element(),
            B::CopyLink => tool_button("tb-copy-link", id.0, ChatIcon::Link.path(), 20.)
                .tip(crate::t!("tb.copy_link"))
                .on_click(move |_, _, cx| ee.update(cx, |a, cx| a.copy_message_link(ch, id, cx)))
                .into_any_element(),
            B::Configure => tool_button("tb-configure", id.0, crate::icons::path("help"), 20.)
                .tip(crate::t!("tb.configure"))
                .on_click(move |_, _, cx| {
                    if let Some(g) = guild {
                        ee.update(cx, |a, cx| a.open_server_settings(g, crate::ui::modals::server_settings::section_index("menu.gs.engagement"), cx));
                    }
                })
                .into_any_element(),
            B::MarkUnread => tool_button("tb-unread", id.0, ChatIcon::Unread.path(), 20.)
                .tip(crate::t!("tb.mark_unread"))
                .on_click(move |_, _, cx| ee.update(cx, |a, cx| a.mark_unread_from(ch, id, cx)))
                .into_any_element(),
            B::Bookmark => tool_button("tb-bookmark", id.0, crate::icons::path(if bookmarked { "inbox/bookmark" } else { "menu/bookmark" }), 20.)
                .tip(if bookmarked { crate::t!("tb.remove_bookmark") } else { crate::t!("msg.bookmark") })
                .on_click(move |_, _, cx| ee.update(cx, |a, cx| if bookmarked { a.unsave(ch, id, cx) } else { a.save_message(ch, id, cx) }))
                .into_any_element(),
            B::Pin => tool_button("tb-pin", id.0, Icon::Pin.path(), 20.)
                .tip(if msg.pinned { crate::t!("msg.unpin") } else { crate::t!("msg.pin") })
                .on_click(move |ev, _, cx| {
                    let skip = ev.modifiers().shift;
                    ee.update(cx, |a, cx| a.request_pin(ch, id, skip, cx))
                })
                .into_any_element(),
            B::Thread => tool_button("tb-thread", id.0, crate::icons::path("chat/thread"), 20.)
                .tip(crate::t!("msg.create_thread"))
                .on_click(move |_, w, cx| ee.update(cx, |a, cx| a.open_create_thread_from(ch, id, w, cx)))
                .into_any_element(),
            B::ReplySelf | B::Reply => tool_button(if b == B::Reply { "tb-reply" } else { "tb-reply-self" }, id.0, Icon::Reply.path(), 20.)
                .tip(crate::t!("msg.reply"))
                .on_click(move |_, w, cx| ee.update(cx, |a, cx| a.start_reply(id, w, cx)))
                .into_any_element(),
            B::AddReaction => tool_button("tb-react", id.0, ChatIcon::AddReaction.path(), 20.)
                .tip(crate::t!("msg.add_reaction"))
                .on_click(move |_, _, cx| ee.update(cx, |a, cx| a.open_emoji_picker(EmojiTarget::Reaction { channel: ch, msg: id }, cx)))
                .into_any_element(),
            B::Edit => tool_button("tb-edit", id.0, ChatIcon::Edit.path(), 18.)
                .tip(crate::t!("tb.edit"))
                .on_click(move |_, w, cx| ee.update(cx, |a, cx| a.start_edit(id, w, cx)))
                .into_any_element(),
            B::Forward => tool_button("tb-forward", id.0, crate::icons::path("chat/forwarded"), 20.)
                .tip(crate::t!("msg.forward"))
                .on_click(move |_, _, cx| ee.update(cx, |a, cx| a.open_forward(ch, id, cx)))
                .into_any_element(),
            B::ViewThread => {
                let t = msg.thread.clone();
                tool_button("tb-view-thread", id.0, crate::icons::path("chat/thread"), 20.)
                    .tip(crate::t!("tb.view_thread"))
                    .on_click(move |_, w, cx| {
                        if let Some(t) = t.clone() {
                            ee.update(cx, |a, cx| a.open_thread(t, w, cx));
                        }
                    })
                    .into_any_element()
            }
            B::Publish if crossposted => tool_button("tb-publish", id.0, crate::icons::path("megaphone"), 20.)
                .tip(crate::t!("tb.published"))
                .opacity(0.5)
                .cursor_not_allowed()
                .into_any_element(),
            B::Publish => tool_button("tb-publish", id.0, crate::icons::path("megaphone"), 20.)
                .tip(crate::t!("tb.publish"))
                .on_click(move |_, _, cx| ee.update(cx, |a, cx| a.blocked_write(crate::actions::guard::GuardedWrite::crosspost(ch.0, id.0), cx)))
                .into_any_element(),
            B::Delete => bar_button("tb-delete")
                .child(icon(ChatIcon::Trash, 20., tk(idx::CONTROL_CRITICAL_SECONDARY_TEXT_DEFAULT)))
                .tip(crate::t!("tb.delete"))
                .on_click(move |ev, _, cx| {
                    let skip = ev.modifiers().shift;
                    ee.update(cx, |a, cx| a.request_delete(ch, id, skip, cx))
                })
                .into_any_element(),
            B::More => tool_button("tb-more", id.0, ChatIcon::More.path(), 20.)
                .tip(crate::t!("tb.more"))
                .on_click(move |ev, _, cx| {
                    let pos = ev.position();
                    ee.update(cx, |a, cx| a.open_message_menu(ch, id, pos, cx))
                })
                .into_any_element(),
        };
        bar = bar.child(el);
    }
    bar.into_any_element()
}

pub fn on_right_click(entity: &Entity<PhoenixApp>, msg: &Message) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
    let e = entity.clone();
    let (ch, id) = (msg.channel, msg.id);
    move |ev, _, cx| {
        cx.stop_propagation();
        let pos = ev.position;
        e.update(cx, |a, cx| a.open_message_menu(ch, id, pos, cx));
    }
}

pub fn reactions(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, after_accessory: bool) -> AnyElement {
    let (ch, id) = (msg.channel, msg.id);
    let hc = crate::theme::high_contrast();
    let mut row = div().pt(z(if after_accessory { 8. } else { 4. })).pb(z(10.)).mb(z(-8.)).flex().flex_wrap().gap(z(4.)).items_center();
    for (ix, r) in msg.reactions.iter().enumerate() {
        if r.burst_count > 0 {
            row = row.child(burst_pill(app, entity, msg, ix, r));
        }
        if r.count > 0 {
            row = row.child(normal_pill(app, entity, msg, ix, r));
        }
    }
    let e = entity.clone();
    row.child(
        div()
            .id("reaction-add")
            .py(z(5.))
            .px(z(7.))
            .rounded(z(8.))
            .border_1()
            .border_color(if hc { tk(idx::BORDER_SUBTLE) } else { gpui::transparent_black() })
            .flex()
            .items_center()
            .bg(tk(idx::REACTION_BACKGROUND_DEFAULT))
            .cursor_pointer()
            .hover(move |s| {
                s.bg(tk(idx::REACTION_BACKGROUND_HOVER))
                    .border_color(tk(if hc { idx::BORDER_STRONG } else { idx::REACTION_BORDER_HOVER }))
            })
            .active(|s| s.bg(tk(idx::REACTION_BACKGROUND_ACTIVE)).border_color(tk(idx::REACTION_BORDER_ACTIVE)))
            .child(icon(ChatIcon::AddReaction, 18., tk(idx::REACTION_TEXT_DEFAULT)))
            .on_click(move |_, _, cx| {
                e.update(cx, |a, cx| a.open_emoji_picker(EmojiTarget::Reaction { channel: ch, msg: id }, cx))
            }),
    )
    .into_any_element()
}

fn count_label(n: u32) -> gpui::Div {
    div().text_size(z(16.)).line_height(z(16.)).font_weight(FontWeight::SEMIBOLD).w(z(12. * n.to_string().len() as f32)).text_center()
}

fn normal_pill(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, ix: usize, r: &crate::model::Reaction) -> AnyElement {
    let (ch, id) = (msg.channel, msg.id);
    let hc = crate::theme::high_contrast();
    let e = entity.clone();
    let rc = r.clone();
    let emoji_el = div().my(z(2.)).child(crate::media::emoji::emoji_element(&r.emoji, 20.));
    let fade_key = super::burst_tip::pill_key(ch, id, r, false);
    let tip = super::burst_tip::layer(app, entity, msg, r, false);
    let e_hover = entity.clone();
    let rh = r.clone();
    div().id(("reaction", ix))
        .map(|d| crate::ui::a11y::named(d, gpui::Role::Button, || crate::ui::a11y::reaction(&r.emoji, r.count, r.me)))
        .relative()
        .py(z(2.))
        .px(z(6.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .gap(z(6.))
        .border_1()
        .when(r.me, |d| {
            if crate::theme::is_light() {
                d.bg(tk(idx::BRAND_160)).border_color(tk(idx::BRAND_500))
            } else {
                d.bg(tk(idx::REACTION_BACKGROUND_REACTED_DEFAULT)).border_color(tk(idx::REACTION_BORDER_REACTED_DEFAULT))
            }
        })
        .when(!r.me, |d| {
            d.child(crate::ui::states::fade_layer_pressed(
                fade_key,
                tk(idx::REACTION_BACKGROUND_DEFAULT),
                tk(idx::REACTION_BACKGROUND_HOVER),
                Some(("reaction", tk(idx::REACTION_BACKGROUND_ACTIVE))),
                100,
                8.,
            ))
            .border_color(if hc { tk(idx::BORDER_SUBTLE) } else { gpui::transparent_black() })
            .hover(move |s| s.border_color(tk(if hc { idx::BORDER_STRONG } else { idx::REACTION_BORDER_HOVER })))
        })
        .active(|s| s.bg(tk(idx::REACTION_BACKGROUND_ACTIVE)).border_color(tk(idx::REACTION_BORDER_ACTIVE)))
        .cursor_pointer()
        .group("reaction")
        .child(super::burst_tip::probe(fade_key))
        .child(emoji_el)
        .child(
            count_label(r.count)
                .text_color(tk(match (r.me, crate::theme::is_light()) {
                    (true, true) => idx::BRAND_560,
                    (true, false) => idx::REACTION_TEXT_REACTED_DEFAULT,
                    _ => idx::TEXT_SUBTLE,
                }))
                .group_hover("reaction", |s| s.text_color(tk(idx::REACTION_TEXT_HOVER)))
                .id("count")
                .group_active("reaction", |s| s.text_color(tk(idx::REACTION_TEXT_ACTIVE)))
                .child(SharedString::from(r.count.to_string())),
        )
        .children(tip)
        .on_hover(move |on, window, cx| {
            if crate::ui::states::set_hover(fade_key, *on) {
                window.refresh();
                super::burst_tip::on_pill_hover(&e_hover, ch, id, &rh, false, *on, cx);
            }
        })
        .on_click(move |_, window, cx| {
            super::burst_tip::dismiss(fade_key);
            window.refresh();
            e.update(cx, |a, cx| a.toggle_reaction(ch, id, &rc, cx));
        })
        .into_any_element()
}

fn burst_pill(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, ix: usize, r: &crate::model::Reaction) -> AnyElement {
    use crate::ui::burst;
    use crate::ui::chat::burst_tip;
    let (ch, id) = (msg.channel, msg.id);
    let hc = crate::theme::high_contrast();
    let pal = burst::palette(&r.burst_colors, !crate::theme::is_light());
    let key = burst::Key::new(ch, id, &r.emoji, r.emoji_id);
    if let Some(p) = pal {
        burst::seen(&key, &r.emoji, r.emoji_id, p.accent, r.burst_count);
    }
    let playing = burst::playing(&key);
    let hide = playing.is_some();
    let emoji_el = div()
        .my(z(2.))
        .children(playing.as_ref().map(|(img, at)| burst::overlay(img.clone(), *at, 20.)))
        .child(div().when(hide, |d| d.opacity(0.)).child(crate::media::emoji::emoji_element(&r.emoji, 20.)));
    let fade_key = burst_tip::pill_key(ch, id, r, true);
    let hovered = crate::ui::states::hovered(fade_key);
    let (e_hover, e_click) = (entity.clone(), entity.clone());
    let (rh, rc) = (r.clone(), r.clone());
    let tip = burst_tip::layer(app, entity, msg, r, true);
    let accent = pal.map(|p| p.accent);
    let me_burst = r.me_burst;
    let shake = match playing.as_ref().map(|(_, at)| at.elapsed().as_secs_f32() / 0.5) {
        Some(t) if t < 1. && !crate::ui::settings::effects::reduced_motion() => Some(burst::shake_at(t)),
        _ => None,
    };
    let (fill, edge): (gpui::Hsla, gpui::Hsla) = match pal {
        Some(p) => (
            p.background_rgba(p.opacity).into(),
            if me_burst {
                p.background_rgba(1.).into()
            } else if hovered {
                tk(if hc { idx::BORDER_STRONG } else { idx::REACTION_BORDER_HOVER })
            } else if hc {
                tk(idx::BORDER_SUBTLE)
            } else {
                gpui::transparent_black()
            },
        ),
        None if me_burst => (tk(idx::REACTION_BACKGROUND_REACTED_DEFAULT), tk(idx::REACTION_BORDER_REACTED_DEFAULT)),
        None if hovered => (tk(idx::REACTION_BACKGROUND_HOVER), tk(idx::REACTION_BORDER_HOVER)),
        None => (tk(idx::REACTION_BACKGROUND_DEFAULT), gpui::transparent_black()),
    };
    let pill = div()
        .id(("reaction-burst", ix))
        .map(|d| crate::ui::a11y::named(d, gpui::Role::Button, || crate::ui::a11y::super_reaction(&r.emoji, r.burst_count, me_burst)))
        .relative()
        .py(z(2.))
        .px(z(6.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .gap(z(6.))
        .border_1()
        .cursor_pointer()
        .child(burst_tip::probe(fade_key))
        .children(pal.map(|p| burst::glow(p.accent_rgba(0.35))))
        .map(|d| match shake {
            Some((_, _, deg)) => d.border_color(gpui::transparent_black()).child(burst::tilted(fill, edge, deg)),
            None => d.bg(fill).border_color(edge),
        })
        .child(emoji_el)
        .child(
            count_label(r.burst_count)
                .map(|d| match pal {
                    Some(p) => d.text_color(p.accent_rgba(1.)),
                    None => d.text_color(tk(if me_burst { idx::REACTION_TEXT_REACTED_DEFAULT } else { idx::TEXT_SUBTLE })),
                })
                .child(SharedString::from(r.burst_count.to_string())),
        )
        .children(tip)
        .on_hover(move |on, window, cx| {
            if crate::ui::states::set_hover(fade_key, *on) {
                window.refresh();
                burst_tip::on_pill_hover(&e_hover, ch, id, &rh, true, *on, cx);
            }
            if *on && let Some(accent) = accent {
                let rh = rh.clone();
                e_hover.update(cx, |_, cx| burst::play(burst::Play { channel: ch, msg: id, token: &rh.emoji, emoji_id: rh.emoji_id, accent, kind: burst::Kind::Hover }, cx));
            }
        })
        .on_click(move |_, window, cx| {
            burst_tip::dismiss(fade_key);
            window.refresh();
            e_click.update(cx, |a, cx| a.toggle_burst_reaction(ch, id, &rc, cx));
        });
    match shake {
        Some((dx, dy, _)) => pill.left(z(dx)).top(z(dy)).into_any_element(),
        None => pill.into_any_element(),
    }
}

pub fn edit_box(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, cx: &App) -> Option<Div> {
    let ed = app.chat.edit.as_ref().filter(|e| e.id == msg.id)?;
    let input = ed.input.clone();
    let len = input.read(cx).value().chars().count() as i64;
    let (remaining, show_count) = super::composer::char_count(app.backend.premium_tier2(), len);
    let emoji_open = matches!(crate::ui::pickers::open_tab(cx), Some((_, crate::ui::pickers::EmojiTarget::Edit)));
    let fz = |v: f32| z(v * crate::theme::font_scale());
    let pad_y = ((56. - 22. * crate::theme::font_scale()) / 2.).max(0.);
    let e_cancel = entity.clone();
    let e_emoji = entity.clone();
    let area = div()
        .id("edit-box")
        .mt(z(8.))
        .relative()
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::BORDER_MUTED))
        .bg(tk(idx::CHAT_BACKGROUND_DEFAULT))
        .child(crate::ui::autocomplete::edit_box_probe())
        .on_mouse_down(gpui::MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .capture_key_down(move |ev, w, cx| {
            if ev.keystroke.key == "escape" && !ev.keystroke.modifiers.shift {
                cx.stop_propagation();
                e_cancel.update(cx, |a, cx| a.cancel_edit(w, cx));
            }
        })
        .child(
            div()
                .relative()
                .min_h(z(if show_count { 88. } else { 56. }))
                .pl(z(15.))
                .flex()
                .items_start()
                .when(show_count, |d| {
                    d.child(
                        div()
                            .absolute()
                            .right(z(14.))
                            .bottom(z(10.))
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(tk(if remaining < 0 { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_DEFAULT }))
                            .child(SharedString::from(remaining.to_string())),
                    )
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .pr(z(10.))
                        .py(z(pad_y))
                        .text_size(fz(16.))
                        .line_height(fz(22.))
                        .text_color(c(Palette::TEXT_NORMAL))
                        .child(
                            div().mx(z(-4.)).child(
                                gpui_component::input::Textarea::new(&input)
                                    .token(super::rich_input::render_token)
                                    .context_menu({
                                        let area = input.clone();
                                        move |_, w, _| {
                                            crate::ui::extra_menus::open_textarea(&area, w);
                                            gpui_component::native_menu::NativeMenu::new()
                                        }
                                    })
                                    .appearance(false)
                                    .bordered(false)
                                    .xsmall()
                                    .text_size(fz(16.))
                                    .line_height(fz(22.)),
                            ),
                        ),
                )
                .child(
                    div().mr(z(10.4)).h(z(56.)).flex().items_center().child(
                        crate::ui::states::composer_button_ext("edit-emoji", Icon::Emoji, 18., true)
                            .when(emoji_open, |d| d.bg(c(crate::theme::tok(idx::INTERACTIVE_BACKGROUND_SELECTED))))
                            .on_click(move |_, _, cx| e_emoji.update(cx, |a, cx| a.open_emoji_picker(EmojiTarget::Edit, cx))),
                    ),
                ),
        );
    Some(div().flex().flex_col().min_w_0().child(area).child(operations(entity, |a, w, cx| a.cancel_edit(w, cx), |a, w, cx| a.save_edit(w, cx))))
}

fn operations(
    entity: &Entity<PhoenixApp>,
    cancel: impl Fn(&mut PhoenixApp, &mut Window, &mut gpui::Context<PhoenixApp>) + 'static,
    save: impl Fn(&mut PhoenixApp, &mut Window, &mut gpui::Context<PhoenixApp>) + 'static,
) -> Div {
    let e1 = entity.clone();
    let e2 = entity.clone();
    let link = |id: &'static str, label: gpui::SharedString| {
        div()
            .id(id)
            .text_color(tk(idx::TEXT_LINK))
            .cursor_pointer()
            .hover(|s| s.underline())
            .child(label)
    };
    div()
        .py(z(7.))
        .flex()
        .text_size(z(12.))
        .line_height(z(12.))
        .font_weight(gpui::FontWeight::NORMAL)
        .text_color(tk(idx::TEXT_DEFAULT))
        .whitespace_nowrap()
        .child(crate::t!("edit.escape_to"))
        .child(link("edit-cancel", crate::t!("edit.cancel")).on_click(move |_, w, cx| e1.update(cx, |a, cx| cancel(a, w, cx))))
        .child(crate::t!("edit.enter_to"))
        .child(link("edit-save", crate::t!("edit.save")).on_click(move |_, w, cx| e2.update(cx, |a, cx| save(a, w, cx))))
}

pub fn edit_frame(
    input: &Entity<gpui_component::input::InputState>,
    entity: &Entity<PhoenixApp>,
    cancel: impl Fn(&mut PhoenixApp, &mut Window, &mut gpui::Context<PhoenixApp>) + Clone + 'static,
    save: impl Fn(&mut PhoenixApp, &mut Window, &mut gpui::Context<PhoenixApp>) + 'static,
) -> Div {
    let cancel2 = cancel.clone();
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .child(
            div()
                .id("edit-box")
                .mt(z(8.))
                .min_h(z(56.))
                .px(z(16.))
                .rounded(z(8.))
                .border_1()
                .border_color(tk(idx::BORDER_SUBTLE))
                .bg(tk(idx::CHAT_BACKGROUND_DEFAULT))
                .flex()
                .items_center()
                .text_size(z(16.))
                .text_color(c(Palette::TEXT_NORMAL))
                .capture_key_down({
                    let e = entity.clone();
                    move |ev, w, cx| {
                        if ev.keystroke.key == "escape" {
                            cx.stop_propagation();
                            let cancel = cancel.clone();
                            e.update(cx, |a, cx| cancel(a, w, cx));
                        }
                    }
                })
                .child(div().flex_1().min_w_0().child(Input::new(input).appearance(false).bordered(false)))
                .child(ui::icon(Icon::Emoji, 22., c(Palette::INTERACTIVE))),
        )
        .child(operations(entity, cancel2, save))
}

pub fn thread_chip(app: &PhoenixApp, entity: &Entity<PhoenixApp>, t: &ThreadRef, system: bool) -> AnyElement {
    let e = entity.clone();
    let t2 = t.clone();
    let recent = crate::thread_recent::get(t.id);
    let count = recent.as_ref().map_or(t.message_count, |_| t.message_count.max(1));
    let group = SharedString::from(format!("thread-card-{}", t.id.0));
    let one = if crate::i18n::locale() == "fr" { count < 2 } else { count == 1 };
    let cta = if one { crate::t!("thread.one_message_cta", n = count) } else { crate::t!("thread.messages_cta", n = count) };
    let bottom = recent.map(|r| {
        let img = app.users.get(&r.author).map_or_else(|| r.avatar.clone(), |u| u.avatar.clone());
        let now = chrono::Utc::now().timestamp();
        div()
            .mt(z(2.))
            .h(z(18.))
            .flex()
            .items_center()
            .text_size(z(14.))
            .line_height(z(18.))
            .text_color(tk(idx::TEXT_DEFAULT))
            .child(
                div()
                    .flex_initial()
                    .min_w_0()
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .child(div().mr(z(8.)).child(ui::avatar_ref(img, 16.)))
                    .child(div().flex_none().mr(z(4.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_STRONG)).opacity(0.64).child(r.name.clone()))
                    .child(div().min_w_0().text_ellipsis().whitespace_nowrap().child(SharedString::from(r.content.replace('\n', " ")))),
            )
            .child(div().flex_none().ml(z(8.)).text_color(tk(idx::TEXT_MUTED)).child(crate::thread_recent::ago(now - r.ts)))
    });
    div()
        .id(("thread-chip", t.id.0))
        .group(group.clone())
        .when(!system, |d| d.mt(z(8.)))
        .max_w(z(480.))
        .p(z(8.))
        .rounded(z(4.))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .flex()
        .flex_col()
        .cursor_pointer()
        .child(
            div()
                .flex()
                .h(z(18.))
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(div().min_w_0().mr(z(8.)).text_ellipsis().whitespace_nowrap().text_color(tk(idx::TEXT_STRONG)).child(t.name.clone()))
                .child(div().flex_none().text_color(tk(idx::TEXT_LINK)).group_hover(group, |s| s.underline()).child(cta)),
        )
        .children(bottom)
        .on_click(move |_, w, cx| {
            let t = t2.clone();
            e.update(cx, |a, cx| a.open_thread_link(t, w, cx));
        })
        .into_any_element()
}

pub fn thread_spine(top: f32) -> Div {
    let mh = crate::theme::space::md();
    let av = super::msg_avatar_size();
    div()
        .absolute()
        .left(z(mh + av / 2. - 1.))
        .top(crate::theme::fz(top))
        .bottom(z(29.))
        .w(z(super::msg_left() - mh - av / 2. - 4.))
        .border_l_2()
        .border_b_2()
        .rounded_bl(z(8.))
        .border_color(tk(idx::SPINE_DEFAULT))
}

pub fn new_divider(before_group: bool) -> AnyElement {
    let line = div().ml(z(16.)).mr(z(14.)).h(z(1.)).bg(tk(idx::BACKGROUND_FEEDBACK_NOTIFICATION)).relative().child(unread_end_cap());
    if before_group {
        div().h_0().relative().child(line.absolute().left_0().right_0().top(z(8.))).into_any_element()
    } else {
        line.my(z(4.)).into_any_element()
    }
}

pub fn unread_end_cap() -> gpui::Div {
    let red = tk(idx::BACKGROUND_FEEDBACK_NOTIFICATION);
    div()
        .absolute()
        .right_0()
        .top(z(-7.))
        .h(z(13.))
        .pl(z(1.))
        .pr(z(4.))
        .rounded_r(z(4.))
        .bg(red)
        .text_size(z(10.))
        .line_height(z(9.))
        .font_weight(FontWeight::BOLD)
        .text_color(tk(idx::WHITE))
        .flex()
        .items_center()
        .justify_center()
        .child(gpui::svg().path(crate::icons::path("chat/unread-cap")).absolute().left(z(-8.)).top_0().w(z(8.)).h(z(13.)).text_color(red))
        .child(crate::t!("chat.new").to_uppercase())
}

pub fn open_author(entity: &Entity<PhoenixApp>, msg: &Message) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
    open_user(entity, msg.author, msg.channel)
}

pub fn open_user(
    entity: &Entity<PhoenixApp>,
    user: crate::model::Id,
    ch: crate::model::Id,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
    let e = entity.clone();
    move |ev, w, cx| {
        cx.stop_propagation();
        let pos = ev.position();
        e.update(cx, |a, cx| {
            let guild = a.channels.get(&ch).and_then(|c| c.guild);
            a.open_profile(user, guild, pos, w, cx)
        });
    }
}

pub fn open_author_menu(entity: &Entity<PhoenixApp>, msg: &Message) -> impl Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static {
    open_user_menu(entity, msg.author, msg.channel)
}

pub fn open_user_menu(
    entity: &Entity<PhoenixApp>,
    user: crate::model::Id,
    ch: crate::model::Id,
) -> impl Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static {
    let e = entity.clone();
    move |ev, w, cx| {
        cx.stop_propagation();
        let guild = e.read(cx).channels.get(&ch).and_then(|c| c.guild);
        crate::ui::user_menu::open(user, guild, ev.position, w);
    }
}

pub(crate) fn words(text: &str, range: std::ops::Range<usize>) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut start = range.start;
    let mut in_space = false;
    for (i, ch) in text[range.clone()].char_indices() {
        let i = range.start + i;
        if ch.is_whitespace() {
            in_space = true;
        } else if in_space {
            out.push(start..i);
            start = i;
            in_space = false;
        }
    }
    if start < range.end {
        out.push(start..range.end);
    }
    out
}

pub(crate) fn has_code(t: &crate::richtext::RichText) -> bool {
    let mono = gpui::font(crate::theme::FONT_MONO()).family;
    t.runs.iter().any(|r| r.font.family == mono && r.background_color != Some(r.color))
}

pub(crate) fn code_ranges(t: &crate::richtext::RichText) -> Vec<std::ops::Range<usize>> {
    let mono = gpui::font(crate::theme::FONT_MONO()).family;
    let mut out: Vec<std::ops::Range<usize>> = Vec::new();
    let mut pos = 0;
    for r in &t.runs {
        let (s, e) = (pos, pos + r.len);
        pos = e;
        if r.font.family != mono || r.background_color == Some(r.color) {
            continue;
        }
        match out.last_mut() {
            Some(last) if last.end == s => last.end = e,
            _ => out.push(s..e),
        }
    }
    out
}

pub(crate) fn split_code(code: &[std::ops::Range<usize>], range: std::ops::Range<usize>) -> Vec<(std::ops::Range<usize>, bool)> {
    let mut out = Vec::new();
    let mut at = range.start;
    for c in code {
        let (a, b) = (c.start.max(range.start), c.end.min(range.end));
        if a >= b {
            continue;
        }
        if at < a {
            out.push((at..a, false));
        }
        out.push((a..b, true));
        at = b;
    }
    if at < range.end {
        out.push((at..range.end, false));
    }
    out
}

fn code_box(open: bool, close: bool, k: f32, child: AnyElement) -> Div {
    let fs = 16. * 0.85 * k;
    let pad = 0.2 * fs;
    let b = div()
        .h(z(19.2 * k))
        .flex()
        .items_center()
        .text_size(z(fs))
        .line_height(z(17.6 * k))
        .bg(tk(idx::BACKGROUND_CODE))
        .border_color(tk(idx::BORDER_NORMAL))
        .border_t(z(1.))
        .border_b(z(1.))
        .when(open, |d| d.border_l(z(1.)).rounded_l(z(4.)).pl(z(pad)))
        .when(close, |d| d.border_r(z(1.)).rounded_r(z(4.)).pr(z(pad)))
        .child(child);
    div().flex_none().max_w_full().h(z(22. * k)).flex().items_center().child(b)
}

pub(crate) fn slice_runs(runs: &[gpui::TextRun], range: std::ops::Range<usize>) -> Vec<gpui::TextRun> {
    let mut out = Vec::new();
    let mut pos = 0;
    for r in runs {
        let (s, e) = (pos, pos + r.len);
        pos = e;
        let (a, b) = (s.max(range.start), e.min(range.end));
        if a < b {
            let mut n = r.clone();
            n.len = b - a;
            out.push(n);
        }
    }
    out
}

pub fn rich_with_emoji_scaled(
    entity: &Entity<PhoenixApp>,
    msg: crate::model::Id,
    bi: usize,
    t: &crate::richtext::RichText,
    k: f32,
) -> AnyElement {
    let jumbo = crate::richtext::is_jumbo_text(t);
    let sel_bi = super::selection::BODY_BASE + bi;
    super::selection::note(msg.0, sel_bi, t.text.clone(), &t.emoji);
    let k = k * crate::theme::font_scale();
    let size = k * if jumbo { 48. } else { 22. };
    let mut row = div().flex().flex_wrap().items_center().text_size(z(16. * k)).line_height(z(22. * k));
    let mut last = 0;
    let mut seg = 0usize;
    let code = code_ranges(t);
    let mentions = t.mentions();
    let hovered = super::mention::hovered_pill(msg, bi).and_then(|s| mentions.iter().find(|(r, _)| r.start == s));
    let mut runs = match hovered {
        Some((r, m)) => super::mention::pill_runs(&t.runs, r, matches!(m, crate::markdown::Mention::Role(_))),
        None => t.runs.clone(),
    };
    let pill_bg: Vec<Option<gpui::Hsla>> = mentions.iter().map(|(r, _)| run_bg_at(&runs, r.start)).collect();
    clear_bg(&mut runs, mentions.iter().map(|(r, _)| r.clone()));
    let runs = &runs;
    let pill = |child: AnyElement, range: &std::ops::Range<usize>, part: u64| -> AnyElement {
        let Some(i) = mentions.iter().position(|(r, _)| r.start <= range.start && range.end <= r.end) else {
            return child;
        };
        let (r, m) = mentions[i].clone();
        let (first, last) = (range.start == r.start, range.end == r.end);
        let e = entity.clone();
        let start = r.start;
        div()
            .id(("pill", msg.0.wrapping_mul(31).wrapping_add(((bi as u64) << 32) | part)))
            .flex_none()
            .h(z(22. * k))
            .flex()
            .items_center()
            .cursor_pointer()
            .on_hover(move |h, w, _| super::mention::hover_pill(msg, bi, start, part, *h, w))
            .on_click(move |_, w, cx| super::mention::click(&e, &m, w, cx))
            .child(mention_box(child, pill_bg[i], first, last, k))
            .into_any_element()
    };
    let push_text = |row: Div, range: std::ops::Range<usize>, seg: &mut usize, code: Option<(bool, bool)>| -> Div {
        if range.is_empty() {
            return row;
        }
        let text: SharedString = t.text[range.clone()].to_owned().into();
        let mut seg_runs = slice_runs(runs, range.clone());
        if code.is_some() {
            let bg = Some(c(crate::theme::tok(idx::BACKGROUND_CODE)));
            for r in seg_runs.iter_mut().filter(|r| r.background_color == bg) {
                r.background_color = None;
            }
        }
        let seg_runs = super::selection::highlight(msg.0, sel_bi, range.start, range.len(), seg_runs);
        let mut ranges = Vec::new();
        let mut targets = Vec::new();
        for (cr, tg) in t.click_ranges.iter().zip(t.click_targets.iter()) {
            if matches!(tg, crate::richtext::ClickTarget::Mention(_)) {
                continue;
            }
            if cr.start < range.end && cr.end > range.start {
                ranges.push(cr.start.max(range.start) - range.start..cr.end.min(range.end) - range.start);
                targets.push(tg.clone());
            }
        }
        *seg += 1;
        let part = *seg as u64;
        let cell = |child: AnyElement| match code {
            None => div().flex_none().max_w_full().child(child),
            Some((open, close)) => code_box(open, close, k, child),
        };
        if ranges.is_empty() {
            let st = gpui::StyledText::new(text).with_runs(seg_runs);
            let probe = super::selection::probe(st.layout().clone(), msg.0, sel_bi, range.start, range.len());
            let el = cell(div().child(st).child(probe).into_any_element()).into_any_element();
            return row.child(pill(el, &range, part));
        }
        let sd = *seg as u64;
        let seg_runs = super::mention::hovered_runs(msg, bi, sd, &ranges, &targets, &seg_runs).unwrap_or(seg_runs);
        let seg_runs = super::mention::anchor_runs(msg, bi, range.start, &seg_runs).unwrap_or(seg_runs);
        let off = range.start;
        let scope = range.clone();
        let anchor = {
            let (fr, ft) = (t.click_ranges.clone(), t.click_targets.clone());
            move |ix: usize| super::mention::anchor_at(&fr, &ft, ix + off)
        };
        let shown: Vec<String> = ranges
            .iter()
            .map(|r| anchor(r.start).map_or_else(|| text.get(r.clone()).unwrap_or_default().to_owned(), |(a, _)| t.text.get(a).unwrap_or_default().to_owned()))
            .collect();
        let full = t.text.clone();
        let styled = gpui::StyledText::new(text).with_runs(seg_runs);
        let probe = super::selection::probe(styled.layout().clone(), msg.0, sel_bi, range.start, range.len());
        let e = entity.clone();
        let te = entity.clone();
        let (hr, ht) = (ranges.clone(), targets.clone());
        let hover_anchor = anchor.clone();
        let rte = msg.0.wrapping_mul(31).wrapping_add(((bi as u64) << 32) | *seg as u64);
        let inner = gpui::InteractiveText::new(("rte", rte), styled)
                .on_hover(move |ix, _ev, w, _cx| {
                    super::mention::on_hover(msg, bi, sd, &hr, &ht, ix, w);
                    super::mention::set_anchor(msg, bi, scope.clone(), ix.and_then(&hover_anchor).map(|a| a.0), w);
                })
                .tooltip(move |ix, w, cx| {
                    let (r, url) = anchor(ix)?;
                    Some(crate::ui::tooltip::tip(super::mention::link_tip(&te, full.get(r).unwrap_or_default(), &url, cx))(w, cx))
                })
                .on_click(
                ranges,
                move |ix, w, cx| match &targets[ix] {
                    crate::richtext::ClickTarget::Link(url) => e.update(cx, |a, cx| crate::ui::modals::leave_link::click(a, &shown[ix], url, w, cx)),
                    crate::richtext::ClickTarget::Spoiler(sp) => {
                        let sp = *sp;
                        e.update(cx, |a, cx| a.reveal_spoiler(msg, sp, cx));
                    }
                    crate::richtext::ClickTarget::Mention(m) => super::mention::click(&e, m, w, cx),
                },
            );
        let guarded = super::mention::leave_guard(("rte-leave", rte), msg, bi, Some(sd), range.clone(), inner);
        let el = cell(div().child(guarded).child(probe).into_any_element()).into_any_element();
        row.child(pill(el, &range, part))
    };
    let bounds: Vec<usize> = mentions.iter().flat_map(|(r, _)| [r.start, r.end]).collect();
    let push_lines = |mut row: Div, range: std::ops::Range<usize>, seg: &mut usize| -> Div {
        let mut start = range.start;
        for (i, piece) in t.text[range.clone()].split('\n').enumerate() {
            let r = start..start + piece.len();
            if i > 0 {
                row = row.child(div().w_full().h_0());
            }
            if piece.is_empty() && r.end < range.end && (i > 0 || range.start == 0) {
                row = row.child(div().w_full().h(z(22. * k)));
            }
            for (piece, in_code) in split_code(&code, r.clone()) {
                let ws: Vec<_> = split_at(&bounds, piece.clone()).into_iter().flat_map(|p| words(&t.text, p)).collect();
                let n = ws.len();
                for (wi, w) in ws.into_iter().enumerate() {
                    let edge = in_code.then(|| {
                        let span = code.iter().find(|c| c.start <= piece.start && piece.end <= c.end);
                        let open = wi == 0 && span.is_some_and(|c| c.start == piece.start);
                        let close = wi + 1 == n && span.is_some_and(|c| c.end == piece.end);
                        (open, close)
                    });
                    row = push_text(row, w, seg, edge);
                }
            }
            start = r.end + 1;
        }
        row
    };
    for (r, token) in &t.emoji {
        if r.start < last {
            continue;
        }
        row = push_lines(row, last..r.start, &mut seg);
        if let Some(name) = crate::richtext::channel_icon_name(token) {
            let icon = div()
                .h(z(22. * k))
                .flex()
                .items_center()
                .pr(z(4.))
                .child(gpui::svg().path(crate::icons::path(name)).flex_none().size(z(16. * k)).text_color(runs_fg_at(runs, r.end)));
            seg += 1;
            row = row.child(pill(icon.into_any_element(), r, seg as u64));
            last = r.end;
            continue;
        }
        if let Some(icon) = crate::richtext::parse_link_token(token) {
            let el = div().h(z(22. * k)).flex().items_center().child(link_icon(&icon, 16. * k, runs_fg_at(runs, r.end)));
            seg += 1;
            row = row.child(pill(el.into_any_element(), r, seg as u64));
            last = r.end;
            continue;
        }
        let target = match crate::media::emoji::parse_custom(token) {
            Some(e) => Some(super::panels::MenuItemTarget { sticker: false, id: e.id, image: Some(e.url(44)), fav: None }),
            None => super::panels::unicode_fav_key(token).map(|k| super::panels::MenuItemTarget { sticker: false, id: 0, image: None, fav: Some(k) }),
        };
        row = row.child(
            div()
                .mx(z(1.))
                .when(jumbo, |d| d.py(z(4.)))
                .when(super::selection::covers(msg.0, sel_bi, r), |d| d.bg(super::selection::selection_bg()))
                .when_some(target, |d, t| d.on_mouse_down(gpui::MouseButton::Right, move |_, _, _| super::panels::set_menu_item(t.clone())))
                .child(crate::media::emoji::emoji_element(token, size)),
        );
        last = r.end;
    }
    row = push_lines(row, last..t.text.len(), &mut seg);
    row.into_any_element()
}

pub(crate) fn mention_box(child: AnyElement, bg: Option<gpui::Hsla>, first: bool, last: bool, k: f32) -> Div {
    div()
        .h(z(MENTION_BOX_EM * 16. * k))
        .flex()
        .items_center()
        .when_some(bg, |d, b| d.bg(b))
        .when(first, |d| d.rounded_l(z(3.)).pl(z(2.)))
        .when(last, |d| d.rounded_r(z(3.)).pr(z(2.)))
        .child(child)
}

const MENTION_BOX_EM: f32 = 1.3;

pub(crate) fn link_icon(icon: &crate::markdown::LinkIcon, em: f32, fg: gpui::Hsla) -> AnyElement {
    use crate::markdown::LinkIcon;
    match icon {
        LinkIcon::Channel(name) => {
            div().flex_none().mr(z(4.)).child(gpui::svg().path(crate::icons::path(name)).size(z(em)).text_color(fg)).into_any_element()
        }
        LinkIcon::Chevron => div()
            .flex_none()
            .ml(z(4.))
            .mr(z(2.))
            .child(gpui::svg().path(crate::icons::path("chevron-right")).size(z(em * 0.5)).text_color(fg))
            .into_any_element(),
        LinkIcon::Guild(url) => div()
            .flex_none()
            .mr(z(3.))
            .child(gpui::img(SharedString::from(url.clone())).size(z(em)).rounded_full())
            .into_any_element(),
        LinkIcon::GuildText(acronym) => div()
            .flex_none()
            .mr(z(3.))
            .size(z(em * 1.05))
            .rounded_full()
            .bg(c(crate::theme::tok(idx::BACKGROUND_MOD_STRONG)))
            .flex()
            .items_center()
            .justify_center()
            .text_size(z(8.))
            .line_height(z(em * 1.05))
            .child(SharedString::from(acronym.clone()))
            .into_any_element(),
    }
}

fn run_bg_at(runs: &[gpui::TextRun], ix: usize) -> Option<gpui::Hsla> {
    let mut at = 0;
    runs.iter().find(|r| {
        at += r.len;
        at > ix
    })?
    .background_color
}

pub(crate) fn runs_fg_at(runs: &[gpui::TextRun], ix: usize) -> gpui::Hsla {
    let mut at = 0;
    runs.iter()
        .find(|r| {
            at += r.len;
            at > ix
        })
        .map_or(c(Palette::MENTION_FG), |r| r.color)
}

fn clear_bg(runs: &mut [gpui::TextRun], ranges: impl Iterator<Item = std::ops::Range<usize>>) {
    let ranges: Vec<_> = ranges.collect();
    let mut at = 0;
    for r in runs {
        let span = at..at + r.len;
        at = span.end;
        if ranges.iter().any(|m| m.start <= span.start && span.end <= m.end) {
            r.background_color = None;
        }
    }
}

pub(crate) fn split_at(bounds: &[usize], range: std::ops::Range<usize>) -> Vec<std::ops::Range<usize>> {
    let mut cuts: Vec<usize> = bounds.iter().copied().filter(|&b| range.start < b && b < range.end).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = Vec::with_capacity(cuts.len() + 1);
    let mut at = range.start;
    for c in cuts {
        out.push(at..c);
        at = c;
    }
    if at < range.end {
        out.push(at..range.end);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_follows_the_client_hover_bar() {
        use ToolbarButton::*;
        let other = ToolbarCaps { can_pin: true, can_start_thread: true, ..Default::default() };
        assert_eq!(toolbar_layout(other), [Quick, AddReaction, Reply, Forward, More]);
        let own = ToolbarCaps { own: true, can_edit: true, can_delete: true, ..other };
        assert_eq!(toolbar_layout(own), [Quick, AddReaction, Edit, Forward, More]);
        let own_poll = ToolbarCaps { can_edit: false, ..own };
        assert_eq!(toolbar_layout(own_poll), [Quick, AddReaction, Forward, More]);
        let shift = ToolbarCaps { expanded: true, dev: true, ..own };
        assert_eq!(toolbar_layout(shift), [CopyId, CopyLink, MarkUnread, Bookmark, Pin, Thread, ReplySelf, AddReaction, Edit, Forward, Delete]);
        let threaded = ToolbarCaps { can_start_thread: false, can_view_thread: true, can_publish: true, ..other };
        assert_eq!(toolbar_layout(threaded), [Quick, AddReaction, Reply, Forward, ViewThread, Publish, More]);
        assert_eq!(toolbar_layout(ToolbarCaps { expanded: true, ..other }).last(), Some(&More));
        let join = ToolbarCaps { expanded: true, can_configure_join: true, ..other };
        assert_eq!(&toolbar_layout(join)[..3], [CopyLink, Configure, MarkUnread]);
        assert!(!toolbar_layout(ToolbarCaps { can_configure_join: true, ..other }).contains(&Configure));
    }
}
