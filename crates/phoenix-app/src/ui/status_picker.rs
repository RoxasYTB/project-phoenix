use crate::theme::z;
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, anchored, deferred, div,
    point, prelude::*, px, svg,
};
use gpui_component::input::{InputEvent, InputState};

use crate::{
    app::PhoenixApp,
    model::Presence,
    presence::status_label,
    theme::{c, generated::idx, tok},
    ui::{
        profile::{avatar_block, banner, ensure_avatar_colour, info, name_block, theme_backdrop},
        status_dot,
    },
};

impl PhoenixApp {
    pub fn toggle_status_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open = !self.people.status_picker;
        self.people.close_overlays();
        if open {
            let cur = self.people.custom_status.clone().unwrap_or_default();
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("status.custom_placeholder")).default_value(cur));
            let sub = cx.subscribe_in(&input, window, |this, input, ev: &InputEvent, _, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    let t = input.read(cx).value().trim().to_owned();
                    this.set_custom_status((!t.is_empty()).then_some(t), cx);
                    this.people.close_overlays();
                }
            });
            self.people.custom_input = Some(input);
            self.people.custom_sub = Some(sub);
            self.people.status_picker = true;
            let me = self.me.id;
            if !self.people.profiles.contains_key(&me) {
                let fut = self.backend.user_profile(me, None);
                cx.spawn(async move |this, cx| {
                    let Ok(p) = fut.await else { return };
                    let _ = this.update(cx, |a, cx| {
                        a.people.profiles.insert(me, p);
                        cx.notify();
                    });
                })
                .detach();
            }
        }
        cx.notify();
    }

    pub fn set_own_status(&mut self, p: Presence, cx: &mut Context<Self>) {
        self.presence = p;
        let custom = self.people.custom_status.as_ref().map(|s| s.to_string());
        cx.background_spawn(self.backend.set_status(p, custom)).detach();
        self.people.close_overlays();
        cx.notify();
    }

    pub fn set_custom_status(&mut self, text: Option<String>, cx: &mut Context<Self>) {
        self.people.custom_status = text.clone().map(Into::into);
        cx.background_spawn(self.backend.set_status(self.presence, text)).detach();
        cx.notify();
    }
}

const CARD_GROUP: &str = "acct-popout";

const ITEMS: [(Presence, Option<&str>); 4] = [
    (Presence::Online, None),
    (Presence::Idle, None),
    (Presence::Dnd, Some("status.dnd_desc")),
    (Presence::Offline, Some("status.invisible_desc")),
];

thread_local! {
    static STATUS_SUB: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ACCOUNTS_SUB: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static WALLET_SUB: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn set_wallet(v: bool, w: &mut Window) {
    let a = ACCOUNTS_SUB.with(|c| c.replace(false));
    let s = STATUS_SUB.with(|c| c.replace(false));
    if WALLET_SUB.with(|c| c.replace(v)) != v || a || s {
        w.refresh();
    }
}

pub fn debug_open_wallet(v: bool) {
    WALLET_SUB.with(|c| c.set(v));
}

const ORBS_HELP: &str = "https://support.discord.com/hc/articles/30593690165783-Discord-Orbs";

fn orb_wallet(balance: u64, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let cta = |id: &'static str, ic: &'static str, label: SharedString| crate::ui::controls::button_icon(id, ic, label, crate::ui::controls::BtnKind::Secondary).flex_1();
    let quests = cta("wallet-quests", "quests", crate::t!("wallet.quests")).on_click(cx.listener(|a, _, w, cx| {
        set_wallet(false, w);
        a.people.close_overlays();
        a.open_home_view(crate::ui::home::data::HomeView::Quests, cx);
    }));
    let shop = cta("wallet-shop", "shop", crate::t!("wallet.shop")).on_click(cx.listener(|a, _, w, cx| {
        set_wallet(false, w);
        a.people.close_overlays();
        a.open_home_view(crate::ui::home::data::HomeView::Shop, cx);
    }));
    let header = div()
        .px(z(16.))
        .flex()
        .flex_row()
        .items_center()
        .gap_x(z(12.))
        .child(div().size(z(48.)).flex_none().flex().items_center().justify_center().child(svg().path(crate::icons::path("orbs")).size(z(40.)).text_color(c(tok(idx::TEXT_STRONG)))))
        .child(
            div()
                .flex()
                .flex_col()
                .justify_center()
                .gap_y(z(4.))
                .child(
                    div()
                        .max_w(z(260.))
                        .pr(z(20.))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .font_family(crate::fonts::HEADLINE)
                        .text_size(z(32.))
                        .line_height(z(34.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(c(tok(idx::TEXT_STRONG)))
                        .child(SharedString::from(balance.to_string())),
                ),
        );
    let footer = div()
        .px(z(16.))
        .w_full()
        .flex()
        .flex_col()
        .items_center()
        .gap_y(z(16.))
        .child(div().w_full().flex().flex_row().justify_center().gap_x(z(12.)).child(quests).child(shop))
        .child(
            div()
                .id("wallet-help")
                .cursor_pointer()
                .text_size(z(12.))
                .line_height(z(16.))
                .text_color(c(tok(idx::TEXT_LINK)))
                .hover(|s| s.underline())
                .on_click(|_, _, cx| cx.open_url(ORBS_HELP))
                .child(crate::t!("wallet.help")),
        );
    div()
        .id("orb-wallet")
        .occlude()
        .w(z(340.))
        .py(z(16.))
        .flex()
        .flex_col()
        .items_start()
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow_lg()
        .on_hover(|h, w, _| {
            if !*h {
                set_wallet(false, w)
            }
        })
        .child(header)
        .child(div().w_full().my(z(16.)).border_t_1().border_color(c(tok(idx::BORDER_SUBTLE))))
        .child(footer)
}

fn set_sub(v: bool, w: &mut Window) {
    let a = ACCOUNTS_SUB.with(|c| c.replace(false)) | WALLET_SUB.with(|c| c.replace(false));
    if STATUS_SUB.with(|c| c.replace(v)) != v || a {
        w.refresh();
    }
}

fn set_accounts_sub(v: bool, w: &mut Window) {
    let s = STATUS_SUB.with(|c| c.replace(false)) | WALLET_SUB.with(|c| c.replace(false));
    if ACCOUNTS_SUB.with(|c| c.replace(v)) != v || s {
        w.refresh();
    }
}

pub fn debug_open_accounts_submenu(v: bool) {
    ACCOUNTS_SUB.with(|c| c.set(v));
}

fn accounts_submenu(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let me = app.users.get(&app.me.id);
    let name = SharedString::from(app.me.name.to_string());
    let row = |id: (&'static str, u64)| {
        div()
            .id(id)
            .min_h(z(32.))
            .p(z(8.))
            .rounded(z(4.))
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(z(14.))
            .line_height(z(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(tok(idx::TEXT_STRONG)))
            .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
    };
    let account = row(("acct-sub-me", 0))
        .gap(z(8.))
        .child(crate::ui::avatar(me, 24.))
        .child(
            div()
                .flex_grow(1.)
                .max_w(z(132.))
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .font_weight(FontWeight::NORMAL)
                .child(name),
        )
        .child(svg().path(crate::icons::path("circle-check")).size(z(20.)).flex_none().text_color(c(tok(idx::BRAND_500))))
        .on_click(cx.listener(|a, _, w, cx| {
            set_accounts_sub(false, w);
            a.people.close_overlays();
            cx.notify();
        }));
    let others: Vec<_> = crate::saved_accounts::load().into_iter().filter(|a| a.id != app.me.id.0).collect();
    let others = others.into_iter().map(|acct| {
        let id = acct.id;
        let user = crate::model::User {
            id: crate::model::Id(acct.id),
            name: acct.username.clone().into(),
            display_name: acct.display_name().to_owned().into(),
            avatar: acct.avatar_ref(),
            bot: false,
            verified_bot: false,
            tag: None,
            tag_guild: None,
            nameplate: None,
            name_style: None,
            decoration: None,
            flags: 0,
        };
        row(("acct-sub-other", id))
            .gap(z(8.))
            .child(crate::ui::avatar(Some(&user), 24.))
            .child(div().flex_grow(1.).max_w(z(132.)).min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().font_weight(FontWeight::NORMAL).child(user.display_name.clone()))
            .on_click(cx.listener(move |a, _, w, cx| {
                set_accounts_sub(false, w);
                a.people.close_overlays();
                a.switch_account(id, cx);
            }))
    });
    let others: Vec<_> = others.collect();
    let manage = row(("acct-sub-manage", 0)).child(crate::t!("status.manage_accounts")).on_click(cx.listener(|a, _, w, cx| {
        set_accounts_sub(false, w);
        a.open_switch_accounts(cx);
    }));
    div()
        .id("accounts-sub")
        .occlude()
        .min_w(z(188.))
        .p(z(8.))
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow_lg()
        .flex()
        .flex_col()
        .on_hover(|h, w, _| {
            if !*h {
                set_accounts_sub(false, w)
            }
        })
        .child(account)
        .children(others)
        .child(div().h(z(1.)).my(z(8.)).bg(c(tok(idx::BORDER_SUBTLE))))
        .child(manage)
}

pub fn debug_open_status_submenu(v: bool) {
    STATUS_SUB.with(|c| c.set(v));
}

fn pop_row(id: &'static str, icon: AnyElement, label: AnyElement, trailing: Option<AnyElement>, caret: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .rounded(z(8.))
        .p(z(8.))
        .flex()
        .items_center()
        .gap(z(8.))
        .cursor_pointer()
        .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
        .hover(|s| s.bg(c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND_HOVER))).text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))))
        .child(div().size(z(16.)).flex_none().flex().items_center().justify_center().child(icon))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .justify_between()
                .gap(z(4.))
                .child(div().text_size(z(14.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).child(label))
                .children(trailing),
        )
        .when(caret, |d| d.child(div().pl(z(8.)).flex_none().child(svg().path(crate::icons::path("chevron-right")).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))))
}

fn icon(name: &str) -> AnyElement {
    svg().path(crate::icons::path(name)).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))).into_any_element()
}

fn text(t: SharedString) -> AnyElement {
    div().child(t).into_any_element()
}

fn pop_group(rows: Vec<AnyElement>) -> gpui::Div {
    let n = rows.len();
    let mut g = div().rounded(z(8.)).p(z(8.)).bg(c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND))).flex().flex_col();
    for (k, r) in rows.into_iter().enumerate() {
        g = g.child(r);
        if k + 1 < n {
            g = g.child(div().h(z(1.)).mx(z(4.)).my(z(8.)).bg(c(tok(idx::USER_PROFILE_BORDER))));
        }
    }
    g
}

fn status_submenu(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let bg = c(tok(idx::BACKGROUND_SURFACE_HIGHER));
    let mut menu = div()
        .id("status-sub")
        .occlude()
        .w(z(300.))
        .p(z(8.))
        .rounded(z(8.))
        .bg(bg)
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow_lg()
        .flex()
        .flex_col()
        .on_hover(|h, w, _| {
            if !*h {
                set_sub(false, w)
            }
        });
    for (k, (p, desc)) in ITEMS.into_iter().enumerate() {
        if k == 1 {
            menu = menu.child(div().h(z(1.)).my(z(8.)).bg(c(tok(idx::BORDER_SUBTLE))));
        }
        let selected = app.presence == p;
        menu = menu.child(
            div()
                .id(("status-item", k))
                .p(z(8.))
                .min_h(z(36.))
                .rounded(z(4.))
                .flex()
                .items_start()
                .cursor_pointer()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER)))
                .when(selected, |d| d.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                .on_click(cx.listener(move |a, _, _, cx| a.set_own_status(p, cx)))
                .child(div().size(z(20.)).mr(z(8.)).flex_none().flex().items_center().justify_center().child(status_dot(p, 10., bg)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .pt(z(1.))
                        .child(status_label(p))
                        .children(desc.map(|d| {
                            div()
                                .mt(z(2.))
                                .text_size(z(12.))
                                .line_height(z(16.))
                                .font_weight(FontWeight::NORMAL)
                                .text_color(c(tok(idx::TEXT_MUTED)))
                                .child(crate::t!(d))
                        })),
                ),
        );
    }
    menu
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let theme = app.people.profiles.get(&app.me.id).and_then(|p| p.theme);
    crate::ui::profile_theme::scoped(theme, || render_inner(app, window, cx))
}

fn render_inner(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let me = app.me.id;
    ensure_avatar_colour(app, me, cx);
    let i = info(app, me, None);
    let theme = i.prof.and_then(|p| p.theme);
    let surface = tok(idx::BACKGROUND_SURFACE_HIGH);
    let ring = theme.map(|(p, _)| {
        let ch = |sh: u32| (((p >> sh) & 0xFF) as f32 * 0.4).round() as u32;
        (ch(16) << 16) | (ch(8) << 8) | ch(0)
    });
    let header = div()
        .relative()
        .flex_none()
        .h(z(151.))
        .child(banner(&i, 105., 6.))
        .child(div().absolute().left(z(8.)).top(z(53.)).child(avatar_block(&i, 80., 6., ring.unwrap_or(surface))));
    let bubble = crate::ui::profile_toolbar::status(app, me, None, crate::ui::profile_toolbar::Layout::Popout, theme.is_some(), i.custom.clone(), cx)
        .map(|s| div().absolute().left(z(105.)).top(z(98.)).child(s));

    let username = app.me.name.to_string();
    let copy_name = div()
        .id("acct-copy-username")
        .size(z(24.))
        .p(z(4.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .opacity(0.)
        .group_hover(CARD_GROUP, |s| s.opacity(1.))
        .on_click(cx.listener(move |_, _, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(username.clone()))))
        .child(icon("chat/copy"))
        .into_any_element();

    let dnd = app.presence == Presence::Dnd;
    let sub_open = STATUS_SUB.with(|c| c.get());
    let status_row = pop_row(
        "acct-status",
        status_dot(app.presence, 12., c(tok(idx::BACKGROUND_SURFACE_HIGH))).into_any_element(),
        text(status_label(app.presence)),
        dnd.then(|| svg().path(crate::icons::path("bell-slash")).size(z(12.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))).into_any_element()),
        true,
    )
    .relative()
    .on_hover(|h, w, _| {
        if *h {
            set_sub(true, w)
        }
    })
    .on_click(|_, w, _| set_sub(!STATUS_SUB.with(|c| c.get()), w))
    .when(sub_open, |d| {
        d.child(deferred(
            anchored().position_mode(gpui::AnchoredPositionMode::Local).position(point(px(252. + 12.), px(-14.))).snap_to_window().child(status_submenu(app, cx)),
        ))
    });
    let hover_off = |id: &'static str, ic: &str, label: SharedString| {
        pop_row(id, icon(ic), text(label), None, false).on_hover(|h, w, _| {
            if *h {
                set_sub(false, w)
            }
        })
    };
    let edit = hover_off("acct-edit", "pen", crate::t!("status.edit_profile")).on_click(cx.listener(|a, _, _, cx| {
        a.people.close_overlays();
        crate::ui::settings::open(a, cx);
        if let Some(s) = a.settings.as_mut() {
            s.page = crate::ui::settings::Page::Profiles;
        }
    }));
    let badges = hover_off("acct-badges", "ribbon", crate::t!("status.badges"))
        .on_click(cx.listener(|a, _, _, cx| a.stub(&crate::t!("status.badges"), cx)));
    let wallet_open = WALLET_SUB.with(|c| c.get());
    let orbs = crate::ui::shop_page::orbs_balance(app, cx).map(|n| {
        pop_row("acct-orbs", icon("orbs"), text(crate::t!("quests.orbs", n = n)), None, true)
            .relative()
            .on_hover(|h, w, _| {
                if *h {
                    set_wallet(true, w)
                }
            })
            .on_click(|_, w, _| set_wallet(!WALLET_SUB.with(|c| c.get()), w))
            .when(wallet_open, |d| {
                d.child(deferred(anchored().position_mode(gpui::AnchoredPositionMode::Local).position(point(px(252. + 12.), px(-16.))).snap_to_window().child(orb_wallet(n, cx))))
            })
    });
    let accounts_open = ACCOUNTS_SUB.with(|c| c.get());
    let switch = pop_row("acct-switch", icon("user-circle"), text(crate::t!("status.switch_accounts")), None, true)
        .relative()
        .on_hover(|h, w, _| {
            if *h {
                set_accounts_sub(true, w)
            }
        })
        .on_click(cx.listener(|a, _, w, cx| {
            set_accounts_sub(false, w);
            a.open_switch_accounts(cx);
        }))
        .when(accounts_open, |d| {
            d.child(deferred(
                anchored().position_mode(gpui::AnchoredPositionMode::Local).position(point(px(252. + 12.), px(-8.))).snap_to_window().child(accounts_submenu(app, cx)),
            ))
        });
    let my_id = me.0.to_string();
    let copy = hover_off("acct-copy-id", "id", crate::t!("common.copy_user_id")).on_click(cx.listener(move |a, _, _, cx| {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(my_id.clone()));
        a.people.close_overlays();
        cx.notify();
    }));
    let menus = div()
        .flex()
        .flex_col()
        .gap(z(12.))
        .child(pop_group([edit.into_any_element(), status_row.into_any_element(), badges.into_any_element()].into_iter().chain(orbs.map(IntoElement::into_any_element)).collect()))
        .child(pop_group(vec![switch.into_any_element(), copy.into_any_element()]));

    let mut body = div()
        .id("acct-body")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&crate::ui::chat::scrollbar::handle("acct-body"))
        .pt(z(4.))
        .pr(z(8.))
        .pb(z(16.))
        .pl(z(16.))
        .flex()
        .flex_col()
        .gap(z(12.))
        .child(div().mb(z(-4.)).child(name_block(&i, false, Some(copy_name))));
    if let Some(bio) = i.prof.map(|p| p.bio.clone()).filter(|b| !b.is_empty()) {
        body = body.child(crate::ui::profile::bio_block(bio, true));
    }
    if let Some(w) = crate::ui::profile_board::get(app, me, cx).widgets.first() {
        let col = crate::ui::settings::profile_widgets::WidgetColors {
            strong: c(tok(idx::TEXT_STRONG)),
            muted: c(tok(idx::TEXT_MUTED)),
            card: c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND)),
            more: false,
        };
        body = body.child(crate::ui::settings::profile_widgets::render_compact(w, &col));
    }
    if app.presence != Presence::Offline {
        for a in i.acts {
            if let Some(b) = crate::ui::profile::activity_block(a, false, false) {
                body = body.child(div().pb(z(8.)).child(b));
            }
        }
        body = match crate::ui::voice_row::stream_activity_card(app, me, cx.entity(), cx) {
            Some(card) => body.child(card),
            None => body.children(crate::ui::voice_row::voice_activity_card(app, me, cx.entity())),
        };
    }
    let body = body.child(menus);

    let max_h = window.viewport_size().height - px(116.);
    let card = div()
        .id("status-picker")
        .group(CARD_GROUP)
        .relative()
        .w(z(304.))
        .max_h(max_h)
        .p(z(2.))
        .rounded(z(8.))
        .overflow_hidden()
        .when(theme.is_none(), |d| d.bg(c(surface)).border_1().border_color(c(tok(idx::BORDER_SUBTLE))))
        .when_some(theme, |d, t| d.children(theme_backdrop(t, 0.)))
        .shadow_lg()
        .occlude()
        .child(
            div()
                .relative()
                .max_h(max_h - px(4.))
                .rounded(z(6.))
                .overflow_hidden()
                .when_some(theme, |d, t| d.bg(crate::ui::profile_theme::overlay(t)))
                .pb(z(4.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(header.children(bubble))
                .child(crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::Thin).flex_1().min_h_0().child(body).children(crate::ui::chat::scrollbar::bar_for("acct-body", crate::ui::chat::scrollbar::Bar::Thin))),
        );

    div().absolute().left(px(10.)).bottom(px(76.)).child(card).into_any_element()
}
