use gpui::{AnyElement, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, px};

pub fn label(text: SharedString, font_px: f32) -> AnyElement {
    label_impl(text, font_px, false, false)
}

pub fn label_with_custom(text: SharedString, font_px: f32) -> AnyElement {
    label_impl(text, font_px, true, false)
}

pub fn status_label(text: SharedString, font_px: f32) -> AnyElement {
    label_impl(text, font_px, true, true)
}

pub fn markup_label(text: SharedString, font_px: f32) -> AnyElement {
    label_inner(text, font_px, true, true, 0.)
}

#[derive(Clone)]
enum StatusClick {
    Link(String, String),
    Spoiler(usize),
}

pub fn voice_status(
    app: &crate::app::PhoenixApp,
    entity: &gpui::Entity<crate::app::PhoenixApp>,
    channel: crate::model::Id,
    src: &str,
    font_px: f32,
) -> AnyElement {
    let src = src.replace(['\r', '\n'], " ");
    let inline = crate::markdown::parse_voice_status(&src, &app.resolver());
    let always = crate::ui::settings::effects::spoilers_always();
    let revealed = |sp: Option<usize>| sp.is_some_and(|i| always || app.revealed.contains(&(channel, i)));
    let text = inline.text.as_str();
    let segment = |range: std::ops::Range<usize>, last: bool| -> AnyElement {
        let off = range.start;
        let mut highlights = Vec::new();
        let mut clicks: Vec<(std::ops::Range<usize>, StatusClick)> = Vec::new();
        for s in &inline.spans {
            let (a, b) = (s.range.start.max(range.start), s.range.end.min(range.end));
            if a >= b {
                continue;
            }
            let shown = revealed(s.spoiler);
            highlights.push((a - off..b - off, crate::richtext::highlight_for(s.style, shown)));
            let target = match (s.spoiler, &s.link) {
                (Some(sp), _) if !shown => Some(StatusClick::Spoiler(sp)),
                (_, Some(url)) => Some(StatusClick::Link(String::new(), url.clone())),
                _ => None,
            };
            let Some(target) = target else { continue };
            let merged = match (clicks.last_mut(), &target) {
                (Some((r, StatusClick::Link(_, u))), StatusClick::Link(_, url)) if r.end == a - off && u.as_str() == url.as_str() => {
                    r.end = b - off;
                    true
                }
                (Some((r, StatusClick::Spoiler(p))), StatusClick::Spoiler(sp)) if r.end == a - off && *p == *sp => {
                    r.end = b - off;
                    true
                }
                _ => false,
            };
            if !merged {
                clicks.push((a - off..b - off, target));
            }
        }
        let piece = &text[range.clone()];
        for (r, t) in &mut clicks {
            if let StatusClick::Link(label, _) = t {
                *label = piece[r.clone()].to_owned();
            }
        }
        let styled = gpui::StyledText::new(SharedString::from(piece.to_owned())).with_highlights(highlights);
        let wrap = if last { div().min_w_0().text_ellipsis() } else { div().flex_none() };
        if clicks.is_empty() {
            return wrap.child(styled).into_any_element();
        }
        let (ranges, targets): (Vec<_>, Vec<_>) = clicks.into_iter().unzip();
        let e = entity.clone();
        let id = channel.0.wrapping_mul(31).wrapping_add(off as u64);
        wrap.child(gpui::InteractiveText::new(("vc-status-rt", id), styled).on_click(ranges, move |ix, w, cx| {
            cx.stop_propagation();
            match targets[ix].clone() {
                StatusClick::Link(label, url) => e.update(cx, |a, cx| crate::ui::modals::leave_link::click(a, &label, &url, w, cx)),
                StatusClick::Spoiler(sp) => e.update(cx, |a, cx| {
                    a.revealed.insert((channel, sp));
                    cx.notify();
                }),
            }
        }))
        .into_any_element()
    };
    let mut row = div().flex().items_center().min_w_0().overflow_hidden().whitespace_nowrap();
    let mut at = 0;
    for (r, tok) in crate::richtext::emoji_ranges(text) {
        if r.start < at {
            continue;
        }
        if r.start > at {
            row = row.child(segment(at..r.start, false));
        }
        let hidden = inline
            .spans
            .iter()
            .find(|s| s.spoiler.is_some() && s.range.start <= r.start && r.end <= s.range.end && !revealed(s.spoiler))
            .and_then(|s| s.spoiler);
        let size = font_px * 1.375;
        row = row.child(match hidden {
            Some(sp) => {
                let e = entity.clone();
                div()
                    .id(("vc-status-spoiler-emoji", channel.0.wrapping_mul(31).wrapping_add(r.start as u64)))
                    .flex_none()
                    .size(px(size))
                    .bg(crate::theme::c(crate::theme::tok(crate::theme::generated::idx::SPOILER_HIDDEN_BACKGROUND)))
                    .cursor_pointer()
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        e.update(cx, |a, cx| {
                            a.revealed.insert((channel, sp));
                            cx.notify();
                        });
                    })
                    .into_any_element()
            }
            None => div().flex_none().child(crate::media::emoji::emoji_element(&tok, size)).into_any_element(),
        });
        at = r.end;
    }
    if at < text.len() {
        row = row.child(segment(at..text.len(), true));
    }
    row.into_any_element()
}

pub fn emoji_tip(token: &str) -> SharedString {
    if let Some(c) = crate::media::emoji::parse_custom(token) {
        return format!(":{}:", c.name).into();
    }
    match crate::ui::pickers::emoji::index_of(token).or_else(|| crate::ui::pickers::emoji::index_of(token.trim_end_matches('\u{fe0f}'))) {
        Some(ix) => format!(":{}:", crate::ui::pickers::emoji::shortcode(ix)).into(),
        None => SharedString::from(token.to_owned()),
    }
}

fn tipped(el: gpui::Div, token: &str, delay: u64, tips: bool) -> gpui::Div {
    use crate::ui::tooltip::Tipped as _;
    if tips { el.tip_delayed(crate::ui::tooltip::Body::Plain(emoji_tip(token)), crate::ui::tooltip::Side::Top, delay) } else { el }
}

pub fn status_wrapped(text: SharedString, tips: bool) -> AnyElement {
    let ranges = crate::richtext::emoji_ranges(&text);
    if ranges.is_empty() {
        return text.into_any_element();
    }
    let zoom = crate::theme::zoom();
    let emoji_only = ranges.len() == 1
        && text[..ranges[0].0.start].trim().is_empty()
        && text[ranges[0].0.end..].trim().is_empty();
    if emoji_only {
        let e = div().flex_none().child(crate::media::emoji::emoji_element(&ranges[0].1, 32. * zoom));
        return div().flex().items_center().justify_center().child(tipped(e, &ranges[0].1, 300, tips)).into_any_element();
    }
    let mut row = div().flex().flex_wrap().items_center();
    let mut at = 0;
    let words = |row: gpui::Div, r: std::ops::Range<usize>| {
        crate::ui::chat::message_ext::words(&text, r)
            .into_iter()
            .fold(row, |row, w| row.child(div().flex_none().whitespace_nowrap().child(SharedString::from(text[w].to_string()))))
    };
    for (r, tok) in &ranges {
        row = words(row, at..r.start);
        let e = div().flex_none().mr(gpui::px(4. * zoom)).child(crate::media::emoji::emoji_element(tok, 16. * zoom));
        row = row.child(tipped(e, tok, 300, tips));
        at = r.end;
    }
    words(row, at..text.len()).into_any_element()
}

pub fn rich_static(t: &crate::richtext::RichText, font_px: f32, emoji_px: f32) -> AnyElement {
    use gpui::StyledText;
    let zoom = crate::theme::zoom();
    let words = |mut row: gpui::Div, range: std::ops::Range<usize>| -> gpui::Div {
        let mut start = range.start;
        for (i, piece) in t.text[range.clone()].split('\n').enumerate() {
            let r = start..start + piece.len();
            if i > 0 {
                row = row.child(div().w_full().h_0());
            }
            for w in crate::ui::chat::message_ext::words(&t.text, r.clone()) {
                let runs = crate::ui::chat::message_ext::slice_runs(&t.runs, w.clone());
                let text: SharedString = t.text[w].to_owned().into();
                row = row.child(div().flex_none().max_w_full().child(StyledText::new(text).with_runs(runs)));
            }
            start = r.end + 1;
        }
        row
    };
    let mut row = div().flex().flex_wrap().items_center().min_w_0().text_size(crate::theme::fz(font_px));
    let mut last = 0;
    for (r, token) in &t.emoji {
        if r.start < last || crate::richtext::channel_icon_name(token).is_some() {
            continue;
        }
        if let Some(icon) = crate::richtext::parse_link_token(token) {
            row = words(row, last..r.start);
            let fg = crate::ui::chat::message_ext::runs_fg_at(&t.runs, r.end);
            row = row.child(div().flex_none().h(crate::theme::fz(font_px * 1.375)).flex().items_center().child(crate::ui::chat::message_ext::link_icon(&icon, font_px, fg)));
            last = r.end;
            continue;
        }
        row = words(row, last..r.start);
        row = row.child(div().flex_none().mx(gpui::px(zoom)).child(crate::media::emoji::emoji_element(token, emoji_px * zoom * crate::theme::font_scale())));
        last = r.end;
    }
    words(row, last..t.text.len()).into_any_element()
}

pub fn inline_markdown(app: &crate::app::PhoenixApp, src: &str, base: gpui::Hsla, weight: gpui::FontWeight, font_px: f32, custom: bool) -> AnyElement {
    let mut t = inline_rich(app, src, base, false, weight);
    if !custom {
        drop_custom(&mut t);
    }
    if t.emoji.is_empty() && t.runs.len() <= 1 {
        return div().child(t.text).into_any_element();
    }
    rich_static(&t, font_px, font_px * 1.375)
}

pub fn drop_custom(t: &mut crate::richtext::RichText) {
    t.emoji.retain(|(_, tok)| !tok.starts_with('<'));
}

pub fn inline_rich(app: &crate::app::PhoenixApp, src: &str, base: gpui::Hsla, links: bool, weight: gpui::FontWeight) -> crate::richtext::RichText {
    inline_rich_with(src, &app.resolver(), base, links, weight)
}

pub fn inline_rich_with(
    src: &str,
    r: &dyn crate::markdown::MentionResolver,
    base: gpui::Hsla,
    links: bool,
    weight: gpui::FontWeight,
) -> crate::richtext::RichText {
    let mut sp = 0;
    let inline = if links { crate::markdown::parse_inline(src, r, &mut sp) } else { crate::markdown::parse_inline_without_links(src, r, &mut sp) };
    let mut t = crate::richtext::rich_inline(&inline, &|_| true, base);
    for run in &mut t.runs {
        if run.font.weight == gpui::FontWeight::NORMAL {
            run.font.weight = weight;
        }
    }
    t
}

fn label_impl(text: SharedString, font_px: f32, custom: bool, images: bool) -> AnyElement {
    label_inner(text, font_px, custom, images, 1.)
}

fn label_inner(text: SharedString, font_px: f32, custom: bool, images: bool, emoji_mx: f32) -> AnyElement {
    let text: SharedString = if text.contains(['\n', '\r']) { text.replace(['\r', '\n'], " ").into() } else { text };
    let ranges: Vec<_> = crate::richtext::emoji_ranges(&text)
        .into_iter()
        .filter(|(r, _)| custom || !text[r.clone()].starts_with('<'))
        .filter(|(r, _)| images || text[r.clone()].starts_with('<') || emoji_presentation(&text[r.clone()]))
        .map(|(r, _)| r)
        .collect();
    if ranges.is_empty() || std::env::var_os("PHX_NO_EMOJI_FONT").is_some() {
        return text.into_any_element();
    }
    let mut row = div().flex().items_center().min_w_0().overflow_hidden().whitespace_nowrap();
    let mut at = 0;
    for r in ranges {
        if r.start > at {
            row = row.child(div().flex_none().child(SharedString::from(text[at..r.start].to_string())));
        }
        if images || text[r.clone()].starts_with('<') {
            row = row.child(div().flex_none().mx(gpui::px(emoji_mx)).child(crate::media::emoji::emoji_element(&text[r.clone()], font_px * 1.375)));
            at = r.end;
            continue;
        }
        let mut e = div().flex_none();
        e.text_style().font_fallbacks =
            Some(gpui::FontFallbacks::from_fonts(vec![crate::fonts::fallback::EMOJI_FAMILY.to_string()]));
        row = row.child(e.child(SharedString::from(text[r.clone()].to_string())));
        at = r.end;
    }
    if at < text.len() {
        row = row.child(div().min_w_0().text_ellipsis().child(SharedString::from(text[at..].to_string())));
    }
    row.into_any_element()
}

fn emoji_presentation(seq: &str) -> bool {
    let mut cs = seq.chars();
    let Some(first) = cs.next() else { return false };
    if seq.contains('\u{FE0F}') || cs.next().is_some() {
        return true;
    }
    let c = first as u32;
    c >= 0x1F000
        || matches!(
            c,
            0x231A..=0x231B | 0x23E9..=0x23EC | 0x23F0 | 0x23F3 | 0x25FD..=0x25FE | 0x2614..=0x2615
                | 0x2648..=0x2653 | 0x267F | 0x2693 | 0x26A1 | 0x26AA..=0x26AB | 0x26BD..=0x26BE
                | 0x26C4..=0x26C5 | 0x26CE | 0x26D4 | 0x26EA | 0x26F2..=0x26F3 | 0x26F5 | 0x26FA
                | 0x26FD | 0x2705 | 0x270A..=0x270B | 0x2728 | 0x274C | 0x274E | 0x2753..=0x2755
                | 0x2757 | 0x2795..=0x2797 | 0x27B0 | 0x27BF | 0x2B1B..=0x2B1C | 0x2B50 | 0x2B55
        )
}

#[cfg(test)]
mod tests {
    struct NoNames;

    impl crate::markdown::MentionResolver for NoNames {
        fn user(&self, _id: u64) -> Option<String> {
            None
        }
        fn channel(&self, _id: u64) -> Option<String> {
            None
        }
    }

    fn rich(src: &str, links: bool) -> crate::richtext::RichText {
        super::inline_rich_with(src, &NoNames, gpui::black(), links, gpui::FontWeight::SEMIBOLD)
    }

    #[test]
    fn embed_titles_parse_markdown_and_both_emoji_kinds() {
        let t = rich("**Hi** \u{1f389} <:pepe:123> <a:wave:456>", true);
        assert_eq!(t.text.as_ref(), "Hi \u{1f389} <:pepe:123> <a:wave:456>");
        let tokens: Vec<&str> = t.emoji.iter().map(|(_, k)| k.as_ref()).collect();
        assert_eq!(tokens, ["\u{1f389}", "<:pepe:123>", "<a:wave:456>"]);
        assert_eq!(t.runs[0].font.weight, gpui::FontWeight::BOLD);
        assert!(t.runs[1..].iter().all(|r| r.font.weight == gpui::FontWeight::SEMIBOLD));
    }

    #[test]
    fn embed_titles_with_a_url_drop_inner_links() {
        let linked = rich("\u{1f4e2} [notes](https://example.com)", true);
        assert!(linked.click_targets.iter().any(|t| matches!(t, crate::richtext::ClickTarget::Link(_))));
        let plain = rich("\u{1f4e2} [notes](https://example.com)", false);
        assert!(!plain.click_targets.iter().any(|t| matches!(t, crate::richtext::ClickTarget::Link(_))));
        assert_eq!(plain.emoji.len(), 1);
    }

    #[test]
    fn poll_text_keeps_custom_emoji_raw() {
        let mut t = rich("Best? \u{1f355} <:pizza:42>", false);
        super::drop_custom(&mut t);
        assert_eq!(t.emoji.len(), 1);
        assert_eq!(t.emoji[0].1.as_ref(), "\u{1f355}");
        assert!(t.text.contains("<:pizza:42>"));
    }

    #[test]
    fn finds_emoji_in_channel_names() {
        let t = "〢❓・aide-membres";
        let r = crate::richtext::emoji_ranges(t);
        assert_eq!(r.len(), 1);
        assert_eq!(&t[r[0].0.clone()], "❓");
        assert!(crate::richtext::emoji_ranges("〢・general").is_empty());
        assert_eq!(crate::richtext::emoji_ranges("ℹ\u{FE0F} Informations").len(), 1);
        assert!(super::emoji_presentation("❓") && super::emoji_presentation("ℹ\u{FE0F}") && super::emoji_presentation("🌸"));
        assert!(!super::emoji_presentation("☲") && !super::emoji_presentation("♡"));
    }

    fn shaped(text: &str, ends: &[(&str, f32)]) -> gpui::LineLayout {
        use unicode_segmentation::UnicodeSegmentation;
        let mut glyphs = Vec::new();
        let mut x = 0.;
        for (ix, g) in text.grapheme_indices(true) {
            for (offset, _) in g.char_indices() {
                glyphs.push(gpui::ShapedGlyph {
                    id: gpui::GlyphId(0),
                    position: gpui::point(gpui::px(x), gpui::px(0.)),
                    index: ix + offset,
                    is_emoji: super::emoji_presentation(g),
                });
            }
            let prefix = &text[..ix + g.len()];
            x = ends.iter().find(|(p, _)| *p == prefix).map_or(x + 1., |(_, end)| *end);
        }
        gpui::LineLayout {
            font_size: gpui::px(14.),
            width: gpui::px(x),
            ascent: gpui::px(11.),
            descent: gpui::px(3.),
            runs: vec![gpui::ShapedRun { font_id: gpui::FontId(0), glyphs }],
            len: text.len(),
        }
    }

    fn ellipsize(text: &str, ends: &[(&str, f32)], room_before_ellipsis: f32) -> String {
        let cx = gpui::TestAppContext::single();
        let font = gpui::font("Zed Plex Sans");
        let size = gpui::px(14.);
        let system = cx.text_system().clone();
        let ellipsis = f32::from(system.layout_width(system.resolve_font(&font), size, '\u{2026}'));
        let ellipsis = (ellipsis * 96.).ceil() / 96.;
        let runs = vec![gpui::TextRun {
            len: text.len(),
            font: font.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }];
        let mut wrapper = system.line_wrapper(font, size);
        let width = gpui::px(room_before_ellipsis + ellipsis);
        let (cut, _) = wrapper.truncate_line_end_shaped(text.to_owned().into(), &shaped(text, ends), width, "\u{2026}", &runs, 1.5);
        cut.to_string()
    }

    #[test]
    fn ellipsis_keeps_the_last_glyph_after_an_emoji() {
        let group = "\u{1f4bb} \u{2500} \u{2632}\u{2632} Responsable du D\u{e9}veloppement \u{2632}\u{2632}";
        let dev = "\u{1f4bb} \u{2500} \u{2632}\u{2632} Responsable du D\u{e9}v";
        let de = "\u{1f4bb} \u{2500} \u{2632}\u{2632} Responsable du D\u{e9}";
        let ends = [(de, 174.315), (dev, 180.811), ("\u{1f4bb} \u{2500} \u{2632}\u{2632} Responsable du D\u{e9}ve", 188.007)];
        assert_eq!(ellipsize(group, &ends, 180.886), format!("{dev}\u{2026}"));
        assert_eq!(ellipsize(group, &ends, 180.5), format!("{de}\u{2026}"));

        let placeholder = "Envoyer un message dans #\u{3022}\u{1f36f}\u{30fb}honeypot";
        let ends = [
            ("Envoyer un message dans #\u{3022}\u{1f36f}\u{30fb}hone", 271.794),
            ("Envoyer un message dans #\u{3022}\u{1f36f}\u{30fb}honey", 279.314),
            ("Envoyer un message dans #\u{3022}\u{1f36f}\u{30fb}honeyp", 288.194),
        ];
        assert_eq!(ellipsize(placeholder, &ends, 279.427), "Envoyer un message dans #\u{3022}\u{1f36f}\u{30fb}honey\u{2026}");
    }

    #[test]
    fn ellipsis_drops_a_whole_emoji_sequence() {
        let text = "ab\u{1f469}\u{200d}\u{1f680}cd";
        let ends = [("a", 8.), ("ab", 16.), ("ab\u{1f469}\u{200d}\u{1f680}", 36.), ("ab\u{1f469}\u{200d}\u{1f680}c", 44.)];
        assert_eq!(ellipsize(text, &ends, 35.9), "ab\u{2026}");
        assert_eq!(ellipsize(text, &ends, 36.01), "ab\u{1f469}\u{200d}\u{1f680}\u{2026}");
        assert_eq!(ellipsize(text, &ends, 0.), "a\u{2026}");
    }
}
