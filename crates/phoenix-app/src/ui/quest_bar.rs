use chrono::{DateTime, Utc};
use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, img, prelude::*};

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::quests_page::{self, Quest, Task},
};

pub const QUEST_BAR_CONTENT: u64 = 2;
pub const RING: f32 = 42.;
pub const STROKE: f32 = 3.;
pub const PAD_Y: f32 = 14.;
pub const PAD_X: f32 = 12.;
pub const HEIGHT: f32 = RING + 2. * PAD_Y;

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

pub fn eligible(q: &Quest, now: DateTime<Utc>) -> bool {
    q.enrolled && !q.claimed && q.dismissed_content & QUEST_BAR_CONTENT == 0 && !q.expires.is_some_and(|e| e <= now)
}

pub fn pick(list: &[Quest], now: DateTime<Utc>) -> Option<Quest> {
    list.iter().filter(|q| eligible(q, now)).max_by_key(|q| q.enrolled_at).cloned()
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

pub fn subtitle(q: &Quest) -> SharedString {
    if q.completed {
        let d = q.rewards_expire.map(|e| e.with_timezone(&chrono::Local).format("%d/%m/%Y").to_string()).unwrap_or_default();
        return crate::t!("quests.bar_claim_by", date = d);
    }
    if matches!(q.task, Task::Video { .. }) {
        return crate::t!("quests.bar_watch");
    }
    if q.progress > 0. { crate::t!("quests.bar_start") } else { crate::t!("quests.bar_no_progress") }
}

pub fn visible(app: &PhoenixApp) -> bool {
    app.selected_guild.is_none() && !app.favorites_view && app.nav.home == crate::ui::home::data::HomeView::Friends && app.voice_channel.is_none()
}

fn ring(pct: f32) -> AnyElement {
    let arc = gpui::canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            let s = f32::from(b.size.width);
            let k = s / RING;
            let w = STROKE * k;
            let r = s / 2. - w / 2.;
            let (cx, cy) = (f32::from(b.origin.x) + s / 2., f32::from(b.origin.y) + s / 2.);
            let pts = |from: f32, to: f32| -> Vec<gpui::Point<gpui::Pixels>> {
                let n = ((to - from) * 96.).ceil().max(2.) as usize;
                (0..=n)
                    .map(|i| {
                        let a = (from + (to - from) * i as f32 / n as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                        gpui::point(gpui::px(cx + r * a.cos()), gpui::px(cy + r * a.sin()))
                    })
                    .collect()
            };
            for _ in 0..2 {
                let mut pb = gpui::PathBuilder::stroke(gpui::px(w));
                pb.add_polygon(&pts(0., 1.), true);
                if let Ok(p) = pb.build() {
                    window.paint_path(p, tk(idx::BACKGROUND_MOD_STRONG));
                }
            }
            if pct > 0. {
                let mut pb = gpui::PathBuilder::stroke(gpui::px(w));
                pb.add_polygon(&pts(0., pct.min(1.)), pct >= 1.);
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
                .child(img(quests_page::REWARD_ART_SMALL).size_full().object_fit(gpui::ObjectFit::Cover)),
        )
        .child(arc)
        .into_any_element()
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if !visible(app) {
        return None;
    }
    quests_page::ensure_loaded(app, cx);
    let q = pick(&quests_page::all_quests()?, Utc::now())?;
    let pct = if q.completed { 1. } else { q.progress };
    let line = |size: f32, lh: f32, weight: FontWeight, color: usize, s: SharedString| {
        div().min_w_0().text_size(z(size)).line_height(z(lh)).font_weight(weight).text_color(tk(color)).text_ellipsis().child(s)
    };
    Some(
        div()
            .id("quest-bar")
            .flex_none()
            .h(z(HEIGHT))
            .px(z(PAD_X))
            .py(z(PAD_Y))
            .bg(tk(idx::BACKGROUND_BASE_LOWER))
            .flex()
            .items_center()
            .gap(z(8.))
            .cursor_pointer()
            .on_click(cx.listener(|a, _, _, cx| a.open_home_view(crate::ui::home::data::HomeView::Quests, cx)))
            .child(ring(pct))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(line(14., 18., FontWeight::SEMIBOLD, idx::TEXT_STRONG, title(&q)))
                    .child(line(12., 16., FontWeight::NORMAL, idx::TEXT_SUBTLE, subtitle(&q))),
            )
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quest(f: impl FnOnce(&mut serde_json::Value)) -> Quest {
        let mut v = serde_json::json!({"quests": [{"id": "1", "config": {"expires_at": "2999-01-01T00:00:00+00:00", "messages": {}, "rewards_config": {"rewards": [{}]},
            "task_config_v2": {"tasks": {"PLAY_ON_DESKTOP": {"target": 900}}}},
            "user_status": {"enrolled_at": "2026-11-20T00:00:00+00:00", "completed_at": null, "claimed_at": null, "dismissed_quest_content": 0,
            "progress": {"PLAY_ON_DESKTOP": {"value": 760}}}}]});
        f(&mut v["quests"][0]);
        quests_page::parse(&v, Utc::now()).remove(0)
    }

    #[test]
    fn enrolled_quest_in_progress_shows_floored_percent() {
        let q = quest(|_| {});
        assert!(eligible(&q, Utc::now()));
        assert_eq!(percent(&q), 84);
        assert_eq!(subtitle(&q), crate::t!("quests.bar_start"));
    }

    #[test]
    fn dismissed_or_unenrolled_quests_hide_the_bar() {
        let q = quest(|v| v["user_status"]["dismissed_quest_content"] = 2.into());
        assert!(!eligible(&q, Utc::now()));
        let q = quest(|v| v["user_status"]["enrolled_at"] = serde_json::Value::Null);
        assert!(pick(&[q], Utc::now()).is_none());
    }
}
