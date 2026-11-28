use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Bounds, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Pixels, SharedString, StatefulInteractiveElement, Styled, anchored, canvas, div, point, prelude::*, px,
};

use crate::actions::{ActionReply, ActionRequest};
use crate::app::PhoenixApp;
use crate::model::{Id, Message, Reaction};
use crate::theme::{c, generated::idx, tok, z};

pub const SHOW_AFTER: Duration = Duration::from_millis(750);
pub const SHOW_AFTER_NORMAL: Duration = Duration::from_millis(500);
pub const HIDE_AFTER: Duration = Duration::from_millis(200);
const MAX_W: f32 = 288.;
const EMOJI: f32 = 48.;
const GAP: f32 = 8.;
const ROOM: f32 = 120.;

const CACHE_CAPACITY: usize = 64;
const CACHE_TTL: Duration = Duration::from_secs(30);

type ReactorKey = (u64, u32, bool);
type ReactorNames = Vec<(Id, SharedString)>;

enum Preview {
    Loading,
    Ready(ReactorNames),
    Failed,
}

struct CacheEntry {
    key: ReactorKey,
    started: Instant,
    state: Preview,
}

thread_local! {
    static BOUNDS: RefCell<HashMap<u64, Bounds<Pixels>>> = RefCell::new(HashMap::new());
    static CACHE: RefCell<VecDeque<CacheEntry>> = const { RefCell::new(VecDeque::new()) };
    static SHOWN: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

pub fn pill_key(channel: Id, msg: Id, r: &Reaction, burst: bool) -> u64 {
    crate::ui::states::key(("reaction-pill", channel, msg, r.emoji_id, r.emoji_id.is_none().then_some(&r.emoji), burst))
}

fn reactor_key(pill: u64, r: &Reaction, burst: bool) -> ReactorKey {
    if burst { (pill, r.burst_count, r.me_burst) } else { (pill, r.count, r.me) }
}

pub fn tip_key(pill: u64) -> u64 {
    crate::ui::states::key(("reaction-tip", pill))
}

pub fn dismiss(pill: u64) {
    super::reaction_emoji::collapse(pill);
    if SHOWN.get() == Some(pill) {
        SHOWN.set(None);
    }
    crate::ui::states::set_hover(pill, false);
    crate::ui::states::set_hover(tip_key(pill), false);
}

pub fn show_after(burst: bool) -> Duration {
    if burst { SHOW_AFTER } else { SHOW_AFTER_NORMAL }
}

pub fn text(names: &[String], total: u32, emoji: &str, burst: bool) -> Option<SharedString> {
    let n = total.saturating_sub(names.len() as u32);
    let ns = n.to_string();
    let prefix = if burst { "burst" } else { "reaction" };
    let key = |base: &str| match (n, base.ends_with('n')) {
        (1, true) => format!("{prefix}.{base}_one"),
        _ => format!("{prefix}.{base}"),
    };
    let s = match names {
        [] => return None,
        [a] if n == 0 => crate::i18n::t(&key("tip_1"), &[("a", a.as_str()), ("emoji", emoji)]),
        [a] => crate::i18n::t(&key("tip_1n"), &[("a", a.as_str()), ("n", ns.as_str()), ("emoji", emoji)]),
        [a, b] if n == 0 => crate::i18n::t(&key("tip_2"), &[("a", a.as_str()), ("b", b.as_str()), ("emoji", emoji)]),
        [a, b] => crate::i18n::t(&key("tip_2n"), &[("a", a.as_str()), ("b", b.as_str()), ("n", ns.as_str()), ("emoji", emoji)]),
        [a, b, cc, ..] if n == 0 => crate::i18n::t(&key("tip_3"), &[("a", a.as_str()), ("b", b.as_str()), ("c", cc.as_str()), ("emoji", emoji)]),
        [a, b, cc, ..] => crate::i18n::t(&key("tip_3n"), &[("a", a.as_str()), ("b", b.as_str()), ("c", cc.as_str()), ("n", ns.as_str()), ("emoji", emoji)]),
    };
    Some(s)
}

pub fn visible(pill: Option<(bool, std::time::Instant)>, tip: Option<(bool, std::time::Instant)>, shown: bool, delay: Duration) -> bool {
    let ready = pill.is_some_and(|(on, at)| on && at.elapsed() >= delay);
    if ready {
        return true;
    }
    if !shown {
        return false;
    }
    let on = |s: Option<(bool, std::time::Instant)>| s.is_some_and(|(o, _)| o);
    let left = |s: Option<(bool, std::time::Instant)>| s.is_some_and(|(o, at)| !o && at.elapsed() < HIDE_AFTER);
    on(pill) || on(tip) || left(pill) || left(tip)
}

pub fn refresh_after(cx: &mut App, d: Duration) {
    cx.spawn(async move |cx| {
        cx.background_executor().timer(d + Duration::from_millis(10)).await;
        cx.update(|cx| cx.refresh_windows());
    })
    .detach();
}

pub fn on_pill_hover(entity: &Entity<PhoenixApp>, channel: Id, id: Id, r: &Reaction, burst: bool, on: bool, cx: &mut App) {
    if !on {
        refresh_after(cx, HIDE_AFTER);
        return;
    }
    let pill = pill_key(channel, id, r, burst);
    let hover = crate::ui::states::state(pill);
    let route = entity.read(cx).selected_channel;
    let entity = entity.downgrade();
    let reaction = r.clone();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(show_after(burst)).await;
        cx.update(|cx| {
            if crate::ui::states::state(pill) == hover && let Some(entity) = entity.upgrade() {
                entity.update(cx, |a, cx| {
                    if a.selected_channel == route {
                        request_names(a, channel, id, &reaction, burst, cx);
                        cx.notify();
                    }
                });
            }
        });
    })
    .detach();
}

pub fn load_names(a: &mut PhoenixApp, channel: Id, id: Id, r: &Reaction, burst: bool, cx: &mut gpui::Context<PhoenixApp>) {
    request_names(a, channel, id, r, burst, cx);
}

fn request_names(a: &mut PhoenixApp, channel: Id, id: Id, r: &Reaction, burst: bool, cx: &mut gpui::Context<PhoenixApp>) {
    let pill = pill_key(channel, id, r, burst);
    let key = reactor_key(pill, r, burst);
    let started = Instant::now();
    let should_request = CACHE.with_borrow_mut(|cache| {
        if let Some(index) = cache.iter().position(|entry| entry.key.0 == pill) {
            let Some(entry) = cache.remove(index) else { return false };
            if entry.key == key && matches!(&entry.state, Preview::Ready(_)) && entry.started.elapsed() < CACHE_TTL {
                cache.push_back(entry);
                return false;
            }
        }
        cache.push_back(CacheEntry { key, started, state: Preview::Loading });
        if cache.len() > CACHE_CAPACITY {
            cache.pop_front();
        }
        true
    });
    if !should_request {
        return;
    }

    let route = a.selected_channel;
    let hover = crate::ui::states::state(pill);
    a.action(ActionRequest::Reactors { channel, msg: id, emoji: r.clone(), burst }, cx, move |a, res, _| {
        CACHE.with_borrow_mut(|cache| {
            let Some(index) = cache.iter().position(|entry| entry.key == key && entry.started == started) else { return };
            let current_hover = crate::ui::states::state(pill);
            let active = if current_hover.is_some_and(|(on, _)| on) {
                current_hover == hover
            } else {
                visible(current_hover, crate::ui::states::state(tip_key(pill)), SHOWN.get() == Some(pill), show_after(burst))
            };
            if a.selected_channel != route || !active {
                cache.remove(index);
                return;
            }
            cache[index].state = match res {
                Ok(ActionReply::Users(users)) => Preview::Ready(users.into_iter().take(100).map(|u| (u.id, u.display_name)).collect()),
                _ => Preview::Failed,
            };
        });
    });
}

fn names(app: &PhoenixApp, msg: &Message, users: &ReactorNames) -> Vec<String> {
    let guild = app.channels.get(&msg.channel).and_then(|c| c.guild);
    users.iter()
        .filter(|(id, _)| !app.nav.is_blocked(*id) && !app.nav.is_ignored(*id))
        .take(3)
        .map(|(id, name)| app.member_nick_in(guild, *id).unwrap_or_else(|| name.clone()).to_string())
        .collect()
}

pub fn probe(pill: u64) -> AnyElement {
    div().absolute().top_0().left_0().size_full().child(bounds_probe(pill)).child(crate::ui::states::hover_guard(pill)).into_any_element()
}

fn bounds_probe(pill: u64) -> AnyElement {
    canvas(
        move |b, _, _| {
            if crate::ui::states::hovered(pill) || SHOWN.get() == Some(pill) {
                BOUNDS.with_borrow_mut(|bounds| {
                    if bounds.len() >= CACHE_CAPACITY && !bounds.contains_key(&pill) {
                        bounds.retain(|k, _| SHOWN.get() == Some(*k));
                    }
                    bounds.insert(pill, b);
                });
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

fn body_text(s: impl Into<SharedString>) -> gpui::Div {
    div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::NORMAL).text_color(c(tok(idx::TEXT_DEFAULT))).child(s.into())
}

pub fn others_label(n: u32) -> Option<SharedString> {
    let ns = n.to_string();
    match n {
        0 => None,
        1 => Some(crate::i18n::t("reaction.tip_others_one", &[("n", ns.as_str())])),
        _ => Some(crate::i18n::t("reaction.tip_others", &[("n", ns.as_str())])),
    }
}

pub fn link_range(line: &str, others: Option<&str>) -> Option<std::ops::Range<usize>> {
    let o = others?;
    let at = line.rfind(o)?;
    Some(at..at + o.len())
}

fn tip_text(line: SharedString, others: Option<SharedString>, hovered: bool) -> gpui::Div {
    let text = SharedString::from(format!("\u{200E}{line}"));
    let styled = match link_range(&text, others.as_deref()) {
        Some(range) => gpui::StyledText::new(text.clone()).with_highlights([(
            range,
            gpui::HighlightStyle {
                color: Some(c(tok(idx::TEXT_LINK))),
                underline: hovered.then(|| gpui::UnderlineStyle { thickness: gpui::px(1.), color: Some(c(tok(idx::TEXT_LINK))), wavy: false }),
                ..Default::default()
            },
        )]),
        None => gpui::StyledText::new(text.clone()),
    };
    div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::NORMAL).text_color(c(tok(idx::TEXT_DEFAULT))).child(styled)
}

fn nitro_icon() -> gpui::Svg {
    gpui::svg().path(crate::icons::path("nitro")).size(z(24.)).flex_none().mr(z(5.)).text_color(c(tok(idx::TEXT_DEFAULT)))
}

pub fn layer(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, r: &Reaction, burst: bool) -> Option<AnyElement> {
    let pill = pill_key(msg.channel, msg.id, r, burst);
    let tip = tip_key(pill);
    let shown = SHOWN.get() == Some(pill);
    if !visible(crate::ui::states::state(pill), crate::ui::states::state(tip), shown, show_after(burst)) {
        if shown {
            super::reaction_emoji::collapse(pill);
            SHOWN.set(None);
        }
        return None;
    }
    let b = BOUNDS.with_borrow(|bounds| bounds.get(&pill).copied())?;
    let label = crate::ui::modals::reactions::tab_label(r);
    let line = CACHE.with_borrow(|cache| match cache.iter().find(|entry| entry.key.0 == pill).map(|entry| &entry.state) {
        Some(Preview::Ready(users)) => text(&names(app, msg, users), if burst { r.burst_count } else { r.count }, &label, burst),
        _ => None,
    });
    let Some(line) = line else {
        if shown {
            SHOWN.set(None);
        }
        return None;
    };
    SHOWN.set(Some(pill));
    let inner_key = crate::ui::states::key(("reaction-tip-inner", pill));
    let shown_names = CACHE.with_borrow(|cache| match cache.iter().find(|entry| entry.key.0 == pill).map(|entry| &entry.state) {
        Some(Preview::Ready(users)) => names(app, msg, users).len() as u32,
        _ => 0,
    });
    let others = others_label((if burst { r.burst_count } else { r.count }).saturating_sub(shown_names));
    let premium = app.backend.has_premium();
    let (ch, id) = (msg.channel, msg.id);
    let e_open = entity.clone();
    let e_add = entity.clone();
    let e_unlock = entity.clone();
    let rc = r.clone();
    let list = &msg.reactions;
    let selected = crate::ui::modals::reactions::entries(list)
        .iter()
        .position(|en| en.burst == burst && list.get(en.index).is_some_and(|x| x.emoji == r.emoji && x.emoji_id == r.emoji_id))
        .unwrap_or(0);
    let inner = div()
        .id(("reaction-tip-inner", pill))
        .cursor_pointer()
        .flex()
        .items_center()
        .child(div().size(z(EMOJI)).flex_none().child(crate::media::emoji::emoji_element(&r.emoji, EMOJI)))
        .on_hover(move |on, window, _| {
            if crate::ui::states::set_hover(inner_key, *on) {
                window.refresh();
            }
        })
        .child(tip_text(line, others, crate::ui::states::hovered(inner_key)).ml(z(12.)).flex_1().min_w_0())
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            dismiss(pill);
            window.refresh();
            e_open.update(cx, |a, cx| a.open_reactions(ch, id, selected, cx));
        });
    let prompt = div().mt(z(12.)).pt(z(8.)).border_t_1().border_color(c(tok(idx::BORDER_SUBTLE))).w_full().flex().items_center();
    let footer = burst.then(|| if r.me_burst {
        prompt.when(premium, |d| d.child(nitro_icon())).child(body_text(crate::t!("burst.tip_sent")).ml(z(12.))).into_any_element()
    } else if premium {
        prompt
            .id(("burst-tip-add", pill))
            .cursor_pointer()
            .child(nitro_icon())
            .child(body_text(crate::t!("burst.tip_add")).ml(z(12.)))
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                dismiss(pill);
                window.refresh();
                e_add.update(cx, |a, cx| a.toggle_burst_reaction(ch, id, &rc, cx));
            })
            .into_any_element()
    } else {
        prompt
            .child(
                div().w_full().flex().flex_col().child(body_text(crate::t!("burst.tip_upsell"))).child(
                    div().mt(z(8.)).w_full().flex().child(
                        crate::ui::controls::button_icon(("burst-tip-unlock", pill), "nitro", crate::t!("burst.tip_unlock"), crate::ui::controls::BtnKind::Expressive)
                            .flex_1()
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                dismiss(pill);
                                window.refresh();
                                e_unlock.update(cx, crate::ui::modals::burst_upsell::open_nitro);
                            }),
                    ),
                ),
            )
            .into_any_element()
    });
    let card = div()
        .id(("reaction-tip", pill))
        .max_w(z(MAX_W + 32.))
        .p(z(16.))
        .rounded(z(crate::theme::generated::global::RADIUS_XS))
        .overflow_hidden()
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .shadow(crate::ui::inbox::popout_shadow())
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .occlude()
        .on_hover(move |on, window, cx| {
            if crate::ui::states::set_hover(tip, *on) {
                window.refresh();
            }
            if !*on {
                refresh_after(cx, HIDE_AFTER);
            }
        })
        .child(inner)
        .child(crate::ui::states::hover_guard(tip))
        .children(footer)
        .children(super::reaction_emoji::block(app, entity, pill, &r.emoji));
    let top = f32::from(b.origin.y);
    let mid = f32::from(b.origin.x + b.size.width / 2.);
    let span = f32::from(z(MAX_W + 64.));
    let below = top < f32::from(z(ROOM));
    let frame = div().w(px(span)).flex().flex_col().items_center().child(card);
    let (corner, y) = if below { (gpui::Anchor::TopLeft, f32::from(b.origin.y + b.size.height) + f32::from(z(GAP))) } else { (gpui::Anchor::BottomLeft, top - f32::from(z(GAP))) };
    let el = crate::media::lru::deferred(
        anchored()
            .position_mode(gpui::AnchoredPositionMode::Window)
            .anchor(corner)
            .snap_to_window_with_margin(z(8.))
            .position(point(px(mid - span / 2.), px(y)))
            .child(frame),
    )
    .with_priority(3);
    Some(div().absolute().top_0().left_0().child(el).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn text_follows_the_client_cases() {
        let n = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let t = crate::i18n::t;
        assert_eq!(text(&[], 3, ":fire:", true), None);
        assert_eq!(text(&n(&["Ann"]), 1, ":fire:", true), Some(t("burst.tip_1", &[("a", "Ann"), ("emoji", ":fire:")])));
        assert_eq!(text(&n(&["Ann"]), 2, ":fire:", true), Some(t("burst.tip_1n_one", &[("a", "Ann"), ("n", "1"), ("emoji", ":fire:")])));
        assert_eq!(
            text(&n(&["Ann", "Bob", "Cy"]), 7, ":fire:", true),
            Some(t("burst.tip_3n", &[("a", "Ann"), ("b", "Bob"), ("c", "Cy"), ("n", "4"), ("emoji", ":fire:")]))
        );
        assert_eq!(text(&n(&["Ann", "Bob"]), 2, ":x:", true), Some(t("burst.tip_2", &[("a", "Ann"), ("b", "Bob"), ("emoji", ":x:")])));
        assert_eq!(text(&n(&["Ann"]), 1, ":sweat_smile:", false), Some(t("reaction.tip_1", &[("a", "Ann"), ("emoji", ":sweat_smile:")])));
        assert_eq!(text(&n(&["Ann", "Bob"]), 3, ":x:", false), Some(t("reaction.tip_2n_one", &[("a", "Ann"), ("b", "Bob"), ("n", "1"), ("emoji", ":x:")])));
        assert_eq!(
            text(&n(&["Ann", "Bob", "Cy"]), 3, ":x:", false),
            Some(t("reaction.tip_3", &[("a", "Ann"), ("b", "Bob"), ("c", "Cy"), ("emoji", ":x:")]))
        );
    }

    #[test]
    fn french_strings_match_the_client() {
        let fr = include_str!("../../../locales/fr.yml");
        let en = include_str!("../../../locales/en.yml");
        for line in [
            "reaction.tip_1: \"{a} a réagi avec {emoji}\"",
            "reaction.tip_1n_one: \"{a} et {n}\u{a0}autre ont réagi avec {emoji}\"",
            "reaction.tip_2n: \"{a}, {b} et {n}\u{a0}autres ont réagi avec {emoji}\"",
            "reaction.tip_3n: \"{a}, {b}, {c} et {n}\u{a0}autres ont réagi avec {emoji}\"",
            "burst.tip_1n: \"{a} et {n}\u{a0}autres ont utilisé la super réaction {emoji}\"",
        ] {
            assert!(fr.contains(line), "{line}");
        }
        let o = others_label(4).unwrap_or_default();
        let line = format!("Ann et {o} ont réagi avec :x:");
        assert_eq!(link_range(&line, Some(&o)), Some(7..7 + o.len()));
        assert_eq!(others_label(0), None);
        assert!(en.contains("reaction.tip_3n: \"{emoji} reacted by {a}, {b}, {c}, and {n} others\""));
    }

    #[test]
    fn tooltip_waits_then_lingers() {
        let now = Instant::now();
        let ago = |ms: u64| now - Duration::from_millis(ms);
        assert!(!visible(Some((true, ago(100))), None, false, SHOW_AFTER));
        assert!(visible(Some((true, ago(800))), None, false, SHOW_AFTER));
        assert!(visible(Some((false, ago(50))), None, true, SHOW_AFTER));
        assert!(!visible(Some((false, ago(400))), None, true, SHOW_AFTER));
        assert!(visible(Some((false, ago(400))), Some((true, ago(300))), true, SHOW_AFTER));
        assert!(!visible(Some((false, ago(50))), None, false, SHOW_AFTER));
        assert!(visible(Some((true, ago(550))), None, false, show_after(false)));
        assert!(!visible(Some((true, ago(550))), None, false, show_after(true)));
    }
}
