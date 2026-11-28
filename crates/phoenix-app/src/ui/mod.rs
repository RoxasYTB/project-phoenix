pub mod add_server;
pub mod a11y;
pub mod spinner;
pub mod boot;
pub mod guild_badge;
pub mod join_flow;
pub mod autocomplete;
pub mod avatar_deco;
pub mod mobile_status;
pub mod profile_theme;
pub mod profile_games;
pub mod profile_board;
pub mod profile_bot;
pub mod profile_wishlist;
pub mod call_activity;
pub mod call_popout;
pub mod game_overlay;
pub mod call_menus;
pub mod call_toasts;
pub mod call_layout;
pub mod call_view;
pub mod incoming_call;
pub mod dm_call;
pub mod pip;
pub mod channel_dnd;
pub mod chat;
pub mod code_copy;
pub mod components;
pub mod community;
pub mod guide;
pub mod events;
pub mod focus_ring;
pub mod favorites;
pub mod forum;
pub mod group_dm;
pub mod inbox;
pub mod inbox_unreads;
#[cfg(test)]
mod popout_tests;
#[cfg(test)]
mod input_qa_tests;
#[cfg(test)]
mod chat_list_tests;
#[cfg(test)]
mod settings_fx_tests;
pub mod context_menu;
pub mod controls;
pub mod guild_menu;
pub mod home;
pub mod locale_inputs;
pub mod invite;
pub mod image_viewer;
pub mod login;
pub mod login_mfa;
pub mod settings;
pub mod easter;
pub mod extra_menus;
pub mod members;
pub mod mod_view;
pub mod activity_traits;
pub mod nameplate;
pub mod profile_fx;
pub mod modal;
pub mod pickers;
pub mod modals;
pub mod profile;
pub mod tag_popout;
pub mod dm_profile_panel;
pub mod avatar_color;
pub mod squircle;
pub mod stage;
pub mod fade_image;
pub mod profile_bg;
pub mod profile_note;
pub mod profile_voice;
pub mod profile_toolbar;
pub mod shop_look;
pub mod shop_pdp;
pub mod shop_collection;
pub mod shop_buy;
pub mod quest_bar;
pub mod quest_modals;
pub mod profile_bio;
pub mod rail_dnd;
pub mod search;
pub mod search_filters;
pub mod search_query;
pub mod server_bar;
pub mod server_pages;
pub mod sidebar;
pub mod sidebar_resize;
pub mod thread_resize;
pub mod states;
pub mod popout_motion;
pub mod hover_icon;
pub mod status_picker;
pub mod status_svg;
pub mod blob_mask;
pub mod role_dot;
pub mod role_gradient;
pub mod switcher;
pub mod nav_menus;
pub mod thread_menu;
pub mod thread_create;
pub mod title_bar;
pub mod window_chrome;
pub mod tooltip;
pub mod channel_unread_bar;
pub mod user_menu;
pub mod view_as;
pub mod voice_effects;
pub mod burst;
pub mod voice_panel;
pub mod go_live;
pub mod voice_row;
pub mod boost_layer;
pub mod app_profile;
pub mod discovery;
pub mod quests_page;
pub mod shop_page;
pub mod emoji_text;
pub mod file_pick;
pub mod shake;

use crate::theme::z;
use gpui::{
    AnimationExt as _, AnyElement, Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::*, px, svg,
};

use crate::{
    assets::Icon,
    model::{ImageRef, Presence, User},
    theme::{DEFAULT_AVATAR, Palette, c, presence_color},
};

pub fn icon(i: Icon, size: f32, color: Hsla) -> gpui::Svg {
    svg().path(i.path()).size(px(size)).flex_none().text_color(color)
}

pub fn avatar(user: Option<&User>, size: f32) -> AnyElement {
    avatar_ref(user.map(|u| u.avatar.clone()).unwrap_or(ImageRef::Default(0)), size)
}

pub fn avatar_cdn(user: Option<&User>, size: f32, cdn: u32) -> AnyElement {
    match user.map(|u| &u.avatar) {
        Some(ImageRef::Url(url)) => crate::media::avatar_image(crate::media::with_cdn_size(url, cdn), size),
        _ => avatar(user, size),
    }
}

pub fn avatar_hover(user: Option<&User>, size: f32, play: bool) -> AnyElement {
    match user.map(|u| &u.avatar) {
        Some(ImageRef::Url(url)) if play && crate::media::emoji::window_focused() && crate::media::is_animated(url) => {
            gpui::img(url.clone()).id(crate::media::anim_id(url)).size(px(size)).rounded_full().flex_none().into_any_element()
        }
        _ => avatar(user, size),
    }
}

pub fn avatar_ref(img_ref: ImageRef, size: f32) -> AnyElement {
    match img_ref {
        ImageRef::Url(url) => crate::media::avatar_image(url, size),
        ImageRef::Default(ix) => div()
            .size(px(size))
            .flex_none()
            .rounded_full()
            .bg(c(DEFAULT_AVATAR[ix as usize % DEFAULT_AVATAR.len()]))
            .child(crate::media::avatar_image(format!("https://cdn.discordapp.com/embed/avatars/{ix}.png").into(), size))
            .into_any_element(),
    }
}

pub fn avatar_with_status(user: Option<&User>, size: f32, p: Presence, ring: Hsla) -> Div {
    avatar_with_status_overlay(user, size, p, ring, |d| d)
}

pub fn avatar_with_status_overlay(user: Option<&User>, size: f32, p: Presence, ring: Hsla, overlay: impl FnOnce(Div) -> Div) -> Div {
    avatar_with_status_deco(user, size, p, ring, overlay, None)
}

pub fn avatar_with_status_deco(
    user: Option<&User>,
    size: f32,
    p: Presence,
    ring: Hsla,
    overlay: impl FnOnce(Div) -> Div,
    deco: Option<SharedString>,
) -> Div {
    avatar_with_status_deco_hover(user, size, p, ring, overlay, deco, false)
}

pub fn avatar_with_status_deco_hover(
    user: Option<&User>,
    size: f32,
    p: Presence,
    ring: Hsla,
    _overlay: impl FnOnce(Div) -> Div,
    deco: Option<SharedString>,
    play: bool,
) -> Div {
    avatar_with_status_speaking(user, size, p, ring, deco, play, None)
}

pub fn avatar_with_status_speaking(
    user: Option<&User>,
    size: f32,
    p: Presence,
    ring: Hsla,
    deco: Option<SharedString>,
    play: bool,
    speaking: Option<AnyElement>,
) -> Div {
    let (cx, _, s) = status_svg::geometry(size);
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(blob_mask::cut(
            div()
                .relative()
                .size(px(size))
                .child(avatar_hover(user, size, play))
                .children(speaking)
                .when_some(deco, |d, src| d.child(avatar_deco::layer(src, size))),
            [blob_mask::status_hole(size)],
        ))
        .child(div().absolute().left(px(cx - s / 2.)).top(px(cx - s / 2.)).child(status_dot(p, s, ring)))
}

pub struct TypingGeometry {
    pub hole: blob_mask::Hole,
    pub pill: (f32, f32, f32, f32),
    pub dots: (f32, f32, f32),
}

pub fn typing_geometry(size: f32) -> TypingGeometry {
    let (_, r, st) = status_svg::geometry(size);
    let k = r - st / 2.;
    let h = st + 2. * k;
    TypingGeometry {
        hole: blob_mask::Hole { x: size - st - (1.5 * st + 2. * k) / 2., y: size - st - k, w: 2.5 * st + 2. * k, h, r: h / 2. },
        pill: (size - 1.75 * st, size - st, 2.5 * st, st),
        dots: (size - 1.375 * st, size - st / 1.333, st / 4.),
    }
}

pub fn typing_dot_frame(i: usize, t: f32) -> (f32, f32) {
    let x = (2. * t - 0.25 * i as f32).rem_euclid(2.);
    let v = if x > 1. { 2. - x } else { x };
    let k = ((v - 0.4) / 0.4).clamp(0., 1.);
    (0.8 + 0.2 * k, 0.3 + 0.7 * k)
}

pub fn avatar_typing(user: Option<&User>, size: f32, p: Presence, deco: Option<SharedString>) -> Div {
    typing_cutout(
        div().relative().size(px(size)).child(avatar_hover(user, size, false)).when_some(deco, |d, src| d.child(avatar_deco::layer(src, size))),
        size,
        p,
    )
}

pub fn typing_cutout(content: impl IntoElement, size: f32, p: Presence) -> Div {
    let g = typing_geometry(size);
    let (px_, py, pw, ph) = g.pill;
    let (dx, dy, dr) = g.dots;
    let focused = crate::media::emoji::window_focused();
    let mut dots = div().absolute().left(px(dx)).top(px(dy)).w(px(7. * dr)).h(px(2. * dr));
    for i in 0..3 {
        let cx = dr + dr * 2.5 * i as f32;
        let dot = div().rounded_full().bg(gpui::white());
        let dot = if focused {
            dot.with_animation(("avatar-typing-dot", i), gpui::Animation::new(std::time::Duration::from_millis(1200)).repeat(), move |d, t| {
                let (scale, opacity) = typing_dot_frame(i, t);
                d.size(px(2. * dr * scale)).opacity(opacity)
            })
            .into_any_element()
        } else {
            dot.size(px(2. * dr)).into_any_element()
        };
        dots = dots.child(div().absolute().left(px(cx - dr)).top_0().size(px(2. * dr)).flex().items_center().justify_center().child(dot));
    }
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(blob_mask::cut(content, [g.hole]))
        .child(div().absolute().left(px(px_)).top(px(py)).w(px(pw)).h(px(ph)).rounded(px(ph / 2.)).bg(presence_color(p)))
        .child(dots)
}

pub fn status_dot(p: Presence, size: f32, _ring: Hsla) -> gpui::Svg {
    gpui::svg().path(status_svg::path(p, false)).size(px(size)).flex_none().text_color(presence_color(p))
}

pub fn badge(n: u32) -> Div {
    let label: SharedString = if n > 99 { "99+".into() } else { n.to_string().into() };
    div()
        .h(z(16.))
        .min_w(z(16.))
        .px(z(4.))
        .rounded(z(8.))
        .bg(c(Palette::RED))
        .text_color(gpui::white())
        .text_size(z(12.))
        .font_weight(FontWeight::BOLD)
        .flex()
        .items_center()
        .justify_center()
        .child(label)
}

pub fn icon_button(id: &'static str, i: Icon, color: Hsla, size: f32) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .size(px(size + 8.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(4.))
        .cursor_pointer()
        .hover(|s| s.bg(c(Palette::BG_MOD_HOVER)))
        .child(icon(i, size, color))
}

#[cfg(test)]
mod typing_avatar_tests {
    use super::*;

    #[test]
    fn typing_cutout_matches_the_client_mask_at_32() {
        let g = typing_geometry(32.);
        assert_eq!(g.hole, blob_mask::Hole { x: 11.5, y: 19., w: 31., h: 16., r: 8. });
        assert_eq!(g.pill, (14.5, 22., 25., 10.));
        assert!((g.dots.0 - 18.25).abs() < 1e-3 && (g.dots.1 - 24.5).abs() < 0.01 && g.dots.2 == 2.5);
    }

    #[test]
    fn typing_dots_pulse_like_the_spring_cycle() {
        assert_eq!(typing_dot_frame(0, 0.), (0.8, 0.3));
        assert_eq!(typing_dot_frame(0, 0.5), (1., 1.));
        assert_eq!(typing_dot_frame(0, 0.2), (0.8, 0.3));
        let (s, o) = typing_dot_frame(1, 0.5);
        assert!(s > 0.8 && s <= 1. && o > 0.3);
    }
}
