use gpui::{AnyElement, Context, ParentElement, SharedString, Styled, div, img, prelude::*, px};

use super::{Pickers, close, empty_state, grid, idx, section_header, tk, types::StickerData, with_app};
use crate::{app::PhoenixApp, media::sticker_url, model::ImageRef};

const CELL: f32 = 98.;
const COLS: usize = 4;
const COL_GAP: f32 = 13.;
const ROW_GAP: f32 = 12.;

pub fn load_packs(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let s = &mut cx.global_mut::<Pickers>().stickers;
    if s.packs.is_some() || s.loading {
        return;
    }
    s.loading = true;
    let fut = app.backend.sticker_packs();
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |_, cx| {
            let s = &mut cx.global_mut::<Pickers>().stickers;
            s.loading = false;
            s.packs = Some(res.unwrap_or_default());
            cx.notify();
        });
    })
    .detach();
}

pub fn matches(s: &StickerData, q: &str) -> bool {
    q.is_empty()
        || s.name.to_lowercase().contains(q)
        || s.tags.to_lowercase().split([',', ' ']).any(|t| !t.is_empty() && t.starts_with(q))
}

fn send(app: &mut PhoenixApp, id: crate::model::Id, window: &mut gpui::Window, cx: &mut Context<PhoenixApp>) {
    let Some(ch) = app.selected_channel else { return };
    super::detach(app.backend.send_sticker(ch, id), cx);
    close(app, window, cx);
}

pub(crate) fn sticker_graphic(s: &StickerData, size: f32) -> AnyElement {
    match sticker_url(s.id.0, s.format) {
        Some(u) => img(u).size(px(size)).flex_none().object_fit(gpui::ObjectFit::Contain).into_any_element(),
        None => {
            let name = s.name.clone();
            crate::media::lottie::image(crate::media::lottie::url(s.id.0), (size * 2.) as u32, true)
                .size(px(size))
                .flex_none()
                .with_fallback(move || lottie_name(name.clone(), size))
                .into_any_element()
        }
    }
}

fn lottie_name(name: SharedString, size: f32) -> AnyElement {
    div()
        .size(px(size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .text_size(px(12.))
        .text_color(tk(idx::TEXT_MUTED))
        .child(name)
        .into_any_element()
}

fn cell(s: &StickerData, section: SharedString, inspected: bool, locked: bool) -> AnyElement {
    let id = s.id;
    let data = s.clone();
    div()
        .id(("sticker", id.0 as usize))
        .relative()
        .size(px(CELL))
        .flex_none()
        .p(px(1.))
        .rounded(px(4.))
        .cursor_pointer()
        .when(inspected, |d| d.child(div().absolute().inset_0().rounded(px(12.)).bg(tk(idx::INTERACTIVE_BACKGROUND_SELECTED))))
        .on_hover(move |h, w, cx| {
            if *h {
                let (data, section) = (data.clone(), section.clone());
                with_app(cx, w, move |_, _, cx| {
                    let st = &mut cx.global_mut::<Pickers>().stickers;
                    if st.inspected.as_ref().map(|(d, _)| d.id) != Some(data.id) {
                        st.inspected = Some((data, section));
                        cx.notify();
                    }
                });
            }
        })
        .when(!locked, |d| d.on_click(move |_, w, cx| with_app(cx, w, move |a, w, cx| send(a, id, w, cx))))
        .child(
            div()
                .relative()
                .size_full()
                .when(locked, |d| d.opacity(if crate::theme::is_light() { 0.4 } else { 0.3 }))
                .child(sticker_graphic(s, CELL - 2.)),
        )
        .into_any_element()
}

type Section = (SharedString, Option<(ImageRef, SharedString)>, Vec<StickerData>, bool);

pub fn render_body(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let p = cx.global::<Pickers>();
    let q = p.open.as_ref().map(|o| o.input.read(cx).value().trim().to_lowercase()).unwrap_or_default();
    let premium = app.backend.has_premium();
    let mut sections: Vec<Section> = Vec::new();
    let mut guilds: Vec<_> = app.guilds.iter().collect();
    if let Some(sel) = app.selected_guild {
        guilds.sort_by_key(|g| g.id != sel);
    }
    for g in guilds {
        let st: Vec<_> = app.backend.guild_stickers(g.id).into_iter().filter(|s| matches(s, &q)).collect();
        if !st.is_empty() {
            let locked = !premium && Some(g.id) != app.selected_guild;
            sections.push((g.name.clone(), Some((g.icon.clone(), g.acronym.clone())), st, locked));
        }
    }
    for pack in p.stickers.packs.iter().flatten() {
        let st: Vec<_> = pack.stickers.iter().filter(|s| matches(s, &q)).cloned().collect();
        if !st.is_empty() {
            sections.push((pack.name.clone(), None, st, false));
        }
    }
    let mut has_frequent = false;
    if q.is_empty()
        && let Some(f) = p.frecency.as_ref()
    {
        let freq: Vec<StickerData> = f
            .stickers
            .iter()
            .filter_map(|id| sections.iter().flat_map(|s| s.2.iter()).find(|d| d.id == *id).cloned())
            .take(20)
            .collect();
        if !freq.is_empty() {
            sections.insert(0, (crate::t!("picker.frequent"), None, freq, false));
            has_frequent = true;
        }
    }
    if sections.is_empty() {
        return empty_state(if p.stickers.loading { crate::t!("picker.loading_stickers") } else { crate::t!("picker.no_stickers") });
    }
    let origin = |id: crate::model::Id| sections.iter().skip(usize::from(has_frequent)).find(|s| s.2.iter().any(|d| d.id == id)).map(|s| s.0.clone());
    let inspected = p
        .stickers
        .inspected
        .clone()
        .or_else(|| sections.first().and_then(|s| s.2.first().map(|d| (d.clone(), s.0.clone()))))
        .map(|(d, from)| {
            let from = origin(d.id).unwrap_or(from);
            (d, from)
        });
    let insp_id = inspected.as_ref().map(|(d, _)| d.id);

    let scroll = crate::ui::chat::scrollbar::handle("sticker-scroll");
    let mut body = div().id("sticker-scroll").track_scroll(&scroll).size_full().overflow_y_scroll().pl(px(8.));
    let mut rail = grid::rail();
    for (si, (title, gicon, st, locked)) in sections.iter().enumerate() {
        let is_frequent = has_frequent && si == 0;
        let clock = |size: f32| gpui::svg().path(crate::icons::path("picker/clock")).size(px(size)).flex_none();
        let lead = if is_frequent {
            Some(clock(16.).text_color(tk(idx::TEXT_DEFAULT)).into_any_element())
        } else {
            gicon.as_ref().and_then(|(i, _)| match i {
                ImageRef::Url(u) => Some(img(u.clone()).size(px(16.)).rounded(px(4.)).flex_none().into_any_element()),
                _ => None,
            })
        };
        body = body.child(section_header(lead, title.clone()).ml(px(-8.)));
        let mut sec = div().flex().flex_col().pb(px(12.));
        for chunk in st.chunks(COLS) {
            let row = div()
                .flex()
                .gap(px(COL_GAP))
                .mb(px(ROW_GAP))
                .children(chunk.iter().map(|s| cell(s, title.clone(), Some(s.id) == insp_id, *locked)));
            sec = sec.child(row);
        }
        body = body.child(sec);
        let content: AnyElement = match gicon {
            _ if is_frequent => clock(24.)
                .text_color(tk(if si == 0 { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::INTERACTIVE_TEXT_DEFAULT }))
                .into_any_element(),
            Some((i, a)) => grid::rail_guild_icon(i, a),
            None => crate::ui::icon(crate::assets::Icon::Sticker, 24., tk(idx::INTERACTIVE_TEXT_DEFAULT)).into_any_element(),
        };
        let sh = scroll.clone();
        rail = rail.child(
            grid::rail_item(("sticker-section", si), gicon.is_some(), si == 0, 8.)
                .on_click(move |_, w, cx| {
                    sh.scroll_to_item(si * 2);
                    with_app(cx, w, |_, _, cx| cx.notify());
                })
                .child(content),
        );
    }
    let shortcut = crate::ui::icon(crate::assets::Icon::Sticker, 24., tk(idx::INTERACTIVE_TEXT_DEFAULT));
    let footer = match inspected {
        Some((d, from)) => {
            let trailing = match app.guilds.iter().find(|g| Some(g.id) == d.guild) {
                Some(g) => grid::rail_guild_icon(&g.icon, &g.acronym),
                None => sticker_graphic(&d, 32.),
            };
            grid::inspector(Some(sticker_graphic(&d, 28.)), Some(d.name.clone()), Some(from), Some(div().ml(px(8.)).flex_none().child(trailing).into_any_element()))
        }
        None => grid::inspector(None, None, None, None),
    };
    div()
        .size_full()
        .flex()
        .child(grid::rail_shell(rail, shortcut))
        .child(div().flex_1().min_w_0().flex().flex_col().child(crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::Thin).flex_1().min_h_0().child(body).children(crate::ui::chat::scrollbar::bar_for("sticker-scroll", crate::ui::chat::scrollbar::Bar::Thin))).child(footer))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::StickerFormat;

    #[test]
    fn sticker_matching() {
        let s = StickerData {
            id: crate::model::Id(1),
            name: "Wumpus Wave".into(),
            description: "".into(),
            tags: "wave, hello".into(),
            format: StickerFormat::Png,
            guild: None,
        };
        assert!(matches(&s, ""));
        assert!(matches(&s, "wump"));
        assert!(matches(&s, "hell"));
        assert!(!matches(&s, "cat"));
    }
}
