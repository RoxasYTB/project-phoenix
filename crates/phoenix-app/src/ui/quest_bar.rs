use std::cell::{Cell, RefCell};

use chrono::{DateTime, Utc};
use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, img, linear_color_stop, linear_gradient, prelude::*, rgba};
use serde_json::Value;

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::quests_page::{self, Quest, Task},
};

pub const QUEST_BAR_CONTENT: u64 = 2;
pub const QUEST_BAR_V2: u8 = 10;
pub const CREATIVE_QUEST: u64 = 1;
pub const DECISION_PATH: &str = "/quests/decision?placement=1";
pub const RING: f32 = 42.;
pub const STROKE: f32 = 3.;
pub const PAD_Y: f32 = 14.;
pub const PAD_X: f32 = 12.;
pub const HEIGHT: f32 = RING + 2. * PAD_Y;
pub const EXPANDED_PAD: f32 = 12.;
pub const HERO_DETAILS_TOP: f32 = 74.;

thread_local! {
    static EXPANDED: Cell<bool> = const { Cell::new(false) };
    static DECISION: RefCell<Option<Option<Quest>>> = const { RefCell::new(None) };
    static DECISION_REQUESTED: Cell<bool> = const { Cell::new(false) };
}

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

pub fn eligible(q: &Quest, now: DateTime<Utc>) -> bool {
    !q.claimed && q.dismissed_content & QUEST_BAR_CONTENT == 0 && !q.expires.is_some_and(|e| e <= now)
}

pub fn pick(list: &[Quest], now: DateTime<Utc>) -> Option<Quest> {
    list.iter().filter(|q| q.enrolled && eligible(q, now)).max_by_key(|q| q.enrolled_at).cloned()
}

pub fn parse_decision(v: &Value, now: DateTime<Utc>) -> Option<Quest> {
    let creative = &v["creative"];
    let config = if creative.is_object() {
        if creative["creative_type"].as_u64() != Some(CREATIVE_QUEST) {
            return None;
        }
        creative["creative_content"].clone()
    } else {
        v["quest"]["config"].clone()
    };
    let id = config["id"].as_str().or_else(|| v["quest"]["id"].as_str())?.to_owned();
    let status = v["user_status"].clone();
    let status = if status["quest_id"].as_str().is_some_and(|q| q != id) { Value::Null } else { status };
    let wrapped = serde_json::json!({"quests": [{"id": id, "config": config, "user_status": status}]});
    quests_page::parse(&wrapped, now).into_iter().find(|q| eligible(q, now))
}

pub fn percent(q: &Quest) -> u32 {
    (q.progress.clamp(0., 1.) * 100.).floor() as u32
}

pub fn title(q: &Quest) -> SharedString {
    if q.completed {
        return crate::t!("quests.bar_done");
    }
    if q.enrolled && q.progress > 0. {
        return crate::t!("quests.bar_percent", n = percent(q));
    }
    if matches!(q.task, Task::Play { .. }) { crate::t!("quests.bar_start") } else { crate::t!("quests.bar_ready") }
}

fn date(d: Option<DateTime<Utc>>) -> String {
    d.map(|e| e.with_timezone(&chrono::Local).format("%d/%m/%Y").to_string()).unwrap_or_default()
}

pub fn subtitle(q: &Quest, expanded: bool) -> SharedString {
    if q.completed {
        return crate::t!("quests.bar_claim_by", date = date(q.rewards_expire));
    }
    if expanded {
        return quests_page::description(q, &reward_label(q));
    }
    if matches!(q.task, Task::Video { .. }) {
        return crate::t!("quests.bar_watch");
    }
    if q.progress > 0. { crate::t!("quests.bar_start") } else { crate::t!("quests.bar_no_progress") }
}

fn reward_label(q: &Quest) -> SharedString {
    match q.orbs {
        Some(n) => crate::t!("quests.orbs", n = n),
        None => q.reward.clone(),
    }
}

fn reward_art(q: &Quest) -> &'static str {
    if q.orbs.unwrap_or(0) >= 700 { quests_page::REWARD_ART_LARGE } else { quests_page::REWARD_ART_SMALL }
}

pub fn visible(app: &PhoenixApp) -> bool {
    app.selected_guild.is_none()
        && !app.favorites_view
        && app.nav.home == crate::ui::home::data::HomeView::Friends
        && app.voice_channel.is_none()
        && !app.clips.active()
}

pub fn force_expanded(on: bool) {
    EXPANDED.with(|e| e.set(on));
}

pub fn expanded() -> bool {
    EXPANDED.with(Cell::get)
}

fn set_expanded(on: bool, cx: &mut Context<PhoenixApp>) {
    if EXPANDED.with(|e| e.replace(on)) != on {
        cx.notify();
    }
}

fn ensure_decision(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if DECISION_REQUESTED.with(|r| r.replace(true)) {
        return;
    }
    let fut = app.backend.quest_decision();
    cx.spawn(async move |this, cx| {
        let v = fut.await.unwrap_or(Value::Null);
        DECISION.with(|d| *d.borrow_mut() = Some(parse_decision(&v, Utc::now())));
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

pub fn current(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<Quest> {
    if !visible(app) {
        return None;
    }
    quests_page::ensure_loaded(app, cx);
    ensure_decision(app, cx);
    let now = Utc::now();
    let enrolled = quests_page::all_quests().and_then(|l| pick(&l, now));
    let decided = DECISION.with(|d| d.borrow().clone().flatten());
    match (enrolled, decided) {
        (_, Some(d)) if d.enrolled => Some(quests_page::quest(d.id).unwrap_or(d)),
        (Some(e), _) => Some(e),
        (None, d) => d,
    }
}

fn ring(q: &Quest) -> AnyElement {
    let pct = if q.completed { 1. } else { q.progress };
    let arc = gpui::canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            let s = f32::from(b.size.width);
            let w = STROKE * s / RING;
            let r = s / 2. - w / 2.;
            let (cx, cy) = (f32::from(b.origin.x) + s / 2., f32::from(b.origin.y) + s / 2.);
            let pts = |to: f32| -> Vec<gpui::Point<gpui::Pixels>> {
                let n = (to * 96.).ceil().max(2.) as usize;
                (0..=n)
                    .map(|i| {
                        let a = to * i as f32 / n as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                        gpui::point(gpui::px(cx + r * a.cos()), gpui::px(cy + r * a.sin()))
                    })
                    .collect()
            };
            for _ in 0..2 {
                let mut pb = gpui::PathBuilder::stroke(gpui::px(w));
                pb.add_polygon(&pts(1.), true);
                if let Ok(p) = pb.build() {
                    window.paint_path(p, tk(idx::BACKGROUND_MOD_STRONG));
                }
            }
            if pct > 0. {
                let mut pb = gpui::PathBuilder::stroke(gpui::px(w));
                pb.add_polygon(&pts(pct.min(1.)), pct >= 1.);
                if let Ok(p) = pb.build() {
                    window.paint_path(p, tk(idx::STATUS_POSITIVE));
                }
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    div()
        .relative()
        .flex_none()
        .size(z(RING))
        .p(z(STROKE + 2.))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size_full()
                .rounded_full()
                .overflow_hidden()
                .bg(tk(idx::BACKGROUND_BASE_LOWEST))
                .child(img(reward_art(q)).size_full().rounded_full().object_fit(gpui::ObjectFit::Cover)),
        )
        .child(arc)
        .into_any_element()
}

fn line(size: f32, lh: f32, weight: FontWeight, color: usize, s: SharedString, wrap: bool) -> gpui::Div {
    let d = div().min_w_0().text_size(z(size)).line_height(z(lh)).font_weight(weight).text_color(tk(color)).child(s);
    if wrap { d } else { d.text_ellipsis() }
}

fn progress_row(q: &Quest, expanded: bool) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(z(8.))
        .child(ring(q))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(line(14., 18., FontWeight::SEMIBOLD, idx::TEXT_STRONG, title(q), false))
                .child(line(12., 16., FontWeight::NORMAL, idx::TEXT_SUBTLE, subtitle(q, expanded), expanded)),
        )
        .into_any_element()
}

fn completed_glow() -> AnyElement {
    let green = tk(idx::TEXT_FEEDBACK_POSITIVE);
    div()
        .absolute()
        .inset_0()
        .rounded_t(z(8.))
        .bg(linear_gradient(180., linear_color_stop(rgba(0x2dc7711a), 1.), linear_color_stop(rgba(0x2dc7710d), 0.)))
        .child(div().absolute().inset_0().rounded_t(z(8.)).border_t(z(2.)).border_l(z(2.)).border_color(green.opacity(0.25)))
        .into_any_element()
}

fn more_button(q: &Quest) -> AnyElement {
    let target = crate::ui::extra_menus::Target::QuestMore(Box::new(q.clone()));
    div()
        .id(("quest-bar-more", q.id))
        .size(z(24.))
        .flex_none()
        .cursor_pointer()
        .opacity(0.9)
        .hover(|s| s.opacity(1.))
        .active(|s| s.opacity(0.7))
        .on_click(move |ev, w, _| crate::ui::extra_menus::open(target.clone(), ev.position(), w))
        .child(gpui::svg().path(crate::icons::path("more-horizontal")).size(z(24.)).text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT)))
        .into_any_element()
}

fn accepted_content(q: &Quest) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .child(div().flex_1().min_w_0().opacity(0.7).child(line(12., 16., FontWeight::MEDIUM, idx::TEXT_DEFAULT, crate::t!("quests.bar_available_until", date = date(q.expires)), false)))
                .child(more_button(q)),
        )
        .child(div().h(z(1.)).w_full().mt(z(8.)).mb(z(12.)).bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
        .child(progress_row(q, true))
        .into_any_element()
}

fn promo_content(q: &Quest, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let id = q.id;
    div()
        .relative()
        .flex()
        .flex_col()
        .children(q.hero.clone().map(|h| {
            div()
                .absolute()
                .top(z(-EXPANDED_PAD))
                .left(z(-EXPANDED_PAD))
                .right(z(-EXPANDED_PAD))
                .h(z(HERO_DETAILS_TOP + 40.))
                .overflow_hidden()
                .rounded_t(z(8.))
                .child(img(h).size_full().object_fit(gpui::ObjectFit::Cover))
                .child(div().absolute().inset_0().bg(linear_gradient(180., linear_color_stop(gpui::transparent_black(), 0.), linear_color_stop(tk(idx::BACKGROUND_BASE_LOW), 1.))))
        }))
        .child(
            div()
                .relative()
                .mt(z(HERO_DETAILS_TOP - 8.))
                .flex()
                .items_end()
                .gap(z(8.))
                .child(div().flex_1())
                .child(more_button(q)),
        )
        .child(
            div()
                .relative()
                .mb(z(2.))
                .child(line(14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("quests.bar_quest_name", name = q.name.clone()), false).mb(z(2.)))
                .child(line(12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT, quests_page::description(q, &reward_label(q)), true)),
        )
        .child(
            crate::ui::controls::button(("quest-bar-accept", id), crate::t!("quests.bar_accept"), crate::ui::controls::BtnKind::Primary)
                .relative()
                .mt(z(8.))
                .w_full()
                .on_click(cx.listener(move |a, _, _, cx| a.blocked_write(crate::actions::guard::GuardedWrite::quest_enroll(id, QUEST_BAR_V2), cx))),
        )
        .into_any_element()
}

fn collapsed(q: &Quest, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let body = if q.enrolled {
        progress_row(q, false)
    } else {
        div()
            .flex()
            .items_center()
            .gap(z(8.))
            .child(img(reward_art(q)).size(z(RING)).flex_none().rounded(z(4.)).object_fit(gpui::ObjectFit::Cover))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(line(14., 18., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!("quests.bar_quest_name", name = q.name.clone()), false))
                    .child(line(12., 16., FontWeight::NORMAL, idx::TEXT_SUBTLE, reward_label(q), false)),
            )
            .into_any_element()
    };
    div()
        .id("quest-bar")
        .relative()
        .flex_none()
        .h(z(HEIGHT))
        .px(z(PAD_X))
        .py(z(PAD_Y))
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .when(q.completed, |d| d.child(completed_glow()))
        .when(!q.enrolled, |d| {
            d.children(q.hero.clone().map(|h| {
                div().absolute().inset_0().overflow_hidden().opacity(0.6).child(img(h).size_full().object_fit(gpui::ObjectFit::Cover))
            }))
        })
        .flex()
        .flex_col()
        .justify_center()
        .on_hover(cx.listener(|_, h: &bool, _, cx| {
            if *h {
                set_expanded(true, cx);
            }
        }))
        .child(div().relative().child(body))
        .into_any_element()
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let q = current(app, cx)?;
    Some(collapsed(&q, cx))
}

pub fn overlay(app: &PhoenixApp, bottom: f32, inset: f32, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if !expanded() {
        return None;
    }
    let Some(q) = current(app, cx) else {
        EXPANDED.with(|e| e.set(false));
        return None;
    };
    let content = if q.enrolled { accepted_content(&q) } else { promo_content(&q, cx) };
    Some(
        div()
            .id("quest-bar-expanded")
            .absolute()
            .left(z(inset))
            .right(z(inset))
            .bottom(z(bottom))
            .min_h(z(HEIGHT))
            .p(z(EXPANDED_PAD))
            .rounded_t(z(8.))
            .overflow_hidden()
            .bg(tk(idx::HOME_BACKGROUND))
            .border_t(z(2.))
            .border_color(tk(idx::BACKGROUND_MOD_SUBTLE))
            .shadow(vec![gpui::BoxShadow { color: gpui::hsla(0., 0., 0., 0.3), offset: gpui::point(gpui::px(0.), z(-2.)), blur_radius: z(4.), spread_radius: gpui::px(0.), inset: false }])
            .when(q.enrolled, |d| d.child(div().absolute().inset_0().bg(linear_gradient(90., linear_color_stop(gpui::transparent_black(), 0.), linear_color_stop(rgba(0xcdd8ff1a), 1.)))))
            .when(q.completed, |d| d.child(completed_glow()))
            .occlude()
            .on_hover(cx.listener(|_, h: &bool, _, cx| {
                if !*h {
                    set_expanded(false, cx);
                }
            }))
            .child(div().relative().child(content))
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw() -> Value {
        serde_json::json!({"id": "1", "config": {"id": "1", "expires_at": "2999-01-01T00:00:00+00:00", "messages": {"quest_name": "Q"}, "rewards_config": {"rewards": [{}]},
            "task_config_v2": {"tasks": {"PLAY_ON_DESKTOP": {"target": 900}}}},
            "user_status": {"enrolled_at": "2026-11-20T00:00:00+00:00", "completed_at": null, "claimed_at": null, "dismissed_quest_content": 0,
            "progress": {"PLAY_ON_DESKTOP": {"value": 760}}}})
    }

    fn quest(f: impl FnOnce(&mut Value)) -> Quest {
        let mut v = serde_json::json!({"quests": [raw()]});
        f(&mut v["quests"][0]);
        quests_page::parse(&v, Utc::now()).remove(0)
    }

    #[test]
    fn enrolled_quest_in_progress_shows_floored_percent() {
        let q = quest(|_| {});
        assert!(pick(std::slice::from_ref(&q), Utc::now()).is_some());
        assert_eq!(percent(&q), 84);
        assert_eq!(subtitle(&q, false), crate::t!("quests.bar_start"));
    }

    #[test]
    fn dismissed_or_unenrolled_quests_hide_the_bar() {
        let q = quest(|v| v["user_status"]["dismissed_quest_content"] = 2.into());
        assert!(!eligible(&q, Utc::now()));
        let q = quest(|v| v["user_status"]["enrolled_at"] = Value::Null);
        assert!(pick(&[q], Utc::now()).is_none());
    }

    #[test]
    fn decision_creative_becomes_a_promoted_quest() {
        let r = raw();
        let v = serde_json::json!({"creative": {"creative_type": 1, "creative_content": r["config"]}, "user_status": null});
        let q = parse_decision(&v, Utc::now()).expect("quest");
        assert_eq!(q.id, 1);
        assert!(!q.enrolled);
        let bounty = serde_json::json!({"creative": {"creative_type": 3, "creative_content": r["config"]}});
        assert!(parse_decision(&bounty, Utc::now()).is_none());
    }
}
