use std::time::Duration;

use gpui_component::Sizable as _;

use gpui::{
    Animation, AnimationExt, Context, FontWeight, IntoElement, KeyDownEvent, ParentElement, SharedString,
    Styled, Window, div, img, prelude::*,
};

use super::{
    icons::{ChatIcon, icon},
    state::EmojiTarget,
};
use crate::{
    app::PhoenixApp,
    model::Id,
    assets::Icon,
    theme::{Palette, c, fz, generated::idx, z},
    ui::icon_button,
};

fn reply_bar(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<impl IntoElement + use<>> {
    let rid = app.replying_to?;
    let e = app.messages.get(&app.selected_channel?)?;
    let m = e.msgs.iter().find(|m| m.id == rid)?;
    let name = app.users.get(&m.author).map(|u| u.display_name.clone()).unwrap_or_else(|| crate::t!("common.unknown"));
    let own = m.author == app.me.id;
    let mention = app.chat.reply_mention;
    Some(
        div()
            .h(z(34.))
            .pl(z(16.))
            .pr(z(4.))
            .rounded_t(z(8.))
            .flex()
            .items_center()
            .text_size(z(14.))
            .line_height(z(18.))
            .text_color(super::tk(idx::TEXT_DEFAULT))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .flex()
                    .child(crate::t!("chat.replying_to"))
                    .child(div().font_weight(FontWeight::SEMIBOLD).overflow_hidden().text_ellipsis().child(name)),
            )
            .when(!own, |d| {
                d.child(
                    div()
                        .id("reply-mention")
                        .px(z(12.))
                        .h(z(32.))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(if mention { Palette::TEXT_LINK } else { Palette::TEXT_MUTED }))
                        .child(gpui::svg().path(crate::icons::path("search/at")).size(z(16.)).mr(z(4.)).flex_none().text_color(c(if mention { Palette::TEXT_LINK } else { Palette::TEXT_MUTED })))
                        .child(if mention { crate::t!("chat.on") } else { crate::t!("chat.off") })
                        .on_click(cx.listener(|a, _, _, cx| {
                            a.chat.reply_mention = !a.chat.reply_mention;
                            cx.notify();
                        })),
                )
                .child(div().w(z(1.)).h(z(20.)).bg(super::tk(idx::BORDER_SUBTLE)))
            })
            .child(
                icon_button("cancel-reply", Icon::Close, super::tk(idx::INTERACTIVE_TEXT_DEFAULT), 16.).px(z(12.)).on_click(cx.listener(
                    |a, _, _, cx| {
                        a.replying_to = None;
                        cx.notify();
                    },
                )),
            ),
    )
}

pub(super) fn upload_tray_for(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<gpui::AnyElement> {
    upload_tray(app, cx).map(IntoElement::into_any_element)
}

fn upload_tray(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<impl IntoElement + use<>> {
    let hero_ch = super::announce::channel(app);
    if app.chat.uploads.iter().all(|f| super::announce::is_hero(hero_ch, f)) {
        return None;
    }
    let mut tray = div()
        .id("upload-tray")
        .pt(z(20.))
        .px(z(10.))
        .pb(z(10.))
        .flex()
        .gap(z(24.))
        .overflow_x_scroll();
    for (ix, f) in app.chat.uploads.iter().enumerate().filter(|(_, f)| !super::announce::is_hero(hero_ch, f)) {
        let shown = if f.spoiler { f.blurred.as_ref().or(f.image.as_ref()) } else { f.image.as_ref() };
        let preview = match shown {
            Some(i) => img(i.clone())
                .size_full()
                .rounded(z(12.))
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element(),
            None => icon(ChatIcon::Upload, 64., super::tk(idx::TEXT_BRAND)).into_any_element(),
        };
        let button = |id: (&'static str, usize), glyph: ChatIcon, color: gpui::Hsla| {
            div()
                .id(id)
                .size(z(28.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(z(6.))
                .cursor_pointer()
                .hover(|s| s.bg(super::tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
                .child(icon(glyph, 20., color))
        };
        let bar = div()
            .absolute()
            .top(z(-16.))
            .right(z(-16.))
            .p(z(2.))
            .flex()
            .rounded(z(8.))
            .bg(super::tk(idx::BACKGROUND_SURFACE_HIGH))
            .border_1()
            .border_color(super::tk(idx::BORDER_MUTED))
            .shadow(crate::ui::inbox::low_shadow())
            .child(
                button(("up-spoiler", ix), ChatIcon::Eye, super::tk(if f.spoiler { idx::TEXT_BRAND } else { idx::INTERACTIVE_TEXT_DEFAULT }))
                    .on_click(cx.listener(move |a, _, _, cx| a.toggle_upload_spoiler(ix, cx))),
            )
            .child(
                button(("up-edit", ix), ChatIcon::Edit, super::tk(idx::INTERACTIVE_TEXT_DEFAULT))
                    .on_click(cx.listener(move |a, _, _, cx| open_upload_edit(a, ix, cx))),
            )
            .child(
                button(("up-remove", ix), ChatIcon::Trash, super::tk(idx::TEXT_FEEDBACK_CRITICAL))
                    .on_click(cx.listener(move |a, _, _, cx| a.remove_upload(ix, cx))),
            );
        tray = tray.child(
            div()
                .relative()
                .flex_none()
                .w(z(200.))
                .h(z(200.))
                .p(z(8.))
                .rounded(z(12.))
                .bg(super::tk(idx::BACKGROUND_SURFACE_HIGH))
                .border_1()
                .border_color(super::tk(idx::BORDER_NORMAL))
                .shadow(crate::ui::inbox::low_shadow())
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .overflow_hidden()
                        .rounded(z(12.))
                        .when(f.spoiler && f.blurred.is_none(), |d| d.opacity(0.15).bg(super::tk(idx::BACKGROUND_MOD_STRONG)))
                        .child(preview),
                )
                .child(
                    div()
                        .mt(z(8.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .text_color(super::tk(idx::TEXT_DEFAULT))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(f.name.clone()),
                )
                .child(bar),
        );
    }
    Some(tray)
}

thread_local! {
    static ATTACH_BTN: std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>> = const { std::cell::Cell::new(None) };
}

fn attach_menu(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<gpui::AnyElement> {
    use crate::ui::context_menu::{self, MenuItem};
    if !app.chat.attach_menu {
        return None;
    }
    let ch = app.selected_channel?;
    let b = ATTACH_BTN.with(|c| c.get())?;
    let e = cx.entity().downgrade();
    let act = |f: fn(&mut PhoenixApp, Id, &mut Window, &mut Context<PhoenixApp>)| {
        let e = e.clone();
        move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut gpui::App| {
            let _ = e.update(cx, |a, cx| {
                a.chat.attach_menu = false;
                f(a, ch, w, cx);
                cx.notify();
            });
        }
    };
    let thread_ok = app
        .channels
        .get(&ch)
        .is_some_and(|c| c.guild.is_some() && matches!(c.kind, crate::model::ChannelKind::Text | crate::model::ChannelKind::Announcement));
    let mut items = vec![MenuItem::action(crate::t!("attach.upload"), Some(crate::icons::path("attach/file-up")), act(|a, _, _, cx| a.prompt_attach(cx)))];
    if !app.composer.read(cx).value().trim().is_empty() {
        items.push(MenuItem::action(crate::t!("attach.text_file"), Some(crate::icons::path("attach/file-up")), act(|a, _, w, cx| a.upload_text_as_file(w, cx))));
    }
    if thread_ok {
        items.push(MenuItem::action(crate::t!("attach.thread"), Some(crate::icons::path("attach/thread-plus")), act(|a, ch, w, cx| a.open_create_thread(ch, w, cx))));
    }
    let guild = app.channels.get(&ch).and_then(|c| c.guild);
    if guild.is_none() || crate::actions::has(app, guild, 1 << 49) {
        items.push(MenuItem::action(crate::t!("attach.poll"), Some(crate::icons::path("attach/poll")), act(|_, ch, w, _cx| {
            crate::actions::open_modal(crate::ui::modals::Modal::CreatePoll(crate::ui::modals::poll_create::State::new(ch)), Some(w));
        })));
    }
    items.push(MenuItem::action(crate::t!("attach.apps"), Some(crate::icons::path("apps")), act(|a, _, w, cx| {
        a.composer.update(cx, |s, cx| {
            if s.value().is_empty() {
                super::rich_input::set_value_at_end(s, "/".into(), w, cx);
            }
            s.focus(w, cx);
        });
    })));
    if let Some(me) = e.upgrade() {
        items.push(MenuItem::submenu_with_icon(crate::t!("attach.schedule"), "inbox/schedule", super::scheduled::menu_items(&me, ch)));
    }
    items.push(MenuItem::action(crate::t!("attach.voice"), Some(crate::icons::path("mic")), act(|a, _, _, cx| super::voice_message::start(a, cx))));
    let dismiss: context_menu::Dismiss = std::rc::Rc::new(move |_, cx| {
        let _ = e.update(cx, |a, cx| {
            a.chat.attach_menu = false;
            cx.notify();
        });
    });
    let pos = gpui::point(b.origin.x, b.origin.y - z(8.));
    Some(context_menu::render_anchored(pos, gpui::Anchor::BottomLeft, items, dismiss))
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> gpui::AnyElement {
    if let Some(notice) = blocked_dm_notice(app, cx) {
        return notice;
    }
    if let Some(ch) = super::announce::active(app, cx) {
        return super::announce::render(app, ch, window, cx);
    }
    render_text_area(app, window, cx).into_any_element()
}

pub(crate) fn channel_read_only(app: &PhoenixApp) -> bool {
    app.selected_guild.is_some_and(crate::ui::join_flow::is_lurking)
        || app.selected_channel.and_then(|c| app.nav.extras.get(&c)).is_some_and(|extra| extra.send_blocked)
}

pub(super) fn placeholder(app: &PhoenixApp) -> SharedString {
    match app.selected_channel.and_then(|c| app.channels.get(&c)) {
        Some(c) if c.guild.is_none() && crate::ui::dm_profile_panel::is_group_dm(app, c.id) => crate::t!("chat.placeholder_thread", name = c.name),
        Some(c) if c.guild.is_none() => crate::t!("chat.placeholder_dm", name = crate::ui::dm_profile_panel::channel_name(app, c)),
        Some(c) if matches!(c.kind, crate::model::ChannelKind::Voice | crate::model::ChannelKind::Stage) => {
            crate::t!("chat.placeholder_voice", name = c.name)
        }
        Some(c) if app.nav.thread_parent(c.id).is_some() => crate::t!("chat.placeholder_thread_quoted", name = c.name),
        Some(c) => crate::t!("chat.placeholder_channel", name = c.name),
        None => "".into(),
    }
}


fn blocked_dm_notice(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<gpui::AnyElement> {
    use crate::ui::home::data::{NavRequest, RelAction, RelationshipKind};
    let user = crate::ui::dm_profile_panel::peer(app)?;
    if app.nav.relationship(user)?.kind != RelationshipKind::Blocked {
        return None;
    }
    let unblock = crate::ui::controls::button_sm("dm-unblock", crate::t!("friends.unblock"), crate::ui::controls::BtnKind::Secondary)
        .on_click(cx.listener(move |a: &mut PhoenixApp, _: &gpui::ClickEvent, _, cx| {
            a.nav.relationships.retain(|x| x.user != user);
            a.nav_request(NavRequest::Relationship { user, action: RelAction::Remove }, cx, |_, _, _| {});
            cx.notify();
        }));
    Some(
        div()
            .flex_none()
            .px(z(crate::theme::space::xs()))
            .mb(z(crate::theme::space::xs()))
            .child(
                div()
                    .h(z(32.))
                    .mb(z(8.))
                    .py(z(12.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .rounded(z(8.))
                    .border_1()
                    .border_color(super::tk(idx::BORDER_SUBTLE))
                    .bg(super::tk(idx::BACKGROUND_BASE_LOW))
                    .child(
                        div().h(z(20.)).ml(z(16.)).min_w_0().flex().items_center().child(
                            div()
                                .overflow_hidden()
                                .text_ellipsis()
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(super::tk(idx::TEXT_STRONG))
                                .child(crate::t!("chat.blocked_dm")),
                        ),
                    )
                    .child(div().flex().flex_none().mr(z(12.)).child(div().flex().items_center().ml(z(12.)).child(unblock))),
            )
            .into_any_element(),
    )
}

fn render_text_area(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let placeholder = placeholder(app);
    let ic = c(Palette::INTERACTIVE);
    let placeholder_empty = app.composer.read(cx).value().is_empty();
    let (remaining, show_count) = char_count(app.backend.premium_tier2(), app.composer.read(cx).value().chars().count() as i64);
    let reply = reply_bar(app, cx);
    let sched = if reply.is_none() { super::scheduled::bar(app, cx) } else { None };
    let has_banner = reply.is_some() || sched.is_some();
    let tray = upload_tray(app, cx);
    let blocked = channel_read_only(app);
    let show_send_button = should_show_send_button(app.prefs.local.flag("send-btn", false), blocked);
    let can_send = !placeholder_empty || !app.chat.uploads.is_empty();
    let placeholder = if blocked { crate::t!("chat.no_permission") } else { placeholder };
    let vbar = super::voice_message::bar(app, window, cx);
    let voice_on = vbar.is_some();
    div()
        .flex_none()
        .relative()
        .flex()
        .flex_col()
        .gap(z(8.))
        .children(attach_menu(app, cx))
        .children(super::slash::render(app, cx))
        .child(
            div().relative().px(z(crate::theme::space::xs())).mb(z(crate::theme::space::xs()))
            .child(div().absolute().top(z(-24.)).left_0().right_0().child(typing(app, window)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .rounded(z(8.))
                    .border_1()
                    .border_color(super::tk(idx::BORDER_MUTED))
                    .bg(crate::theme::client_theme::fixed_bg(crate::theme::tok(idx::CHAT_BACKGROUND_DEFAULT)))
                    .relative()
                    .children(crate::theme::client_theme::fixed_layer(crate::theme::client_theme::Level::Highest, z(7.)))
                    .when(crate::ui::focus_ring::tab_focused(&gpui::Focusable::focus_handle(app.composer.read(cx), cx), window), |d| {
                        d.child(crate::ui::focus_ring::ring_box(8., 0., 0.))
                    })
                    .child(super::width_probe(&TEXT_AREA_W))
                    .children(crate::ui::autocomplete::render(app, window, cx))
                    .children(reply)
                    .children(sched)
                    .children(tray)
                    .child(
                        div()
                            .id("composer")
                            .min_h(z(56.))
                            .when(has_banner, |d| d.rounded_b(z(8.)))
                            .relative()
                            .when(show_count, |d| {
                                d.child(
                                    div()
                                        .absolute()
                                        .right(z(14.))
                                        .bottom(z(10.))
                                        .text_size(z(14.))
                                        .line_height(z(18.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(super::tk(if remaining < 0 { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_DEFAULT }))
                                        .child(SharedString::from(remaining.to_string())),
                                )
                            })
                            .flex()
                            .items_start()
                            .capture_key_down(cx.listener(|a, ev: &KeyDownEvent, w, cx| {
                                let k = &ev.keystroke;
                                if k.modifiers.number_of_modifiers() == 0 && super::slash::on_key(a, &k.key, w, cx) {
                                    cx.stop_propagation();
                                }
                            }))
                            .when_some(vbar, |d, bar| d.child(bar))
                            .when(!voice_on, |d| d.child(
                                div().pl(z(15.)).h(z(56.)).flex().items_center().when(blocked, |d| d.invisible()).child(div().px(z(14.)).child(
                                    div()
                                        .relative()
                                        .ml(z(-16.))
                                        .mr(z(10.))
                                        .child(
                                            crate::ui::states::composer_button("attach", Icon::Plus).on_click(cx.listener(|a, _, _, cx| {
                                                a.chat.attach_menu = !a.chat.attach_menu;
                                                cx.notify();
                                            })),
                                        )
                                        .child(
                                            gpui::canvas(|b, _, _| ATTACH_BTN.with(|c| c.set(Some(b))), |_, _, _, _| ())
                                                .absolute()
                                                .top_0()
                                                .left_0()
                                                .size_full(),
                                        ),
                                )),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .relative()
                                    .pt(z(text_pad_y() - TEXT_LIFT))
                                    .pb(z(text_pad_y() + TEXT_LIFT))
                                    .text_size(fz(16.))
                                    .line_height(fz(22.))
                                    .text_color(c(Palette::TEXT_NORMAL))
                                    .child(
                                        div().mx(z(-4.)).key_context("Composer").child(
                                            gpui_component::input::Textarea::new(&app.composer)
                                                .aria_label(placeholder.clone())
                                                .token(super::rich_input::render_token)
                                                .context_menu({
                                                    let area = app.composer.clone();
                                                    move |_, w, _| {
                                                        crate::ui::extra_menus::open_textarea(&area, w);
                                                        gpui_component::native_menu::NativeMenu::new()
                                                    }
                                                })
                                                .on_paste({
                                                    let this = cx.entity().downgrade();
                                                    move |item, _, cx| this.update(cx, |a, cx| a.paste_clipboard(item, cx)).unwrap_or(false)
                                                })
                                                .appearance(false)
                                                .bordered(false)
                                                .xsmall()
                                                .text_size(fz(16.))
                                                .line_height(fz(22.))
                                                .disabled(blocked),
                                        ),
                                    )
                                    .when(placeholder_empty, |d| {
                                        d.child(
                                            div()
                                                .absolute()
                                                .top(z(text_pad_y() - TEXT_LIFT))
                                                .h(fz(22.))
                                                .left_0()
                                                .right(z(10.))
                                                .flex()
                                                .items_center()
                                                .text_color(super::tk(idx::CHANNEL_TEXT_AREA_PLACEHOLDER))
                                                .child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(placeholder)),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .when(blocked, |d| d.invisible())
                                    .mr(z(12.))
                                    .h(z(56.))
                                    .flex()
                                    .gap(z(8.))
                                    .items_center()
                                    .child(crate::ui::states::composer_button_ext("gift", Icon::Gift, 20., false).on_click(cx.listener(|a, _, _, cx| {
                                        let recipient = crate::ui::dm_profile_panel::peer(a);
                                        a.open_gift_nitro(recipient, cx);
                                    })))
                                    .when(full_buttons(), |d| {
                                        d.child(crate::ui::states::composer_button("gif", Icon::Gif).on_click(cx.listener(|a, _, w, cx| crate::ui::pickers::toggle(a, crate::ui::pickers::Tab::Gif, crate::ui::pickers::EmojiTarget::Composer, w, cx))))
                                            .child(crate::ui::states::composer_button("sticker", Icon::Sticker).on_click(cx.listener(|a, _, w, cx| crate::ui::pickers::toggle(a, crate::ui::pickers::Tab::Sticker, crate::ui::pickers::EmojiTarget::Composer, w, cx))))
                                    })
                                    .child({
                                        let emoji_open = matches!(
                                            crate::ui::pickers::open_tab(cx),
                                            Some((crate::ui::pickers::Tab::Emoji, crate::ui::pickers::EmojiTarget::Composer))
                                        );
                                        crate::ui::states::composer_button_full("emoji", Icon::Emoji, 18., true, emoji_open)
                                            .when(emoji_open, |d| d.bg(c(crate::theme::tok(idx::INTERACTIVE_BACKGROUND_SELECTED))))
                                            .on_click(cx.listener(|a, _, _, cx| a.open_emoji_picker(EmojiTarget::Composer, cx)))
                                    })
                                    .child(crate::ui::focus_ring::ring_inset(
                                        crate::ui::states::track(div().id("apps"), crate::ui::states::key(("composer-btn", "apps")))
                                            .relative()
                                            .size(z(32.))
                                            .rounded(z(8.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            .group("apps")
                                            .hover(|s| s.bg(c(crate::theme::tok(idx::INTERACTIVE_BACKGROUND_SELECTED))))
                                            .child(crate::ui::hover_icon::icon(
                                                crate::ui::states::key(("composer-btn", "apps")),
                                                crate::icons::lottie_icons::Lottie::Activities,
                                                false,
                                                20. * crate::theme::zoom(),
                                                (ic, c(crate::theme::tok(idx::INTERACTIVE_TEXT_HOVER))),
                                                || {
                                                    gpui::svg()
                                                        .path(crate::icons::path("apps"))
                                                        .size(z(20.))
                                                        .flex_none()
                                                        .text_color(ic)
                                                        .group_hover("apps", |s| s.text_color(c(crate::theme::tok(idx::INTERACTIVE_TEXT_HOVER))))
                                                        .id("g")
                                                        .group_active("apps", |s| s.text_color(c(crate::theme::tok(idx::INTERACTIVE_TEXT_ACTIVE))))
                                                        .into_any_element()
                                                },
                                            ))
                                            .child(super::app_launcher::anchor_probe())
                                            .on_click(cx.listener(|a, _, w, cx| {
                                                crate::ui::hover_icon::clicked(crate::ui::states::key(("composer-btn", "apps")));
                                                super::app_launcher::toggle(a, w, cx)
                                            })),
                                        crate::ui::states::key(("composer-btn", "apps")),
                                        8.,
                                        -4.,
                                        4.,
                                    ))
                                    .when(show_send_button, |d| {
                                        d.child(div().w(z(1.)).h(fz(22.)).bg(super::tk(idx::BORDER_NORMAL)))
                                            .child(
                                                div()
                                                    .id("send")
                                                    .size(z(32.))
                                                    .rounded(z(8.))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        gpui::svg()
                                                            .path(Icon::Send.path())
                                                            .size(z(20.))
                                                            .flex_none()
                                                            .text_color(super::tk(if can_send { idx::TEXT_BRAND } else { idx::ICON_MUTED })),
                                                    )
                                                    .when(can_send, |b| {
                                                        b.cursor_pointer()
                                                            .hover(|s| s.bg(c(crate::theme::tok(idx::INTERACTIVE_BACKGROUND_SELECTED))))
                                                            .tooltip(crate::ui::tooltip::tip(crate::t!("menu.send_message")))
                                                            .on_click(cx.listener(|a, _, w, cx| a.submit_composer(w, cx)))
                                                    })
                                                    .when(!can_send, |b| b.cursor(gpui::CursorStyle::OperationNotAllowed)),
                                            )
                                    }),
                            )),
                    ),
            ),
        )
}

fn open_upload_edit(app: &mut PhoenixApp, ix: usize, cx: &mut Context<PhoenixApp>) {
    use crate::ui::modals::{Field, Form, Modal};
    let Some(f) = app.chat.uploads.get(ix) else { return };
    let preview = f.image.clone().and_then(|im| {
        let (w, h) = image::ImageReader::new(std::io::Cursor::new(f.bytes.as_slice())).with_guessed_format().ok()?.into_dimensions().ok()?;
        Some((im, w, h))
    });
    crate::actions::open_modal(
        Modal::Form(Form {
            title: crate::t!("upload.modify"),
            subtitle: None,
            fields: vec![
                Field::new(crate::t!("upload.filename"), "".into(), f.name.to_string()),
                Field::new(crate::t!("upload.description"), crate::t!("upload.description_ph"), f.description.clone()).rows(3).max_len(1024),
            ],
            choices: Vec::new(),
            choice: None,
            check: Some((crate::t!("upload.mark_spoiler"), f.spoiler)),
            submit: crate::t!("upload.save"),
            danger: false,
            preview,
            width: 480.,
            on_submit: std::rc::Rc::new(move |a: &mut PhoenixApp, v: Vec<String>, _, cx: &mut Context<PhoenixApp>| {
                let mut v = v.into_iter();
                let (name, description, spoiler) = (v.next().unwrap_or_default(), v.next().unwrap_or_default(), v.next().is_some_and(|s| s == "1"));
                let toggle = if let Some(u) = a.chat.uploads.get_mut(ix) {
                    if !name.trim().is_empty() {
                        u.name = name.trim().to_string().into();
                    }
                    u.description = description.trim().to_string();
                    u.spoiler != spoiler
                } else {
                    false
                };
                if toggle {
                    a.toggle_upload_spoiler(ix, cx);
                }
                cx.notify();
            }),
        }),
        None,
    );
    cx.notify();
}

pub fn char_count(premium: bool, len: i64) -> (i64, bool) {
    let max = if premium { 4000 } else { 2000 };
    let remaining = max - len;
    (remaining, remaining <= max / 10 || (premium && len > 2000 && remaining >= 0))
}

pub fn too_long(premium: bool, text: &str) -> bool {
    char_count(premium, text.chars().count() as i64).0 < 0
}

fn should_show_send_button(enabled: bool, blocked: bool) -> bool {
    enabled && !blocked
}

thread_local! {
    static TEXT_AREA_W: std::cell::Cell<f32> = const { std::cell::Cell::new(0.) };
}

fn full_buttons() -> bool {
    let w = TEXT_AREA_W.with(std::cell::Cell::get);
    w <= 0. || w >= 450.
}

thread_local! {
    static LAST_SENT: std::cell::RefCell<std::collections::HashMap<crate::model::Id, std::time::Instant>> = Default::default();
}

fn slowmode_of(app: &PhoenixApp, ch: crate::model::Id) -> u32 {
    app.nav.extras.get(&ch).filter(|x| !x.bypass_slowmode).map_or(0, |x| x.slowmode)
}

pub fn cooldown_left(app: &PhoenixApp, ch: crate::model::Id) -> u64 {
    let secs = slowmode_of(app, ch) as u64;
    LAST_SENT.with(|m| m.borrow().get(&ch).map_or(0, |t| secs.saturating_sub(t.elapsed().as_secs())))
}

pub fn send_held(app: &PhoenixApp, ch: crate::model::Id) -> bool {
    let held = cooldown_left(app, ch) > 0;
    if held {
        emphasize_cooldown();
    }
    held
}

pub fn note_sent(ch: crate::model::Id) {
    LAST_SENT.with(|m| {
        m.borrow_mut().insert(ch, std::time::Instant::now());
    });
}

pub fn cooling() -> bool {
    LAST_SENT.with(|m| m.borrow().values().any(|t| t.elapsed().as_secs() < 6 * 3600))
}

pub fn format_cooldown(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

thread_local! {
    static EMPHASIZED: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) };
}

pub fn emphasize_cooldown() {
    EMPHASIZED.with(|e| e.set(Some(std::time::Instant::now())));
    crate::ui::shake::start(200, 2.);
}

fn emphasized() -> bool {
    EMPHASIZED.with(|e| e.get().is_some_and(|t| t.elapsed().as_millis() < 1000))
}

pub fn slowmode_tip(secs: u32) -> String {
    slowmode_tip_in(secs, false)
}

pub fn slowmode_tip_in(secs: u32, post: bool) -> String {
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    let one = |n: u32| if crate::i18n::locale() == "fr" { n < 2 } else { n == 1 };
    let part = |n: u32, k: &str| -> String {
        if n == 0 { String::new() } else { crate::i18n::t(&format!("chat.slowmode_{k}{}", if one(n) { "_one" } else { "" }), &[("n", &n.to_string())]).to_string() }
    };
    let prefix = if post { crate::t!("forum.slowmode_tip") } else { crate::t!("chat.slowmode_tip") };
    let sep: SharedString = if post { crate::t!("forum.slowmode_sep") } else { " ".into() };
    if h > 0 {
        format!("{prefix}{} {}{sep}{}", part(h, "h"), part(m, "m"), part(s, "s"))
    } else if m > 0 {
        format!("{prefix}{}{sep}{}", part(m, "m"), part(s, "s"))
    } else {
        let k = match (post, one(secs)) {
            (false, true) => "chat.slowmode_s_alone_one",
            (false, false) => "chat.slowmode_s_alone",
            (true, true) => "chat.slowmode_s_one",
            (true, false) => "chat.slowmode_s",
        };
        format!("{prefix}{}.", crate::i18n::t(k, &[("n", &secs.to_string())]))
    }
}

pub fn slowmode_label(app: &PhoenixApp, ch: crate::model::Id) -> Option<SharedString> {
    let x = app.nav.extras.get(&ch).filter(|x| x.slowmode > 0)?;
    let left = cooldown_left(app, ch);
    Some(if x.bypass_slowmode {
        crate::t!("chat.slowmode_immune")
    } else if left > 0 {
        format_cooldown(left).into()
    } else {
        crate::t!("chat.slowmode_on")
    })
}

fn cooldown(app: &PhoenixApp) -> Option<gpui::AnyElement> {
    use crate::ui::tooltip::Tipped as _;
    let ch = app.selected_channel?;
    let text = slowmode_label(app, ch)?;
    let x = app.nav.extras.get(&ch)?;
    let color = super::tk(if emphasized() { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_MUTED });
    Some(
        div()
            .id("slowmode-cooldown")
            .ml_auto()
            .pt(z(3.))
            .pb(z(3.))
            .pl(z(8.))
            .pr(z(16.))
            .flex()
            .items_center()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .whitespace_nowrap()
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(color)
                    .child(div().ml(z(4.)).child(gpui::svg().path(crate::icons::path("chat/timer")).size(z(12.)).text_color(color)))
                    .child(text),
            )
            .tip(slowmode_tip(x.slowmode))
            .into_any_element(),
    )
}

pub(crate) fn dot_timing(reduced: bool) -> (u64, f32) {
    if reduced { (2400, 24.) } else { (1200, 12.) }
}

pub(crate) fn dot_frame(i: usize, t: f32, steps: f32) -> (f32, f32) {
    let p = (t + 1.0 - i as f32 * 0.125).rem_euclid(1.0);
    let step = |x: f32| (x * steps).floor() / steps;
    let k = if p < 0.25 {
        step(p / 0.25)
    } else if p < 0.5 {
        1. - step((p - 0.25) / 0.25)
    } else {
        0.
    };
    (0.32 + 0.68 * k, 0.8 + 0.2 * k)
}

thread_local! {
    static DOTS_BOX_W: std::cell::Cell<f32> = const { std::cell::Cell::new(f32::MAX) };
}

fn text_w(window: &Window, text: &str, weight: FontWeight) -> f32 {
    let mut font = crate::richtext::ui_font();
    font.weight = weight;
    let run = gpui::TextRun { len: text.len(), font, color: gpui::black(), background_color: None, underline: None, strikethrough: None };
    f32::from(window.text_system().layout_line(text, z(12.), &[run], None).width)
}

pub(super) fn typing(app: &PhoenixApp, window: &Window) -> impl IntoElement + use<> {
    let names: Vec<SharedString> = app
        .selected_channel
        .and_then(|c| app.typing.get(&c).map(|v| (c, v)))
        .map(|(c, v)| v.iter().filter_map(|(u, _)| app.typing_name(c, *u)).collect())
        .unwrap_or_default();
    let bold = |s: SharedString| div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(super::tk(idx::TEXT_DEFAULT)).child(s);
    let mut line = div().flex().items_center().min_w_0().overflow_hidden().whitespace_nowrap().ml(z(crate::theme::space::md() + 8.));
    let full_w = || -> f32 {
        let (strong, rest): (Vec<&str>, String) = match names.as_slice() {
            [a] => (vec![&a[..]], crate::t!("chat.typing_suffix_one").to_string()),
            [a, b] => (vec![&a[..], &b[..]], format!("{}{}", crate::t!("chat.typing_and"), crate::t!("chat.typing_suffix_many"))),
            [a, b, c3] => (vec![&a[..], &b[..], &c3[..]], format!(", {}{}", crate::t!("chat.typing_comma_and"), crate::t!("chat.typing_suffix_many"))),
            _ => (vec![], String::new()),
        };
        strong.iter().map(|t| text_w(window, t, FontWeight::SEMIBOLD)).sum::<f32>() + text_w(window, &rest, FontWeight::MEDIUM)
    };
    let box_w = DOTS_BOX_W.with(|w| w.get());
    let overflow = (1..=3).contains(&names.len()) && box_w > f32::from(z(56.)) && full_w() + f32::from(z(56.)) > box_w;
    match names.as_slice() {
        [] => {}
        _ if overflow => line = line.child(bold(crate::t!("chat.typing_multiple"))).child(crate::t!("chat.typing_suffix_multiple")),
        [a] => line = line.child(bold(a.clone())).child(crate::t!("chat.typing_suffix_one")),
        [a, b] => line = line.child(bold(a.clone())).child(crate::t!("chat.typing_and")).child(bold(b.clone())).child(crate::t!("chat.typing_suffix_many")),
        [a, b, c3] => {
            line = line
                .child(bold(a.clone()))
                .child(", ")
                .child(bold(b.clone()))
                .child(crate::t!("chat.typing_comma_and"))
                .child(bold(c3.clone()))
                .child(crate::t!("chat.typing_suffix_many"))
        }
        _ => line = line.child(bold(crate::t!("chat.typing_several"))).child(crate::t!("chat.typing_suffix_many")),
    }
    let active = !names.is_empty();
    let (period, steps) = dot_timing(crate::ui::settings::effects::reduced_motion());
    let dot = |i: usize| {
        div().size(z(7.)).flex().items_center().justify_center().child(
            div().rounded_full().bg(super::tk(idx::TEXT_DEFAULT)).with_animation(
                ("typing-dot", i),
                Animation::new(Duration::from_millis(period)).repeat(),
                move |d, t| {
                    let (o, s) = dot_frame(i, t, steps);
                    d.size(z(7. * s)).opacity(o)
                },
            ),
        )
    };
    div()
        .h(z(24.))
        .pl(z(crate::theme::space::md()))
        .flex()
        .items_center()
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(super::tk(idx::TEXT_SUBTLE))
        .child(
            div().relative().flex_1().min_w_0().overflow_hidden().flex().items_center().pl(z(8.))
            .child(gpui::canvas(|b, _, _| DOTS_BOX_W.with(|w| w.set(f32::from(b.size.width))), |_, _, _, _| {}).absolute().top_0().left_0().w_full().h_full())
            .when(active, |d| {
                d.child(div().flex_none().flex().items_center().gap(z(2.)).child(dot(0)).child(dot(1)).child(dot(2))).child(line)
            }),
        )
        .when_some(app.chat.error.clone(), |d, err| {
            d.child(div().text_color(c(Palette::RED)).text_ellipsis().child(err))
        })
        .children(cooldown(app))
}

pub(super) const TEXT_LIFT: f32 = 1.;

pub(super) fn text_pad_y() -> f32 {
    ((56. - 22. * crate::theme::font_scale()) / 2.).max(0.)
}

#[cfg(test)]
mod tests {
    use super::{char_count, dot_frame, dot_timing, format_cooldown, should_show_send_button, slowmode_tip, too_long};

    #[test]
    fn send_button_requires_enabled_nonempty_sendable_draft() {
        assert!(!should_show_send_button(false, false));
        assert!(should_show_send_button(true, false));
        assert!(!should_show_send_button(true, true));
    }

    #[test]
    fn typing_dots_follow_the_keyframes() {
        assert_eq!(dot_frame(0, 0.0, 12.), (0.32, 0.8));
        assert_eq!(dot_frame(0, 0.25, 12.), (1.0, 1.0));
        assert_eq!(dot_frame(0, 0.6, 12.), (0.32, 0.8));
        assert_eq!(dot_frame(1, 0.375, 12.), (1.0, 1.0));
    }

    #[test]
    fn reduced_motion_dots_run_slower_with_finer_steps() {
        assert_eq!(dot_timing(false), (1200, 12.));
        assert_eq!(dot_timing(true), (2400, 24.));
        let coarse = dot_frame(0, 0.012, 12.);
        let fine = dot_frame(0, 0.012, 24.);
        assert_eq!(coarse, (0.32, 0.8));
        assert!(fine.0 > 0.32);
    }

    #[test]
    fn counter_follows_the_client() {
        assert_eq!(char_count(false, 1799), (201, false));
        assert_eq!(char_count(false, 1800), (200, true));
        assert_eq!(char_count(false, 2100), (-100, true));
        assert_eq!(char_count(true, 2000), (2000, false));
        assert_eq!(char_count(true, 2001), (1999, true));
        assert_eq!(char_count(true, 3600), (400, true));
    }

    #[test]
    fn send_limit_counts_characters() {
        assert!(!too_long(false, &"a".repeat(2000)));
        assert!(too_long(false, &"a".repeat(2001)));
        assert!(!too_long(false, &"я".repeat(2000)));
        assert!(!too_long(false, &"字".repeat(2000)));
        assert!(too_long(false, &"字".repeat(2001)));
        assert!(!too_long(true, &"a".repeat(4000)));
        assert!(too_long(true, &"a".repeat(4001)));
        assert!(!too_long(true, &"я".repeat(4000)));
    }

    #[test]
    fn cooldown_text() {
        assert_eq!(format_cooldown(5), "0:05");
        assert_eq!(format_cooldown(125), "2:05");
        assert_eq!(format_cooldown(3725), "1:02:05");
    }

    #[test]
    fn slowmode_tooltip_follows_the_client_strings() {
        if crate::i18n::locale() == "fr" {
            assert!(slowmode_tip(10).ends_with("toutes les 10\u{a0}secondes."));
            assert!(slowmode_tip(60).ends_with("toutes les 1\u{a0}minute "));
            assert!(slowmode_tip(1).ends_with("toutes les secondes."));
            assert!(slowmode_tip(3725).ends_with("1\u{a0}heure 2\u{a0}minutes 5\u{a0}secondes"));
        } else {
            assert_eq!(slowmode_tip(10), "Slowmode is enabled. Members can send one message every 10 seconds.");
            assert_eq!(slowmode_tip(60), "Slowmode is enabled. Members can send one message every 1 minute ");
            assert!(slowmode_tip(7200).ends_with("every 2 hours  "));
        }
    }
}
