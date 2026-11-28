use std::ops::Range;

pub mod highlight;

use gpui::{
    Font, FontStyle, FontWeight, HighlightStyle, Hsla, SharedString, StrikethroughStyle, TextRun, UnderlineStyle,
    font, px,
};

use crate::{
    markdown::{self, Block, Inline, Mention, MentionResolver, Style},
    model::Id,
    theme::{FONT_MONO, FONT_UI, Palette, c, ca, generated::idx, tok},
};

pub const CHANNEL_ICON_TOKEN: &str = "\u{1}channel-icon";
pub const LINK_TOKEN: &str = "\u{1}link:";

pub fn channel_icon_token(name: &str) -> String {
    if name == "hash" { CHANNEL_ICON_TOKEN.to_owned() } else { format!("{CHANNEL_ICON_TOKEN}:{name}") }
}

pub fn channel_icon_name(token: &str) -> Option<&'static str> {
    let rest = token.strip_prefix(CHANNEL_ICON_TOKEN)?;
    if rest.is_empty() {
        return Some("hash");
    }
    let name = rest.strip_prefix(':')?;
    markdown::MENTION_ICONS.iter().copied().find(|n| *n == name)
}

pub fn link_token(icon: &markdown::LinkIcon) -> String {
    match icon {
        markdown::LinkIcon::Channel(name) => format!("{LINK_TOKEN}icon:{name}"),
        markdown::LinkIcon::Chevron => format!("{LINK_TOKEN}chevron"),
        markdown::LinkIcon::Guild(url) => format!("{LINK_TOKEN}guild:{url}"),
        markdown::LinkIcon::GuildText(acronym) => format!("{LINK_TOKEN}text:{acronym}"),
    }
}

pub fn parse_link_token(token: &str) -> Option<markdown::LinkIcon> {
    let t = token.strip_prefix(LINK_TOKEN)?;
    if t == "chevron" {
        return Some(markdown::LinkIcon::Chevron);
    }
    if let Some(name) = t.strip_prefix("icon:") {
        return Some(markdown::LinkIcon::Channel(link_icon_name(name)));
    }
    if let Some(u) = t.strip_prefix("guild:") {
        return Some(markdown::LinkIcon::Guild(u.to_owned()));
    }
    t.strip_prefix("text:").map(|a| markdown::LinkIcon::GuildText(a.to_owned()))
}

fn link_icon_name(name: &str) -> &'static str {
    ["hash", "speaker", "megaphone", "forum", "stage", "threads", "chat", "lock"].into_iter().find(|n| *n == name).unwrap_or("hash")
}

#[derive(Clone, Debug)]
pub enum ClickTarget {
    Link(SharedString),
    Spoiler(usize),
    Mention(Mention),
}

#[derive(Clone)]
pub struct RichText {
    pub text: SharedString,
    pub runs: Vec<TextRun>,
    pub click_ranges: Vec<Range<usize>>,
    pub click_targets: Vec<ClickTarget>,
    pub emoji: Vec<(Range<usize>, SharedString)>,
}

impl RichText {
    pub fn mentions(&self) -> Vec<(Range<usize>, Mention)> {
        self.click_ranges
            .iter()
            .zip(&self.click_targets)
            .filter_map(|(r, t)| match t {
                ClickTarget::Mention(m) => Some((r.clone(), m.clone())),
                _ => None,
            })
            .collect()
    }

    pub fn has_mention(&self) -> bool {
        self.click_targets.iter().any(|t| matches!(t, ClickTarget::Mention(_)))
    }
}

#[derive(Clone)]
pub enum RBlock {
    Text(RichText),
    Quote(RichText),
    Code { lang: Option<SharedString>, code: SharedString, runs: Vec<TextRun> },
    Heading(u8, RichText),
    Subtext(RichText),
    ListItem { ordered: Option<u32>, depth: u8, text: RichText },
}

impl RBlock {
    pub fn text(&self) -> Option<&RichText> {
        match self {
            RBlock::Text(t) | RBlock::Quote(t) | RBlock::Heading(_, t) | RBlock::Subtext(t) => Some(t),
            RBlock::ListItem { text, .. } => Some(text),
            RBlock::Code { .. } => None,
        }
    }
}

pub fn code_runs(code: &str, lang: Option<&str>) -> Vec<TextRun> {
    highlight::highlight(code, lang)
        .into_iter()
        .map(|(r, t)| TextRun {
            len: r.len(),
            font: font(FONT_MONO()),
            color: c(t.color()),
            background_color: None,
            underline: None,
            strikethrough: None,
        })
        .collect()
}

pub fn ui_font() -> Font {
    Font { fallbacks: Some(crate::fonts::ui_fallbacks()), ..font(FONT_UI()) }
}

pub fn highlight_for(style: Style, revealed: bool) -> HighlightStyle {
    let mut h = HighlightStyle::default();
    if style.has(Style::BOLD) {
        h.font_weight = Some(FontWeight::BOLD);
    }
    if style.has(Style::ITALIC) {
        h.font_style = Some(FontStyle::Italic);
    }
    if style.has(Style::MENTION) {
        h.color = Some(c(Palette::MENTION_FG));
        h.background_color = Some(ca(Palette::MENTION_BG));
        h.font_weight = Some(FontWeight::MEDIUM);
    }
    if style.has(Style::TIMESTAMP) {
        h.background_color = Some(c(tok(idx::BACKGROUND_MOD_NORMAL)));
    }
    if style.has(Style::LINK) {
        h.color = Some(c(tok(idx::TEXT_LINK)));
    }
    if style.has(Style::UNDERLINE) || (style.has(Style::LINK) && crate::ui::settings::effects::underline_links()) {
        h.underline = Some(UnderlineStyle { thickness: px(1.), color: h.color, wavy: false });
    }
    if style.has(Style::STRIKE) {
        h.strikethrough = Some(StrikethroughStyle { thickness: px(1.), color: h.color });
    }
    if style.has(Style::SPOILER) {
        if revealed {
            h.background_color = Some(c(tok(idx::SPOILER_REVEALED_BACKGROUND)));
        } else {
            h.color = Some(c(tok(idx::SPOILER_HIDDEN_BACKGROUND)));
            h.background_color = Some(c(tok(idx::SPOILER_HIDDEN_BACKGROUND)));
        }
    }
    h
}

fn run_for(len: usize, style: Style, revealed: bool, base: Hsla, ui: &Font) -> TextRun {
    let mut f = if style.has(Style::CODE) { font(FONT_MONO()) } else { ui.clone() };
    if style.has(Style::BOLD) {
        f.weight = FontWeight::BOLD;
    }
    if style.has(Style::ITALIC) {
        f.style = FontStyle::Italic;
    }
    let mut color = base;
    let mut background_color = None;
    if style.has(Style::CODE) {
        background_color = Some(c(tok(idx::BACKGROUND_CODE)));
    }
    if style.has(Style::MENTION) {
        color = c(Palette::MENTION_FG);
        background_color = Some(ca(Palette::MENTION_BG));
        f.weight = FontWeight::MEDIUM;
    }
    if style.has(Style::SOUNDBOARD) {
        color = c(Palette::MENTION_FG);
        background_color = Some(ca(Palette::MENTION_BG));
        f.weight = FontWeight::MEDIUM;
    }
    if style.has(Style::TIMESTAMP) {
        background_color = Some(c(tok(idx::BACKGROUND_MOD_NORMAL)));
    }
    if style.has(Style::LINK) {
        color = c(tok(idx::TEXT_LINK));
    }
    let link_ul = style.has(Style::LINK) && crate::ui::settings::effects::underline_links();
    let underline = (style.has(Style::UNDERLINE) || link_ul).then(|| UnderlineStyle {
        thickness: px(1.),
        color: Some(color),
        wavy: false,
    });
    if style.has(Style::SPOILER) {
        if revealed {
            background_color = Some(c(tok(idx::SPOILER_REVEALED_BACKGROUND)));
        } else {
            color = c(tok(idx::SPOILER_HIDDEN_BACKGROUND));
            background_color = Some(c(tok(idx::SPOILER_HIDDEN_BACKGROUND)));
        }
    }
    TextRun {
        len,
        font: f,
        color,
        background_color,
        underline,
        strikethrough: style.has(Style::STRIKE).then(|| StrikethroughStyle {
            thickness: px(1.),
            color: Some(color),
        }),
    }
}

pub fn rich_inline(inline: &Inline, revealed: &dyn Fn(usize) -> bool, base: Hsla) -> RichText {
    let mut runs = Vec::with_capacity(inline.spans.len());
    let mut click_ranges = Vec::new();
    let mut click_targets = Vec::new();
    let ui = ui_font();
    for s in &inline.spans {
        let rev = s.spoiler.is_some_and(|i| crate::ui::settings::effects::spoilers_always() || revealed(i));
        runs.push(run_for(s.range.len(), s.style, rev, base, &ui));
        if let Some(sp) = s.spoiler
            && !rev
        {
            click_ranges.push(s.range.clone());
            click_targets.push(ClickTarget::Spoiler(sp));
        } else if let Some(l) = &s.link {
            click_ranges.push(s.range.clone());
            click_targets.push(ClickTarget::Link(l.clone().into()));
        } else if let Some(m) = &s.mention {
            click_ranges.push(s.range.clone());
            click_targets.push(ClickTarget::Mention(m.clone()));
        }
    }
    let in_code = |r: &Range<usize>| {
        inline.spans.iter().any(|s| s.style.has(Style::CODE) && s.range.start <= r.start && r.end <= s.range.end)
    };
    let in_mention =
        |r: &Range<usize>| inline.spans.iter().any(|s| s.mention.is_some() && s.range.start <= r.start && r.end <= s.range.end);
    let mut emoji: Vec<(Range<usize>, SharedString)> =
        emoji_ranges(&inline.text).into_iter().filter(|(r, _)| !in_code(r) && !in_mention(r)).collect();
    for s in &inline.spans {
        if matches!(s.mention, Some(Mention::Channel(_))) && inline.text[s.range.clone()].starts_with('#') {
            let name = inline.channel_icons.iter().find(|(at, _)| *at == s.range.start).map_or("hash", |(_, n)| *n);
            emoji.push((s.range.start..s.range.start + 1, channel_icon_token(name).into()));
        }
        if let Some(Mention::Link(l)) = &s.mention {
            for (r, icon) in &l.icons {
                emoji.push((s.range.start + r.start..s.range.start + r.end, link_token(icon).into()));
            }
        }
    }
    emoji.sort_by_key(|(r, _)| r.start);
    RichText {
        text: inline.text.clone().into(),
        runs,
        click_ranges,
        click_targets,
        emoji,
    }
}

pub fn render_blocks(
    blocks: &[Block],
    revealed: &dyn Fn(usize) -> bool,
    base: Hsla,
) -> Vec<RBlock> {
    blocks
        .iter()
        .map(|b| match b {
            Block::Paragraph(i) => RBlock::Text(rich_inline(i, revealed, base)),
            Block::Quote(i) => RBlock::Quote(rich_inline(i, revealed, c(tok(idx::TEXT_SUBTLE)))),
            Block::Code { lang, code } => RBlock::Code {
                lang: lang.clone().map(Into::into),
                code: code.clone().into(),
                runs: code_runs(code, lang.as_deref()),
            },
            Block::Heading(l, i) => {
                let mut t = rich_inline(i, revealed, c(Palette::TEXT_HEADER));
                for r in &mut t.runs {
                    r.font.weight = FontWeight::BOLD;
                }
                RBlock::Heading(*l, t)
            }
            Block::Subtext(i) => RBlock::Subtext(rich_inline(i, revealed, c(tok(idx::TEXT_SUBTLE)))),
            Block::ListItem { ordered, depth, inline } => RBlock::ListItem {
                ordered: *ordered,
                depth: *depth,
                text: rich_inline(inline, revealed, base),
            },
        })
        .collect()
}

#[derive(Clone)]
pub struct RenderedMessage {
    pub group_start: bool,
    pub day_divider: Option<SharedString>,
    pub time_full: SharedString,
    pub time_short: SharedString,
    pub blocks: Vec<RBlock>,
    pub mentions_me: bool,
    pub preview: SharedString,
    pub preview_mentions: Vec<Range<usize>>,
    pub parsed: Vec<Block>,
}

pub fn build(
    content: &str,
    resolver: &dyn MentionResolver,
    me: Id,
    revealed: &dyn Fn(usize) -> bool,
) -> (Vec<Block>, Vec<RBlock>, bool, SharedString, Vec<Range<usize>>) {
    let parsed = markdown::parse(content, resolver);
    let mentions_me = parsed.iter().filter_map(Block::inline).any(|i| {
        i.spans.iter().any(|s| {
            matches!(s.mention, Some(Mention::User(u)) if u == me.0)
                || matches!(s.mention, Some(Mention::Everyone | Mention::Here))
                || matches!(s.mention, Some(Mention::Role(r)) if resolver.has_role(r))
        })
    });
    let (preview, preview_mentions) = preview_with_mentions(&parsed);
    let mut blocks = render_blocks(&parsed, revealed, c(Palette::TEXT_NORMAL));
    color_role_mentions(&parsed, &mut blocks, resolver);
    (parsed, blocks, mentions_me, preview.into(), preview_mentions)
}

pub fn plain_text(parsed: &[Block]) -> String {
    let mut text = String::new();
    for (n, b) in parsed.iter().enumerate() {
        if n > 0 {
            text.push(' ');
        }
        match b {
            Block::Code { code, .. } => text.push_str(code),
            b => {
                if let Some(i) = b.inline() {
                    text.push_str(&i.text);
                }
            }
        }
    }
    text.replace('\n', " ")
}

pub fn preview_with_mentions(parsed: &[Block]) -> (String, Vec<Range<usize>>) {
    let mut text = String::new();
    let mut mentions = Vec::new();
    for (n, b) in parsed.iter().enumerate() {
        if n > 0 {
            text.push(' ');
        }
        let at = text.len();
        match b {
            Block::Code { code, .. } => text.push_str(code),
            b => {
                if let Some(i) = b.inline() {
                    text.push_str(&i.text);
                    mentions.extend(i.spans.iter().filter(|s| s.mention.is_some()).map(|s| s.range.start + at..s.range.end + at));
                }
            }
        }
    }
    let text = text.replace('\n', " ");
    let cut = text.char_indices().nth(120).map_or(text.len(), |(i, _)| i);
    let mentions = mentions.into_iter().filter(|r| r.start < cut).map(|r| r.start..r.end.min(cut)).collect();
    (text[..cut].to_string(), mentions)
}

fn color_role_mentions(parsed: &[Block], blocks: &mut [RBlock], r: &dyn MentionResolver) {
    for (b, rb) in parsed.iter().zip(blocks.iter_mut()) {
        let (Some(inl), Some(t)) = (b.inline(), rich_text_mut(rb)) else { continue };
        for (s, run) in inl.spans.iter().zip(t.runs.iter_mut()) {
            if let Some(Mention::Role(id)) = s.mention
                && let Some(rgb) = r.role_color(id).filter(|&v| v != 0)
            {
                let col: Hsla = gpui::rgb(crate::theme::user_color(rgb)).into();
                run.color = col;
                run.background_color = Some(Hsla { a: 0.1, ..col });
            }
        }
    }
}

fn rich_text_mut(b: &mut RBlock) -> Option<&mut RichText> {
    match b {
        RBlock::Text(t) | RBlock::Quote(t) | RBlock::Heading(_, t) | RBlock::Subtext(t) => Some(t),
        RBlock::ListItem { text, .. } => Some(text),
        RBlock::Code { .. } => None,
    }
}

pub fn apply_highlights(parsed: &[Block], blocks: &mut [RBlock], hl: &crate::search_highlight::Highlighter) {
    if hl.is_empty() {
        return;
    }
    let bg = ca(crate::search_highlight::HIGHLIGHT_RGBA);
    for (b, rb) in parsed.iter().zip(blocks.iter_mut()) {
        let (Some(inl), Some(t)) = (b.inline(), rich_text_mut(rb)) else { continue };
        let skip: Vec<Range<usize>> = inl
            .spans
            .iter()
            .filter(|s| s.style.has(Style::TIMESTAMP) || s.style.has(Style::SOUNDBOARD))
            .map(|s| s.range.clone())
            .chain(t.emoji.iter().map(|(r, _)| r.clone()))
            .collect();
        let ranges: Vec<Range<usize>> = hl
            .ranges(&t.text)
            .into_iter()
            .filter(|r| !skip.iter().any(|s| s.start < r.end && r.start < s.end))
            .collect();
        if !ranges.is_empty() {
            t.runs = split_runs(&t.runs, &ranges, bg);
        }
    }
}

pub fn highlighted_inline(segments: &[(String, bool)], r: &dyn MentionResolver, revealed: &dyn Fn(usize) -> bool, base: Hsla, bg: Hsla) -> RichText {
    let mut inline = Inline::default();
    let mut spoilers = 0usize;
    let mut marks: Vec<Range<usize>> = Vec::new();
    for (text, hl) in segments.iter().filter(|(t, _)| !t.is_empty()) {
        let at = inline.text.len();
        if *hl {
            inline.text.push_str(text);
            let range = at..inline.text.len();
            inline.spans.push(markdown::Span { range: range.clone(), style: Style::PLAIN, link: None, mention: None, spoiler: None });
            marks.push(range);
        } else {
            let part = markdown::parse_inline_without_links(text, r, &mut spoilers);
            inline.channel_icons.extend(part.channel_icons.iter().map(|(p, n)| (p + at, *n)));
            inline.text.push_str(&part.text);
            inline.spans.extend(part.spans.into_iter().map(|mut s| {
                s.range = s.range.start + at..s.range.end + at;
                s
            }));
        }
    }
    let parsed = [Block::Paragraph(inline)];
    let mut blocks = render_blocks(&parsed, revealed, base);
    color_role_mentions(&parsed, &mut blocks, r);
    let Some(RBlock::Text(mut t)) = blocks.pop() else {
        return RichText { text: SharedString::default(), runs: Vec::new(), click_ranges: Vec::new(), click_targets: Vec::new(), emoji: Vec::new() };
    };
    t.emoji.retain(|(e, _)| !marks.iter().any(|m| m.start < e.end && e.start < m.end));
    if !marks.is_empty() {
        t.runs = split_runs(&t.runs, &marks, bg);
    }
    t
}

pub fn split_runs(runs: &[TextRun], ranges: &[Range<usize>], bg: Hsla) -> Vec<TextRun> {
    let total: usize = runs.iter().map(|r| r.len).sum();
    let mut cuts: Vec<usize> = Vec::with_capacity(runs.len() + ranges.len() * 2 + 1);
    cuts.push(0);
    let mut acc = 0;
    for r in runs {
        acc += r.len;
        cuts.push(acc);
    }
    for r in ranges {
        cuts.push(r.start.min(total));
        cuts.push(r.end.min(total));
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = Vec::with_capacity(cuts.len());
    let (mut ri, mut run_end) = (0, runs.first().map_or(0, |r| r.len));
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        while run_end <= a && ri + 1 < runs.len() {
            ri += 1;
            run_end += runs[ri].len;
        }
        let Some(base) = runs.get(ri) else { break };
        let mut r = base.clone();
        r.len = b - a;
        if ranges.iter().any(|x| x.start <= a && b <= x.end) {
            r.background_color = Some(bg);
        }
        out.push(r);
    }
    out
}

pub fn emoji_ranges(text: &str) -> Vec<(Range<usize>, SharedString)> {
    if text.is_ascii() && !text.as_bytes().contains(&b'<') {
        return Vec::new();
    }
    let custom = crate::media::emoji::find_custom(text);
    let mut out: Vec<(Range<usize>, SharedString)> = Vec::new();
    let mut ci = custom.iter().peekable();
    let mut i = 0;
    while i < text.len() {
        if let Some((r, _)) = ci.peek()
            && r.start == i
        {
            let r = r.clone();
            out.push((r.clone(), text[r.clone()].to_owned().into()));
            ci.next();
            i = r.end;
            continue;
        }
        if let Some(end) = crate::media::emoji::unicode_emoji_at(text, i) {
            out.push((i..end, text[i..end].to_owned().into()));
            i = end;
            continue;
        }
        i += text[i..].chars().next().map_or(1, char::len_utf8);
    }
    out
}

pub fn is_jumbo_text(t: &RichText) -> bool {
    let mut last = 0;
    let mut n = 0;
    for (r, _) in &t.emoji {
        if !t.text[last..r.start].trim().is_empty() {
            return false;
        }
        last = r.end;
        n += 1;
    }
    if !t.text[last..].trim().is_empty() {
        return false;
    }
    n > 0 && n <= 30
}

pub fn is_jumbo(blocks: &[RBlock]) -> bool {
    let mut n = 0;
    for b in blocks {
        let RBlock::Text(t) = b else { return false };
        let mut last = 0;
        for (r, _) in &t.emoji {
            if !t.text[last..r.start].trim().is_empty() {
                return false;
            }
            last = r.end;
            n += 1;
        }
        if !t.text[last..].trim().is_empty() {
            return false;
        }
    }
    n > 0 && n <= 30
}

#[cfg(test)]
mod emoji_tests {
    use super::*;

    #[test]
    fn highlights_split_runs_and_skip_emoji() {
        struct R;
        impl MentionResolver for R {
            fn user(&self, _: u64) -> Option<String> {
                Some("Serveur".into())
            }
            fn channel(&self, _: u64) -> Option<String> {
                None
            }
        }
        let hl = crate::search_highlight::Highlighter::new("serveur", false);
        let (parsed, mut blocks, _, _, _) = build("le **serveur** <@1> [serveur](https://x.y) `serveur` <:serveur:12>\n```\nserveur\n```", &R, Id(9), &|_| true);
        apply_highlights(&parsed, &mut blocks, &hl);
        let lit = |b: &RBlock| -> Vec<String> {
            let t = b.text().unwrap();
            let mut at = 0;
            let mut out = Vec::new();
            for r in &t.runs {
                if r.background_color == Some(ca(crate::search_highlight::HIGHLIGHT_RGBA)) {
                    out.push(t.text[at..at + r.len].to_string());
                }
                at += r.len;
            }
            assert_eq!(at, t.text.len());
            out
        };
        assert_eq!(lit(&blocks[0]), ["serveur", "Serveur", "serveur", "serveur"]);
        assert!(matches!(blocks.last(), Some(RBlock::Code { .. })));
    }
    #[test]
    fn automod_content_keeps_markdown_around_the_raw_keyword() {
        struct R;
        impl MentionResolver for R {
            fn user(&self, _: u64) -> Option<String> {
                Some("Alex".into())
            }
            fn channel(&self, _: u64) -> Option<String> {
                None
            }
        }
        let bg = Hsla { h: 0.1, s: 1., l: 0.5, a: 0.3 };
        let segs = vec![("**gros** <@1> https://x.y ".to_string(), false), ("<:nul:5>".to_string(), true), (" [a](https://b.c) 👍".to_string(), false)];
        let t = highlighted_inline(&segs, &R, &|_| true, Hsla::default(), bg);
        assert_eq!(t.text.as_ref(), "gros @Alex https://x.y <:nul:5> [a](https://b.c) 👍");
        assert_eq!(t.runs[0].font.weight, FontWeight::BOLD);
        assert!(matches!(t.click_targets.as_slice(), [ClickTarget::Mention(Mention::User(1))]));
        let mut at = 0;
        let mut lit = Vec::new();
        for run in &t.runs {
            if run.background_color == Some(bg) {
                lit.push(t.text[at..at + run.len].to_string());
            }
            at += run.len;
        }
        assert_eq!(lit, ["<:nul:5>"]);
        assert_eq!(t.emoji.iter().map(|(_, e)| e.to_string()).collect::<Vec<_>>(), ["👍"]);
    }

    #[test]
    fn jumbo_up_to_thirty_emoji() {
        let block = |s: String| RBlock::Text(RichText { emoji: emoji_ranges(&s), text: s.into(), runs: vec![], click_ranges: vec![], click_targets: vec![] });
        assert!(is_jumbo(&[block("👍 <a:d:9>".into())]));
        assert!(is_jumbo(&[block("👍".repeat(30))]));
        assert!(!is_jumbo(&[block("👍".repeat(31))]));
        assert!(!is_jumbo(&[block("hi 👍".into())]));
    }

    #[test]
    fn finds_unicode_and_custom() {
        let t = "hi 👍🏽 <:blob:12> 👨\u{200D}👩\u{200D}👧 🇫🇷 ok 1";
        let e: Vec<String> = emoji_ranges(t).into_iter().map(|(_, s)| s.to_string()).collect();
        assert_eq!(e, vec!["👍🏽", "<:blob:12>", "👨\u{200D}👩\u{200D}👧", "🇫🇷"]);
        assert_eq!(emoji_ranges("a → b ★ ∞ ✓ ■ ♡ ↗ x").len(), 1);
        let e: Vec<String> = emoji_ranges("\u{1FAE9} ☺ 1\u{20E3} 🙂\u{200D}↔\u{FE0F} ❤\u{FE0F}\u{200D}🔥").into_iter().map(|(_, s)| s.to_string()).collect();
        assert_eq!(e, vec!["\u{1FAE9}", "☺", "1\u{20E3}", "🙂\u{200D}↔\u{FE0F}", "❤\u{FE0F}\u{200D}🔥"]);
    }

    #[test]
    fn builds_five_thousand_synthetic_messages() {
        const SAMPLES: [&str; 6] = [
            "plain ascii message without markdown",
            "has **bold**, *italic*, and `inline code`",
            "check https://example.com and <#100>",
            "> quoted line\nfollow up text",
            "```rust\nfn main() {}\n```",
            "emoji 👍 and <:custom:42>",
        ];
        let mut total_blocks = 0usize;
        for i in 0..5000 {
            let src = SAMPLES[i % SAMPLES.len()];
            let (_, blocks, _, preview, _) = build(src, &crate::markdown::NoResolve, Id(1), &|_| false);
            assert!(!preview.is_empty());
            total_blocks += blocks.len();
        }
        assert!(total_blocks >= 5000);
    }
}
