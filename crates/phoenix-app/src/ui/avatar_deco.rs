use gpui::{img, px, InteractiveElement, SharedString, Styled};

use crate::model::User;

pub const RATIO: f32 = 1.2;

pub fn geometry(size: f32) -> (f32, f32) {
    let side = size * RATIO;
    ((size - side) / 2., side)
}

pub fn layer(src: SharedString, size: f32) -> gpui::Stateful<gpui::Img> {
    let (off, side) = geometry(size);
    img(src.clone()).id(crate::media::anim_id(&src)).absolute().left(px(off)).top(px(off)).size(px(side))
}

pub fn source(user: Option<&User>, key: Option<u64>) -> Option<SharedString> {
    let deco = user?.decoration.clone()?;
    Some(key.map(|k| src_for(&deco, k)).unwrap_or(deco))
}

#[cfg(test)]
mod tests {
    #[test]
    fn border_position_is_minus_ten_percent() {
        let (off, side) = super::geometry(32.);
        assert!((off + 3.2).abs() < 1e-4 && (side - 38.4).abs() < 1e-4);
    }

    #[test]
    fn decoration_hole_is_the_avatar_cut() {
        for (size, c, r) in [(32., 0.786_458_3, 0.208_333_3), (80., 0.791_666_7, 0.145_833_3), (96., 0.777_777_8, 0.156_25), (120., 0.777_777_8, 0.138_888_9)] {
            let (off, side) = super::geometry(size);
            let (cx, cr, _) = crate::ui::status_svg::geometry(size);
            assert!((off + c * side - cx).abs() < 0.05, "{size}: centre {} vs {cx}", off + c * side);
            assert!((r * side - cr).abs() < 0.05, "{size}: radius {} vs {cr}", r * side);
        }
    }
}

thread_local! {
    static HOVER: std::cell::Cell<Option<(u64, std::time::Instant)>> = const { std::cell::Cell::new(None) };
    static PAINTED: std::cell::Cell<Option<(u64, u64)>> = const { std::cell::Cell::new(None) };
}

pub fn clear() {
    HOVER.with(|h| h.set(None));
}

pub fn sweep(frame: u64) {
    let Some((key, _)) = HOVER.with(|h| h.get()) else { return };
    let seen = PAINTED.with(|p| p.get()).filter(|(k, _)| *k == key).map(|(_, f)| f);
    if seen.is_none_or(|f| f + 1 < frame) {
        clear();
    }
}

pub fn guard(key: u64) -> gpui::AnyElement {
    use gpui::{IntoElement, Styled};
    gpui::canvas(
        |bounds, window, _| window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal),
        move |_, hitbox, window, _| {
            if hovered(key) {
                PAINTED.with(|p| p.set(Some((key, crate::ui::states::frame()))));
                if !hitbox.is_hovered(window) {
                    clear();
                    window.refresh();
                }
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

pub const REDUCED_MOTION_HOVER_DELAY: std::time::Duration = std::time::Duration::from_secs(1);

pub fn may_animate(focused: bool, reduced: bool, hovered: Option<std::time::Duration>, only_hover: bool) -> bool {
    let wanted = focused && (hovered.is_some() || (!reduced && !only_hover));
    wanted && (!reduced || hovered.is_some_and(|held| held >= REDUCED_MOTION_HOVER_DELAY))
}

pub fn ambient() -> bool {
    may_animate(crate::media::emoji::window_focused(), crate::ui::settings::effects::reduced_motion(), None, false)
}

pub fn plays(key: u64) -> bool {
    let held = HOVER.with(|h| h.get()).filter(|(k, _)| *k == key).map(|(_, at)| at.elapsed());
    may_animate(crate::media::emoji::window_focused(), crate::ui::settings::effects::reduced_motion(), held, true)
}

pub fn hover_key(kind: u8, id: u64) -> u64 {
    ((kind as u64) << 56) ^ id
}

pub fn hovered(key: u64) -> bool {
    HOVER.with(|h| h.get()).is_some_and(|(k, _)| k == key)
}

pub fn set_hover(key: u64, hovered: bool, cx: &mut gpui::App) -> bool {
    let changed = store_hover(key, hovered);
    if changed && hovered {
        PAINTED.with(|p| p.set(Some((key, crate::ui::states::frame()))));
    }
    if changed && hovered && crate::ui::settings::effects::reduced_motion() {
        crate::ui::chat::burst_tip::refresh_after(cx, REDUCED_MOTION_HOVER_DELAY);
    }
    changed
}

fn store_hover(key: u64, hovered: bool) -> bool {
    HOVER.with(|h| {
        let cur = h.get();
        let next = if hovered {
            Some(cur.filter(|(k, _)| *k == key).unwrap_or_else(|| (key, std::time::Instant::now())))
        } else if cur.is_some_and(|(k, _)| k == key) {
            None
        } else {
            cur
        };
        h.set(next);
        cur.map(|(k, _)| k) != next.map(|(k, _)| k)
    })
}

pub fn url(d: &serde_json::Value) -> Option<SharedString> {
    let asset = d.get("asset")?.as_str().filter(|a| !a.is_empty())?;
    Some(match d.get("sku_id").and_then(|s| s.as_str()).filter(|s| !s.is_empty()) {
        Some(sku) => format!("https://cdn.discordapp.com/media/v1/collectibles-shop/{sku}/static?asset={asset}").into(),
        None => format!("https://cdn.discordapp.com/avatar-decoration-presets/{asset}.png?size=96&passthrough=false").into(),
    })
}

pub fn animated(url: &str) -> Option<SharedString> {
    let asset = match url.split_once("?asset=") {
        Some((_, a)) => a,
        None => url.split_once(".png?")?.0.rsplit('/').next()?,
    };
    asset.starts_with("a_").then(|| format!("https://cdn.discordapp.com/avatar-decoration-presets/{asset}.webp?size=96&animated=true").into())
}

pub fn src(deco: &SharedString, play: bool) -> SharedString {
    play.then(|| animated(deco)).flatten().unwrap_or_else(|| deco.clone())
}

pub fn src_for(deco: &SharedString, key: u64) -> SharedString {
    src(deco, plays(key))
}

#[cfg(test)]
mod anim_tests {
    use std::time::Duration;

    #[test]
    fn decorations_follow_the_client_animation_gate() {
        let held = Some(Duration::from_millis(1200));
        let quick = Some(Duration::from_millis(300));
        assert!(super::may_animate(true, false, None, false));
        assert!(!super::may_animate(false, false, None, false));
        assert!(!super::may_animate(false, false, held, true));
        assert!(!super::may_animate(true, false, None, true));
        assert!(super::may_animate(true, false, quick, true));
        assert!(!super::may_animate(true, true, None, false));
        assert!(!super::may_animate(true, true, quick, true));
        assert!(super::may_animate(true, true, held, true));
    }

    #[test]
    fn animated_url_only_for_animated_assets() {
        let s = "https://cdn.discordapp.com/avatar-decoration-presets/a_12ab.png?size=96&passthrough=false";
        assert_eq!(super::animated(s).as_deref(), Some("https://cdn.discordapp.com/avatar-decoration-presets/a_12ab.webp?size=96&animated=true"));
        assert!(super::animated("https://cdn.discordapp.com/avatar-decoration-presets/12ab.png?size=96&passthrough=false").is_none());
        let u = super::url(&serde_json::json!({"asset": "a_12ab", "sku_id": "77"})).unwrap();
        assert_eq!(u.as_ref(), "https://cdn.discordapp.com/media/v1/collectibles-shop/77/static?asset=a_12ab");
        assert_eq!(super::animated(&u).as_deref(), Some("https://cdn.discordapp.com/avatar-decoration-presets/a_12ab.webp?size=96&animated=true"));
        assert!(super::store_hover(super::hover_key(1, 5), true));
        assert!(!super::store_hover(super::hover_key(1, 5), true));
        assert!(super::store_hover(super::hover_key(1, 5), false));
    }
}
