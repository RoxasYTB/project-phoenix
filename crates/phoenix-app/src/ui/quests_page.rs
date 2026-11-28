use std::cell::{Cell, RefCell};

use chrono::{DateTime, Utc};
use gpui::{AnyElement, Context, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div, img, prelude::*, rgba};
use serde_json::Value;

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
};

pub const CDN: &str = "https://cdn.discordapp.com/";
pub const TILE_MIN_W: f32 = 336.;
pub const GAP: f32 = 24.;
pub const HERO_ART: &str = "https://cdn.discordapp.com/assets/content/fff9144c814ffc3126fdcaf5ec5bc68eff6d09e23ac8af5a3fbe61b05970cda0.png";
pub const REWARD_ART_SMALL: &str = "https://cdn.discordapp.com/assets/content/d4fc1c99b3e3044530f08ede0156d1d43a13bf78f0f539e240b63f2d4dad3554.webm?format=webp";
pub const REWARD_ART_LARGE: &str = "https://cdn.discordapp.com/assets/content/864e5841fd9af8da3c50ad6d6220bee54e8c2da2d36d3c85d0a5ae95cdbf095e.webm?format=webp";
pub const APP_FEATURE: u64 = 26;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    Video { seconds: u64 },
    Play { minutes: u64 },
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Quest {
    pub id: u64,
    pub name: SharedString,
    pub game: SharedString,
    pub publisher: SharedString,
    pub hero: Option<SharedString>,
    pub logotype: Option<SharedString>,
    pub expires: Option<DateTime<Utc>>,
    pub reward: SharedString,
    pub orbs: Option<u64>,
    pub premium_orbs: Option<u64>,
    pub task: Task,
    pub app_quest: bool,
    pub cta: Option<SharedString>,
    pub cta_link: Option<SharedString>,
    pub enrolled: bool,
    pub completed: bool,
    pub claimed: bool,
    pub progress: f32,
    pub starts: Option<DateTime<Utc>>,
    pub enrolled_at: Option<DateTime<Utc>>,
    pub reward_types: Vec<u64>,
    pub tasks: Vec<String>,
    pub video_thumb: Option<SharedString>,
    pub video_portrait: bool,
    pub dismissed_content: u64,
    pub rewards_expire: Option<DateTime<Utc>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Suggested,
    MostRecent,
    ExpiringSoon,
    RecentlyEnrolled,
}

pub const SORTS: [Sort; 4] = [Sort::Suggested, Sort::MostRecent, Sort::ExpiringSoon, Sort::RecentlyEnrolled];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Orbs,
    Collectible,
    InGame,
    Play,
    Video,
}

pub const REWARD_FILTERS: [Filter; 3] = [Filter::Orbs, Filter::Collectible, Filter::InGame];
pub const TASK_FILTERS: [Filter; 2] = [Filter::Play, Filter::Video];
pub const PINNED_QUEST: u64 = 1410358070831480904;
pub const STALE_ENROLLMENT_MS: i64 = 20 * 60 * 1000;
pub const VIDEO_TASKS: [&str; 2] = ["WATCH_VIDEO", "WATCH_VIDEO_ON_MOBILE"];
pub const PLAY_TASKS: [&str; 7] = ["PLAY_ON_DESKTOP", "STREAM_ON_DESKTOP", "PLAY_ON_PLAYSTATION", "PLAY_ON_XBOX", "ACHIEVEMENT_IN_GAME", "ACHIEVEMENT_IN_ACTIVITY", "PLAY_ACTIVITY"];

pub fn matches(q: &Quest, f: Filter) -> bool {
    match f {
        Filter::Orbs => q.reward_types.contains(&4),
        Filter::Collectible => q.reward_types.contains(&3),
        Filter::InGame => q.reward_types.iter().any(|t| *t == 2 || *t == 1),
        Filter::Play => q.tasks.iter().any(|t| PLAY_TASKS.contains(&t.as_str())),
        Filter::Video => q.tasks.iter().any(|t| VIDEO_TASKS.contains(&t.as_str())),
    }
}

pub fn apply_filters(list: Vec<Quest>, filters: &[Filter]) -> Vec<Quest> {
    let rewards: Vec<Filter> = filters.iter().copied().filter(|f| REWARD_FILTERS.contains(f)).collect();
    let tasks: Vec<Filter> = filters.iter().copied().filter(|f| TASK_FILTERS.contains(f)).collect();
    list.into_iter().filter(|q| (rewards.is_empty() || rewards.iter().any(|f| matches(q, *f))) && (tasks.is_empty() || tasks.iter().any(|f| matches(q, *f)))).collect()
}

fn expired(q: &Quest, now: DateTime<Utc>) -> bool {
    q.expires.is_some_and(|e| e <= now)
}

fn stale(q: &Quest, now: DateTime<Utc>) -> bool {
    q.enrolled_at.is_some_and(|e| !q.completed && (now - e).num_milliseconds() > STALE_ENROLLMENT_MS)
}

fn by_date(a: Option<DateTime<Utc>>, b: Option<DateTime<Utc>>, ascending: bool) -> std::cmp::Ordering {
    let o = a.cmp(&b);
    if ascending { o } else { o.reverse() }
}

fn suggested(a: &Quest, b: &Quest, now: DateTime<Utc>) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    let pa = a.id == PINNED_QUEST && !a.completed;
    let pb = b.id == PINNED_QUEST && !b.completed;
    if pa != pb {
        return if pa { Less } else { Greater };
    }
    if !expired(a, now) {
        let (sa, sb) = (stale(a, now), stale(b, now));
        if sa != sb {
            return if sa { Greater } else { Less };
        }
        if a.claimed != b.claimed {
            return if a.claimed { Greater } else { Less };
        }
        if a.enrolled != b.enrolled {
            return if a.enrolled { Less } else { Greater };
        }
        return by_date(a.expires, b.expires, true);
    }
    if a.claimed != b.claimed {
        return if a.claimed { Less } else { Greater };
    }
    if a.enrolled != b.enrolled {
        return if a.enrolled { Less } else { Greater };
    }
    by_date(a.expires, b.expires, false)
}

pub fn sort_quests(mut list: Vec<Quest>, sort: Sort, now: DateTime<Utc>) -> Vec<Quest> {
    list.sort_by(|a, b| {
        let (ea, eb) = (expired(a, now), expired(b, now));
        if ea != eb {
            return if ea { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less };
        }
        match sort {
            Sort::Suggested => suggested(a, b, now),
            Sort::MostRecent => by_date(a.starts, b.starts, false),
            Sort::ExpiringSoon => by_date(a.expires, b.expires, true),
            Sort::RecentlyEnrolled => match (a.enrolled_at, b.enrolled_at) {
                (None, None) => by_date(a.expires, b.expires, false),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (x, y) => by_date(x, y, false),
            },
        }
    });
    list
}

pub fn sort_label(s: Sort) -> SharedString {
    match s {
        Sort::Suggested => crate::t!("quests.suggested"),
        Sort::MostRecent => crate::t!("quests.sort_recent"),
        Sort::ExpiringSoon => crate::t!("quests.sort_expiring"),
        Sort::RecentlyEnrolled => crate::t!("quests.sort_started"),
    }
}

pub fn filter_label(f: Filter) -> SharedString {
    match f {
        Filter::Orbs => crate::t!("quests.filter_orbs"),
        Filter::Collectible => crate::t!("quests.filter_deco"),
        Filter::InGame => crate::t!("quests.filter_ingame"),
        Filter::Play => crate::t!("quests.filter_game"),
        Filter::Video => crate::t!("quests.filter_video"),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Claimed {
    pub id: u64,
    pub name: SharedString,
    pub art: Option<SharedString>,
    pub orbs: bool,
    pub logotype: Option<SharedString>,
    pub claimed_at: Option<DateTime<Utc>>,
}

pub fn parse_claimed(v: &Value) -> Vec<Claimed> {
    let mut out: Vec<Claimed> = v
        .get("quests")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|q| {
            let cfg = &q["config"];
            let r = cfg["rewards"].get(0).or_else(|| cfg["rewards_config"]["rewards"].get(0)).cloned().unwrap_or(Value::Null);
            Some(Claimed {
                id: q["id"].as_str()?.parse().ok()?,
                name: cfg["messages"]["quest_name"].as_str().unwrap_or_default().to_owned().into(),
                art: asset(&r["asset"]).filter(|a| !a.ends_with(".mp4") && !a.ends_with(".webm")),
                orbs: r["type"].as_u64() == Some(4),
                logotype: asset(&cfg["assets"]["logotype_dark"]).or_else(|| asset(&cfg["assets"]["logotype"])),
                claimed_at: date(&q["user_status"]["claimed_at"]),
            })
        })
        .collect();
    out.sort_by_key(|c| std::cmp::Reverse(c.claimed_at));
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Popout {
    Sort,
    Filter,
}

thread_local! {
    static CLAIMED_TAB: Cell<bool> = const { Cell::new(false) };
    static CLAIMED: RefCell<Option<Vec<Claimed>>> = const { RefCell::new(None) };
    static CLAIMED_REQUESTED: Cell<bool> = const { Cell::new(false) };
    static SORT: Cell<Sort> = const { Cell::new(Sort::Suggested) };
    static FILTERS: RefCell<Vec<Filter>> = const { RefCell::new(Vec::new()) };
    static POPOUT: Cell<Option<(Popout, gpui::Point<gpui::Pixels>)>> = const { Cell::new(None) };
    static BUTTONS: Cell<[Option<gpui::Bounds<gpui::Pixels>>; 2]> = const { Cell::new([None, None]) };
    static QUESTS: RefCell<Option<Vec<Quest>>> = const { RefCell::new(None) };
    static BALANCE: Cell<Option<u64>> = const { Cell::new(None) };
    static REQUESTED: Cell<bool> = const { Cell::new(false) };
    static FAILED: Cell<bool> = const { Cell::new(false) };
    static CLAIMED_FAILED: Cell<bool> = const { Cell::new(false) };
    static HERO: RefCell<Option<HomeHero>> = const { RefCell::new(None) };
    static HERO_REQUESTED: Cell<bool> = const { Cell::new(false) };
    static HERO_SEEN: RefCell<Option<SeenStore>> = const { RefCell::new(None) };
}

pub const AD_SEEN_FILE: &str = "ad-content-seen.json";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeenStore(pub std::collections::BTreeMap<u64, Vec<String>>);

impl SeenStore {
    pub fn parse(v: &Value) -> Self {
        let mut out = std::collections::BTreeMap::new();
        if let Some(m) = v["seenContentIds"].as_object() {
            for (k, ids) in m {
                let Ok(t) = k.parse::<u64>() else { continue };
                let ids: Vec<String> = ids.as_array().into_iter().flatten().filter_map(|i| i.as_str().map(str::to_owned)).collect();
                out.insert(t, ids);
            }
        }
        Self(out)
    }

    pub fn to_json(&self) -> Value {
        let m: serde_json::Map<String, Value> = self.0.iter().map(|(k, v)| (k.to_string(), Value::from(v.clone()))).collect();
        serde_json::json!({ "seenContentIds": m })
    }

    pub fn has_seen(&self, kind: u64, id: &str) -> bool {
        self.0.get(&kind).is_some_and(|s| s.iter().any(|x| x == id))
    }

    pub fn mark_seen(&mut self, kind: u64, id: &str) -> bool {
        let set = self.0.entry(kind).or_default();
        if set.iter().any(|x| x == id) {
            return false;
        }
        set.push(id.to_owned());
        true
    }

    pub fn keep_only(&mut self, kind: u64, id: Option<&str>) -> bool {
        let Some(set) = self.0.get_mut(&kind) else { return false };
        let before = set.len();
        set.retain(|x| Some(x.as_str()) == id);
        set.len() != before
    }
}

fn with_seen<R>(f: impl FnOnce(&mut SeenStore) -> (R, bool)) -> R {
    HERO_SEEN.with(|s| {
        let mut s = s.borrow_mut();
        let store = s.get_or_insert_with(|| {
            crate::paths::config_file(AD_SEEN_FILE)
                .and_then(|p| std::fs::read(p).ok())
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .map(|v| SeenStore::parse(&v))
                .unwrap_or_default()
        });
        let (r, changed) = f(store);
        if changed && let Some(p) = crate::paths::config_file(AD_SEEN_FILE) {
            let _ = std::fs::write(p, store.to_json().to_string());
        }
        r
    })
}

pub const QUEST_HOME_BANNER_DESKTOP: u8 = 3;
pub const CREATIVE_QUEST_HOME_HERO: u64 = 2;
pub const QUEST_HOME_ENTRYPOINT_ONBOARDING: usize = 632;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HomeHero {
    pub id: String,
    pub image: Option<SharedString>,
    pub has_content: bool,
    pub tooltip: Option<(SharedString, SharedString)>,
    pub sponsored: Option<Sponsored>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sponsored {
    pub title: SharedString,
    pub subtitle: SharedString,
    pub image: Option<SharedString>,
    pub sponsor: Option<SharedString>,
    pub cta: Option<(SharedString, SharedString)>,
}

pub fn home_hero_path() -> String {
    format!("/quests/get-decisions?placement={QUEST_HOME_BANNER_DESKTOP}&num_decisions_requested=1")
}

fn filled(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.is_empty())
}

fn gradient(v: &Value) -> bool {
    filled(&v["start"]) && filled(&v["end"])
}

pub fn parse_home_hero(v: &Value) -> Option<HomeHero> {
    let c = &v["decisions"][0]["creative"];
    if c["creative_type"].as_u64() != Some(CREATIVE_QUEST_HOME_HERO) {
        return None;
    }
    let content = &c["creative_content"];
    let e = &content["quest_home_entrypoint"];
    let id = match &content["id"] {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    };
    let image = asset(&e["image"]);
    let tooltip = filled(&e["tooltip_title"]) && filled(&e["tooltip_subtitle"]);
    let has_content = !e.is_null() && (image.is_some() || gradient(&e["linear_gradient"]) || gradient(&e["radial_gradient"]) || tooltip);
    let text = |v: &Value| v.as_str().map(|s| SharedString::from(s.to_owned()));
    let tooltip = text(&e["tooltip_title"]).map(|t| (t, text(&e["tooltip_subtitle"]).unwrap_or_default()));
    let sponsored = text(&content["label_title"]).map(|title| Sponsored {
        title,
        subtitle: text(&content["label_subtitle"]).unwrap_or_default(),
        image: asset(&content["hero_image"]),
        sponsor: asset(&content["sponsor_image"]),
        cta: text(&content["cta"]["url"]).zip(text(&content["cta"]["button_label"])),
    });
    Some(HomeHero { id, image, has_content, tooltip, sponsored })
}

pub fn dismissed(contents: &[u8], id: usize) -> bool {
    contents.get(id / 8).is_some_and(|b| b & (1 << (id % 8)) != 0)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntryPoint {
    pub onboarding: bool,
    pub hero_badge: bool,
    pub hero: Option<HomeHero>,
}

pub fn entry_point(has_quests: bool, onboarding_dismissed: bool, hero: Option<HomeHero>, seen: bool) -> EntryPoint {
    let hero = hero.filter(|h| h.has_content);
    let shows_hero = hero.is_some();
    EntryPoint { onboarding: !shows_hero && has_quests && !onboarding_dismissed, hero_badge: shows_hero && !seen, hero }
}

pub fn sidebar_entry(onboarding_dismissed: bool) -> EntryPoint {
    let has_quests = QUESTS.with(|q| q.borrow().as_ref().is_some_and(|l| !l.is_empty()));
    let hero = HERO.with(|h| h.borrow().clone());
    let seen = hero.as_ref().is_some_and(|h| with_seen(|s| (s.has_seen(CREATIVE_QUEST_HOME_HERO, &h.id), false)));
    entry_point(has_quests, onboarding_dismissed, hero, seen)
}

pub fn mark_hero_seen() {
    if let Some(id) = HERO.with(|h| h.borrow().as_ref().map(|h| h.id.clone())) {
        with_seen(|s| ((), s.mark_seen(CREATIVE_QUEST_HOME_HERO, &id)));
    }
}

pub(crate) fn ensure_hero(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if HERO_REQUESTED.with(|r| r.replace(true)) {
        return;
    }
    let fut = app.backend.quest_home_hero();
    cx.spawn(async move |this, cx| {
        let got = fut.await;
        let ok = got.is_ok();
        let v = got.unwrap_or(Value::Null);
        let hero = parse_home_hero(&v);
        if ok {
            let id = hero.as_ref().map(|h| h.id.clone());
            with_seen(|s| ((), s.keep_only(CREATIVE_QUEST_HOME_HERO, id.as_deref())));
        }
        HERO.with(|h| *h.borrow_mut() = hero);
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

fn asset(v: &Value) -> Option<SharedString> {
    let p = v.as_str()?;
    if p.is_empty() || p == "PLACEHOLDER" {
        return None;
    }
    Some(if p.starts_with("http") { p.to_owned().into() } else { format!("{CDN}{p}").into() })
}

fn date(v: &Value) -> Option<DateTime<Utc>> {
    v.as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc))
}

fn task_of(tasks: &serde_json::Map<String, Value>) -> Task {
    for k in ["WATCH_VIDEO", "WATCH_VIDEO_ON_MOBILE"] {
        if let Some(t) = tasks.get(k) {
            return Task::Video { seconds: t["target"].as_u64().unwrap_or(0) };
        }
    }
    for k in ["PLAY_ON_DESKTOP", "PLAY_ACTIVITY", "STREAM_ON_DESKTOP", "PLAY_ON_XBOX", "PLAY_ON_PLAYSTATION"] {
        if let Some(t) = tasks.get(k) {
            return Task::Play { minutes: t["target"].as_u64().unwrap_or(0) / 60 };
        }
    }
    Task::Other
}

pub fn parse(v: &Value, now: DateTime<Utc>) -> Vec<Quest> {
    let list = v.get("quests").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut out = Vec::new();
    for q in &list {
        let cfg = &q["config"];
        let expires = date(&cfg["expires_at"]);
        if expires.is_some_and(|e| e <= now) {
            continue;
        }
        let msgs = &cfg["messages"];
        let reward = &cfg["rewards_config"]["rewards"][0];
        let us = &q["user_status"];
        let tasks = cfg["task_config_v2"]["tasks"].as_object().cloned().unwrap_or_default();
        let progress = tasks
            .iter()
            .filter_map(|(k, t)| {
                let target = t["target"].as_f64().filter(|t| *t > 0.)?;
                let value = us["progress"][k]["value"].as_f64().unwrap_or(0.);
                Some((value / target).clamp(0., 1.) as f32)
            })
            .fold(0f32, f32::max);
        let features: Vec<u64> = cfg["features"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
        out.push(Quest {
            id: q["id"].as_str().and_then(|s| s.parse().ok()).unwrap_or_default(),
            name: msgs["quest_name"].as_str().unwrap_or_default().to_owned().into(),
            game: msgs["game_title"].as_str().unwrap_or_default().to_owned().into(),
            publisher: msgs["game_publisher"].as_str().unwrap_or_default().to_owned().into(),
            hero: asset(&cfg["assets"]["hero"]),
            logotype: asset(&cfg["assets"]["logotype_dark"]).or_else(|| asset(&cfg["assets"]["logotype"])),
            expires,
            reward: reward["messages"]["name"].as_str().unwrap_or_default().to_owned().into(),
            orbs: reward["orb_quantity"].as_u64(),
            premium_orbs: reward["premium_orb_quantity"].as_u64(),
            task: task_of(&tasks),
            app_quest: features.contains(&APP_FEATURE),
            cta: cfg["cta_config"]["button_label"].as_str().filter(|s| !s.is_empty()).map(|s| s.to_owned().into()),
            cta_link: cfg["cta_config"]["link"].as_str().filter(|s| !s.is_empty()).map(|s| s.to_owned().into()),
            enrolled: !us["enrolled_at"].is_null(),
            completed: !us["completed_at"].is_null(),
            claimed: !us["claimed_at"].is_null(),
            progress,
            starts: date(&cfg["starts_at"]),
            enrolled_at: date(&us["enrolled_at"]),
            reward_types: cfg["rewards_config"]["rewards"].as_array().map(|r| r.iter().filter_map(|x| x["type"].as_u64()).collect()).unwrap_or_default(),
            tasks: tasks.keys().cloned().collect(),
            video_thumb: asset(&tasks.get("WATCH_VIDEO").map_or(Value::Null, |t| t["assets"]["video"]["thumbnail"].clone())).or_else(|| asset(&cfg["assets"]["hero"])),
            video_portrait: tasks
                .get("WATCH_VIDEO")
                .is_some_and(|t| t["assets"]["video"]["height"].as_u64().unwrap_or(0) > t["assets"]["video"]["width"].as_u64().unwrap_or(u64::MAX)),
            dismissed_content: us["dismissed_quest_content"].as_u64().unwrap_or(0),
            rewards_expire: date(&cfg["rewards_config"]["rewards_expire_at"]),
        });
    }
    out.retain(|q| !q.claimed);
    out
}

pub fn reward_amount(q: &Quest, premium: bool) -> Option<u64> {
    if premium { q.premium_orbs.or(q.orbs) } else { q.orbs }
}

pub fn multiplier(q: &Quest) -> Option<String> {
    let (base, boosted) = (q.orbs?, q.premium_orbs?);
    if base == 0 || boosted <= base {
        return None;
    }
    let m = boosted as f64 / base as f64;
    let s = format!("{:.1}", m);
    Some(format!("x{}", if crate::i18n::locale().starts_with("fr") { s.replace('.', ",") } else { s }))
}

pub fn duration_short(seconds: u64) -> String {
    if seconds < 60 { crate::t!("quests.seconds_short", n = seconds).to_string() } else { crate::t!("quests.minutes_short", n = seconds.div_ceil(60)).to_string() }
}

pub fn description(q: &Quest, reward: &str) -> SharedString {
    match q.task {
        Task::Video { .. } => crate::t!("quests.desc_video", reward = reward),
        Task::Play { minutes } if q.app_quest => crate::t!("quests.desc_use", game = q.game.clone(), n = minutes, reward = reward),
        Task::Play { minutes } => crate::t!("quests.desc_play", game = q.game.clone(), n = minutes, reward = reward),
        Task::Other => SharedString::default(),
    }
}

pub fn after_resync() {
    if FAILED.with(|f| f.replace(false)) {
        QUESTS.with(|q| *q.borrow_mut() = None);
        REQUESTED.with(|r| r.set(false));
    }
    if CLAIMED_FAILED.with(|f| f.replace(false)) {
        CLAIMED.with(|c| *c.borrow_mut() = None);
        CLAIMED_REQUESTED.with(|r| r.set(false));
    }
}

pub(crate) fn ensure_loaded(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if REQUESTED.with(|r| r.replace(true)) {
        return;
    }
    let fut = app.backend.quests();
    let bal = app.backend.orbs_balance();
    cx.spawn(async move |this, cx| {
        let r = fut.await;
        FAILED.with(|f| f.set(r.is_err()));
        let v = r.unwrap_or(Value::Null);
        QUESTS.with(|q| *q.borrow_mut() = Some(parse(&v, Utc::now())));
        let b = bal.await.ok().flatten();
        BALANCE.with(|c| c.set(b));
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

fn tk(i: usize) -> Hsla {
    c(tok(i))
}

fn text(size: f32, line: f32, weight: FontWeight, color: Hsla, t: impl Into<SharedString>) -> gpui::Div {
    div().text_size(z(size)).line_height(z(line)).font_weight(weight).text_color(color).child(t.into())
}

fn date_dm(e: DateTime<Utc>) -> String {
    e.with_timezone(&chrono::Local).format("%d/%m").to_string()
}

fn orb_icon(size: f32) -> gpui::Svg {
    gpui::svg().path(crate::icons::path("orbs")).size(z(size)).flex_none().text_color(tk(idx::TEXT_STRONG))
}

pub const QUEST_HOME_DESKTOP: u8 = 11;
pub const PRIVACY_ARTICLE: &str = "22225542459415";

pub fn privacy_url() -> String {
    format!("https://support.discord.com/hc/{}/articles/{PRIVACY_ARTICLE}", crate::i18n::locale().to_lowercase())
}

pub fn disclosure(q: &Quest) -> SharedString {
    if matches!(q.task, Task::Video { .. }) {
        crate::t!("quests.disclosure_video", publisher = q.publisher.clone())
    } else {
        crate::t!("quests.disclosure_game", publisher = q.publisher.clone(), game = q.game.clone())
    }
}

pub const PAID_TERMS_ORBS: &str = "https://discord.com/terms/paid-services-terms#14";

#[derive(Clone, Debug, PartialEq)]
pub enum Cta {
    Enroll(u64),
    Open(SharedString),
    Preview(SharedString),
    Video { quest: u64, enroll: bool },
}

pub fn primary_cta(q: &Quest, label: &SharedString) -> Cta {
    match (q.task, q.enrolled) {
        (Task::Video { .. }, enrolled) => Cta::Video { quest: q.id, enroll: !enrolled },
        (_, true) => Cta::Preview(label.clone()),
        (_, false) => Cta::Enroll(q.id),
    }
}

pub fn premium_multiplier() -> Option<f64> {
    QUESTS.with(|q| {
        q.borrow().as_ref()?.iter().find_map(|x| {
            let (base, boosted) = (x.orbs?, x.premium_orbs?);
            (base > 0 && boosted > base).then(|| boosted as f64 / base as f64)
        })
    })
}

pub fn first_video_quest() -> Option<Quest> {
    QUESTS.with(|q| q.borrow().as_ref().and_then(|l| l.iter().find(|x| matches!(x.task, Task::Video { .. })).cloned()))
}

pub fn first_quest() -> Option<Quest> {
    QUESTS.with(|q| q.borrow().as_ref().and_then(|l| l.first().cloned()))
}

pub fn all_quests() -> Option<Vec<Quest>> {
    QUESTS.with(|q| q.borrow().clone())
}

pub fn quest(id: u64) -> Option<Quest> {
    QUESTS.with(|q| q.borrow().as_ref().and_then(|l| l.iter().find(|x| x.id == id).cloned()))
}

pub fn secondary_cta(q: &Quest) -> Option<(SharedString, Cta)> {
    let label = q.cta.clone()?;
    let cta = q.cta_link.clone().map_or_else(|| Cta::Preview(label.clone()), Cta::Open);
    Some((label, cta))
}

pub(crate) fn run(a: &mut PhoenixApp, cta: &Cta, cx: &mut Context<PhoenixApp>) {
    match cta {
        Cta::Enroll(id) => a.blocked_write(crate::actions::guard::GuardedWrite::quest_enroll(*id, QUEST_HOME_DESKTOP), cx),
        Cta::Open(url) => cx.open_url(url),
        Cta::Preview(l) => a.stub(l, cx),
        Cta::Video { quest: id, enroll } => {
            if *enroll {
                a.blocked_write(crate::actions::guard::GuardedWrite::quest_enroll(*id, QUEST_HOME_DESKTOP), cx);
            }
            if let Some(q) = quest(*id) {
                a.open_quest_video(q, cx);
            }
        }
    }
}

fn open_orbs_shop(a: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    crate::ui::shop_page::select_tab(crate::ui::shop_page::ORBS_TAB);
    a.open_home_view(crate::ui::home::data::HomeView::Shop, cx);
}

fn button(id: impl Into<gpui::ElementId>, label: SharedString, primary: bool, cta: Cta, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let kind = if primary { crate::ui::controls::BtnKind::Primary } else { crate::ui::controls::BtnKind::Secondary };
    crate::ui::controls::button(id, label, kind).flex_1().min_w_0().on_click(cx.listener(move |a, _, _, cx| run(a, &cta, cx)))
}

fn util_button(icon: &'static str) -> gpui::Div {
    div()
        .size(z(24.))
        .rounded_full()
        .bg(rgba(0x1e1f227a))
        .flex()
        .items_center()
        .justify_center()
        .child(gpui::svg().path(crate::icons::path(icon)).size(z(16.)).text_color(tk(idx::INTERACTIVE_TEXT_ACTIVE)))
}

fn tile(q: &Quest, premium: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let surface = tk(idx::BACKGROUND_SURFACE_HIGH);
    let banner_h = 95.;
    let mut top = div().relative().w_full().h(z(146.)).flex_none().rounded_t(z(8.)).overflow_hidden().bg(surface);
    if let Some(h) = q.hero.clone() {
        top = top.child(div().absolute().top_0().left_0().right_0().h(z(banner_h)).overflow_hidden().bg(tk(idx::BACKGROUND_BASE_LOW)).child(img(h).size_full().object_fit(gpui::ObjectFit::Cover)));
    }
    top = top
        .child(div().absolute().top_0().left_0().right_0().h(z(banner_h)).bg(gpui::linear_gradient(0., gpui::linear_color_stop(surface, 0.), gpui::linear_color_stop(surface.opacity(0.), 1.))))
        .child(
            div()
                .absolute()
                .inset_0()
                .p(z(12.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(z(16.))
                        .when(matches!(q.task, Task::Video { .. }), |d| {
                            let cta = Cta::Video { quest: q.id, enroll: !q.enrolled };
                            d.child(util_button("play").id(("quest-play", q.id)).cursor_pointer().on_click(cx.listener(move |a, _, _, cx| run(a, &cta, cx))))
                        })
                        .child({
                            let target = crate::ui::extra_menus::Target::QuestMore(Box::new(q.clone()));
                            util_button("more-horizontal").id(("quest-more", q.id)).cursor_pointer().on_click(move |ev, w, _| {
                                crate::ui::extra_menus::open(target.clone(), ev.position(), w);
                            })
                        }),
                )
                .child(div().flex_1())
                .children(q.logotype.clone().map(|l| div().mb(z(4.)).h(z(34.)).w(z(121.)).flex().items_end().child(img(l).max_h(z(34.)).max_w(z(121.)).object_fit(gpui::ObjectFit::Contain))))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_start()
                        .gap(z(4.))
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(
                            div()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(z(3.))
                                .child(div().text_color(tk(idx::TEXT_MUTED)).child(crate::t!("quests.promoted_by")))
                                .child(crate::ui::discovery::verified_check_badge())
                                .child(div().min_w_0().text_color(gpui::white()).child(q.publisher.clone())),
                        )
                        .children(q.expires.map(|e| div().flex_none().text_color(tk(idx::TEXT_MUTED)).child(crate::t!("quests.ends_on", date = date_dm(e))))),
                ),
        );
    let amount = reward_amount(q, premium);
    let reward_label: SharedString = match amount {
        Some(n) => crate::t!("quests.orbs", n = n),
        None => q.reward.clone(),
    };
    let art = if amount.unwrap_or(0) >= 700 { REWARD_ART_LARGE } else { REWARD_ART_SMALL };
    let mut header = div().flex().items_center().flex_wrap().gap(z(8.)).mb(z(2.)).child(
        div()
            .flex()
            .items_center()
            .gap(z(3.))
            .text_size(z(16.))
            .line_height(z(20.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(tk(idx::TEXT_STRONG))
            .child(crate::t!("quests.claim"))
            .when(amount.is_some(), |d| d.child(orb_icon(14.)))
            .child(reward_label.clone()),
    );
    if premium && let Some(m) = multiplier(q) {
        header = header.child(
            div()
                .h(z(19.))
                .pl(z(4.))
                .pr(z(8.))
                .rounded_full()
                .bg(gpui::linear_gradient(90., gpui::linear_color_stop(rgba(0x8c138180), 0.), gpui::linear_color_stop(rgba(0x612caf80), 1.)))
                .flex()
                .items_center()
                .gap(z(4.))
                .child(gpui::svg().path(crate::icons::path("nitro")).size(z(16.)).text_color(gpui::white()))
                .child(text(12., 16., FontWeight::SEMIBOLD, gpui::white(), m)),
        );
    }
    let mid = div()
        .w_full()
        .h(z(104.))
        .flex_none()
        .bg(surface)
        .flex()
        .flex_col()
        .child(
            div()
                .w_full()
                .h(z(104.))
                .flex()
                .child(div().pt(z(12.)).pl(z(12.)).pb(z(12.)).child(img(art).size(z(80.)).rounded(z(9.4)).object_fit(gpui::ObjectFit::Cover)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .p(z(12.))
                        .flex()
                        .flex_col()
                        .justify_center()
                        .overflow_hidden()
                        .child(text(12., 16., FontWeight::BOLD, tk(idx::TEXT_BRAND), crate::t!("quests.quest_name", name = q.name.clone()).to_uppercase()).mb(z(4.)))
                        .child(header)
                        .child(text(12., 16., FontWeight::MEDIUM, tk(idx::TEXT_MUTED), description(q, &reward_label))),
                ),
        );
    let primary: SharedString = match q.task {
        Task::Video { seconds } if !q.enrolled => crate::t!("quests.watch", d = duration_short(seconds)),
        _ if !q.enrolled => crate::t!("quests.accept"),
        _ => crate::t!("quests.view"),
    };
    let cta = div()
        .w_full()
        .min_h(z(62.))
        .p(z(12.))
        .border_t_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .flex()
        .items_center()
        .gap(z(8.))
        .children(secondary_cta(q).map(|(l, cta)| button(("quest-cta2", q.id), l, false, cta, cx)))
        .child({
            let cta = primary_cta(q, &primary);
            button(("quest-cta1", q.id), primary, true, cta, cx)
        });
    div()
        .id(("quest-tile", q.id))
        .w_full()
        .min_w_0()
        .max_h(z(316.))
        .flex()
        .flex_col()
        .rounded(z(10.))
        .overflow_hidden()
        .bg(surface)
        .border_1()
        .border_color(tk(idx::BORDER_MUTED))
        .shadow_sm()
        .child(top)
        .child(mid)
        .child(cta)
        .into_any_element()
}

pub fn columns(width: f32) -> usize {
    (((width + GAP) / (TILE_MIN_W + GAP)).floor() as usize).max(1)
}

fn header_bar(cx: &mut Context<PhoenixApp>) -> AnyElement {
    let balance = BALANCE.with(|b| b.get());
    let claimed_tab = CLAIMED_TAB.with(|c| c.get());
    let tab = |id: &'static str, label: SharedString, _on: bool, claimed: bool| {
        div()
            .id(id)
            .on_click(move |_, w, _| {
                CLAIMED_TAB.with(|c| c.set(claimed));
                POPOUT.with(|p| p.set(None));
                w.refresh();
            })
            .h_full()
            .flex()
            .items_center()
            .cursor_pointer()
            .child(text(16., 20., FontWeight::MEDIUM, tk(idx::TEXT_DEFAULT), label))
    };
    div()
        .flex_none()
        .h(z(49.))
        .pl(z(20.))
        .pr(z(14.))
        .border_b_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .flex()
        .items_center()
        .child(gpui::svg().path(crate::icons::path("quests")).size(z(24.)).flex_none().text_color(tk(idx::ICON_STRONG)))
        .child(div().ml(z(44.)).flex_1().h_full().flex().gap(z(24.)).child(tab("quests-tab-all", crate::t!("quests.all"), !claimed_tab, false)).child(tab("quests-tab-claimed", crate::t!("quests.claimed_tab"), claimed_tab, true)))
        .child(
            div()
                .id("quests-balance")
                .pl(z(10.))
                .cursor_pointer()
                .on_click(cx.listener(|a, _, _, cx| open_orbs_shop(a, cx)))
                .child(
                    div()
                        .h(z(32.))
                        .px(z(12.))
                        .rounded(z(8.))
                        .bg(rgba(0x97979f1f))
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .child(orb_icon(20.))
                        .child(text(16., 20., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), balance.map(|b| b.to_string()).unwrap_or_default())),
                ),
        )
        .into_any_element()
}

fn sponsored_hero(h: Sponsored, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let btn = |id: &'static str, light: bool| {
        div()
            .id(id)
            .h(z(40.))
            .px(z(16.))
            .rounded(z(8.))
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(z(16.))
            .line_height(z(20.))
            .font_weight(FontWeight::MEDIUM)
            .when(light, |d| {
                d.bg(tk(idx::CONTROL_OVERLAY_PRIMARY_BACKGROUND_DEFAULT))
                    .text_color(tk(idx::CONTROL_OVERLAY_PRIMARY_TEXT_DEFAULT))
                    .hover(|s| s.bg(tk(idx::CONTROL_OVERLAY_PRIMARY_BACKGROUND_HOVER)))
            })
            .when(!light, |d| d.text_color(tk(idx::TEXT_STRONG)).hover(|s| s.underline()))
    };
    let mut row = div().flex().items_center().gap(z(8.));
    if let Some((url, label)) = h.cta.clone() {
        row = row.child(btn("quests-hero-cta", true).child(label).on_click(move |_, _, cx| cx.open_url(&url)));
    }
    row = row.child(btn("quests-hero-list", false).child(crate::t!("quests.see_quests")).on_click(cx.listener(|_, _, _, cx| {
        crate::ui::chat::scrollbar::handle("quests-scroll").set_offset(gpui::point(gpui::px(0.), -z(HERO_H + 64.)));
        cx.notify();
    })));
    div()
        .relative()
        .w_full()
        .h(z(HERO_H))
        .rounded(z(16.))
        .overflow_hidden()
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(gpui::black())
        .children(h.image.clone().map(|u| img(u).absolute().inset_0().size_full().rounded(z(16.)).object_fit(gpui::ObjectFit::Cover)))
        .child(div().absolute().inset_0().bg(gpui::linear_gradient(0., gpui::linear_color_stop(gpui::black().opacity(0.7), 0.), gpui::linear_color_stop(gpui::black().opacity(0.), 0.6))))
        .children(h.sponsor.clone().map(|u| img(u).absolute().top(z(32.)).left(z(32.)).h(z(32.)).max_w(z(160.)).object_fit(gpui::ObjectFit::Contain)))
        .child(
            div()
                .absolute()
                .top(z(24.))
                .right(z(24.))
                .px(z(8.))
                .py(z(4.))
                .rounded(z(8.))
                .bg(gpui::black().opacity(0.56))
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(gpui::white())
                .child(crate::t!("quests.sponsored")),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .p(z(32.))
                .flex()
                .flex_col()
                .justify_end()
                .gap(z(16.))
                .child(text(44., 44., FontWeight::BOLD, tk(idx::TEXT_STRONG), h.title.to_uppercase()))
                .when(!h.subtitle.is_empty(), |d| d.child(text(16., 20., FontWeight::NORMAL, tk(idx::TEXT_STRONG), h.subtitle.clone()).opacity(0.8)))
                .child(row),
        )
        .into_any_element()
}

const HERO_H: f32 = 500.;

fn hero(cx: &mut Context<PhoenixApp>) -> AnyElement {
    if let Some(h) = HERO.with(|h| h.borrow().as_ref().and_then(|h| h.sponsored.clone())) {
        return sponsored_hero(h, cx);
    }
    let shop = crate::t!("quests.explore_shop");
    let terms = crate::t!("quests.terms");
    let overlay_btn = |id: &'static str, label: SharedString, light: bool| {
        div()
            .id(id)
            .h(z(40.))
            .px(z(16.))
            .rounded(z(8.))
            .flex()
            .items_center()
            .cursor_pointer()
            .border_1()
            .map(|d| {
                let [bg, bgh, bga, br, brh, bra, fg, fgh, fga] = if light {
                    [
                        idx::CONTROL_OVERLAY_PRIMARY_BACKGROUND_DEFAULT,
                        idx::CONTROL_OVERLAY_PRIMARY_BACKGROUND_HOVER,
                        idx::CONTROL_OVERLAY_PRIMARY_BACKGROUND_ACTIVE,
                        idx::CONTROL_OVERLAY_PRIMARY_BORDER_DEFAULT,
                        idx::CONTROL_OVERLAY_PRIMARY_BORDER_HOVER,
                        idx::CONTROL_OVERLAY_PRIMARY_BORDER_ACTIVE,
                        idx::CONTROL_OVERLAY_PRIMARY_TEXT_DEFAULT,
                        idx::CONTROL_OVERLAY_PRIMARY_TEXT_HOVER,
                        idx::CONTROL_OVERLAY_PRIMARY_TEXT_ACTIVE,
                    ]
                } else {
                    [
                        idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT,
                        idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_HOVER,
                        idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_ACTIVE,
                        idx::CONTROL_OVERLAY_SECONDARY_BORDER_DEFAULT,
                        idx::CONTROL_OVERLAY_SECONDARY_BORDER_HOVER,
                        idx::CONTROL_OVERLAY_SECONDARY_BORDER_ACTIVE,
                        idx::CONTROL_OVERLAY_SECONDARY_TEXT_DEFAULT,
                        idx::CONTROL_OVERLAY_SECONDARY_TEXT_HOVER,
                        idx::CONTROL_OVERLAY_SECONDARY_TEXT_ACTIVE,
                    ]
                };
                d.bg(tk(bg))
                    .border_color(tk(br))
                    .text_color(tk(fg))
                    .hover(move |s| s.bg(tk(bgh)).border_color(tk(brh)).text_color(tk(fgh)))
                    .active(move |s| s.bg(tk(bga)).border_color(tk(bra)).text_color(tk(fga)))
            })
            .text_size(z(16.))
            .line_height(z(20.))
            .font_weight(FontWeight::MEDIUM)
            .child(label)
            .on_click(cx.listener(move |a, _, _, cx| if light { open_orbs_shop(a, cx) } else { cx.open_url(PAID_TERMS_ORBS) }))
    };
    div()
        .relative()
        .w_full()
        .min_h(z(500.))
        .rounded(z(16.))
        .overflow_hidden()
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(gpui::black())
        .child(img(HERO_ART).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover))
        .child(div().absolute().inset_0().opacity(0.25).bg(gpui::linear_gradient(0., gpui::linear_color_stop(tk(idx::BRAND_500), 0.), gpui::linear_color_stop(tk(idx::BRAND_500).opacity(0.), 0.33))))
        .child(
            div()
                .absolute()
                .inset_0()
                .p(z(32.))
                .flex()
                .flex_col()
                .justify_end()
                .child(
                    div()
                        .max_w(z(540.))
                        .flex()
                        .flex_col()
                        .gap(z(16.))
                        .child(text(44., 44., FontWeight::BOLD, tk(idx::TEXT_STRONG), crate::t!("quests.hero_title").to_uppercase()))
                        .child(text(16., 20., FontWeight::NORMAL, tk(idx::TEXT_STRONG), crate::t!("quests.hero_subtitle")).opacity(0.8))
                        .child(div().flex().flex_col().items_start().gap(z(7.)).child(overlay_btn("quests-hero-shop", shop, true)).child(overlay_btn("quests-hero-terms", terms, false))),
                ),
        )
        .into_any_element()
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    ensure_loaded(app, cx);
    ensure_hero(app, cx);
    let quests = QUESTS.with(|q| q.borrow().clone());
    let premium = app.backend.has_premium();
    let page_w = f32::from(window.viewport_size().width) / crate::theme::zoom() - crate::ui::sidebar_resize::width() - 72.;
    let inner = page_w.min(1350.) - 40. - 8.;
    let cols = columns(inner);
    if CLAIMED_TAB.with(|c| c.get()) {
        ensure_claimed(app, cx);
        let list = CLAIMED.with(|c| c.borrow().clone());
        let cols = claimed_columns(page_w.min(1350.) - 40. - 8.);
        let mut content = div().w_full().max_w(z(1350.)).mx_auto().px(z(20.)).py(z(24.)).flex().flex_col();
        match list {
            None => content = content.child(text(16., 20., FontWeight::MEDIUM, tk(idx::TEXT_MUTED), crate::t!("common.loading"))),
            Some(list) => {
                let mut grid = div().grid().grid_cols(cols as u16).gap(z(CLAIMED_GAP));
                for q in &list {
                    grid = grid.child(claimed_tile(q));
                }
                content = content.child(grid);
            }
        }
        return div()
            .size_full()
            .flex()
            .flex_col()
            .bg(tk(idx::BACKGROUND_BASE_LOWER))
            .child(header_bar(cx))
            .child(div().id("quests-claimed-scroll").flex_1().min_h_0().overflow_y_scroll().pr(z(8.)).child(content))
            .into_any_element();
    }
    let sort = SORT.with(|s| s.get());
    let filters = FILTERS.with(|f| f.borrow().clone());
    let small_btn = |id: &'static str, label: SharedString, icon: &'static str, which: Popout, active: bool| {
        div()
            .id(id)
            .h(z(32.))
            .px(z(12.))
            .rounded(z(8.))
            .bg(rgba(0x97979f1f))
            .border_1()
            .border_color(if active { tk(idx::BORDER_STRONG) } else { tk(idx::CONTROL_SECONDARY_BORDER_DEFAULT) })
            .hover(|s| s.bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)))
            .flex()
            .items_center()
            .gap(z(4.))
            .cursor_pointer()
            .text_size(z(14.))
            .line_height(z(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(tk(idx::CONTROL_SECONDARY_TEXT_DEFAULT))
            .on_mouse_down(gpui::MouseButton::Left, move |e: &gpui::MouseDownEvent, w, _| {
                let open = POPOUT.with(|p| p.get()).is_some_and(|(k, _)| k == which);
                POPOUT.with(|p| p.set(if open { None } else { Some((which, e.position)) }));
                w.refresh();
            })
            .relative()
            .child(gpui::canvas(move |b, _, _| BUTTONS.with(|c| {
                let mut v = c.get();
                v[which as usize] = Some(b);
                c.set(v);
            }), |_, _, _, _| {}).absolute().inset_0())
            .child(label)
            .child(gpui::svg().path(crate::icons::path(icon)).size(z(16.)).text_color(tk(idx::CONTROL_SECONDARY_TEXT_DEFAULT)))
    };
    let heading = div()
        .h(z(32.))
        .flex()
        .items_center()
        .justify_between()
        .gap(z(16.))
        .child(text(20., 24., FontWeight::MEDIUM, tk(idx::TEXT_STRONG), crate::t!("quests.available_title")))
        .child(
            div()
                .flex()
                .gap(z(8.))
                .child(small_btn("quests-sort", sort_label(sort), "chevron", Popout::Sort, false))
                .child(small_btn("quests-filter", crate::t!("quests.filter"), "inbox/filter", Popout::Filter, false)),
        );
    let mut content = div().w_full().max_w(z(1350.)).mx_auto().px(z(20.)).py(z(24.)).flex().flex_col().child(hero(cx)).child(div().h(z(64.))).child(heading);
    match quests {
        None => content = content.child(text(16., 20., FontWeight::MEDIUM, tk(idx::TEXT_MUTED), crate::t!("common.loading")).mt(z(24.))),
        Some(list) => {
            let list = sort_quests(apply_filters(list, &filters), sort, Utc::now());
            if list.is_empty() {
                content = content.child(div().py(z(80.)).flex().justify_center().child(text(24., 32., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), crate::t!("quests.none_title"))));
            } else {
                let mut grid = div().mt(z(GAP)).grid().grid_cols(cols as u16).gap(z(GAP));
                for q in &list {
                    grid = grid.child(tile(q, premium, cx));
                }
                content = content.child(grid);
            }
        }
    }
    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .child(header_bar(cx))
        .child(div().id("quests-scroll").flex_1().min_h_0().overflow_y_scroll().track_scroll(&crate::ui::chat::scrollbar::handle("quests-scroll")).pr(z(8.)).child(content))
        .children(popout(sort, &filters))
        .into_any_element()
}

pub fn debug_open(claimed: bool, popout: &str) {
    CLAIMED_TAB.with(|c| c.set(claimed));
    let which = match popout {
        "sort" => Some(Popout::Sort),
        "filter" => Some(Popout::Filter),
        _ => None,
    };
    POPOUT.with(|p| p.set(which.map(|w| (w, gpui::point(gpui::px(0.), gpui::px(0.))))));
}

pub const CLAIMED_TILE: f32 = 240.;
pub const CLAIMED_GAP: f32 = 24.;

pub fn claimed_columns(width: f32) -> usize {
    (((width + CLAIMED_GAP) / (CLAIMED_TILE + CLAIMED_GAP)).floor() as usize).max(1)
}

fn ensure_claimed(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if CLAIMED_REQUESTED.with(|r| r.replace(true)) {
        return;
    }
    let fut = app.backend.directory_get("/quests/@me/claimed".into());
    cx.spawn(async move |this, cx| {
        let r = fut.await;
        CLAIMED_FAILED.with(|f| f.set(r.is_err()));
        let v = r.unwrap_or(Value::Null);
        CLAIMED.with(|c| *c.borrow_mut() = Some(parse_claimed(&v)));
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

fn claimed_tile(q: &Claimed) -> AnyElement {
    let art = q.art.clone().or_else(|| q.orbs.then(|| SharedString::from(REWARD_ART_SMALL)));
    div()
        .id(("quest-claimed", q.id))
        .relative()
        .w_full()
        .aspect_ratio(1.)
        .rounded(z(16.))
        .overflow_hidden()
        .bg(gpui::linear_gradient(0., gpui::linear_color_stop(gpui::rgb(0x15193c), 0.), gpui::linear_color_stop(gpui::black(), 0.33)))
        .children(art.map(|a| img(a).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover)))
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h_1_2()
                .bg(gpui::linear_gradient(0., gpui::linear_color_stop(gpui::rgb(0x242429), 0.), gpui::linear_color_stop(gpui::Hsla::from(gpui::rgb(0x242429)).opacity(0.), 1.))),
        )
        .children(q.logotype.clone().map(|l| div().absolute().left_0().right_0().bottom(z(20.)).flex().justify_center().child(img(l).w(z(140.)).max_h(z(36.)).object_fit(gpui::ObjectFit::Contain))))
        .into_any_element()
}

fn popout(sort: Sort, filters: &[Filter]) -> Option<AnyElement> {
    let (which, pos) = POPOUT.with(|p| p.get())?;
    let heading = |t: SharedString| text(14., 18., FontWeight::SEMIBOLD, tk(idx::TEXT_SUBTLE), t).mb(z(16.));
    let mut card = div()
        .id("quests-popout")
        .occlude()
        .w(z(242.))
        .p(z(16.))
        .rounded(z(8.))
        .bg(tk(idx::BACKGROUND_BASE_LOW))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .shadow_lg()
        .flex()
        .flex_col()
        .on_mouse_down_out(|_, w, _| {
            POPOUT.with(|p| p.set(None));
            w.refresh();
        });
    match which {
        Popout::Sort => {
            let mut group = div().flex().flex_col().gap(z(16.)).py(z(8.));
            for (n, s) in SORTS.iter().copied().enumerate() {
                group = group.child(crate::ui::controls::radio_option(("quests-sort-opt", n), sort_label(s), s == sort).on_click(move |_, w, _| {
                    SORT.with(|c| c.set(s));
                    POPOUT.with(|p| p.set(None));
                    w.refresh();
                }));
            }
            card = card.child(heading(crate::t!("quests.sort_by"))).child(group);
        }
        Popout::Filter => {
            let section = |title: SharedString, list: &[Filter], base: usize| {
                let mut g = div().flex().flex_col().gap(z(16.)).py(z(8.));
                for (n, f) in list.iter().copied().enumerate() {
                    let on = filters.contains(&f);
                    g = g.child(crate::ui::controls::checkbox_option(("quests-filter-opt", base + n), filter_label(f), on).on_click(move |_, w, _| {
                        FILTERS.with(|c| {
                            let mut c = c.borrow_mut();
                            if let Some(i) = c.iter().position(|x| *x == f) {
                                c.remove(i);
                            } else {
                                c.push(f);
                            }
                        });
                        w.refresh();
                    }));
                }
                div().flex().flex_col().child(heading(title)).child(g)
            };
            let divider = || div().mx(z(-8.)).my(z(8.)).h(z(1.)).bg(tk(idx::BORDER_SUBTLE));
            card = card
                .child(section(crate::t!("quests.filter_reward"), &REWARD_FILTERS, 0))
                .child(divider())
                .child(section(crate::t!("quests.filter_type"), &TASK_FILTERS, 10))
                .child(divider())
                .child(crate::ui::controls::button("quests-filter-clear", crate::t!("quests.filter_clear"), crate::ui::controls::BtnKind::Secondary).mx(z(-8.)).h(z(32.)).on_click(|_, w, _| {
                    FILTERS.with(|c| c.borrow_mut().clear());
                    w.refresh();
                }));
        }
    }
    let at = BUTTONS.with(|c| c.get()[which as usize]).map_or(gpui::point(pos.x, pos.y - z(16.)), |b| gpui::point(b.right(), b.top() - z(8.)));
    Some(crate::media::lru::deferred(gpui::anchored().position(at).anchor(gpui::Anchor::BottomRight).snap_to_window_with_margin(z(8.)).child(card)).with_priority(20).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_quests_are_asked_again_after_resync() {
        REQUESTED.with(|r| r.set(true));
        FAILED.with(|f| f.set(true));
        QUESTS.with(|q| *q.borrow_mut() = Some(Vec::new()));
        after_resync();
        assert!(!REQUESTED.with(|r| r.get()));
        assert!(QUESTS.with(|q| q.borrow().is_none()));
        REQUESTED.with(|r| r.set(true));
        after_resync();
        assert!(REQUESTED.with(|r| r.get()));
    }

    fn sample() -> Value {
        serde_json::json!({"quests": [
            {"id": "2", "config": {"expires_at": "2030-01-01T00:00:00+00:00", "features": [26, 15], "messages": {"quest_name": "A", "game_title": "G", "game_publisher": "P"},
              "assets": {"hero": "quests/2/h.jpg", "logotype_dark": "PLACEHOLDER", "logotype": "quests/2/l.png"},
              "rewards_config": {"rewards": [{"type": 4, "messages": {"name": "700 Orbs"}, "orb_quantity": 700, "premium_orb_quantity": 840}]},
              "task_config_v2": {"tasks": {"PLAY_ON_DESKTOP": {"target": 900}}}, "cta_config": {"button_label": "Télécharger", "link": "https://example.com/get"}},
             "user_status": {"enrolled_at": "x", "completed_at": null, "claimed_at": null, "progress": {"PLAY_ON_DESKTOP": {"value": 450}}}},
            {"id": "3", "config": {"expires_at": "2030-01-01T00:00:00+00:00", "messages": {"quest_name": "V"},
              "rewards_config": {"rewards": [{"type": 4, "orb_quantity": 200, "premium_orb_quantity": 240}]},
              "task_config_v2": {"tasks": {"WATCH_VIDEO": {"target": 35, "assets": {"video": {"thumbnail": "quests/3/t.jpg", "width": 1080, "height": 1920}}}}}}, "user_status": null},
            {"id": "4", "config": {"expires_at": "2030-01-01T00:00:00+00:00", "messages": {"quest_name": "C"}}, "user_status": {"claimed_at": "x", "enrolled_at": "x", "completed_at": "x"}},
            {"id": "1", "config": {"expires_at": "2020-01-01T00:00:00+00:00"}, "user_status": null}
        ]})
    }

    #[test]
    fn parses_quests_like_the_client_tiles() {
        let q = parse(&sample(), Utc::now());
        assert_eq!(q.iter().map(|x| x.id).collect::<Vec<_>>(), [2, 3]);
        assert_eq!(q[0].hero.as_deref(), Some("https://cdn.discordapp.com/quests/2/h.jpg"));
        assert_eq!(q[0].logotype.as_deref(), Some("https://cdn.discordapp.com/quests/2/l.png"));
        assert_eq!((q[0].orbs, q[0].premium_orbs), (Some(700), Some(840)));
        assert_eq!(q[0].task, Task::Play { minutes: 15 });
        assert!(q[0].app_quest && q[0].enrolled && !q[0].completed);
        assert_eq!(q[0].cta.as_deref(), Some("Télécharger"));
        assert_eq!(q[0].cta_link.as_deref(), Some("https://example.com/get"));
        assert_eq!(secondary_cta(&q[0]), Some(("Télécharger".into(), Cta::Open("https://example.com/get".into()))));
        assert_eq!(primary_cta(&q[0], &"x".into()), Cta::Preview("x".into()));
        assert_eq!(primary_cta(&q[1], &"x".into()), Cta::Video { quest: 3, enroll: true });
        assert_eq!(q[1].video_thumb.as_deref(), Some("https://cdn.discordapp.com/quests/3/t.jpg"));
        assert!(q[1].video_portrait);
        assert!(disclosure(&q[1]).len() > 10);
        assert!(secondary_cta(&q[1]).is_none());
        assert!((q[0].progress - 0.5).abs() < 1e-6);
        assert_eq!(q[1].task, Task::Video { seconds: 35 });
        assert!(!q[1].app_quest && q[1].cta.is_none());
        assert_eq!(reward_amount(&q[1], true), Some(240));
        assert_eq!(reward_amount(&q[1], false), Some(200));
        assert!(multiplier(&q[1]).is_some_and(|m| m.starts_with("x1") && m.ends_with('2')));
        assert_eq!(columns(1071.), 3);
        assert_eq!(columns(300.), 1);
    }

    fn q(id: u64, f: impl FnOnce(&mut Quest)) -> Quest {
        let mut x = Quest {
            id,
            name: "".into(),
            game: "".into(),
            publisher: "".into(),
            hero: None,
            logotype: None,
            expires: None,
            reward: "".into(),
            orbs: None,
            premium_orbs: None,
            task: Task::Other,
            app_quest: false,
            cta: None,
            cta_link: None,
            enrolled: false,
            completed: false,
            claimed: false,
            progress: 0.,
            starts: None,
            enrolled_at: None,
            reward_types: Vec::new(),
            tasks: Vec::new(),
            video_thumb: None,
            video_portrait: false,
            dismissed_content: 0,
            rewards_expire: None,
        };
        f(&mut x);
        x
    }

    #[test]
    fn suggested_order_matches_the_client_comparator() {
        let now = Utc::now();
        let d = |h: i64| Some(now + chrono::Duration::hours(h));
        let list = vec![
            q(1, |x| x.expires = d(10)),
            q(2, |x| x.expires = d(5)),
            q(3, |x| {
                x.expires = d(50);
                x.enrolled = true;
                x.enrolled_at = Some(now - chrono::Duration::minutes(5));
            }),
            q(4, |x| {
                x.expires = d(1);
                x.enrolled = true;
                x.enrolled_at = Some(now - chrono::Duration::hours(2));
            }),
            q(PINNED_QUEST, |x| x.expires = d(99)),
        ];
        let ids = |l: Vec<Quest>| l.iter().map(|x| x.id).collect::<Vec<_>>();
        assert_eq!(ids(sort_quests(list.clone(), Sort::Suggested, now)), [PINNED_QUEST, 3, 2, 1, 4]);
        assert_eq!(ids(sort_quests(list.clone(), Sort::ExpiringSoon, now)), [4, 2, 1, 3, PINNED_QUEST]);
        assert_eq!(ids(sort_quests(list, Sort::RecentlyEnrolled, now))[..2], [3, 4]);
    }

    #[test]
    fn filters_group_by_reward_and_task() {
        let list = vec![
            q(1, |x| {
                x.reward_types = vec![4];
                x.tasks = vec!["WATCH_VIDEO".into()];
            }),
            q(2, |x| {
                x.reward_types = vec![3];
                x.tasks = vec!["PLAY_ON_DESKTOP".into()];
            }),
            q(3, |x| {
                x.reward_types = vec![2];
                x.tasks = vec!["PLAY_ACTIVITY".into()];
            }),
        ];
        let ids = |l: Vec<Quest>| l.iter().map(|x| x.id).collect::<Vec<_>>();
        assert_eq!(ids(apply_filters(list.clone(), &[Filter::Orbs, Filter::InGame])), [1, 3]);
        assert_eq!(ids(apply_filters(list.clone(), &[Filter::Play])), [2, 3]);
        assert_eq!(ids(apply_filters(list.clone(), &[Filter::Collectible, Filter::Video])), Vec::<u64>::new());
        assert_eq!(ids(apply_filters(list, &[])), [1, 2, 3]);
    }

    #[test]
    fn claimed_quests_newest_first() {
        let v = serde_json::json!({"quests": [
            {"id": "1", "config": {"messages": {"quest_name": "Badges"}, "assets": {"logotype": "PLACEHOLDER", "logotype_dark": "quests/1/l.png"}, "rewards": [{"type": 4, "asset": null}]}, "user_status": {"claimed_at": "2026-11-17T21:12:50+00:00"}},
            {"id": "2", "config": {"messages": {"quest_name": "Dune"}, "assets": {}, "rewards": [{"type": 3, "asset": "quests/2/a.png"}]}, "user_status": {"claimed_at": "2026-11-20T10:00:00+00:00"}}
        ]});
        let c = parse_claimed(&v);
        assert_eq!(c.iter().map(|x| x.id).collect::<Vec<_>>(), [2, 1]);
        assert!(c[1].orbs && c[1].art.is_none());
        assert_eq!(c[1].logotype.as_deref(), Some("https://cdn.discordapp.com/quests/1/l.png"));
        assert_eq!(c[0].art.as_deref(), Some("https://cdn.discordapp.com/quests/2/a.png"));
        assert_eq!(claimed_columns(1071.), 4);
    }

    #[test]
    fn home_hero_entrypoint_follows_the_client() {
        let v = serde_json::json!({"decisions": [{"creative": {"creative_type": 2, "creative_content": {"id": "77", "quest_home_entrypoint": {"image": "quests/h/e.png"}}}}]});
        let h = parse_home_hero(&v).unwrap();
        assert_eq!(h, HomeHero { id: "77".into(), image: Some("https://cdn.discordapp.com/quests/h/e.png".into()), has_content: true, tooltip: None, sponsored: None });
        let tooltip_only = serde_json::json!({"decisions": [{"creative": {"creative_type": 2, "creative_content": {"id": 5, "quest_home_entrypoint": {"tooltip_title": "a"}}}}]});
        assert!(!parse_home_hero(&tooltip_only).unwrap().has_content);
        let gradient = serde_json::json!({"decisions": [{"creative": {"creative_type": 2, "creative_content": {"id": 5, "quest_home_entrypoint": {"linear_gradient": {"start": "#000", "end": "#fff"}}}}}]});
        assert!(parse_home_hero(&gradient).unwrap().has_content);
        assert_eq!(parse_home_hero(&serde_json::json!({"decisions": [{"creative": {"creative_type": 1}}]})), None);
        let dune = serde_json::json!({"decisions": [{"creative": {"creative_type": 2, "creative_content": {"id": "9", "label_title": "Dune: Part Three", "label_subtitle": "Only in Theaters", "hero_image": "quests/9/h.jpg", "sponsor_image": "quests/9/s.png", "cta": {"url": "https://x.test/t", "button_label": "Obtenir des tickets"}}}}]});
        let s = parse_home_hero(&dune).unwrap().sponsored.unwrap();
        assert_eq!((s.title.as_ref(), s.subtitle.as_ref()), ("Dune: Part Three", "Only in Theaters"));
        assert_eq!(s.image.as_deref(), Some("https://cdn.discordapp.com/quests/9/h.jpg"));
        assert_eq!(s.cta, Some(("https://x.test/t".into(), "Obtenir des tickets".into())));
        assert_eq!(parse_home_hero(&Value::Null), None);
        assert_eq!(home_hero_path(), "/quests/get-decisions?placement=3&num_decisions_requested=1");
    }

    #[test]
    fn seen_store_round_trips_like_the_client() {
        let mut s = SeenStore::parse(&serde_json::json!({"seenContentIds": {"2": ["a"], "1": ["q"]}}));
        assert!(s.has_seen(2, "a") && !s.has_seen(2, "b"));
        assert!(s.mark_seen(2, "b") && !s.mark_seen(2, "b"));
        assert!(s.keep_only(2, Some("b")));
        assert!(!s.has_seen(2, "a") && s.has_seen(2, "b") && s.has_seen(1, "q"));
        assert_eq!(SeenStore::parse(&s.to_json()), s);
        assert!(s.keep_only(2, None) && !s.has_seen(2, "b"));
    }

    #[test]
    fn quests_row_badges() {
        let hero = HomeHero { id: "1".into(), image: None, has_content: true, tooltip: None, sponsored: None };
        assert!(entry_point(true, false, None, false).onboarding);
        assert!(!entry_point(true, true, None, false).onboarding);
        assert!(!entry_point(false, false, None, false).onboarding);
        let e = entry_point(true, false, Some(hero.clone()), false);
        assert!(!e.onboarding && e.hero_badge && e.hero.is_some());
        assert!(!entry_point(true, false, Some(hero), true).hero_badge);
        let empty = HomeHero { has_content: false, ..Default::default() };
        assert_eq!(entry_point(false, true, Some(empty), false), EntryPoint::default());
        let mut bits = vec![0u8; 80];
        assert!(!dismissed(&bits, QUEST_HOME_ENTRYPOINT_ONBOARDING));
        bits[79] = 1;
        assert!(dismissed(&bits, QUEST_HOME_ENTRYPOINT_ONBOARDING));
        assert!(!dismissed(&[], QUEST_HOME_ENTRYPOINT_ONBOARDING));
    }
}
