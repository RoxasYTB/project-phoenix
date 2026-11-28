use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use gpui::{AnyElement, App, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, prelude::*};
use serde_json::Value;

use crate::actions::{ActionReply, ActionRequest};
use crate::app::PhoenixApp;
use crate::theme::{c, generated::idx, tok, z};

#[derive(Clone)]
enum Source {
    Loading,
    Ready(Value),
    Missing,
}

thread_local! {
    static EXPANDED: Cell<Option<u64>> = const { Cell::new(None) };
    static SOURCES: RefCell<HashMap<u64, Source>> = RefCell::new(HashMap::new());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cta {
    JoinGuild,
    GetPremium,
    Unavailable,
}

pub fn collapse(pill: u64) {
    if EXPANDED.get() == Some(pill) {
        EXPANDED.set(None);
    }
}

pub fn expanded(pill: u64) -> bool {
    EXPANDED.get() == Some(pill)
}

fn discoverable(guild: &Value) -> bool {
    guild.get("features").and_then(Value::as_array).is_some_and(|f| f.iter().any(|x| x.as_str() == Some("DISCOVERABLE")))
}

pub fn description(source: Option<&Value>, premium: bool) -> SharedString {
    let kind = source.and_then(|s| s.get("type")).and_then(Value::as_str);
    if kind == Some("APPLICATION") && let Some(name) = source.and_then(|s| s.pointer("/application/name")).and_then(Value::as_str) {
        return crate::i18n::t("reaction_emoji.from_app", &[("app", name)]);
    }
    let disc = kind == Some("GUILD") && source.and_then(|s| s.get("guild")).is_some_and(discoverable);
    match (premium, disc) {
        (true, true) => crate::t!("reaction_emoji.join_to_use"),
        (true, false) => crate::t!("reaction_emoji.unavailable"),
        (false, true) => crate::t!("reaction_emoji.nitro_and_join"),
        (false, false) => crate::t!("reaction_emoji.unavailable_more"),
    }
}

pub fn cta(source: Option<&Value>, premium: bool) -> Cta {
    let disc = source.and_then(|s| (s.get("type").and_then(Value::as_str) == Some("GUILD")).then(|| s.get("guild")).flatten()).is_some_and(discoverable);
    match (premium, disc) {
        (true, true) => Cta::JoinGuild,
        (false, true) => Cta::GetPremium,
        _ => Cta::Unavailable,
    }
}

pub fn other_emojis(guild: &Value, current: u64) -> Vec<String> {
    guild
        .get("emojis")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .take(13)
                .filter_map(|e| {
                    let id: u64 = e.get("id").and_then(Value::as_str).and_then(|s| s.parse().ok())?;
                    let name = e.get("name").and_then(Value::as_str)?;
                    let animated = e.get("animated").and_then(Value::as_bool).unwrap_or(false);
                    (id != current).then(|| format!("<{}:{name}:{id}>", if animated { "a" } else { "" }))
                })
                .take(12)
                .collect()
        })
        .unwrap_or_default()
}

fn joined(app: &PhoenixApp, id: u64) -> bool {
    app.guilds.iter().any(|g| app.backend.guild_emojis(g.id).iter().any(|e| e.id.0 == id))
}

fn fetch(a: &mut PhoenixApp, id: u64, cx: &mut gpui::Context<PhoenixApp>) {
    let start = SOURCES.with_borrow_mut(|m| {
        if m.contains_key(&id) {
            return false;
        }
        m.insert(id, Source::Loading);
        true
    });
    if !start {
        return;
    }
    a.action(ActionRequest::EmojiSource(id), cx, move |_, res, cx| {
        let src = match res {
            Ok(ActionReply::Json(v)) if v.get("type").is_some() => Source::Ready(v),
            _ => Source::Missing,
        };
        SOURCES.with_borrow_mut(|m| m.insert(id, src));
        cx.notify();
    });
}

pub fn expand(pill: u64, a: &mut PhoenixApp, id: u64, cx: &mut gpui::Context<PhoenixApp>) {
    EXPANDED.set(Some(pill));
    fetch(a, id, cx);
}

fn small(s: impl Into<SharedString>) -> gpui::Div {
    div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::NORMAL).text_color(c(tok(idx::TEXT_DEFAULT))).child(s.into())
}

fn xs(s: impl Into<SharedString>, color: usize) -> gpui::Div {
    div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::NORMAL).text_color(c(tok(color))).child(s.into())
}

fn guild_card(entity: &Entity<PhoenixApp>, pill: u64, guild: &Value, cta: Cta) -> AnyElement {
    let id: u64 = guild.get("id").and_then(Value::as_str).and_then(|s| s.parse().ok()).unwrap_or(0);
    let name = guild.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
    let icon = guild.get("icon").and_then(Value::as_str).map(|h| SharedString::from(format!("https://cdn.discordapp.com/icons/{id}/{h}.webp?size=32")));
    let online = guild.get("approximate_presence_count").and_then(Value::as_u64);
    let open = |e: &Entity<PhoenixApp>| {
        let e = e.clone();
        move |_: &gpui::ClickEvent, window: &mut gpui::Window, cx: &mut App| {
            cx.stop_propagation();
            super::burst_tip::dismiss(pill);
            window.refresh();
            e.update(cx, |a, cx| crate::ui::discovery::lurk(a, id, cx));
        }
    };
    let avatar = div().size(z(32.)).flex_none().rounded(z(10.)).overflow_hidden().bg(c(tok(idx::BACKGROUND_BASE_LOWER))).flex().items_center().justify_center();
    let avatar = match icon {
        Some(url) => avatar.child(crate::ui::squircle::image(url, 32.).size_full()),
        None => avatar.child(xs(crate::model::acronym(&name), idx::TEXT_DEFAULT)),
    };
    let status = match online {
        Some(n) => div()
            .flex()
            .items_center()
            .child(xs(crate::i18n::t("reaction_emoji.online", &[("n", &crate::ui::discovery::group_digits(n))]), idx::TEXT_DEFAULT))
            .child(div().size(z(4.)).mx(z(8.)).flex_none().rounded_full().bg(c(tok(idx::BACKGROUND_MOD_MUTED))))
            .child(if cta != Cta::JoinGuild {
                div()
                    .id(("reaction-emoji-join-link", pill))
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .child(xs(crate::t!("reaction_emoji.join"), idx::TEXT_LINK))
                    .on_click(open(entity))
                    .into_any_element()
            } else {
                xs(crate::t!("reaction_emoji.discoverable"), idx::TEXT_DEFAULT).into_any_element()
            })
            .into_any_element(),
        None => xs(crate::t!("reaction_emoji.invite_only"), idx::TEXT_DEFAULT).into_any_element(),
    };
    div()
        .mt(z(16.))
        .mb(z(8.))
        .flex()
        .items_center()
        .child(div().id(("reaction-emoji-guild-icon", pill)).cursor_pointer().child(avatar).on_click(open(entity)))
        .child(
            div()
                .ml(z(8.))
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .id(("reaction-emoji-guild-name", pill))
                        .cursor_pointer()
                        .min_w_0()
                        .truncate()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(tok(idx::TEXT_STRONG)))
                        .child(name)
                        .on_click(open(entity)),
                )
                .child(status),
        )
        .into_any_element()
}

pub fn block(app: &PhoenixApp, entity: &Entity<PhoenixApp>, pill: u64, token: &str) -> Option<AnyElement> {
    let emoji = crate::media::emoji::parse_custom(token)?;
    if joined(app, emoji.id) {
        return None;
    }
    let premium = app.backend.has_premium();
    let open = EXPANDED.get() == Some(pill);
    let state = SOURCES.with_borrow(|m| m.get(&emoji.id).cloned());
    let loaded = match &state {
        Some(Source::Ready(v)) => Some(Some(v.clone())),
        Some(Source::Missing) => Some(None),
        _ => None,
    };
    let mut out = div().flex().flex_col();
    match (open, loaded) {
        (true, Some(source)) => {
            let src = source.as_ref();
            let desc = description(src, premium);
            let kind = cta(src, premium);
            out = out.child(div().my(z(16.)).border_b_1().border_color(c(tok(idx::BORDER_SUBTLE))));
            if kind != Cta::Unavailable {
                out = out.child(small(desc.clone()));
            }
            let guild = src.filter(|s| s.get("type").and_then(Value::as_str) == Some("GUILD")).and_then(|s| s.get("guild")).filter(|g| discoverable(g));
            match guild {
                Some(g) => {
                    out = out.child(guild_card(entity, pill, g, kind));
                    let others = other_emojis(g, emoji.id);
                    if g.get("emojis").and_then(Value::as_array).is_some_and(|l| l.len() > 1) {
                        out = out.child(div().flex().flex_wrap().children(others.iter().map(|t| div().size(z(32.)).mx(z(2.)).my(z(4.)).child(crate::media::emoji::emoji_element(t, 32.)))));
                        let gid: u64 = g.get("id").and_then(Value::as_str).and_then(|s| s.parse().ok()).unwrap_or(0);
                        let e = entity.clone();
                        out = match kind {
                            Cta::JoinGuild => out.child(
                                div().mt(z(16.)).w_full().flex().child(
                                    crate::ui::controls::button(("reaction-emoji-join", pill), crate::t!("reaction_emoji.join"), crate::ui::controls::BtnKind::Primary)
                                        .flex_1()
                                        .on_click(move |_, window, cx| {
                                            cx.stop_propagation();
                                            super::burst_tip::dismiss(pill);
                                            window.refresh();
                                            e.update(cx, |a, cx| crate::ui::discovery::lurk(a, gid, cx));
                                        }),
                                ),
                            ),
                            Cta::GetPremium => out.child(
                                div().mt(z(16.)).w_full().flex().child(
                                    crate::ui::controls::button_icon(("reaction-emoji-nitro", pill), "nitro", crate::t!("reaction_emoji.get_nitro"), crate::ui::controls::BtnKind::Expressive)
                                        .flex_1()
                                        .on_click(move |_, window, cx| {
                                            cx.stop_propagation();
                                            super::burst_tip::dismiss(pill);
                                            window.refresh();
                                            e.update(cx, crate::ui::modals::burst_upsell::open_nitro);
                                        }),
                                ),
                            ),
                            Cta::Unavailable => out,
                        };
                    }
                }
                None if !premium => {
                    let e = entity.clone();
                    out = out.child(
                        div().flex().flex_wrap().child(small(format!("{desc}\u{a0}"))).child(
                            div()
                                .id(("reaction-emoji-learn", pill))
                                .cursor_pointer()
                                .hover(|s| s.underline())
                                .child(small(crate::t!("reaction_emoji.learn_more")).text_color(c(tok(idx::TEXT_LINK))))
                                .on_click(move |_, window, cx| {
                                    cx.stop_propagation();
                                    super::burst_tip::dismiss(pill);
                                    window.refresh();
                                    e.update(cx, crate::ui::modals::burst_upsell::open_nitro);
                                }),
                        ),
                    );
                }
                None => out = out.child(small(desc)),
            }
        }
        (true, None) => out = out.child(div().mt(z(8.)).flex().justify_center().child(crate::ui::spinner::spinner())),
        (false, _) => {
            let bg = if crate::theme::is_light() { c(tok(idx::PRIMARY_700)).opacity(0.1) } else { c(tok(idx::PRIMARY_300)).opacity(0.1) };
            let e = entity.clone();
            let id = emoji.id;
            out = out.child(
                div()
                    .id(("reaction-emoji-details", pill))
                    .mt(z(8.))
                    .py(z(6.))
                    .rounded(z(crate::theme::generated::global::RADIUS_XS))
                    .bg(bg)
                    .cursor_pointer()
                    .flex()
                    .justify_center()
                    .items_center()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER)))
                    .hover(|s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE))))
                    .child(crate::t!("reaction_emoji.see_details"))
                    .child(gpui::svg().path(crate::icons::path("chevron")).size(z(12.)).mt(z(2.)).text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))).with_transformation(gpui::Transformation::rotate(gpui::radians(-std::f32::consts::FRAC_PI_2))))
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        e.update(cx, |a, cx| expand(pill, a, id, cx));
                        window.refresh();
                    }),
            );
        }
    }
    Some(out.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn guild(disc: bool) -> Value {
        json!({ "type": "GUILD", "guild": { "id": "1", "name": "G", "features": if disc { json!(["DISCOVERABLE"]) } else { json!([]) }, "emojis": [{ "id": "5", "name": "a" }, { "id": "6", "name": "b", "animated": true }] } })
    }

    #[test]
    fn description_follows_the_client() {
        let t = |k: &str| crate::i18n::t(k, &[]);
        assert_eq!(description(Some(&guild(true)), true), t("reaction_emoji.join_to_use"));
        assert_eq!(description(Some(&guild(false)), true), t("reaction_emoji.unavailable"));
        assert_eq!(description(Some(&guild(true)), false), t("reaction_emoji.nitro_and_join"));
        assert_eq!(description(None, false), t("reaction_emoji.unavailable_more"));
        let app = json!({ "type": "APPLICATION", "application": { "id": "2", "name": "Bot" } });
        assert_eq!(description(Some(&app), true), crate::i18n::t("reaction_emoji.from_app", &[("app", "Bot")]));
        assert_eq!(cta(Some(&guild(true)), true), Cta::JoinGuild);
        assert_eq!(cta(Some(&guild(true)), false), Cta::GetPremium);
        assert_eq!(cta(Some(&guild(false)), false), Cta::Unavailable);
    }

    #[test]
    fn other_emojis_skip_the_current_one() {
        let g = guild(true);
        assert_eq!(other_emojis(&g["guild"], 5), vec!["<a:b:6>".to_string()]);
    }
}
