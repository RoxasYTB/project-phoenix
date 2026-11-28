const ROW_RADIUS: f32 = 8.;

use crate::ui::tooltip::Tipped as _;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled,
    Transformation, div, list, prelude::*, px,
};

use crate::{
    app::{PhoenixApp, SidebarRow},
    ui::home::data::HomeView,
    assets::Icon,
    model::{ChannelKind, Id},
    theme::{Palette, c, generated::idx, m, space, tok, z},
    ui::{badge, icon},
};

fn t(i: usize) -> gpui::Hsla {
    c(tok(i))
}

pub fn channel_icon(kind: ChannelKind) -> Icon {
    match kind {
        ChannelKind::Text | ChannelKind::Category => Icon::Hash,
        ChannelKind::Voice => Icon::Speaker,
        ChannelKind::Announcement => Icon::Megaphone,
        ChannelKind::Forum => Icon::Forum,
        ChannelKind::Stage => Icon::Stage,
    }
}

pub fn drop_empty_categories(rows: &[SidebarRow], collapsed: &std::collections::HashSet<Id>) -> Vec<SidebarRow> {
    rows.iter()
        .enumerate()
        .filter(|(i, r)| match r {
            SidebarRow::Category(cat) => !collapsed.contains(cat) && matches!(rows.get(i + 1), Some(SidebarRow::Channel(_))),
            _ => true,
        })
        .map(|(_, r)| *r)
        .collect()
}

pub fn build_rows(app: &mut PhoenixApp) {
    let anchor = app.sidebar_list.logical_scroll_top();
    let anchor_row = anchor.item_ix.checked_sub(1).and_then(|i| app.sidebar_rows.get(i).copied());
    let old_guild = app.sidebar_rows_guild;
    app.sidebar_rows_guild = if app.favorites_view { Some(crate::ui::favorites::GUILD) } else { app.selected_guild };
    app.sidebar_rows.clear();
    match app.selected_guild {
        _ if app.favorites_view => {
            app.sidebar_rows = crate::ui::favorites::rows(app);
        }
        None => {
            let (reqs, spam) = (&app.nav.message_requests, &app.nav.spam_requests);
            let rows: Vec<SidebarRow> =
                app.dms.iter().copied()
                    .filter(|id| !reqs.contains(id) && !spam.contains(id))
                    .filter(|id| app.channels.get(id).is_some_and(|channel| channel.guild.is_none()))
                    .map(SidebarRow::Channel)
                    .collect();
            let mut rows = rows;
            crate::actions::pins::apply(&mut rows);
            app.sidebar_rows = rows;
        }
        Some(g) => {
            let hide_muted = app.nav.hide_muted.contains(&g);
            let ids = app.guild_channels.get(&g).cloned().unwrap_or_default();
            let mut current_cat: Option<Id> = None;
            let mut rows = Vec::with_capacity(ids.len());
            for id in ids {
                let Some(ch) = app.channels.get(&id) else { continue };
                if ch.kind == ChannelKind::Category {
                    current_cat = Some(id);
                    rows.push(SidebarRow::Category(id));
                    continue;
                }
                let selected = Some(id) == app.selected_channel
                    || app.selected_channel.and_then(|s| app.nav.thread_parent(s)) == Some(id);
                if !crate::ui::view_as::visible(app, g, id) {
                    continue;
                }
                let collapsed = current_cat.is_some_and(|c| app.collapsed.contains(&c));
                let all_threads = selected || (!collapsed && !ch.muted);
                let threads: Vec<Id> = app
                    .nav
                    .threads
                    .get(&id)
                    .into_iter()
                    .flatten()
                    .filter(|t| all_threads || (t.unread && !t.muted) || Some(t.id) == app.selected_channel)
                    .map(|t| t.id)
                    .collect();
                if collapsed && !selected && threads.is_empty() && !(ch.unread && !ch.muted) && ch.voice_users.is_empty() {
                    continue;
                }
                if hide_muted && ch.muted && !selected && threads.is_empty() {
                    continue;
                }
                rows.push(SidebarRow::Channel(id));
                rows.extend(threads.into_iter().map(SidebarRow::Thread));
                let stage = ch.kind == ChannelKind::Stage;
                let mut vu: Vec<((bool, String, String), Id)> = ch
                    .voice_users
                    .iter()
                    .filter(|u| !stage || !crate::ui::voice_panel::flags(**u).suppress)
                    .map(|u| (voice_sort_key(app, *u), *u))
                    .collect();
                vu.sort();
                let take = if crate::ui::voice_row::names_hidden(id) { 1 } else { usize::MAX };
                rows.extend(vu.into_iter().take(take).map(|(_, u)| SidebarRow::VoiceUser { user: u }));
                if stage && ch.voice_users.iter().any(|u| crate::ui::voice_panel::flags(*u).suppress) {
                    rows.push(SidebarRow::StageAudience(id));
                }
            }
            if hide_muted {
                let keep: Vec<SidebarRow> = rows
                    .iter()
                    .enumerate()
                    .filter(|(i, r)| match r {
                        SidebarRow::Category(cat) => {
                            app.collapsed.contains(cat) || matches!(rows.get(i + 1), Some(SidebarRow::Channel(_)))
                        }
                        _ => true,
                    })
                    .map(|(_, r)| *r)
                    .collect();
                rows = keep;
            }
            if crate::ui::view_as::roles_for(g).is_some() {
                rows = drop_empty_categories(&rows, &app.collapsed);
            }
            crate::actions::pins::apply(&mut rows);
            app.sidebar_rows = rows;
        }
    }
    app.sidebar_list.reset(app.sidebar_rows.len() + 1);
    if old_guild == app.sidebar_rows_guild {
        let new_ix = match anchor_row {
            Some(r) => app.sidebar_rows.iter().position(|x| *x == r).map(|i| i + 1),
            None => Some(0),
        };
        if let Some(item_ix) = new_ix {
            app.sidebar_list.scroll_to(gpui::ListOffset { item_ix, offset_in_item: anchor.offset_in_item });
        }
    }
}

pub fn voice_sort_key(app: &PhoenixApp, user: Id) -> (bool, String, String) {
    (!crate::ui::voice_panel::flags(user).stream, voice_user_name(app, user).to_lowercase(), user.0.to_string())
}

pub fn voice_user_name(app: &PhoenixApp, user: Id) -> SharedString {
    app.member_nick(user)
        .or_else(|| app.users.get(&user).map(|u| u.display_name.clone()))
        .unwrap_or_default()
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if crate::ui::discovery::current().is_some() {
        return div()
            .w(z(crate::ui::sidebar_resize::width() + m::sidebar_extra_w()))
            .flex_none()
            .h_full()
            .rounded_tl(z(m::FRAME_RADIUS))
            .overflow_hidden()
            .relative()
            .child(crate::ui::discovery::sidebar(cx))
            .child(div().absolute().inset_0().border_l_1().border_t_1().border_color(c(Palette::APP_FRAME_BORDER)).rounded_tl(z(m::FRAME_RADIUS)))
            .child(crate::ui::sidebar_resize::handle(cx))
            .into_any_element();
    }
    if app.selected_guild.is_none() && !app.favorites_view {
        crate::ui::quests_page::ensure_loaded(app, cx);
        crate::ui::quests_page::ensure_hero(app, cx);
    }
    let entity = cx.entity();
    let weak = entity.downgrade();
    app.sidebar_list.set_scroll_handler(move |ev: &gpui::ListScrollEvent, _w, cx: &mut App| {
        if ev.visible_range.start <= 1 {
            let _ = weak.update(cx, |_, cx| cx.notify());
        }
    });
    crate::ui::a11y::named(div().id("channels-nav"), gpui::Role::Navigation, || match app.selected_guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)) {
        Some(g) => crate::t!("a11y.guild_nav", name = g.name),
        None => crate::t!("a11y.direct_messages"),
    })
        .w(z(crate::ui::sidebar_resize::width() + m::sidebar_extra_w()))
        .flex_none()
        .h_full()
        .flex()
        .flex_col()
        .bg(crate::theme::client_theme::surface(crate::theme::client_theme::Level::AppFrame, Some(crate::theme::client_theme::Level::AppFrame), Palette::BG_TERTIARY))
        .rounded_tl(z(m::FRAME_RADIUS))
        .overflow_hidden()
        .relative()
        .child(header(app, cx))
        .child(
            crate::ui::a11y::named(div().id("channels-list"), gpui::Role::List, || crate::t!("a11y.channels"))
                .flex_1()
                .min_h_0()
                .relative()
                .group(crate::ui::chat::scrollbar::THIN_FADE_GROUP)
                .when(app.favorites_view, |d| {
                    d.on_mouse_down(gpui::MouseButton::Right, |ev, w, _| {
                        crate::ui::nav_menus::open(crate::ui::nav_menus::Target::FavoritesAdd, ev.position, w)
                    })
                })
                .child(list(app.sidebar_list.clone(), move |ix, _w, cx| row(&entity, ix, cx)).size_full())
                .children(crate::ui::chat::scrollbar::thin_fade_scrollbar(&app.sidebar_list))
                .children(crate::ui::channel_unread_bar::render(app)),
        )
        .when(app.favorites_view && app.nav.guild_menu, |d| d.child(crate::ui::guild_menu::render_favorites(m::HEADER_H + 4., cx)))
        .when_some(app.selected_guild.filter(|_| app.nav.guild_menu && !app.favorites_view), |d, g| {
            d.child(crate::ui::guild_menu::render(app, g, m::HEADER_H + 4., cx))
        })
        .child(
            div()
                .absolute()
                .inset_0()
                .border_l_1()
                .border_t_1()
                .border_color(c(Palette::APP_FRAME_BORDER))
                .rounded_tl(z(m::FRAME_RADIUS)),
        )
        .child(crate::ui::sidebar_resize::handle(cx))
        .into_any_element()
}

fn favorites_header(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let menu_open = app.nav.guild_menu;
    let fg = t(idx::TEXT_STRONG);
    div()
        .h(px(m::HEADER_H + 1.))
        .flex_none()
        .p(z(10.))
        .flex()
        .items_center()
        .justify_between()
        .gap(z(4.))
        .border_b_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .child(
            div()
                .id("favorites-header")
                .h(z(32.))
                .min_w_0()
                .px(z(10.))
                .flex()
                .items_center()
                .gap(z(2.))
                .rounded(z(8.))
                .cursor_pointer()
                .hover(|s| s.bg(t(idx::BACKGROUND_MOD_NORMAL)))
                .when(menu_open, |d| d.bg(t(idx::BACKGROUND_MOD_NORMAL)))
                .on_click(cx.listener(|a, _, _, cx| {
                    a.nav.guild_menu = !a.nav.guild_menu;
                    cx.notify();
                }))
                .child(
                    div()
                        .min_w_0()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(fg)
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(crate::t!("fav.title")),
                )
                .child(icon(Icon::Chevron, 16. * crate::theme::zoom(), fg).when(menu_open, |i| i.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI))))),
        )
        .child(
            div()
                .id("favorites-add")
                .tooltip(crate::ui::tooltip::tip(crate::t!("fav.add_title")))
                .size(z(32.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(z(8.))
                .cursor_pointer()
                .hover(|s| s.bg(t(idx::BACKGROUND_MOD_NORMAL)))
                .on_mouse_down(gpui::MouseButton::Left, |ev, w, cx| {
                    cx.stop_propagation();
                    let pos = gpui::point(ev.position.x - z(16.), ev.position.y + z(20.));
                    crate::ui::nav_menus::open(crate::ui::nav_menus::Target::FavoritesAdd, pos, w);
                })
                .child(gpui::svg().path(crate::icons::path("plus-medium")).size(z(20.)).flex_none().text_color(fg)),
        )
        .into_any_element()
}

fn header(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if app.favorites_view {
        return favorites_header(app, cx);
    }
    match app.selected_guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)) {
        None => div()
            .h(px(m::HEADER_H + 1.))
            .flex_none()
            .px(z(10.))
            .flex()
            .items_center()
            .border_b_1()
            .border_color(t(idx::BORDER_SUBTLE))
            .child(
                div()
                    .w_full()
                    .h(z(31.))
                    .rounded(z(8.))
                    .bg(t(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
                    .border_1()
                    .border_color(t(idx::BORDER_MUTED))
                    .px(z(11.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t(idx::TEXT_STRONG))
                    .child(div().min_w_0().text_ellipsis().child(crate::t!("dm.find"))),
            )
            .id("dm-search")
            .cursor_pointer()
            .on_click(cx.listener(|a, _, w, cx| a.open_switcher(w, cx)))
            .into_any_element(),
        Some(g) => {
            let hash = app.nav.banners.get(&g.id).cloned();
            let full_h = ((crate::ui::sidebar_resize::width() + m::sidebar_extra_w()) * 9. / 21.).round();
            let top = app.sidebar_list.logical_scroll_top();
            let scrolled = if top.item_ix > 0 { f32::MAX } else { f32::from(top.offset_in_item) };
            let banner_h = full_h - scrolled.min(full_h);
            let banner = hash
                .as_ref()
                .filter(|_| banner_h > m::HEADER_H)
                .map(|_| g.banner_color.unwrap_or(Palette::BG_SECONDARY));
            let menu_open = app.nav.guild_menu;
            let gid = g.id.0;
            let mut features = app.backend.guild_features(g.id);
            if g.discoverable {
                features.push("DISCOVERABLE".into());
            }
            let badge_kind = crate::ui::guild_badge::kind(&features);
            let guild_badge_el = crate::ui::guild_badge::element(badge_kind, crate::ui::guild_badge::premium(g.boosts, g.boost_tier), 18.);
            div()
                .id("guild-banner")
                .group("guild-banner")
                .flex_none()
                .relative()
                .when_some(banner, |d, col| {
                    d.h(px(banner_h))
                        .bg(c(col))
                        .overflow_hidden()
                        .when_some(hash.clone(), |d, hash| {
                            let url = |ext: &str| -> SharedString {
                                format!("https://cdn.discordapp.com/banners/{gid}/{hash}.{ext}?size=480").into()
                            };
                            let animated = hash.starts_with("a_");
                            d.child(gpui::img(url("png")).absolute().bottom_0().left_0().right_0().w_full().h(px(full_h)).object_fit(gpui::ObjectFit::Cover))
                                .when(animated, |d| {
                                    d.child(
                                        div()
                                            .absolute()
                                            .inset_0()
                                            .opacity(0.)
                                            .group_hover("guild-banner", |s| s.opacity(1.))
                                            .child(gpui::img(url("gif")).id("banner-frames").size_full().object_fit(gpui::ObjectFit::Cover)),
                                    )
                                })
                        })
                        .child(
                            div().absolute().top_0().left_0().right_0().h(z(56.)).bg(gpui::linear_gradient(
                                180.,
                                gpui::linear_color_stop(gpui::black().opacity(0.4), 0.),
                                gpui::linear_color_stop(gpui::transparent_black(), 1.),
                            )),
                        )
                        .child(
                            div().absolute().bottom_0().left_0().right_0().h(z(40.)).bg(gpui::linear_gradient(
                                180.,
                                gpui::linear_color_stop(gpui::transparent_black(), 0.),
                                gpui::linear_color_stop(c(Palette::BG_TERTIARY).opacity(0.8), 1.),
                            )),
                        )
                })
                .child({
                    let fg = if banner.is_some() { gpui::white() } else { t(idx::TEXT_STRONG) };
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .h(px(m::HEADER_H + 1.))
                        .p(z(10.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(z(4.))
                        .when(banner.is_none(), |d| d.border_b_1().border_color(t(idx::BORDER_SUBTLE)))
                        .on_mouse_down(gpui::MouseButton::Right, move |ev, w, cx| {
                            cx.stop_propagation();
                            crate::ui::nav_menus::open(crate::ui::nav_menus::Target::Guild(crate::model::Id(gid)), ev.position, w)
                        })
                        .child(
                            crate::ui::a11y::named(div().id("guild-header"), gpui::Role::Button, || crate::t!("a11y.guild_actions", name = g.name))
                                .when(crate::ui::a11y::on(), |d| d.aria_expanded(menu_open))
                                .h(z(32.))
                                .min_w_0()
                                .px(z(10.))
                                .flex()
                                .items_center()
                                .gap(z(2.))
                                .rounded(z(8.))
                                .cursor_pointer()
                                .hover(|s| s.bg(t(idx::BACKGROUND_MOD_NORMAL)))
                                .when(menu_open, |d| d.bg(t(idx::BACKGROUND_MOD_NORMAL)))
                                .on_click(cx.listener(|a, _, _, cx| {
                                    a.nav.guild_menu = !a.nav.guild_menu;
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .min_w_0()
                                        .flex()
                                        .items_center()
                                        .gap(z(10.))
                                        .when_some(guild_badge_el, |d, b| d.child(div().w(z(20.)).flex_none().flex().justify_center().child(b)))
                                        .when(badge_kind == crate::ui::guild_badge::Kind::Discoverable, |d| {
                                            d.child(
                                                div().w(z(20.)).flex_none().flex().justify_center().child(
                                                    div()
                                                        .relative()
                                                        .size(z(18.))
                                                        .child(
                                                            gpui::svg()
                                                                .path(crate::icons::path("discovery-badge"))
                                                                .absolute()
                                                                .size(z(18.))
                                                                .text_color(t(idx::BADGE_INVERTED_BACKGROUND)),
                                                        )
                                                        .child(
                                                            gpui::svg()
                                                                .path(crate::icons::path("discovery-badge-fg"))
                                                                .absolute()
                                                                .size(z(18.))
                                                                .text_color(t(idx::BADGE_INVERTED_TEXT)),
                                                        ),
                                                ),
                                            )
                                        })
                                        .child(
                                    div()
                                        .min_w_0()
                                        .text_size(z(16.))
                                        .line_height(z(20.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(fg)
                                        .whitespace_nowrap()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(g.name.clone()),
                                        ),
                                )
                                .child(icon(Icon::Chevron, 16. * crate::theme::zoom(), fg).when(menu_open, |i| i.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI))))),
                        )
                        .child(
                            crate::ui::a11y::named(div().id("guild-invite"), gpui::Role::Button, || crate::t!("a11y.invite_server"))
                                .tooltip(crate::ui::tooltip::tip(crate::t!("menu.invite")))
                                .on_click(cx.listener(move |a, _, _, cx| a.open_invite_people(Id(gid), cx)))
                                .size(z(32.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(z(8.))
                                .cursor_pointer()
                                .hover(|s| s.bg(t(idx::BACKGROUND_MOD_NORMAL)))
                                .child(
                                    gpui::svg()
                                        .path(crate::icons::path("invite"))
                                        .size(z(20.))
                                        .flex_none()
                                        .text_color(fg),
                                ),
                        )
                })
                .when(banner.is_none(), |d| d.h(px(m::HEADER_H + 1.)))
                .into_any_element()
        }
    }
}

fn row(entity: &Entity<PhoenixApp>, ix: usize, cx: &mut App) -> AnyElement {
    let app = entity.read(cx);
    if ix == 0 {
        return if app.favorites_view {
            div().w_full().pt(z(12.)).into_any_element()
        } else if app.selected_guild.is_none() {
            dm_top(entity, cx).into_any_element()
        } else {
            guild_top(app, entity.clone(), cx).into_any_element()
        };
    }
    let Some(r) = app.sidebar_rows.get(ix - 1).copied() else {
        return div().into_any_element();
    };
    if !cx.has_active_drag() {
        crate::ui::channel_dnd::set_over(None);
    }
    match r {
        SidebarRow::Category(id) => reorderable(app, entity, id, category_row(app, entity, id)),
        SidebarRow::Channel(id) => reorderable(app, entity, id, channel_row(app, entity, id)),
        _ => other_row(app, entity, r, ix),
    }
}

fn category_row(app: &PhoenixApp, entity: &Entity<PhoenixApp>, id: Id) -> AnyElement {
    let ch = &app.channels[&id];
    let collapsed = app.collapsed.contains(&id);
    let fav_cat = ch.guild == Some(crate::ui::favorites::GUILD);
    let cat_guild = ch.guild;
    let manage = cat_guild.is_some_and(|g| crate::ui::inbox::has_perm(app.backend.guild_permissions(g), 1 << 4));
    let add_entity = entity.clone();
    let e = entity.clone();
    let muted = ch.muted && !fav_cat;
    let rest = if muted { idx::TEXT_MUTED } else { idx::CHANNELS_DEFAULT };
    crate::ui::a11y::named(div().id(("cat", id.0)), gpui::Role::Button, || crate::ui::a11y::category(&ch.name))
        .when(crate::ui::a11y::on(), |d| d.aria_expanded(!collapsed))
        .group("cat")
        .pt(z(space::category_before()))
        .pl(z(20.))
        .pr(z(6. + 8.))
        .h(z(24. + space::category_before()))
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .cursor_pointer()
        .text_color(t(rest))
        .hover(|s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER)))
        .on_click(move |_, _, cx| e.update(cx, |a, cx| a.toggle_category_saved(id, cx)))
        .when(!fav_cat, |d| {
            d.on_mouse_down(gpui::MouseButton::Right, move |ev, w, cx| {
                cx.stop_propagation();
                crate::ui::nav_menus::open(crate::ui::nav_menus::Target::Channel(id), ev.position, w)
            })
        })
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap(z(4.))
                .child(
                    div()
                        .min_w_0()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_ellipsis()
                        .when(muted, |d| d.opacity(0.5))
                        .child(crate::ui::emoji_text::label(ch.name.clone(), 14.)),
                )
                .child(
                    icon(Icon::Chevron, 12., t(rest))
                        .when(muted, |d| d.opacity(0.5))
                        .group_hover("cat", |s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER)))
                        .with_transformation(Transformation::rotate(gpui::radians(if collapsed {
                            -std::f32::consts::FRAC_PI_2
                        } else {
                            0.
                        }))),
                ),
        )
        .when(!fav_cat && manage, |d| d.child(
            div()
                .size(z(24.))
                .flex_none()
                .p(z(4.))
                .id(("cat-add", id.0))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    if let Some(g) = cat_guild {
                        add_entity.update(cx, |a, cx| a.open_create_channel(g, Some(id), ChannelKind::Text, cx));
                    }
                })
                .tooltip(crate::ui::tooltip::tip(crate::t!("menu.create_channel")))
                .group("cat-add")
                .active(|s| s.pt(z(4.5)))
                .child(
                    icon(Icon::Plus, 16. * crate::theme::zoom(), t(idx::CHANNELS_DEFAULT))
                        .group_hover("cat-add", |s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER))),
                ),
        ))
        .into_any_element()
}

fn reorder_slots(app: &PhoenixApp, g: Id) -> Vec<crate::ui::channel_dnd::Slot> {
    app.guild_channels
        .get(&g)
        .into_iter()
        .flatten()
        .filter_map(|id| app.channels.get(id))
        .map(|c| crate::ui::channel_dnd::Slot { id: c.id, kind: c.kind, parent: c.parent, position: c.position })
        .collect()
}

#[derive(Clone)]
pub struct DraggedChannel {
    pub id: Id,
    name: SharedString,
}

impl gpui::Render for DraggedChannel {
    fn render(&mut self, _w: &mut gpui::Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .h(z(32.))
            .px(z(8.))
            .rounded(z(4.))
            .flex()
            .items_center()
            .bg(t(idx::BACKGROUND_SURFACE_HIGH))
            .text_size(z(16.))
            .line_height(z(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t(idx::INTERACTIVE_TEXT_ACTIVE))
            .opacity(0.8)
            .child(self.name.clone())
    }
}

fn reorder_line(after: bool) -> gpui::Div {
    let line = div()
        .absolute()
        .left(z(8.))
        .right_0()
        .h(z(4.))
        .rounded(z(2.))
        .bg(t(idx::GREEN_360))
        .shadow(vec![gpui::BoxShadow {
            color: t(idx::OPACITY_BLACK_40),
            offset: gpui::point(px(0.), px(0.)),
            blur_radius: z(3.),
            spread_radius: px(0.),
            inset: false,
        }]);
    if after { line.bottom(z(-2.)) } else { line.top(z(-2.)) }
}

fn reorderable(app: &PhoenixApp, entity: &Entity<PhoenixApp>, id: Id, el: AnyElement) -> AnyElement {
    let Some(ch) = app.channels.get(&id) else { return el };
    let Some(g) = ch.guild.filter(|g| *g != crate::ui::favorites::GUILD) else { return el };
    if !crate::ui::inbox::has_perm(app.backend.guild_permissions(g), 1 << 4) {
        return el;
    }
    let slots = reorder_slots(app, g);
    let mark = crate::ui::channel_dnd::over().filter(|(_, target)| *target == id).map(|(dragged, _)| {
        reorder_line(match crate::ui::channel_dnd::category_drop(&slots, dragged, id) {
            Some(d) => d.after,
            None => crate::ui::channel_dnd::lands_after(&slots, dragged, id),
        })
    });
    let drop_e = entity.clone();
    div()
        .id(("channel-dnd", id.0))
        .w_full()
        .relative()
        .on_drag(DraggedChannel { id, name: ch.name.clone() }, |d: &DraggedChannel, _, _, cx: &mut App| cx.new(|_| d.clone()))
        .on_drag_move::<DraggedChannel>(move |ev, window, cx| {
            let dragged = ev.drag(cx).id;
            let next = if ev.bounds.contains(&ev.event.position) {
                (crate::ui::channel_dnd::can_drop(&slots, dragged, id) || crate::ui::channel_dnd::category_drop(&slots, dragged, id).is_some())
                    .then_some((dragged, id))
            } else if crate::ui::channel_dnd::over().is_some_and(|(_, t)| t == id) {
                None
            } else {
                return;
            };
            if crate::ui::channel_dnd::set_over(next) {
                window.refresh();
            }
        })
        .on_drop(move |d: &DraggedChannel, _, cx| {
            crate::ui::channel_dnd::set_over(None);
            drop_e.update(cx, |a, cx| a.drop_channel(d.id, id, cx))
        })
        .child(el)
        .children(mark)
        .into_any_element()
}

impl PhoenixApp {
    pub fn drop_channel(&mut self, dragged: Id, target: Id, cx: &mut gpui::Context<Self>) {
        cx.notify();
        let Some(g) = self.channels.get(&target).and_then(|c| c.guild) else { return };
        let slots = reorder_slots(self, g);
        if let Some(d) = crate::ui::channel_dnd::category_drop(&slots, dragged, target) {
            self.drop_into_category(g, dragged, d, &slots, cx);
            return;
        }
        let Some(body) = crate::ui::channel_dnd::reorder_body(&slots, dragged, target) else { return };
        self.blocked_write(crate::actions::guard::GuardedWrite::batch_channel_update(g.0, body), cx);
    }

    fn drop_into_category(&mut self, g: Id, dragged: Id, d: crate::ui::channel_dnd::CategoryDrop, slots: &[crate::ui::channel_dnd::Slot], cx: &mut gpui::Context<Self>) {
        let old_parent = self.channels.get(&dragged).and_then(|c| c.parent);
        let moved = old_parent != d.parent;
        let perms = |c: Id| self.backend.channel_permissions(c);
        if moved && let Some(cat) = d.parent {
            let ok = perms(cat).is_none_or(|p| p & (1 << 10) != 0 && p & (1 << 4) != 0);
            if !ok {
                return;
            }
        }
        let Some(body) = crate::ui::channel_dnd::move_body(slots, dragged, d.reference, d.parent) else { return };
        let category = d.parent.filter(|_| moved);
        let prompt = category.is_some_and(|cat| {
            let roles = |c: Id| perms(c).is_some_and(|p| p & (1 << 28) != 0);
            crate::ui::channel_dnd::wants_sync_prompt(
                roles(dragged) && roles(cat),
                self.backend.overwrites_match(dragged, cat),
                old_parent,
                old_parent.and_then(|p| self.backend.overwrites_match(dragged, p)),
            )
        });
        match category {
            Some(cat) if prompt => {
                SYNC_PROMPT.with(|p| *p.borrow_mut() = Some(SyncPrompt { guild: g, channel: dragged, category: cat, body }));
            }
            _ => self.blocked_write(crate::actions::guard::GuardedWrite::batch_channel_update(g.0, body), cx),
        }
    }

    fn finish_sync_prompt(&mut self, sync: bool, cx: &mut gpui::Context<Self>) {
        let Some(mut p) = SYNC_PROMPT.with(|s| s.borrow_mut().take()) else { return };
        if sync && let Some(items) = p.body.as_array_mut() {
            let id = serde_json::json!(p.channel.0.to_string());
            if let Some(m) = items.iter_mut().find(|m| m.get("id") == Some(&id)) {
                m["lock_permissions"] = serde_json::json!(true);
            }
        }
        self.blocked_write(crate::actions::guard::GuardedWrite::batch_channel_update(p.guild.0, p.body), cx);
        cx.notify();
    }
}

struct SyncPrompt {
    guild: Id,
    channel: Id,
    category: Id,
    body: serde_json::Value,
}

thread_local! {
    static SYNC_PROMPT: std::cell::RefCell<Option<SyncPrompt>> = const { std::cell::RefCell::new(None) };
}

pub fn sync_prompt(app: &PhoenixApp, cx: &mut gpui::Context<PhoenixApp>) -> Option<AnyElement> {
    let (channel, category) = SYNC_PROMPT.with(|p| p.borrow().as_ref().map(|p| (p.channel, p.category)))?;
    let ch = app.channels.get(&channel)?;
    let channel_name = if crate::ui::channel_dnd::class(ch.kind) == crate::ui::channel_dnd::Class::Text {
        format!("#{}", ch.name)
    } else {
        ch.name.to_string()
    };
    let category_name = app.channels.get(&category).map(|c| c.name.to_string()).unwrap_or_default();
    let text = crate::t!("dnd.sync_body", channel = channel_name.as_str(), category = category_name.as_str()).to_string();
    let bold = gpui::HighlightStyle { font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() };
    let mut ranges = Vec::new();
    if let Some(a) = text.find(channel_name.as_str()) {
        ranges.push((a..a + channel_name.len(), bold));
        if let Some(b) = text[a + channel_name.len()..].find(category_name.as_str()).filter(|_| !category_name.is_empty()) {
            let b = b + a + channel_name.len();
            ranges.push((b..b + category_name.len(), bold));
        }
    }
    let (ok, cancel, close) = (cx.entity(), cx.entity(), cx.entity());
    Some(crate::ui::modal::confirm_labeled(
        crate::ui::modal::Confirm {
            title: crate::t!("dnd.sync_title"),
            body: vec![gpui::StyledText::new(text).with_highlights(ranges).into_any_element()],
            confirm_label: crate::t!("dnd.sync_confirm"),
            danger: false,
            on_confirm: std::rc::Rc::new(move |_, cx| ok.update(cx, |a, cx| a.finish_sync_prompt(true, cx))),
            on_cancel: std::rc::Rc::new(move |_, cx| cancel.update(cx, |a, cx| a.finish_sync_prompt(false, cx))),
        },
        crate::t!("dnd.sync_keep"),
        std::rc::Rc::new(move |_, cx| {
            SYNC_PROMPT.with(|p| *p.borrow_mut() = None);
            close.update(cx, |_, cx| cx.notify());
        }),
    ))
}

fn other_row(app: &PhoenixApp, entity: &Entity<PhoenixApp>, r: SidebarRow, ix: usize) -> AnyElement {
    match r {
        SidebarRow::Category(id) | SidebarRow::Channel(id) => channel_row(app, entity, id),
        SidebarRow::Thread(id) => thread_row(app, entity, id),
        SidebarRow::StageAudience(id) => crate::ui::voice_row::stage_audience(app, id),
        SidebarRow::VoiceUser { user } => {
            let hidden = app.channels.values().find(|c| c.voice_users.contains(&user) && crate::ui::voice_row::names_hidden(c.id));
            if let Some(ch) = hidden {
                let mut vu: Vec<((bool, String, String), Id)> =
                    ch.voice_users.iter().map(|u| (voice_sort_key(app, *u), *u)).collect();
                vu.sort();
                let users: Vec<Id> = vu.into_iter().map(|(_, u)| u).collect();
                return crate::ui::voice_row::collapsed_list(app, &users);
            }
            let last = !matches!(app.sidebar_rows.get(ix), Some(SidebarRow::VoiceUser { .. } | SidebarRow::StageAudience(_)));
            let row = crate::ui::voice_panel::voice_user_row(app, entity, user);
            div().w_full().flex().flex_col().when(last, |d| d.pb(z(10.))).child(row).into_any_element()
        }
    }
}

fn text_badge(label: SharedString, bg: usize, fg: usize) -> gpui::Div {
    div()
        .relative()
        .flex_none()
        .flex()
        .items_center()
        .px(z(8.))
        .rounded_full()
        .bg(t(bg))
        .text_color(t(fg))
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(SharedString::from(label.to_uppercase()))
}

fn dm_top(entity: &Entity<PhoenixApp>, cx: &App) -> impl IntoElement {
    let app = entity.read(cx);
    let home = app.nav.home;
    let requests = app.nav.message_requests.len() as u32;
    let pending = app.nav.pending_requests();
    let nitro_tier2 = app.backend.premium_tier2();
    let entry = crate::ui::quests_page::sidebar_entry(app.nav.quest_onboarding_dismissed);
    let nav = |id: &'static str, i: Icon, label: SharedString, view: Option<HomeView>, count: u32| {
        let selected = view.is_some_and(|v| v == home);
        let e = entity.clone();
        let stub = label.clone();
        let quest = if id == "quests" { entry.clone() } else { Default::default() };
        let strong = quest.onboarding;
        let hero = quest.hero.clone();
        div().w_full().pl(z(8.)).pr(z(8.)).py(z(1.)).child(
            div()
                .id(id)
                .when(id == "quests", |d| d.group("quests-row"))
                .when(id == "quests" && hero.is_some(), |d| {
                    crate::ui::states::track(d, crate::ui::states::key("quests-hero-hover"))
                })
                .relative()
                .when(strong || hero.is_some(), |d| d.overflow_hidden())
                .when(strong, |d| {
                    d.child(
                        div()
                            .absolute()
                            .top(z(1.))
                            .bottom(z(1.))
                            .left_0()
                            .right_0()
                            .rounded(z(8.))
                            .bg(gpui::linear_gradient(
                                90.,
                                gpui::linear_color_stop(gpui::rgb(0x394aff), 0.),
                                gpui::linear_color_stop(gpui::rgb(0xe901d9), 1.),
                            ))
                            .opacity(0.16)
                            .group_hover("quests-row", |s| s.opacity(0.2)),
                    )
                })
                .when_some(hero.as_ref().and_then(|h| h.image.clone()), |d, src| {
                    d.child(
                        div().absolute().top_0().bottom_0().right_0().child(crate::ui::states::opacity_fade(
                            crate::ui::states::key("quests-hero-hover"),
                            125,
                            div().h_full().flex().justify_end().child(gpui::img(src).h_full().object_fit(gpui::ObjectFit::Contain)),
                        )),
                    )
                })
                .h(z(38.))
                .pl(z(10.))
                .pr(z(16.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .gap(z(12.))
                .cursor_pointer()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t(if selected { idx::TEXT_DEFAULT } else if strong { idx::TEXT_STRONG } else { idx::TEXT_MUTED }))
                .when(selected, |d| d.bg(t(idx::INTERACTIVE_BACKGROUND_SELECTED)))
                .when(!selected, |d| {
                    let hover_fg = if id == "quests" { idx::TEXT_STRONG } else { idx::INTERACTIVE_TEXT_HOVER };
                    d.hover(move |s| s.bg(t(idx::INTERACTIVE_BACKGROUND_HOVER)).text_color(t(hover_fg)))
                        .active(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_ACTIVE)))
                })
                .on_click(move |_, _, cx| {
                    if id == "quests" {
                        crate::ui::quests_page::mark_hero_seen();
                    }
                    e.update(cx, |a, cx| match view {
                        Some(v) => a.open_home_view(v, cx),
                        None => a.stub(&stub, cx),
                    })
                })
                .child({
                    let base = if selected || strong { idx::ICON_STRONG } else { idx::ICON_MUTED };
                    let hover = hero.is_some() && !selected;
                    icon(i, 20. * crate::theme::zoom(), t(base)).when(hover, |s| s.group_hover("quests-row", |s| s.text_color(t(idx::ICON_STRONG))))
                })
                .child(div().flex_1().min_w_0().text_ellipsis().child(label))
                .when(quest.onboarding, |d| d.child(text_badge(crate::t!("dm.quests_new"), idx::BADGE_PRIMARY_BACKGROUND, idx::BADGE_PRIMARY_TEXT)))
                .when(quest.hero_badge, |d| d.child(text_badge(crate::t!("dm.quests_new"), idx::BADGE_EXPRESSIVE_BACKGROUND, idx::BADGE_EXPRESSIVE_TEXT)))
                .when(count > 0, |d| d.child(badge(count).bg(c(Palette::NOTIFICATION))))
                .when_some(hero.as_ref().and_then(|h| h.tooltip.clone()), |d, (title, note)| d.tip_rich(title, note, crate::ui::tooltip::Side::Top)),
        )
    };
    let e = entity.clone();
    div()
        .pt(z(8.))
        .w_full()
        .flex()
        .flex_col()
        .child(nav("friends", Icon::Friends, crate::t!("friends.title"), Some(HomeView::Friends), pending))
        .when(requests > 0, |d| {
            d.child(nav("msg-requests", Icon::Envelope, crate::t!("friends.message_requests"), Some(HomeView::MessageRequests), requests))
        })
        .child(nav("nitro", Icon::Nitro, if nitro_tier2 { crate::t!("dm.nitro") } else { crate::t!("dm.nitro_tab") }, Some(HomeView::Nitro), 0))
        .child(nav("shop", Icon::Shop, crate::t!("dm.shop"), Some(HomeView::Shop), 0))
        .child(nav("quests", Icon::Quests, crate::t!("dm.quests"), Some(HomeView::Quests), 0))
        .child(div().mx(z(8.)).my(z(12.)).h(z(1.)).bg(t(idx::BORDER_SUBTLE)))
        .child(
            div()
                .id("dm-header")
                .group("dm-header")
                .h(z(24.))
                .pl(z(20.))
                .pr(z(6.))
                .pb(z(4.))
                .flex()
                .justify_between()
                .items_center()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t(idx::CHANNELS_DEFAULT))
                .hover(|s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER)))
                .child(div().flex_1().min_w_0().mr(z(10.)).text_ellipsis().child(crate::t!("dm.header")))
                .child({
                    let btn = |id: &'static str, glyph: gpui::Svg| {
                        div()
                            .id(id)
                            .group(id)
                            .size(z(24.))
                            .p(z(4.))
                            .flex()
                            .cursor_pointer()
                            .child(glyph.size(z(16.)).text_color(t(idx::ICON_MUTED)).group_hover(id, |s| s.text_color(t(idx::ICON_SUBTLE))))
                    };
                    let e2 = e.clone();
                    div()
                        .flex()
                        .gap(z(4.))
                        .child(
                            btn("dm-more", gpui::svg().path(crate::icons::path("more-horizontal")))
                                .when(crate::ui::extra_menus::opened() != Some(crate::ui::extra_menus::Target::DmMore), |d| d.opacity(0.))
                                .group_hover("dm-header", |s| s.opacity(1.))
                                .tip(crate::t!("header.more"))
                                .on_click(move |ev, w, cx| {
                                    let p = ev.position();
                                    crate::ui::extra_menus::open(crate::ui::extra_menus::Target::DmMore, gpui::point(p.x - gpui::px(12.), p.y + gpui::px(20.)), w);
                                    e2.update(cx, |_, cx| cx.notify());
                                }),
                        )
                        .child(
                            btn("new-dm", gpui::svg().path(crate::icons::path("plus-medium")))
                                .tip(crate::t!("groupdm.create"))
                                .on_click(move |_, _, cx| e.update(cx, |a, cx| a.open_group_dm_picker(cx))),
                        )
                }),
        )
}

fn lock_icon_name(kind: ChannelKind) -> Option<&'static str> {
    Some(match kind {
        ChannelKind::Text => "text-lock",
        ChannelKind::Voice => "voice-lock",
        ChannelKind::Announcement => "announcement-lock",
        ChannelKind::Forum => "forum-lock",
        ChannelKind::Stage => "stage-lock",
        ChannelKind::Category => return None,
    })
}

pub(crate) fn channel_icon_override(kind: ChannelKind, rules: bool, locked: bool) -> Option<&'static str> {
    if rules { Some("rules-channel") } else { lock_icon_name(kind).filter(|_| locked) }
}

pub(crate) fn is_rules_channel(app: &PhoenixApp, ch: &crate::model::Channel) -> bool {
    app.guilds.iter().any(|guild| Some(guild.id) == ch.guild && guild.rules_channel == Some(ch.id))
}

fn decorated_icon(kind: ChannelKind, rules: bool, color: gpui::Hsla, locked: bool, nsfw: bool, bg: gpui::Hsla, hover: Option<SharedString>) -> AnyElement {
    let hov = move |g: gpui::Svg| match hover.clone() {
        Some(h) => g.group_hover(h, |s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER))),
        None => g,
    };
    div()
        .relative()
        .size(z(20.))
        .flex_none()
        .map(|d| match channel_icon_override(kind, rules, locked && !nsfw) {
            Some(name) => d.child(hov(gpui::svg().path(crate::icons::path(name)).size(z(20.)).flex_none().text_color(color))),
            None => d.child(hov(icon(channel_icon(kind), 20., color))),
        })
        .when(nsfw, |d| {
            d.child(
                div()
                    .absolute()
                    .right(z(-3.))
                    .top(z(-3.))
                    .size(z(11.))
                    .rounded_full()
                    .bg(bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(z(9.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(color)
                    .child("!"),
            )
        })
        .into_any_element()
}

fn user_limit_pill(users: u32, limit: u32) -> AnyElement {
    let cell = |txt: String| {
        div()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .text_size(z(12.))
            .line_height(z(16.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t(idx::INTERACTIVE_TEXT_DEFAULT))
            .child(txt)
    };
    let slant = gpui::canvas(
        |_, _, _| {},
        |b, _, window, _| {
            let (x, y, h) = (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.height));
            let w = f32::from(b.size.width);
            let mut pb = gpui::PathBuilder::fill();
            pb.move_to(gpui::point(gpui::px(x + w), gpui::px(y)));
            pb.line_to(gpui::point(gpui::px(x + w), gpui::px(y + h)));
            pb.line_to(gpui::point(gpui::px(x), gpui::px(y + h)));
            pb.close();
            if let Ok(p) = pb.build() {
                window.paint_path(p, t(idx::BACKGROUND_MOD_STRONG));
            }
        },
    )
    .absolute()
    .top_0()
    .left(z(-6.))
    .w(z(6.))
    .h_full();
    div()
        .flex_none()
        .ml(z(12.))
        .pr(z(4.))
        .child(
            div()
                .h(z(20.))
                .flex()
                .overflow_hidden()
                .rounded(z(crate::theme::generated::global::RADIUS_MD))
                .border_1()
                .border_color(t(idx::BORDER_NORMAL))
                .bg(t(idx::BACKGROUND_BASE_LOW))
                .child(cell(format!("{users:02}")).when(users < 100 && limit < 100, |d| d.w(z(28.))).px(z(4.)))
                .child(
                    cell(format!("{limit:02}"))
                        .relative()
                        .min_w(z(20.))
                        .pl(z(2.))
                        .pr(z(4.))
                        .rounded_r(z(crate::theme::generated::global::RADIUS_MD - 1.))
                        .bg(t(idx::BACKGROUND_MOD_STRONG))
                        .child(slant),
                ),
        )
        .into_any_element()
}

fn channel_row(app: &PhoenixApp, entity: &Entity<PhoenixApp>, id: Id) -> AnyElement {
    let Some(ch) = app.channels.get(&id) else { return div().into_any_element() };
    let selected = app.selected_channel == Some(id);
    let is_dm = ch.guild.is_none();
    let connected = !is_dm && app.voice_channel == Some(id);
    let extra = app.nav.extras.get(&id);
    let is_group = extra.is_some_and(|e| e.recipients > 1 || e.group_icon.is_some());
    let unread = ch.unread && !ch.muted && (is_dm || app.backend.unreads_all(id));
    let guild_dim = !is_dm && ch.muted && !selected;
    let dm_dim = is_dm && ch.muted && !selected;
    let fg = if (selected || connected) && !is_dm {
        tok(idx::TEXT_STRONG)
    } else if selected || connected || unread {
        tok(idx::INTERACTIVE_TEXT_ACTIVE)
    } else if ch.muted {
        tok(idx::TEXT_MUTED)
    } else {
        tok(idx::CHANNELS_DEFAULT)
    };
    let e = entity.clone();
    let lead: AnyElement = if is_dm && is_group {
        let ex = extra.cloned().unwrap_or_default();
        match ex.group_icon {
            Some(crate::model::ImageRef::Url(u)) => {
                let pic = gpui::img(u).size(z(32.)).rounded_full().flex_none();
                if app.typing.get(&id).is_some_and(|v| v.iter().any(|(who, _)| *who != app.me.id)) {
                    crate::ui::typing_cutout(pic, 32. * crate::theme::zoom(), crate::model::Presence::Online).into_any_element()
                } else {
                    pic.into_any_element()
                }
            }
            _ if !ex.members.is_empty() => {
                let size = 32. * crate::theme::zoom();
                let ring = if selected { t(idx::INTERACTIVE_BACKGROUND_SELECTED) } else { c(Palette::BG_TERTIARY) };
                if app.typing.get(&id).is_some_and(|v| v.iter().any(|(who, _)| *who != app.me.id)) {
                    crate::ui::typing_cutout(crate::ui::group_dm::group_avatar(app, id, size, ring), size, crate::model::Presence::Online).into_any_element()
                } else {
                    crate::ui::group_dm::group_avatar_with_status(app, id, size, ring)
                }
            }
            _ => div()
                .size(z(32.))
                .flex_none()
                .rounded_full()
                .bg(c(crate::theme::DEFAULT_AVATAR[(id.0 % 5) as usize]))
                .flex()
                .items_center()
                .justify_center()
                .child(icon(Icon::Members, 18., gpui::white()))
                .into_any_element(),
        }
    } else if is_dm {
        let u = ch.voice_users.first().and_then(|u| app.users.get(u));
        let p = ch.voice_users.first().map(|u| app.presence_of(*u)).unwrap_or_default();
        let g = SharedString::from(format!("chrow{}", id.0));
        let deco = crate::ui::avatar_deco::source(u, None);
        let typing = ch.voice_users.first().is_some_and(|r| app.typing.get(&id).is_some_and(|v| v.iter().any(|(who, _)| who == r)));
        if typing {
            crate::ui::avatar_typing(u, 32., p, deco).into_any_element()
        } else {
            crate::ui::avatar_with_status_deco(
                u,
                32.,
                p,
                c(Palette::BG_TERTIARY),
                move |d| {
                    if selected {
                        d.bg(t(idx::INTERACTIVE_BACKGROUND_SELECTED))
                    } else {
                        d.group_hover(g, |s| s.bg(t(idx::INTERACTIVE_BACKGROUND_HOVER)))
                    }
                },
                deco,
            )
            .into_any_element()
        }
    } else {
        let col = t(if app.voice_channel == Some(id) {
            idx::TEXT_FEEDBACK_POSITIVE
        } else if selected {
            idx::ICON_STRONG
        } else if ch.muted {
            idx::ICON_MUTED
        } else if unread {
            idx::ICON_SUBTLE
        } else {
            idx::CHANNEL_ICON
        });
        decorated_icon(
            ch.kind,
            is_rules_channel(app, ch),
            col,
            extra.is_some_and(|e| e.locked),
            extra.is_some_and(|e| e.nsfw),
            if selected { t(idx::INTERACTIVE_BACKGROUND_SELECTED) } else { c(Palette::BG_TERTIARY) },
            (!selected && !ch.muted && app.voice_channel != Some(id)).then(|| SharedString::from(format!("chrow{}", id.0))),
        )
    };
    let name: SharedString = extra
        .and_then(|e| e.group_name.clone())
        .filter(|_| is_dm)
        .or_else(|| crate::ui::dm_profile_panel::friend_nickname(app, ch))
        .or_else(|| crate::ui::favorites::nickname(app, id))
        .unwrap_or_else(|| ch.name.clone());
    let members: Option<SharedString> =
        (is_dm && is_group).then(|| crate::t!("invite.members", n = extra.map(|e| e.recipients + 1).unwrap_or(1)));
    let dm_sub = (is_dm && !is_group).then(|| ch.voice_users.first().and_then(|u| crate::ui::members::dm_sub_text(app, *u))).flatten();
    let dm_tag = (is_dm && !is_group)
        .then(|| ch.voice_users.first().and_then(|u| app.users.get(u)).and_then(|u| u.tag.clone()))
        .flatten();
    let group = SharedString::from(format!("chrow{}", id.0));
    let plated = is_dm
        && !is_group
        && ch.voice_users.first().and_then(|u| app.users.get(u)).is_some_and(|u| u.nameplate.is_some());
    let close = is_dm.then(|| {
        let e = entity.clone();
        let fg = if plated { gpui::hsla(0., 0., 1., 0.9) } else { c(fg) };
        let (plate_bg, plate_hover) = if crate::theme::is_light() {
            (gpui::hsla(0., 0., 1., 0.22), gpui::hsla(0., 0., 1., 0.33))
        } else {
            (gpui::hsla(0., 0., 0., 0.22), gpui::hsla(0., 0., 0., 0.33))
        };
        div()
            .flex()
            .flex_none()
            .justify_end()
            .items_center()
            .pl(z(4.))
            .when(plated && selected, |d| d.min_w(z(32.)))
            .when(plated && !selected, |d| d.group_hover(group.clone(), |s| s.min_w(z(32.))))
            .child(
                div()
                    .id(("dm-close", id.0))
                    .hidden()
                    .group_hover(group.clone(), |s| s.flex())
                    .size(z(24.))
                    .p(z(3.))
                    .mr(z(6.))
                    .when(!plated, |d| d.opacity(0.7).hover(|s| s.opacity(1.)))
                    .tip(if is_group { crate::t!("menu.leave_group") } else { crate::t!("menu.close_dm") })
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        e.update(cx, |a, cx| a.close_dm(id, cx));
                    })
                    .child(
                        div()
                            .p(z(1.))
                            .rounded_full()
                            .when(plated, |d| d.bg(plate_bg).hover(|s| s.bg(plate_hover)))
                            .child(icon(Icon::Close, 16., fg)),
                    ),
            )
    });
    let can_edit = ch.guild.is_some_and(|g| {
        crate::ui::inbox::has_perm(app.backend.guild_permissions(g), 1 << 4)
    });
    let action = |i: Icon, path: Option<&'static str>| {
        let col = t(if selected { idx::ICON_STRONG } else { idx::INTERACTIVE_TEXT_DEFAULT });
        let hov = move |s: gpui::StyleRefinement| s.text_color(t(if selected { idx::ICON_STRONG } else { idx::INTERACTIVE_TEXT_HOVER }));
        let act = |s: gpui::StyleRefinement| s.text_color(t(idx::INTERACTIVE_TEXT_ACTIVE)).mt(z(0.5));
        div().id(gpui::ElementId::Name(format!("ch-act-{}", path.unwrap_or("gear")).into())).size(z(24.)).p(z(4.)).flex_none().group("ch-action").child(match path {
            Some(p) => gpui::svg().path(crate::icons::path(p)).size(z(16.)).text_color(col).group_hover("ch-action", hov).id("g").group_active("ch-action", act).into_any_element(),
            None => icon(i, 16., col).group_hover("ch-action", hov).id("g").group_active("ch-action", act).into_any_element(),
        })
    };
    let vc_status = (ch.kind == ChannelKind::Voice).then(|| crate::voice::start_time::status(id.0)).flatten();
    let can_set_status = connected && app.backend.channel_permissions(id).is_none_or(|p| p & (1 << 48) != 0);
    let status_line = vc_status.is_some() || can_set_status;
    let timer = (matches!(ch.kind, ChannelKind::Voice | ChannelKind::Stage) && !ch.voice_users.is_empty())
        .then(|| crate::voice::start_time::get(id.0))
        .flatten()
        .map(|st| {
            div()
                .flex_none()
                .group_hover(group.clone(), |s| s.hidden())
                .ml(z(12.))
                .pr(z(4.))
                .whitespace_nowrap()
                .font_features(gpui::FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)])))
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::NORMAL)
                .text_color(t(idx::TEXT_FEEDBACK_POSITIVE))
                .child(crate::voice::start_time::format(crate::voice::start_time::now_s() - st))
        });
    let limit_pill = (timer.is_none() && ch.kind == ChannelKind::Voice)
        .then(|| app.backend.channel_user_limit(id))
        .flatten()
        .map(|limit| div().flex_none().group_hover(group.clone(), |s| s.hidden()).child(user_limit_pill(ch.voice_users.len() as u32, limit)));
    let has_info = timer.is_some() || limit_pill.is_some();
    let is_voice = matches!(ch.kind, ChannelKind::Voice | ChannelKind::Stage);
    let chat_entity = entity.clone();
    let invite_entity = entity.clone();
    let guild = ch.guild;
    let invite_hover = (!is_dm).then(|| {
        div()
            .flex()
            .flex_none()
            .my(z(-4.))
            .mr(z(-4.))
            .when(!selected || has_info, |d| d.hidden().group_hover(group.clone(), |s| s.flex()))
            .when(is_voice, |d| {
                d.child(
                    div()
                        .id(("vc-chat", id.0))
                        .cursor_pointer()
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            chat_entity.update(cx, |a, cx| a.open_voice_chat(id, cx));
                        })
                        .tooltip(crate::ui::tooltip::tip(crate::t!("channel.open_chat")))
                        .child(action(Icon::Members, Some("chat"))),
                )
            })
            .child(
                action(Icon::Members, Some("invite"))
                    .cursor_pointer()
                    .on_click({
                        let e = invite_entity.clone();
                        move |_, _, cx| {
                            cx.stop_propagation();
                            if let Some(g) = guild {
                                e.update(cx, |a, cx| a.open_invite_to(g, Some(id), cx));
                            }
                        }
                    })
                    .tooltip(crate::ui::tooltip::tip(crate::t!("perm.create_invite"))),
            )
            .when(can_edit, |d| {
                d.child(
                    action(Icon::Gear, None)
                        .cursor_pointer()
                        .on_click({
                            let e = invite_entity.clone();
                            move |_, _, cx| {
                                cx.stop_propagation();
                                e.update(cx, |a, cx| a.open_channel_settings(id, cx));
                            }
                        })
                        .tooltip(crate::ui::tooltip::tip(crate::t!("menu.edit_channel"))),
                )
            })
    });
    div()
        .id(("ch", id.0))
        .relative()
        .pl(z(if is_dm { 8. } else { 10. }))
        .pr(z(8.))
        .py(z(1.))
        .child(
            div()
                .absolute()
                .left(z(1.))
                .top(z(if is_dm { dm_row_h() / 2. - 3. } else { 15. }))
                .w(z(4.))
                .h(z(8.))
                .rounded_r(z(4.))
                .when(unread && !selected, |d| d.bg(t(idx::INTERACTIVE_TEXT_ACTIVE))),
        )
        .child(
            crate::ui::a11y::named(div().id(("chrow", id.0)), if matches!(ch.kind, ChannelKind::Voice | ChannelKind::Stage) { gpui::Role::Button } else { gpui::Role::Link }, || {
                let info = crate::ui::a11y::ChannelInfo {
                    unread,
                    mentions: ch.mentions,
                    locked: extra.is_some_and(|e| e.locked),
                    nsfw: extra.is_some_and(|e| e.nsfw),
                    voice_users: if is_dm { Vec::new() } else { ch.voice_users.iter().map(|u| voice_user_name(app, *u)).collect() },
                    members: extra.map(|e| e.recipients as u32 + 1).unwrap_or(1),
                    user_limit: (ch.kind == ChannelKind::Voice).then(|| app.backend.channel_user_limit(id)).flatten(),
                    call_secs: (matches!(ch.kind, ChannelKind::Voice | ChannelKind::Stage) && !ch.voice_users.is_empty())
                        .then(|| crate::voice::start_time::get(id.0))
                        .flatten()
                        .map(|st| crate::voice::start_time::now_s() - st),
                    activities: if ch.kind == ChannelKind::Voice {
                        crate::ui::call_activity::channel_activities(app, id).iter().filter_map(|(a, _)| crate::ui::call_activity::info(*a).name).collect()
                    } else {
                        Vec::new()
                    },
                };
                crate::ui::a11y::channel(crate::ui::a11y::Kind::of(ch.kind, is_dm, is_group), &name, &info)
            })
                .when(crate::ui::a11y::on(), |d| d.aria_selected(selected))
                .group(group)
                .h(z(if is_dm { dm_row_h() } else if status_line { space::channels_name_lh() + 24. } else { space::channels_name_lh() + 8. }))
                .pl(z(10.))
                .pr(z(if is_dm { 0. } else { 10. }))
                .rounded(z(ROW_RADIUS))
                .flex()
                .items_center()
                .gap(z(if is_dm { 12. } else { 10. }))
                .cursor_pointer()
                .text_color(c(fg))
                .when(selected, |d| d.bg(t(idx::INTERACTIVE_BACKGROUND_SELECTED)))
                .when(!selected, |d| {
                    d.hover(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_HOVER)).text_color(t(idx::INTERACTIVE_TEXT_HOVER)))
                    .active(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_ACTIVE)))
                })
                .when(is_dm, |d| d.child(crate::ui::avatar_deco::guard(crate::ui::avatar_deco::hover_key(4, id.0))))
                .on_hover({
                    let e = e.clone();
                    move |hovered: &bool, _, cx| {
                        if *hovered {
                            e.update(cx, |a, cx| crate::history_ui::hover_channel(a, id, cx));
                        }
                        if is_dm && crate::ui::avatar_deco::set_hover(crate::ui::avatar_deco::hover_key(4, id.0), *hovered, cx) {
                            e.update(cx, |_, cx| cx.notify());
                        }
                    }
                })
                .on_mouse_down(gpui::MouseButton::Left, {
                    let e = e.clone();
                    move |_, _, cx| e.update(cx, |a, cx| crate::history_ui::preload(a, id, cx))
                })
                .on_click(move |_, _, cx| e.update(cx, |a, cx| a.select_channel(id, cx)))
                .on_mouse_down(gpui::MouseButton::Right, move |ev, w, cx| {
                    cx.stop_propagation();
                    crate::ui::nav_menus::open(crate::ui::nav_menus::Target::Channel(id), ev.position, w)
                })
                .relative()
                .when_some(
                    (is_dm && !is_group)
                        .then(|| ch.voice_users.first().and_then(|u| app.users.get(u)))
                        .flatten()
                        .and_then(|u| {
                            let fade = crate::ui::nameplate::Fade::Measured {
                                place: crate::ui::nameplate::Place::Channel,
                                id: id.0,
                                offset: 32. + 12. - 5.,
                                fallback: 220.,
                            };
                            crate::ui::nameplate::layer_with_fade(u, dm_row_h(), crate::ui::nameplate::Place::Channel, !selected, selected, fade)
                        }),
                    |d, l| {
                        d.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .when(!selected, |w| w.opacity(0.).group_hover(SharedString::from(format!("chrow{}", id.0)), |s| s.opacity(1.)))
                                .child(l),
                        )
                    },
                )
                .child(
                    if guild_dim {
                        div().flex_none().opacity(0.5).child(top_line(lead, status_line)).into_any_element()
                    } else if dm_dim {
                        div()
                            .flex_none()
                            .opacity(0.3)
                            .group_hover(SharedString::from(format!("chrow{}", id.0)), |s| s.opacity(1.))
                            .child(top_line(lead, status_line))
                            .into_any_element()
                    } else {
                        top_line(lead, status_line)
                    },
                )
                .child(
                    div()
                        .when(guild_dim, |d| d.opacity(0.5))
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(z(16.))
                                .line_height(z(if is_dm { 20. } else { space::channels_name_lh() }))
                                .font_weight(FontWeight::MEDIUM)
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .relative()
                                        .min_w_0()
                                        .flex()
                                        .items_center()
                                        .child(
                                            {
                                                let dm_user = (is_dm && !is_group).then(|| ch.voice_users.first().and_then(|u| app.users.get(u))).flatten();
                                                let hovered = crate::ui::avatar_deco::hovered(crate::ui::avatar_deco::hover_key(4, id.0));
                                                let effect = selected || hovered;
                                                let motion = if hovered {
                                                    crate::fonts::Motion::Loop
                                                } else if selected {
                                                    crate::fonts::Motion::Once
                                                } else {
                                                    crate::fonts::Motion::Still
                                                };
                                                crate::fonts::styled_name(div().min_w_0().text_ellipsis(), dm_user, false)
                                                    .child(crate::fonts::role_name_text_moving(name, dm_user, 16., false, None, effect, motion))
                                            },
                                        )
                                        .when_some(dm_tag, |d, t| {
                                            d.child(crate::ui::members::server_tag(&t).ml(z(4.)).when(dm_dim, |d| {
                                                d.opacity(0.3).group_hover(SharedString::from(format!("chrow{}", id.0)), |s| s.opacity(1.))
                                            }))
                                        })
                                        .when(plated, |d| {
                                            d.child(crate::ui::nameplate::width_probe(crate::ui::nameplate::Place::Channel, id.0))
                                        }),
                                ),
                        )
                        .when(status_line, |d| {
                            let status_entity = entity.clone();
                            let text = vc_status.clone();
                            d.child(
                                div()
                                    .id(("vc-status", id.0))
                                    .group("vc-status")
                                    .flex()
                                    .items_center()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_size(z(12.))
                                    .line_height(z(16.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(t(idx::CHANNELS_DEFAULT))
                                    .when(can_set_status, |d| {
                                        d.hover(|s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER))).on_click(move |_, _, cx| {
                                            cx.stop_propagation();
                                            status_entity.update(cx, |a, cx| a.open_voice_status(id, cx));
                                        })
                                    })
                                    .map(|d| match text {
                                        Some(st) => d.child(
                                            div().min_w_0().text_ellipsis().whitespace_nowrap().child(crate::ui::emoji_text::voice_status(app, entity, id, &st, 12.)),
                                        ),
                                        None => d
                                            .child(div().min_w_0().text_ellipsis().whitespace_nowrap().child(crate::t!("voice.set_status")))
                                            .child(
                                                gpui::svg()
                                                    .path(crate::icons::path("pen"))
                                                    .ml(z(4.))
                                                    .size(z(12.))
                                                    .flex_none()
                                                    .text_color(t(idx::CHANNELS_DEFAULT))
                                                    .group_hover("vc-status", |s| s.text_color(t(idx::INTERACTIVE_TEXT_HOVER))),
                                            ),
                                    }),
                            )
                        })
                        .when_some(dm_sub, |d, s| d.child(s))
                        .when_some(members, |d, m| {
                            d.child(
                                div()
                                    .text_size(z(12.))
                                    .line_height(z(14.))
                                    .text_color(c(Palette::TEXT_MUTED))
                                    .child(m),
                            )
                        }),
                )
                .when(ch.mentions > 0, |d| d.child(badge(ch.mentions).bg(c(Palette::NOTIFICATION))))
                .when(ch.mentions == 0, |d| d.children(invite_hover.map(|h| top_line(h, status_line))))
                .children(timer.map(|t| top_line(t, status_line)))
                .children(limit_pill)
                .children(close)
                .map(|d| crate::ui::focus_ring::ring_inset_edges(d, crate::ui::states::key(("chrow", id.0)), ROW_RADIUS, 0., 2., 4., 2.)),
        )
        .into_any_element()
}

fn top_line(e: impl IntoElement, connected: bool) -> AnyElement {
    if connected {
        div().self_start().mt(z(4.)).h(z(space::channels_name_lh())).flex().items_center().flex_none().child(e).into_any_element()
    } else {
        e.into_any_element()
    }
}

pub fn thread_unread(app: &PhoenixApp, id: Id) -> (bool, u32) {
    let live = app.channels.get(&id);
    match app.backend.thread_info(id) {
        Some(_) if app.selected_channel == Some(id) => (false, 0),
        Some(i) => (i.unread && !i.member_muted, i.mentions),
        None => (live.is_some_and(|c| c.unread), live.map(|c| c.mentions).unwrap_or(0)),
    }
}

fn thread_row(app: &PhoenixApp, entity: &Entity<PhoenixApp>, id: Id) -> AnyElement {
    let Some(th) = app.nav.find_thread(id) else { return div().into_any_element() };
    let selected = app.selected_channel == Some(id);
    let (unread, mentions) = thread_unread(app, id);
    let e = entity.clone();
    div()
        .id(("thread", id.0))
        .relative()
        .pl(z(space::xs()))
        .pr(z(8.))
        .py(z(1.))
        .child(
            div()
                .absolute()
                .left(z(1.))
                .top(z(15.))
                .w(z(4.))
                .h(z(8.))
                .rounded_r(z(4.))
                .when(unread && !selected, |d| d.bg(t(idx::INTERACTIVE_TEXT_ACTIVE))),
        )
        .child(
            div()
                .absolute()
                .left(z(space::spine_left()))
                .top_0()
                .w(z(10.))
                .h(z(19.))
                .border_l_1()
                .border_b_1()
                .rounded_bl(z(8.))
                .border_color(t(idx::SPINE_DEFAULT)),
        )
        .child(
            div()
                .id(("threadrow", id.0))
                .ml(z(space::spine_left()))
                .h(z(space::channels_name_lh() + 8.))
                .px(z(10.))
                .rounded(z(ROW_RADIUS))
                .flex()
                .items_center()
                .cursor_pointer()
                .text_size(z(14.))
                .line_height(z(space::channels_name_lh()))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t(if selected || unread { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::CHANNELS_DEFAULT }))
                .when(selected, |d| d.bg(t(idx::INTERACTIVE_BACKGROUND_SELECTED)))
                .when(!selected, |d| {
                    d.hover(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_HOVER)).text_color(t(idx::INTERACTIVE_TEXT_HOVER)))
                        .active(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_ACTIVE)))
                })
                .on_click(move |_, _, cx| e.update(cx, |a, cx| a.select_thread(id, cx)))
                .on_mouse_down(gpui::MouseButton::Right, move |ev, w, cx| {
                    cx.stop_propagation();
                    crate::ui::nav_menus::open(crate::ui::nav_menus::Target::Thread(id), ev.position, w)
                })
                .child(div().flex_1().min_w_0().text_ellipsis().child(crate::ui::emoji_text::label(th.name.clone(), 14.)))
                .when(mentions > 0, |d| d.child(badge(mentions).bg(c(Palette::NOTIFICATION)))),
        )
        .into_any_element()
}

pub fn panels(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    div()
        .flex_none()
        .w(z(m::SERVER_BAR_W + crate::ui::sidebar_resize::width()))
        .mt(z(-10.))
        .relative()
        .child(div().absolute().top(z(10.)).bottom_0().left_0().w(z(m::server_bar_w())).bg(crate::theme::client_theme::surface(crate::theme::client_theme::Level::AppFrame, Some(crate::theme::client_theme::Level::AppFrame), Palette::BG_TERTIARY)))
        .child(
            div()
                .absolute()
                .top(z(10.))
                .bottom_0()
                .right_0()
                .left(z(m::server_bar_w()))
                .border_l_1()
                .border_color(c(Palette::APP_FRAME_BORDER))
                .bg(crate::theme::client_theme::surface(crate::theme::client_theme::Level::AppFrame, Some(crate::theme::client_theme::Level::AppFrame), Palette::BG_SECONDARY)),
        )
        .child(panels_card(app, cx))
        .children(crate::ui::quest_bar::overlay(app, 10. + 1. + m::USER_PANEL_H, 11., cx))
}

fn panels_card(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    crate::ui::a11y::named(div().id("user-area"), gpui::Role::Region, || crate::t!("a11y.user_area"))
        .flex_none()
        .m(z(10.))
        .rounded(z(8.))
        .border_1()
        .border_color(t(idx::BORDER_MUTED))
        .bg(crate::theme::client_theme::fixed_bg(crate::theme::tok(idx::BACKGROUND_BASE_LOW)))
        .overflow_hidden()
        .relative()
        .flex()
        .flex_col()
        .children(crate::theme::client_theme::fixed_layer(crate::theme::client_theme::Level::Highest, z(7.)))
        .children(crate::ui::quest_bar::render(app, cx))
        .when(app.voice_channel.is_some(), |d| d.child(crate::ui::voice_panel::voice_panel(app, cx)))
        .when(app.clips.active(), |d| d.child(crate::clips::activity(app, cx)))
        .child(user_panel(app, cx))
}

fn user_panel(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    use crate::ui::voice_panel::{Audio, audio_button, panel_button, panel_glyph};
    let status = match app.presence {
        crate::model::Presence::Online => crate::t!("status.online"),
        crate::model::Presence::Idle => crate::t!("status.idle"),
        crate::model::Presence::Dnd => crate::t!("status.dnd"),
        crate::model::Presence::Offline => crate::t!("status.invisible"),
    };
    let muted = app.mic_muted || app.deafened;
    div()
        .relative()
        .overflow_hidden()
        .h(z(m::USER_PANEL_H))
        .flex_none()
        .px(z(14.))
        .flex()
        .items_center()
        .gap(z(10.))
        .when_some(
            crate::ui::nameplate::layer(&app.me, m::USER_PANEL_H, crate::ui::nameplate::Place::Account, false, false),
            |d, l| d.child(l),
        )
        .child(
            crate::ui::a11y::named(div().id("me"), gpui::Role::Button, || crate::t!("a11y.manage_profile"))
                .flex_1()
                .min_w_0()
                .ml(z(-6.))
                .py(z(5.))
                .px(z(6.))
                .flex()
                .items_center()
                .gap(z(8.))
                .rounded(z(4.))
                .cursor_pointer()
                .group("user-panel-avatar")
                .hover(|s| s.bg(t(idx::BACKGROUND_MOD_NORMAL)))
                .on_click(cx.listener(|a, _, w, cx| a.toggle_status_picker(w, cx)))
                .child(crate::ui::avatar_deco::guard(crate::ui::avatar_deco::hover_key(3, 0)))
                .child(crate::ui::states::hover_guard(crate::ui::states::key("user-panel-roll")))
                .on_hover(cx.listener(|_, h: &bool, _, cx| {
                    let deco = crate::ui::avatar_deco::set_hover(crate::ui::avatar_deco::hover_key(3, 0), *h, cx);
                    if crate::ui::states::set_hover(crate::ui::states::key("user-panel-roll"), *h) || deco {
                        cx.notify();
                    }
                }))
                .overflow_hidden()
                .child(crate::ui::avatar_with_status_speaking(
                    Some(&app.me),
                    32.,
                    app.presence,
                    c(Palette::BG_SECONDARY_ALT),
                    crate::ui::avatar_deco::source(Some(&app.me), Some(crate::ui::avatar_deco::hover_key(3, 0))),
                    false,
                    crate::ui::voice_panel::self_speaking_ring(app),
                ))
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .mt(z(-1.))
                                .mb(z(2.))
                                .text_size(z(16.))
                                .line_height(z(22.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t(idx::TEXT_STRONG))
                                .text_ellipsis()
                                .child(
                                    crate::fonts::styled_name(div().text_ellipsis(), Some(&app.me), false)
                                        .child(crate::ui::emoji_text::label(app.me.display_name.clone(), 14.)),
                                ),
                        )
                        .child(crate::ui::states::hover_roll(crate::ui::states::key("user-panel-roll"), 13. * crate::theme::zoom(), match crate::ui::members::self_sub_text(app) {
                            Some(row) => row.into_any_element(),
                            None => div()
                                .text_size(z(12.))
                                .line_height(z(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t(idx::INTERACTIVE_TEXT_DEFAULT))
                                .text_ellipsis()
                                .child(crate::ui::emoji_text::status_label(
                                    {
                                        let act = app.people.activities.get(&app.me.id).and_then(|v| {
                                            v.iter().find(|a| a.kind == crate::presence::ActivityKind::Custom)
                                        });
                                        match (&app.people.custom_status, act) {
                                            (Some(c), Some(a)) if a.state.as_ref() == Some(c) => a.headline(),
                                            (Some(c), _) => c.clone(),
                                            (None, Some(a)) if app.people.custom_status.is_none() && a.state.is_none() => a.headline(),
                                            _ => status,
                                        }
                                    },
                                    14. * crate::theme::zoom() / 1.375,
                                ))
                                .into_any_element(),
                        },
                        div()
                            .text_size(z(12.))
                            .line_height(z(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t(idx::INTERACTIVE_TEXT_DEFAULT))
                            .text_ellipsis()
                            .child(app.me.name.clone())
                            .into_any_element(),
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .gap(z(8.))
                .child(audio_button(
                    Audio::Mic,
                    if muted { Icon::MicOff } else { Icon::Mic },
                    muted,
                    if muted { crate::t!("voice.unmute") } else { crate::t!("voice.mute") },
                    |a, cx| {
                        a.mic_muted = !a.mic_muted;
                        crate::voice::sync_self_state(a);
                        cx.notify();
                    },
                    cx,
                ))
                .child(audio_button(
                    Audio::Deafen,
                    if app.deafened { Icon::HeadphonesOff } else { Icon::Headphones },
                    app.deafened,
                    if app.deafened { crate::t!("voice.undeafen") } else { crate::t!("voice.deafen") },
                    |a, cx| {
                        a.deafened = !a.deafened;
                        crate::voice::sync_self_state(a);
                        cx.notify();
                    },
                    cx,
                ))
                .child(
                    panel_button(
                        "settings",
                        crate::ui::voice_panel::lottie_glyph(
                            crate::ui::voice_panel::panel_key("settings", false),
                            crate::icons::lottie_icons::Lottie::Settings,
                            false,
                            false,
                            || panel_glyph(Icon::Gear, false),
                        ),
                        false,
                    )
                        .tip(crate::t!("settings.user_settings"))
                        .map(|d| crate::ui::a11y::named(d, gpui::Role::Button, || crate::t!("a11y.user_settings")))
                        .on_click(cx.listener(|a, _, _, cx| crate::ui::settings::open(a, cx))),
                ),
        )
}

fn boost_goal_max(features: &[String]) -> u32 {
    const TIER_3: u32 = 14;
    const PERKS: u32 = 3 + 3 + 3 + 5 + 3 + 2 + 3;
    let base = if features.iter().any(|f| f == "PREMIUM_TIER_3_OVERRIDE") { 0 } else { TIER_3 };
    base + PERKS
}

fn boost_goal(guild: Id, admin: bool, count: u32, enabled: bool, max: u32) -> Option<gpui::AnyElement> {
    if !enabled || count == 0 {
        return None;
    }
    let pct = (count as f32 / max as f32).min(1.);
    let pink = t(idx::GUILD_BOOSTING_PINK_REFRESH);
    let text = |s: SharedString| {
        div()
            .text_size(z(12.))
            .line_height(z(16.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(t(idx::TEXT_DEFAULT))
            .text_ellipsis()
            .child(s)
    };
    let count_text: SharedString = if count >= max {
        crate::t!("guild.boost_goal.count_done", n = count)
    } else {
        crate::t!("guild.boost_goal.count", n = count, max = max)
    };
    Some(
        div().w_full().pl(z(8.)).pr(z(8.)).mb(z(12.)).child(
        div()
            .id("boost-goal")
            .group("boost-goal")
            .on_click(move |_, w, _| {
                crate::ui::boost_layer::open(guild);
                w.refresh();
            })
            .when(admin, |d| {
                d.on_mouse_down(gpui::MouseButton::Right, move |ev, w, cx| {
                    cx.stop_propagation();
                    crate::ui::extra_menus::open(crate::ui::extra_menus::Target::BoostBar(guild), ev.position, w);
                })
            })
            .w_full()
            .pt(z(4.))
            .px(z(4.))
            .pb(z(16.))
            .border_b_1()
            .border_color(t(idx::BORDER_SUBTLE))
            .cursor_pointer()
            .child(
                div()
                    .relative()
                    .h(z(26.))
                    .rounded_full()
                    .bg(t(idx::BACKGROUND_BASE_LOW))
                    .child(
                        div().absolute().top_0().bottom_0().left_0().w(gpui::relative(pct)).p(z(2.)).child(
                        div()
                            .size_full()
                            .rounded_full()
                            .bg(gpui::linear_gradient(
                                270.,
                                gpui::linear_color_stop(pink.opacity(0.4), 0.),
                                gpui::linear_color_stop(pink.opacity(0.1), 1.),
                            ))
                            .group_hover("boost-goal", |s| {
                                s.bg(gpui::linear_gradient(
                                    270.,
                                    gpui::linear_color_stop(pink.opacity(0.7), 0.),
                                    gpui::linear_color_stop(pink.opacity(0.25), 1.),
                                ))
                            })
                            .shadow(crate::ui::inbox::low_shadow()),
                        ),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(z(8.))
                            .right(z(8.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(z(8.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(z(2.))
                                    .min_w_0()
                                    .child(text(crate::t!("guild.boost_goal.title")))
                                    .when(count >= max, |d| d.child(text("\u{1F389}".into()))),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(z(2.))
                                    .flex_none()
                                    .opacity(0.7)
                                    .group_hover("boost-goal", |s| s.opacity(1.))
                                    .child(text(count_text))
                                    .child(
                                        gpui::svg()
                                            .path(crate::icons::path("chevron-right"))
                                            .size(z(12.))
                                            .flex_none()
                                            .text_color(t(idx::TEXT_DEFAULT)),
                                    ),
                            ),
                    ),
            ),
        )
        .into_any_element(),
    )
}

fn guild_top(app: &PhoenixApp, entity: Entity<PhoenixApp>, cx: &App) -> impl IntoElement {
    let ev_entity = entity.clone();
    let boost_entity = entity.clone();
    let perms = app.selected_guild.map(|g| app.backend.guild_permissions(g)).unwrap_or(0);
    let mods = crate::ui::inbox::has_perm(perms, (1 << 1) | (1 << 2) | (1 << 5));
    let features = app.selected_guild.map(|g| app.backend.guild_features(g)).unwrap_or_default();
    let has = |f: &str| features.iter().any(|x| x == f);
    let browse: Option<SharedString> = if has("GUILD_ONBOARDING") {
        Some(crate::t!("guild.top.channels_roles"))
    } else if has("COMMUNITY") {
        Some(crate::t!("guild.top.browse"))
    } else {
        None
    };
    let guide = has("COMMUNITY") && crate::ui::guide::has_guide(&features);
    let (guide_on, page_on) = (crate::ui::guide::is_open(app, cx), crate::ui::community::is_open(app, cx));
    let item = |id: &'static str, glyph: &'static str, label: SharedString, on: bool| {
        div().pl(z(10.)).pr(z(8.)).py(z(1.)).child(
            div()
                .id(id)
                .h(z(36.))
                .px(z(10.))
                .rounded(z(ROW_RADIUS))
                .flex()
                .items_center()
                .gap(z(10.))
                .cursor_pointer()
                .text_color(t(if on { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::CHANNELS_DEFAULT }))
                .when(on, |d| d.bg(t(idx::INTERACTIVE_BACKGROUND_SELECTED)))
                .when(!on, |d| d.hover(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_HOVER)).text_color(t(idx::INTERACTIVE_TEXT_HOVER))))
                .child(gpui::svg().path(crate::icons::path(glyph)).size(z(20.)).flex_none().text_color(t(if on { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::CHANNEL_ICON })))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(z(16.))
                        .line_height(z(28.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_ellipsis()
                        .child(label),
                ),
        )
    };
    div()
        .w_full()
        .pt(z(12.))
        .flex()
        .flex_col()
        .children(app.selected_guild.and_then(|g| {
            let (n, on) = app.backend.guild_boosts(g)?;
            boost_goal(g, crate::actions::has(app, Some(g), crate::actions::ADMINISTRATOR), n, on, boost_goal_max(&app.backend.guild_features(g)))
        }))
        .when(guide, |d| {
            let ent = entity.clone();
            d.child(
                div()
                    .id("g-guide-open")
                    .on_click(move |_, _, cx| {
                        ent.update(cx, |a, cx| {
                            if let Some(g) = a.selected_guild {
                                crate::ui::guide::open(a, g, cx);
                            }
                        })
                    })
                    .child(item("g-guide", "signpost", crate::t!("guild.top.guide"), guide_on)),
            )
        })
        .child(
            div()
                .id("g-events-open")
                .on_click(move |_, _, cx| {
                    ev_entity.update(cx, |a, cx| {
                        if let Some(g) = a.selected_guild {
                            crate::ui::events::open(a, g, cx);
                        }
                    })
                })
                .child(item("g-events", "calendar", crate::t!("guild.top.events"), false)),
        )
        .when_some(browse, |d, label| {
            let ent = entity.clone();
            d.child(
                div()
                    .id("g-browse-open")
                    .on_click(move |_, _, cx| {
                        ent.update(cx, |a, cx| {
                            if let Some(g) = a.selected_guild {
                                crate::ui::community::open(a, g, cx);
                            }
                        })
                    })
                    .child(item("g-browse", "channel-browse", label, page_on)),
            )
        })
        .when(mods, |d| d.child(item("g-members", "members", crate::t!("guild.top.members"), false)))
        .child(
            div()
                .id("g-boosts-open")
                .on_click(move |_, _, cx| {
                    boost_entity.update(cx, |a, cx| {
                        if let Some(g) = a.selected_guild {
                            crate::ui::boost_layer::open(g);
                            cx.notify();
                        }
                    })
                })
                .child(item("g-boosts", "boost", crate::t!("guild.top.boosts"), false)),
        )
        .child(div().mt(z(15.)).ml(z(20.)).mr(z(20.)).h(z(1.)).bg(t(idx::BORDER_SUBTLE)))
        .child(div().h(z(12.)))
}

fn dm_row_h() -> f32 {
    [38., 42., 48.][crate::theme::density() as usize]
}
