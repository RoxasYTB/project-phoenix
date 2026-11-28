use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub struct Style(pub u16);

impl Style {
    pub const PLAIN: Style = Style(0);
    pub const BOLD: Style = Style(1);
    pub const ITALIC: Style = Style(1 << 1);
    pub const UNDERLINE: Style = Style(1 << 2);
    pub const STRIKE: Style = Style(1 << 3);
    pub const CODE: Style = Style(1 << 4);
    pub const SPOILER: Style = Style(1 << 5);
    pub const LINK: Style = Style(1 << 6);
    pub const MENTION: Style = Style(1 << 7);
    pub const TIMESTAMP: Style = Style(1 << 8);
    pub const SOUNDBOARD: Style = Style(1 << 9);

    pub fn has(self, other: Style) -> bool {
        self.0 & other.0 == other.0 && other.0 != 0
    }
}

impl std::ops::BitOr for Style {
    type Output = Style;
    fn bitor(self, rhs: Style) -> Style {
        Style(self.0 | rhs.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mention {
    User(u64),
    Channel(u64),
    Role(u64),
    Everyone,
    Here,
    Link(Box<ChannelLink>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub style: Style,
    pub link: Option<String>,
    pub mention: Option<Mention>,
    pub spoiler: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Inline {
    pub text: String,
    pub spans: Vec<Span>,
    pub channel_icons: Vec<(usize, &'static str)>,
}

pub const MENTION_ICONS: [&str; 14] = [
    "hash", "lock", "text-nsfw", "speaker", "voice-nsfw", "megaphone", "announcement-nsfw", "forum", "forum-nsfw", "image", "media-nsfw", "stage", "threads", "chat",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MentionChannel {
    pub kind: Option<crate::model::ChannelKind>,
    pub thread: bool,
    pub forum_post: bool,
    pub nsfw: bool,
    pub connect_denied: bool,
    pub media: bool,
}

pub fn mention_icon(c: MentionChannel) -> &'static str {
    use crate::model::ChannelKind as K;
    if c.thread {
        return if c.forum_post { "chat" } else { "threads" };
    }
    match c.kind {
        Some(K::Voice) if c.connect_denied => "lock",
        Some(K::Voice) if c.nsfw => "voice-nsfw",
        Some(K::Voice) => "speaker",
        Some(K::Stage) if c.connect_denied => "lock",
        Some(K::Stage) => "stage",
        Some(K::Forum) if c.media => if c.nsfw { "media-nsfw" } else { "image" },
        Some(K::Forum) => if c.nsfw { "forum-nsfw" } else { "forum" },
        Some(K::Announcement) => if c.nsfw { "announcement-nsfw" } else { "megaphone" },
        _ => if c.nsfw { "text-nsfw" } else { "hash" },
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Paragraph(Inline),
    Quote(Inline),
    Code { lang: Option<String>, code: String },
    Heading(u8, Inline),
    Subtext(Inline),
    ListItem { ordered: Option<u32>, depth: u8, inline: Inline },
}

impl Block {
    pub fn inline(&self) -> Option<&Inline> {
        match self {
            Block::Paragraph(i) | Block::Quote(i) | Block::Heading(_, i) | Block::Subtext(i) => Some(i),
            Block::ListItem { inline, .. } => Some(inline),
            Block::Code { .. } => None,
        }
    }
}

fn line_block(line: &str, r: &dyn MentionResolver, sp: &mut usize) -> Option<Block> {
    for (pfx, lvl) in [("### ", 3u8), ("## ", 2), ("# ", 1)] {
        if let Some(t) = line.strip_prefix(pfx)
            && !t.trim().is_empty()
        {
            return Some(Block::Heading(lvl, parse_inline(t.trim(), r, sp)));
        }
    }
    if let Some(t) = line.strip_prefix("-# ")
        && !t.trim().is_empty()
    {
        return Some(Block::Subtext(parse_inline(t.trim(), r, sp)));
    }
    let trimmed = line.trim_start_matches(' ');
    let depth = ((line.len() - trimmed.len()) / 2).min(2) as u8;
    if let Some(t) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* "))
        && !t.trim().is_empty()
    {
        return Some(Block::ListItem { ordered: None, depth, inline: parse_inline(t, r, sp) });
    }
    let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0
        && digits < 10
        && let Some(t) = trimmed[digits..].strip_prefix(". ")
        && !t.trim().is_empty()
    {
        let n = trimmed[..digits].parse().unwrap_or(1);
        return Some(Block::ListItem { ordered: Some(n), depth, inline: parse_inline(t, r, sp) });
    }
    None
}

pub fn format_timestamp(ts: i64, style: char) -> Option<String> {
    use chrono::{Local, TimeZone};
    let dt = Local.timestamp_opt(ts, 0).single()?;
    Some(match style {
        't' => crate::i18n::clock(dt),
        'T' => if crate::ui::settings::effects::hour24().unwrap_or(crate::i18n::locale() != "en") { dt.format("%H:%M:%S").to_string() } else { dt.format("%-I:%M:%S %p").to_string() },
        'd' => crate::i18n::short_date(dt),
        'D' => crate::i18n::long_date(dt.date_naive()),
        'F' => crate::i18n::full_datetime(dt),
        'R' => relative(ts - Local::now().timestamp()),
        _ => format!("{} {}", crate::i18n::long_date(dt.date_naive()), crate::i18n::clock(dt)),
    })
}

fn relative(delta: i64) -> String {
    crate::i18n::relative_long(delta)
}

pub trait MentionResolver {
    fn user(&self, id: u64) -> Option<String>;
    fn channel(&self, id: u64) -> Option<String>;
    fn mention_channel(&self, _id: u64) -> Option<MentionChannel> {
        None
    }
    fn role(&self, _id: u64) -> Option<String> {
        None
    }
    fn role_color(&self, _id: u64) -> Option<u32> {
        None
    }
    fn has_role(&self, _id: u64) -> bool {
        false
    }
    fn soundboard(&self, _guild_id: u64, _sound_id: u64) -> Option<SoundboardMention> {
        None
    }
    fn current_guild(&self) -> Option<u64> {
        None
    }
    fn link_channel(&self, _id: u64) -> Option<LinkChannel> {
        None
    }
    fn link_guild(&self, _id: u64) -> Option<LinkGuild> {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundboardMention {
    pub name: String,
    pub emoji_name: Option<String>,
}

pub struct NoResolve;
impl MentionResolver for NoResolve {
    fn user(&self, _: u64) -> Option<String> {
        None
    }
    fn channel(&self, _: u64) -> Option<String> {
        None
    }
}

fn push_quote_line(quote: &mut String, line: &str) {
    let trimmed = line.trim_start_matches(' ');
    if let Some(item) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
        quote.push_str(&line[..line.len() - trimmed.len()]);
        quote.push_str("• ");
        quote.push_str(item);
    } else {
        quote.push_str(line);
    }
    quote.push('\n');
}

pub fn parse(src: &str, r: &dyn MentionResolver) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut rest = src;
    let mut para = String::new();
    let mut quote = String::new();
    let mut spoilers = 0usize;

    let flush = |buf: &mut String, quote: bool, blocks: &mut Vec<Block>, sp: &mut usize| {
        let t = buf.trim_end_matches('\n');
        if !t.is_empty() {
            let inline = parse_inline(t, r, sp);
            blocks.push(if quote { Block::Quote(inline) } else { Block::Paragraph(inline) });
        }
        buf.clear();
    };

    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("```")
            && let Some(end) = after.find("```")
        {
            flush(&mut para, false, &mut blocks, &mut spoilers);
            flush(&mut quote, true, &mut blocks, &mut spoilers);
            let body = &after[..end];
            let (lang, code) = match body.find('\n') {
                Some(nl)
                    if !body[..nl].trim().is_empty()
                        && !body[..nl].trim().contains(char::is_whitespace) =>
                {
                    (Some(body[..nl].trim().to_string()), &body[nl + 1..])
                }
                Some(0) => (None, &body[1..]),
                _ => (None, body),
            };
            blocks.push(Block::Code {
                lang,
                code: code.trim_end_matches('\n').to_string(),
            });
            rest = &after[end + 3..];
            rest = rest.strip_prefix('\n').unwrap_or(rest);
            continue;
        }
        let (line, next) = match rest.find('\n') {
            Some(i) => (&rest[..i], &rest[i + 1..]),
            None => (rest, ""),
        };
        if line.contains("```") && rest.find("```").is_some_and(|i| i < line.len()) {
            let i = rest.find("```").unwrap_or(0);
            if i > 0 {
                para.push_str(&rest[..i]);
                rest = &rest[i..];
                continue;
            }
        }
        if let Some(first) = line.strip_prefix(">>> ").or_else(|| (line == ">>>").then_some("")) {
            flush(&mut para, false, &mut blocks, &mut spoilers);
            flush(&mut quote, true, &mut blocks, &mut spoilers);
            if !first.is_empty() {
                push_quote_line(&mut quote, first);
            }
            for line in next.lines() {
                push_quote_line(&mut quote, line);
            }
            break;
        }
        if let Some(b) = line_block(line, r, &mut spoilers) {
            flush(&mut para, false, &mut blocks, &mut spoilers);
            flush(&mut quote, true, &mut blocks, &mut spoilers);
            blocks.push(b);
        } else if let Some(q) = line.strip_prefix("> ").or_else(|| (line == ">").then_some("")) {
            flush(&mut para, false, &mut blocks, &mut spoilers);
            push_quote_line(&mut quote, q);
        } else {
            flush(&mut quote, true, &mut blocks, &mut spoilers);
            para.push_str(line);
            para.push('\n');
        }
        rest = next;
    }
    flush(&mut para, false, &mut blocks, &mut spoilers);
    flush(&mut quote, true, &mut blocks, &mut spoilers);
    blocks
}

pub mod channel_link;
pub mod input;
mod link;
pub use channel_link::{ChannelLink, LinkChannel, LinkGuild, LinkIcon};
pub use link::title_for as link_title_attr;

thread_local! {
    static EMOJI_LINKS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static NO_LINKS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static VOICE_STATUS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn voice_status() -> bool {
    VOICE_STATUS.with(std::cell::Cell::get)
}

pub fn parse_voice_status(src: &str, r: &dyn MentionResolver) -> Inline {
    let prev = VOICE_STATUS.with(|e| e.replace(true));
    let mut sp = 0;
    let out = parse_inline(src, r, &mut sp);
    VOICE_STATUS.with(|e| e.set(prev));
    out
}

fn no_links() -> bool {
    NO_LINKS.with(std::cell::Cell::get)
}

pub fn parse_inline_without_links(src: &str, r: &dyn MentionResolver, spoiler_counter: &mut usize) -> Inline {
    let prev = NO_LINKS.with(|e| e.replace(true));
    let out = parse_inline(src, r, spoiler_counter);
    NO_LINKS.with(|e| e.set(prev));
    out
}

fn emoji_links() -> bool {
    EMOJI_LINKS.with(std::cell::Cell::get)
}

pub fn parse_emoji_links(src: &str, r: &dyn MentionResolver) -> Vec<Block> {
    let prev = EMOJI_LINKS.with(|e| e.replace(true));
    let out = parse(src, r);
    EMOJI_LINKS.with(|e| e.set(prev));
    out
}

pub fn parse_inline(src: &str, r: &dyn MentionResolver, spoiler_counter: &mut usize) -> Inline {
    let mut out = Inline::default();
    let mut ctx = Ctx {
        out: &mut out,
        r,
        spoilers: spoiler_counter,
    };
    ctx.run(src, Style::PLAIN, None);
    out
}

struct Ctx<'a> {
    out: &'a mut Inline,
    r: &'a dyn MentionResolver,
    spoilers: &'a mut usize,
}

impl Ctx<'_> {
    fn push(&mut self, text: &str, style: Style, spoiler: Option<usize>) {
        self.push_full(text, style, None, None, spoiler)
    }

    fn push_full(
        &mut self,
        text: &str,
        style: Style,
        link: Option<String>,
        mention: Option<Mention>,
        spoiler: Option<usize>,
    ) {
        if text.is_empty() {
            return;
        }
        let start = self.out.text.len();
        self.out.text.push_str(text);
        let end = self.out.text.len();
        if let Some(last) = self.out.spans.last_mut()
            && last.style == style
            && last.link.is_none()
            && link.is_none()
            && last.mention.is_none()
            && mention.is_none()
            && last.spoiler == spoiler
            && last.range.end == start
        {
            last.range.end = end;
            return;
        }
        self.out.spans.push(Span {
            range: start..end,
            style,
            link,
            mention,
            spoiler,
        });
    }

    fn run(&mut self, src: &str, style: Style, spoiler: Option<usize>) {
        let bytes = src.as_bytes();
        let mut i = 0;
        let mut plain_start = 0;
        while i < bytes.len() {
            let rest = &src[i..];
            let c = bytes[i];

            if c == b'\\' && i + 1 < bytes.len() && !bytes[i + 1].is_ascii_alphanumeric() && !src[i + 1..].starts_with(char::is_whitespace) {
                self.push(&src[plain_start..i], style, spoiler);
                let ch_len = src[i + 1..].chars().next().map_or(1, char::len_utf8);
                self.push(&src[i + 1..i + 1 + ch_len], style, spoiler);
                i += 1 + ch_len;
                plain_start = i;
                continue;
            }

            if c == b'`' && !voice_status() {
                let ticks = if rest.starts_with("``") { 2 } else { 1 };
                let close = &rest[..ticks];
                if let Some(end) = rest[ticks..].find(close)
                    && end > 0
                {
                    self.push(&src[plain_start..i], style, spoiler);
                    let inner = &rest[ticks..ticks + end];
                    self.push(inner, style | Style::CODE, spoiler);
                    i += ticks * 2 + end;
                    plain_start = i;
                    continue;
                }
            }

            if c == b'<'
                && let Some(end) = rest.find('>')
            {
                let inner = &rest[1..end];
                let vs = voice_status();
                if let Some((guild, sound)) = inner
                    .strip_prefix("sound:")
                    .and_then(|ids| ids.split_once(':'))
                    .and_then(|(guild, sound)| Some((guild.parse::<u64>().ok()?, sound.parse::<u64>().ok()?)))
                    .filter(|_| !vs)
                    && let Some(sound) = self.r.soundboard(guild, sound)
                {
                    self.push(&src[plain_start..i], style, spoiler);
                    let mut label = String::new();
                    if let Some(emoji) = sound.emoji_name.filter(|e| !e.is_empty()) {
                        label.push_str(&emoji);
                        label.push(' ');
                    }
                    label.push_str(if sound.name.is_empty() { "Unknown" } else { &sound.name });
                    self.push_full(&label, style | Style::SOUNDBOARD, None, None, spoiler);
                    i += end + 1;
                    plain_start = i;
                    continue;
                }
                if let Some(t) = inner.strip_prefix("t:") {
                    let (num, st) = t.split_once(':').unwrap_or((t, "f"));
                    if let (Ok(ts), Some(stc)) = (num.parse::<i64>(), st.chars().next())
                        && st.len() == 1
                        && let Some(label) = format_timestamp(ts, stc)
                    {
                        self.push(&src[plain_start..i], style, spoiler);
                        self.push_full(&label, style | Style::TIMESTAMP, None, None, spoiler);
                        i += end + 1;
                        plain_start = i;
                        continue;
                    }
                }
                let parsed = if vs && inner.starts_with('@') {
                    None
                } else if let Some(id) = inner.strip_prefix("@&") {
                    id.parse().ok().map(|id| {
                        let n = self.r.role(id).unwrap_or_else(|| "unknown-role".into());
                        (Mention::Role(id), format!("@{n}"))
                    })
                } else if let Some(id) = inner.strip_prefix("@!").or_else(|| inner.strip_prefix('@')) {
                    id.parse().ok().map(|id| {
                        let n = self.r.user(id).unwrap_or_else(|| crate::t!("common.unknown_user").to_string());
                        (Mention::User(id), format!("@{n}"))
                    })
                } else if let Some(id) = inner.strip_prefix('#') {
                    id.parse().ok().map(|id| {
                        let n = self.r.channel(id).unwrap_or_else(|| crate::t!("common.unknown_lower").to_string());
                        (Mention::Channel(id), format!("#{n}"))
                    })
                } else {
                    None
                };
                if let Some((Mention::Channel(id), _)) = &parsed
                    && let Some(mc) = self.r.mention_channel(*id)
                {
                    let at = self.out.text.len() + src[plain_start..i].len();
                    self.out.channel_icons.push((at, mention_icon(mc)));
                }
                if let Some((m, label)) = parsed {
                    self.push(&src[plain_start..i], style, spoiler);
                    self.push_full(&label, style | Style::MENTION, None, Some(m), spoiler);
                    i += end + 1;
                    plain_start = i;
                    continue;
                }
                if !vs && autolink(inner) {
                    self.push(&src[plain_start..i], style, spoiler);
                    match link::validate_url(inner) {
                        Some(target) => self.push_full(inner, style | Style::LINK, Some(target), None, spoiler),
                        None => self.push(inner, style, spoiler),
                    }
                    i += end + 1;
                    plain_start = i;
                    continue;
                }
            }
            if c == b'@' && !voice_status() {
                let (tok, m) = if rest.starts_with("@everyone") {
                    ("@everyone", Some(Mention::Everyone))
                } else if rest.starts_with("@here") {
                    ("@here", Some(Mention::Here))
                } else {
                    ("", None)
                };
                if let Some(m) = m {
                    self.push(&src[plain_start..i], style, spoiler);
                    self.push_full(tok, style | Style::MENTION, None, Some(m), spoiler);
                    i += tok.len();
                    plain_start = i;
                    continue;
                }
            }

            if c == b'['
                && !no_links()
                && let Some(m) = link::match_at(rest)
            {
                let label = &rest[m.label.clone()];
                let target = link::validate_url(&rest[m.url.clone()]).filter(|_| link::label_ok(label, emoji_links()));
                if let Some(url) = target {
                    self.push(&src[plain_start..i], style, spoiler);
                    let first = self.out.spans.len();
                    let from = self.out.text.len();
                    self.run(label, style | Style::LINK, spoiler);
                    for s in &mut self.out.spans[first..] {
                        s.link = Some(url.clone());
                    }
                    if let Some(tr) = m.title.clone().filter(|r| !r.is_empty()) {
                        link::remember_title(&self.out.text[from..], &url, &rest[tr]);
                    }
                    i += m.len;
                    plain_start = i;
                } else {
                    i += m.len;
                }
                continue;
            }

            if c == b'h'
                && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
                && !no_links()
                && !voice_status()
                && let Some(m) = channel_link::match_url(rest)
                && let Some(l) = channel_link::resolve(self.r, &m)
            {
                self.push(&src[plain_start..i], style, spoiler);
                let st = if l.italic { style | Style::MENTION | Style::ITALIC } else { style | Style::MENTION };
                self.push_full(&l.text, st, None, Some(l.mention), spoiler);
                i += m.len;
                plain_start = i;
                continue;
            }

            if matches!(c, b'h' | b's')
                && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
                && !no_links()
                && !voice_status()
                && let Some(url) = bare_url(rest)
            {
                self.push(&src[plain_start..i], style, spoiler);
                match link::validate_url(url) {
                    Some(target) => self.push_full(url, style | Style::LINK, Some(target), None, spoiler),
                    None => self.push(url, style, spoiler),
                }
                i += url.len();
                plain_start = i;
                continue;
            }

            if c == b'<'
                && let Some(len) = custom_emoji_len(rest)
            {
                i += len;
                continue;
            }

            if let Some((len, body, s)) = emphasis(rest) {
                self.push(&src[plain_start..i], style, spoiler);
                let sp = if s == Style::SPOILER {
                    let n = *self.spoilers;
                    *self.spoilers += 1;
                    Some(n)
                } else {
                    spoiler
                };
                self.run(&rest[body], style | s, sp);
                i += len;
                plain_start = i;
                continue;
            }
            i += src[i..].chars().next().map_or(1, char::len_utf8);
        }
        self.push(&src[plain_start..], style, spoiler);
    }
}

fn char_len(s: &str, j: usize) -> usize {
    s[j..].chars().next().map_or(1, char::len_utf8)
}

fn is_word(b: Option<&u8>) -> bool {
    b.is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
}

fn escape_len(s: &str, j: usize) -> Option<usize> {
    (s.as_bytes()[j] == b'\\' && j + 1 < s.len()).then(|| 1 + char_len(s, j + 1))
}

fn em_underscore(s: &str) -> Option<(usize, Range<usize>)> {
    let b = s.as_bytes();
    if b.first() != Some(&b'_') {
        return None;
    }
    let mut j = 1;
    while j < b.len() {
        if j > 1 && b[j] == b'_' && !is_word(b.get(j + 1)) {
            return Some((j + 1, 1..j));
        }
        if s[j..].starts_with("__") {
            j += 2;
        } else if let Some(n) = escape_len(s, j) {
            j += n;
        } else if b[j] != b'\\' && b[j] != b'_' {
            j += char_len(s, j);
        } else {
            return None;
        }
    }
    None
}

fn em_star(s: &str, loose: bool) -> Option<(usize, Range<usize>)> {
    let b = s.as_bytes();
    if b.first() != Some(&b'*') || s[1..].chars().next().is_none_or(char::is_whitespace) {
        return None;
    }
    let plain = |j: usize| b[j] != b'*' && b[j] != b'\\' && !s[j..].starts_with(char::is_whitespace);
    let mut j = 1;
    while j < b.len() {
        if j > 1 {
            let lo = usize::from(loose);
            let spaces = if loose { b[j..].iter().take(2).take_while(|&&c| c == b' ').count() } else { 0 };
            for k in (lo..=spaces).rev() {
                if b.get(j + k) == Some(&b'*') && b.get(j + k + 1) != Some(&b'*') {
                    return Some((j + k + 1, 1..j));
                }
            }
        }
        if s[j..].starts_with("**") {
            j += 2;
        } else if let Some(n) = escape_len(s, j) {
            j += n;
        } else if s[j..].starts_with(char::is_whitespace) {
            let ws = s[j..].find(|ch: char| !ch.is_whitespace()).map_or(s.len(), |n| j + n);
            if ws >= b.len() {
                return None;
            }
            j = if s[ws..].starts_with("**") {
                ws + 2
            } else if let Some(n) = escape_len(s, ws) {
                ws + n
            } else if plain(ws) {
                ws + char_len(s, ws)
            } else {
                return None;
            };
        } else if plain(j) {
            j += char_len(s, j);
        } else {
            return None;
        }
    }
    None
}

fn paired(s: &str, d: &str, escapes: bool, not_after: Option<u8>) -> Option<(usize, Range<usize>)> {
    if !s.starts_with(d) {
        return None;
    }
    let b = s.as_bytes();
    let n = d.len();
    let mut j = n;
    while j < b.len() {
        if j > n && s[j..].starts_with(d) && not_after.is_none_or(|x| b.get(j + n) != Some(&x)) {
            return Some((j + n, n..j));
        }
        if escapes {
            if let Some(e) = escape_len(s, j) {
                j += e;
                continue;
            }
            if b[j] == b'\\' {
                return None;
            }
        }
        j += char_len(s, j);
    }
    None
}

pub(crate) fn emphasis(s: &str) -> Option<(usize, Range<usize>, Style)> {
    match s.as_bytes().first()? {
        b'*' | b'_' => {
            let candidates = [
                (em_underscore(s).or_else(|| em_star(s, false)), Style::ITALIC, 0.2),
                (em_star(s, true), Style::ITALIC, 0.2),
                (paired(s, "**", true, Some(b'*')), Style::BOLD, 0.1),
                (paired(s, "__", true, Some(b'_')), Style::UNDERLINE, 0.0),
            ];
            let mut best: Option<(f32, usize, Range<usize>, Style)> = None;
            for (m, style, q) in candidates {
                if let Some((len, body)) = m {
                    let quality = len as f32 + q;
                    if best.as_ref().is_none_or(|b| quality > b.0) {
                        best = Some((quality, len, body, style));
                    }
                }
            }
            best.map(|(_, len, body, style)| (len, body, style))
        }
        b'~' => paired(s, "~~", false, Some(b'_')).map(|(l, r)| (l, r, Style::STRIKE)),
        b'|' => paired(s, "||", false, None).map(|(l, r)| (l, r, Style::SPOILER)),
        _ => None,
    }
}

fn autolink(inner: &str) -> bool {
    let Some(colon) = inner.find(':') else { return false };
    colon > 0
        && !inner[..colon].contains([' ', '>'])
        && inner[colon + 1..].starts_with('/')
        && inner.len() > colon + 2
        && !inner.contains([' ', '>'])
}

fn bare_url(rest: &str) -> Option<&str> {
    let scheme = ["https://", "http://", "steam://"].into_iter().find(|p| rest.starts_with(p))?;
    let end = rest.find(|ch: char| ch.is_whitespace() || ch == '<').unwrap_or(rest.len());
    let mut url = rest[..end].trim_end_matches(['.', ',', ':', ';', '"', '\'', ']']);
    if url.len() <= scheme.len() {
        return None;
    }
    if url.ends_with(')') {
        let mut from = 0;
        let mut k = url.len();
        while k > 0 && url.as_bytes()[k - 1] == b')' {
            match url[from..].find('(') {
                None => {
                    url = &url[..url.len() - 1];
                    break;
                }
                Some(p) => from += p + 1,
            }
            k -= 1;
        }
    }
    Some(url)
}

fn custom_emoji_len(s: &str) -> Option<usize> {
    let end = s.find('>')?;
    crate::media::emoji::parse_custom(&s[..=end]).map(|_| end + 1)
}

pub fn is_single_link(src: &str) -> bool {
    let blocks = parse(src, &NoResolve);
    let [Block::Paragraph(i)] = blocks.as_slice() else { return false };
    let mut spans: Vec<&Span> = i.spans.iter().filter(|s| !s.range.is_empty()).collect();
    spans.sort_by_key(|s| s.range.start);
    let Some(link) = spans.first().and_then(|s| s.link.as_deref()) else { return false };
    let mut at = 0;
    for s in spans {
        if s.range.start != at || s.link.as_deref() != Some(link) {
            return false;
        }
        at = s.range.end;
    }
    at == i.text.len()
}

pub fn clickable(inline: &Inline) -> impl Iterator<Item = &Span> {
    inline
        .spans
        .iter()
        .filter(|s| s.link.is_some() || s.spoiler.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct R;
    impl MentionResolver for R {
        fn user(&self, id: u64) -> Option<String> {
            (id == 42).then(|| "wumpus".into())
        }
        fn channel(&self, id: u64) -> Option<String> {
            (id == 7).then(|| "general".into())
        }
        fn mention_channel(&self, id: u64) -> Option<MentionChannel> {
            (id == 8).then_some(MentionChannel { kind: Some(crate::model::ChannelKind::Voice), ..Default::default() })
        }
        fn soundboard(&self, guild_id: u64, sound_id: u64) -> Option<SoundboardMention> {
            (guild_id == 9 && sound_id == 77).then(|| SoundboardMention {
                name: "Chime".into(),
                emoji_name: Some("🎵".into()),
            })
        }
    }

    #[test]
    fn channel_mention_icon_follows_channel_type() {
        use crate::model::ChannelKind as K;
        let m = |kind, f: fn(&mut MentionChannel)| {
            let mut c = MentionChannel { kind: Some(kind), ..Default::default() };
            f(&mut c);
            mention_icon(c)
        };
        assert_eq!(m(K::Voice, |_| {}), "speaker");
        assert_eq!(m(K::Voice, |c| c.nsfw = true), "voice-nsfw");
        assert_eq!(m(K::Voice, |c| c.connect_denied = true), "lock");
        assert_eq!(m(K::Stage, |_| {}), "stage");
        assert_eq!(m(K::Stage, |c| c.connect_denied = true), "lock");
        assert_eq!(m(K::Text, |_| {}), "hash");
        assert_eq!(m(K::Text, |c| c.nsfw = true), "text-nsfw");
        assert_eq!(m(K::Announcement, |_| {}), "megaphone");
        assert_eq!(m(K::Announcement, |c| c.nsfw = true), "announcement-nsfw");
        assert_eq!(m(K::Forum, |_| {}), "forum");
        assert_eq!(m(K::Forum, |c| c.nsfw = true), "forum-nsfw");
        assert_eq!(m(K::Forum, |c| c.media = true), "image");
        assert_eq!(m(K::Forum, |c| (c.media, c.nsfw) = (true, true)), "media-nsfw");
        assert_eq!(m(K::Text, |c| c.thread = true), "threads");
        assert_eq!(m(K::Text, |c| (c.thread, c.forum_post) = (true, true)), "chat");
        for k in [K::Voice, K::Stage, K::Text, K::Announcement, K::Forum] {
            for bits in 0..32u8 {
                let c = MentionChannel {
                    kind: Some(k),
                    thread: bits & 1 != 0,
                    forum_post: bits & 2 != 0,
                    nsfw: bits & 4 != 0,
                    connect_denied: bits & 8 != 0,
                    media: bits & 16 != 0,
                };
                assert!(MENTION_ICONS.contains(&mention_icon(c)));
            }
        }
    }

    #[test]
    fn voice_channel_mention_records_its_icon() {
        let mut sp = 0;
        let i = parse_inline("join **now** <#8> or <#7>", &R, &mut sp);
        let voice = i.text.find('#').unwrap();
        assert_eq!(i.channel_icons, vec![(voice, "speaker")]);
        let rt = crate::richtext::rich_inline(&i, &|_| false, gpui::Hsla::default());
        let tokens: Vec<_> = rt.emoji.iter().map(|(r, t)| (r.start, crate::richtext::channel_icon_name(t))).collect();
        assert_eq!(tokens, vec![(voice, Some("speaker")), (i.text.rfind('#').unwrap(), Some("hash"))]);
    }

    #[test]
    fn triple_stars_are_bold_italic() {
        let i = inline("***both***");
        assert_eq!(i.text, "both");
        assert!(i.spans.iter().all(|s| s.style.has(Style::BOLD) && s.style.has(Style::ITALIC)));
        let u = inline("__*under*__");
        assert_eq!(u.text, "under");
    }

    #[test]
    fn voice_status_keeps_styles_links_and_timestamps_only() {
        let i = parse_voice_status("**on** _air_ ||secret|| [site](https://example.com) <t:0:R>", &R);
        let st = styled(&i);
        assert!(st.iter().any(|(t, s)| *t == "on" && s.has(Style::BOLD)));
        assert!(st.iter().any(|(t, s)| *t == "air" && s.has(Style::ITALIC)));
        assert!(st.iter().any(|(t, s)| *t == "secret" && s.has(Style::SPOILER)));
        assert!(i.spans.iter().any(|s| s.link.as_deref() == Some("https://example.com/") || s.link.as_deref() == Some("https://example.com")));
        assert!(i.spans.iter().any(|s| s.style.has(Style::TIMESTAMP)));
        let raw = parse_voice_status("`x` <@42> @everyone <@&1> https://a.com <https://b.com> <sound:9:77>", &R);
        assert_eq!(raw.text, "`x` <@42> @everyone <@&1> https://a.com <https://b.com> <sound:9:77>");
        assert!(raw.spans.iter().all(|s| s.style == Style::PLAIN && s.link.is_none() && s.mention.is_none()));
        let ch = parse_voice_status("<#7>", &R);
        assert_eq!(ch.text, "#general");
        assert!(!inline("`x`").text.contains('`'));
    }

    fn inline(s: &str) -> Inline {
        let mut n = 0;
        parse_inline(s, &R, &mut n)
    }

    fn styled(i: &Inline) -> Vec<(&str, Style)> {
        i.spans.iter().map(|s| (&i.text[s.range.clone()], s.style)).collect()
    }

    fn italic_text(i: &Inline) -> Vec<&str> {
        styled(i).into_iter().filter(|(_, st)| st.has(Style::ITALIC)).map(|(t, _)| t).collect()
    }

    #[test]
    fn underscore_italic_follows_simple_markdown_em() {
        assert_eq!(styled(&inline("_italique_")), vec![("italique", Style::ITALIC)]);
        assert_eq!(italic_text(&inline("du texte en _italique_.")), vec!["italique"]);
        assert_eq!(italic_text(&inline("mot_collé_ ici")), vec!["collé"]);
        assert_eq!(italic_text(&inline("(_x_) é_y_")), vec!["x", "y"]);
        assert_eq!(italic_text(&inline("_ espace _")), vec![" espace "]);
        let i = inline("_snake_case_");
        assert_eq!(i.text, "_snakecase");
        assert_eq!(italic_text(&i), vec!["case"]);
        for plain in ["snake_case_var", "_x_y", "_x__", "a _ b", "__"] {
            let i = inline(plain);
            assert_eq!(i.text, plain);
            assert!(italic_text(&i).is_empty(), "{plain}");
        }
        let i = inline("_a__b_");
        assert_eq!(styled(&i), vec![("a__b", Style::ITALIC)]);
        let i = inline(r"\_x\_");
        assert_eq!(i.text, "_x_");
        assert!(italic_text(&i).is_empty());
    }

    #[test]
    fn underscores_in_urls_and_custom_emoji_stay_literal() {
        let i = inline("https://a.b/x_y_z _it_");
        assert_eq!(i.text, "https://a.b/x_y_z it");
        assert_eq!(italic_text(&i), vec!["it"]);
        let src = "<:a_b:1> mid <:c_d:2> <a:e_:3>_f_";
        let i = inline(src);
        assert!(i.text.starts_with("<:a_b:1> mid <:c_d:2> <a:e_:3>"));
        assert_eq!(italic_text(&i), vec!["f"]);
    }

    #[test]
    fn emphasis_picks_the_best_quality_like_the_client() {
        let i = inline("___x___");
        assert_eq!(styled(&i), vec![("x", Style::UNDERLINE | Style::ITALIC)]);
        let i = inline("***x***");
        assert_eq!(styled(&i), vec![("x", Style::BOLD | Style::ITALIC)]);
        assert_eq!(styled(&inline("*x*")), vec![("x", Style::ITALIC)]);
        assert_eq!(styled(&inline("__x__")), vec![("x", Style::UNDERLINE)]);
        assert_eq!(styled(&inline("~~x~~")), vec![("x", Style::STRIKE)]);
        assert_eq!(styled(&inline("||x||")), vec![("x", Style::SPOILER)]);
        assert_eq!(styled(&inline("**x**")), vec![("x", Style::BOLD)]);
        assert_eq!(styled(&inline("*loose *")), vec![("loose", Style::ITALIC)]);
        assert_eq!(inline("~~x~~_").text, "~~x~~_");
        assert_eq!(inline("* x*").text, "* x*");
        let i = inline("*a **b** c*");
        assert_eq!(styled(&i), vec![("a ", Style::ITALIC), ("b", Style::ITALIC | Style::BOLD), (" c", Style::ITALIC)]);
        let i = inline("**gras _et italique_**");
        assert_eq!(styled(&i), vec![("gras ", Style::BOLD), ("et italique", Style::BOLD | Style::ITALIC)]);
    }

    #[test]
    fn plain_text() {
        let i = inline("hello world");
        assert_eq!(styled(&i), vec![("hello world", Style::PLAIN)]);
    }

    #[test]
    fn soundboard_mention_resolves_name_and_unicode_emoji() {
        let i = inline("try <sound:9:77> now");
        assert_eq!(i.text, "try 🎵 Chime now");
        let sound = i.spans.iter().find(|s| s.style.has(Style::SOUNDBOARD)).unwrap();
        assert_eq!(&i.text[sound.range.clone()], "🎵 Chime");
        assert!(sound.mention.is_none());
    }

    #[test]
    fn unresolved_soundboard_mention_stays_literal() {
        let i = inline("<sound:9:88>");
        assert_eq!(i.text, "<sound:9:88>");
        assert!(!i.spans.iter().any(|s| s.style.has(Style::SOUNDBOARD)));
    }

    #[test]
    fn bold_italic_nested() {
        let i = inline("a **b *c* d** e");
        assert_eq!(i.text, "a b c d e");
        assert_eq!(
            styled(&i),
            vec![
                ("a ", Style::PLAIN),
                ("b ", Style::BOLD),
                ("c", Style::BOLD | Style::ITALIC),
                (" d", Style::BOLD),
                (" e", Style::PLAIN),
            ]
        );
    }

    #[test]
    fn underline_strike_spoiler() {
        let i = inline("__u__ ~~s~~ ||x||");
        assert_eq!(i.text, "u s x");
        assert_eq!(i.spans[0].style, Style::UNDERLINE);
        assert_eq!(i.spans[2].style, Style::STRIKE);
        assert_eq!(i.spans[4].style, Style::SPOILER);
        assert_eq!(i.spans[4].spoiler, Some(0));
    }

    #[test]
    fn inline_code_is_literal() {
        let i = inline("run `**not bold**` now");
        assert_eq!(i.text, "run **not bold** now");
        assert_eq!(styled(&i)[1], ("**not bold**", Style::CODE));
    }

    #[test]
    fn snake_case_not_italic() {
        let i = inline("my_var_name and _it_");
        assert_eq!(i.text, "my_var_name and it");
        assert_eq!(styled(&i).last().unwrap(), &("it", Style::ITALIC));
    }

    #[test]
    fn unmatched_delimiters_stay() {
        let i = inline("2 * 3 = 6 and **open");
        assert_eq!(i.text, "2 * 3 = 6 and **open");
        assert_eq!(i.spans.len(), 1);
    }

    #[test]
    fn links_bare_and_masked() {
        let i = inline("see https://example.com/a, or [docs](https://docs.rs).");
        assert_eq!(i.text, "see https://example.com/a, or docs.");
        let links: Vec<_> = i.spans.iter().filter_map(|s| s.link.as_deref()).collect();
        assert_eq!(links, vec!["https://example.com/a", "https://docs.rs"]);
        assert_eq!(clickable(&i).count(), 2);
    }

    #[test]
    fn masked_link_label_keeps_markdown() {
        let i = inline("[**Burn** by **Usher**](https://x.y)");
        assert_eq!(i.text, "Burn by Usher");
        assert!(i.spans.iter().all(|s| s.link.as_deref() == Some("https://x.y")));
        assert!(i.spans.iter().any(|s| s.style.has(Style::BOLD)));
    }

    #[test]
    fn mentions() {
        let i = inline("hi <@42> in <#7>, <@!99> @everyone");
        assert_eq!(i.text, "hi @wumpus in #general, @Unknown user @everyone");
        let m: Vec<_> = i.spans.iter().filter_map(|s| s.mention.clone()).collect();
        assert_eq!(
            m,
            vec![Mention::User(42), Mention::Channel(7), Mention::User(99), Mention::Everyone]
        );
    }

    #[test]
    fn escapes() {
        let i = inline(r"\*not italic\*");
        assert_eq!(i.text, "*not italic*");
        assert_eq!(i.spans.len(), 1);
    }

    #[test]
    fn code_block_with_lang() {
        let b = parse("before\n```rust\nfn main() {}\n```\nafter", &R);
        assert_eq!(b.len(), 3);
        assert!(matches!(&b[0], Block::Paragraph(i) if i.text == "before"));
        assert_eq!(
            b[1],
            Block::Code { lang: Some("rust".into()), code: "fn main() {}".into() }
        );
        assert!(matches!(&b[2], Block::Paragraph(i) if i.text == "after"));
    }

    #[test]
    fn code_block_inline_fence() {
        let b = parse("```just code```", &R);
        assert_eq!(b, vec![Block::Code { lang: None, code: "just code".into() }]);
    }

    #[test]
    fn quotes() {
        let b = parse("> quoted **line**\n> two\nnormal", &R);
        assert_eq!(b.len(), 2);
        match &b[0] {
            Block::Quote(i) => {
                assert_eq!(i.text, "quoted line\ntwo");
                assert!(i.spans.iter().any(|s| s.style == Style::BOLD));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn multiline_quoted_rules_preserve_formatting_and_emoji() {
        let blocks = parse(">>> - **Important** <a:badge:123>\n- Next entry\nlast line", &NoResolve);
        let [Block::Quote(inline)] = blocks.as_slice() else { panic!("{blocks:?}") };
        assert_eq!(inline.text, "• Important <a:badge:123>\n• Next entry\nlast line");
        assert!(inline.spans.iter().any(|span| span.style.has(Style::BOLD)));
        let rich = crate::richtext::rich_inline(inline, &|_| true, gpui::black());
        assert_eq!(rich.emoji[0].1.as_ref(), "<a:badge:123>");
    }

    #[test]
    fn multiple_spoilers_are_numbered() {
        let b = parse("||a|| and ||b||", &NoResolve);
        let Block::Paragraph(i) = &b[0] else { panic!() };
        let ids: Vec<_> = i.spans.iter().filter_map(|s| s.spoiler).collect();
        assert_eq!(ids, vec![0, 1]);
    }

    #[test]
    fn unicode_safe() {
        let i = inline("héllo **wörld** 🎉 _ok_");
        assert_eq!(i.text, "héllo wörld 🎉 ok");
    }

    #[test]
    fn headings_lists_subtext() {
        let b = parse("# Big\n-# small\n- one\n  - two\n3. three\nplain", &NoResolve);
        assert!(matches!(&b[0], Block::Heading(1, i) if i.text == "Big"));
        assert!(matches!(&b[1], Block::Subtext(i) if i.text == "small"));
        assert!(matches!(&b[2], Block::ListItem { ordered: None, depth: 0, inline } if inline.text == "one"));
        assert!(matches!(&b[3], Block::ListItem { ordered: None, depth: 1, .. }));
        assert!(matches!(&b[4], Block::ListItem { ordered: Some(3), .. }));
        assert!(matches!(&b[5], Block::Paragraph(i) if i.text == "plain"));
    }

    #[test]
    fn timestamps_and_masked_links() {
        let i = inline("at <t:0:D> see [x](<https://a.b>)");
        assert!(i.spans.iter().any(|s| s.style.has(Style::TIMESTAMP)));
        assert!(i.spans.iter().any(|s| s.link.as_deref() == Some("https://a.b")));
        assert_eq!(relative(-120), "2 minutes ago");
        assert_eq!(relative(3600), "in an hour");
    }

    #[test]
    fn bare_urls_follow_the_client_regex() {
        let links = |src: &str| -> Vec<(String, Option<String>)> {
            let b = parse(src, &R);
            let i = b[0].inline().unwrap();
            i.spans.iter().filter(|s| s.style.has(Style::LINK)).map(|s| (i.text[s.range.clone()].to_owned(), s.link.clone())).collect()
        };
        let one = |u: &str| vec![(u.to_owned(), Some(u.to_owned()))];
        assert_eq!(links("voir https://a.b/c."), one("https://a.b/c"));
        assert_eq!(links("ok? https://a.b/c?x=1!"), one("https://a.b/c?x=1!"));
        assert_eq!(links("(https://a.b/c)"), one("https://a.b/c"));
        assert_eq!(links("https://en.wikipedia.org/wiki/A_(b)"), one("https://en.wikipedia.org/wiki/A_(b)"));
        assert_eq!(links("\"https://a.b/c\""), one("https://a.b/c"));
        assert_eq!(links("https://a.b/c>d"), one("https://a.b/c>d"));
        assert!(links("steam://run/440").is_empty());
    }

    #[test]
    fn angle_bracket_autolinks_drop_their_brackets() {
        let b = parse("lien <https://a.b/c> fin", &R);
        let i = b[0].inline().unwrap();
        assert_eq!(i.text, "lien https://a.b/c fin");
        assert!(i.spans.iter().any(|s| s.link.as_deref() == Some("https://a.b/c") && &i.text[s.range.clone()] == "https://a.b/c"));
        let b = parse("<ftp://a.b/c>", &R);
        let i = b[0].inline().unwrap();
        assert_eq!(i.text, "ftp://a.b/c");
        assert!(i.spans.iter().all(|s| s.link.is_none()));
        let b = parse("<pas un lien>", &R);
        assert_eq!(b[0].inline().unwrap().text, "<pas un lien>");
    }

    #[test]
    fn single_link_content() {
        assert!(is_single_link("https://tenor.com/view/cat-gif-123"));
        assert!(is_single_link("[**chat**](https://tenor.com/view/cat-gif-123)"));
        assert!(is_single_link("<https://tenor.com/view/cat-gif-123>"));
        assert!(!is_single_link("https://tenor.com/a https://tenor.com/b"));
        assert!(!is_single_link("https://tenor.com/a\nhttps://tenor.com/b"));
        assert!(!is_single_link("regarde https://tenor.com/a"));
        assert!(!is_single_link(" https://tenor.com/a"));
        assert!(!is_single_link("`https://tenor.com/a`"));
        assert!(!is_single_link(""));
    }

    #[test]
    fn masked_link_takes_the_nearest_bracket() {
        let src = "[Test Phoenix] envoyé depuis le client officiel — surlignage de recherche : phxsurligne **phxsurligne** *phxsurligne* __phxsurligne__ ~~phxsurligne~~ [phxsurligne](https://example.com/a) `phxsurligne` ||phxsurligne|| <#7> fire 🔥 https://example.com/phxsurligne";
        let i = inline(src);
        assert!(i.text.starts_with("[Test Phoenix] envoyé"), "{}", i.text);
        assert_eq!(
            i.text,
            "[Test Phoenix] envoyé depuis le client officiel — surlignage de recherche : phxsurligne phxsurligne phxsurligne phxsurligne phxsurligne phxsurligne phxsurligne phxsurligne #general fire 🔥 https://example.com/phxsurligne"
        );
        let first = &i.spans[0];
        assert!(first.link.is_none() && !first.style.has(Style::LINK));
        let links: Vec<_> = i.spans.iter().filter_map(|s| s.link.as_deref().map(|l| (&i.text[s.range.clone()], l))).collect();
        assert_eq!(links, vec![("phxsurligne", "https://example.com/a"), ("https://example.com/phxsurligne", "https://example.com/phxsurligne")]);
        let code: Vec<_> = styled(&i).into_iter().filter(|(_, st)| st.has(Style::CODE)).map(|(t, _)| t).collect();
        assert_eq!(code, vec!["phxsurligne"]);
        let at = i.text.find("phxsurligne phxsurligne phxsurligne #").unwrap();
        assert!(i.spans.iter().any(|s| s.range.start == at && s.link.is_some()));
    }

    #[test]
    fn rejected_masked_links_stay_literal() {
        for src in ["[a <@42>](https://x.y)", "[www.evil.com](https://x.y)", "[go **now**](javascript:alert(1))", "[🔥](https://x.y)", "[ ](https://x.y)"] {
            let i = inline(src);
            assert_eq!(i.text, src);
            assert_eq!(styled(&i), vec![(src, Style::PLAIN)], "{src}");
        }
        let b = parse_emoji_links("[🔥 hot](https://x.y)", &R);
        let i = b[0].inline().unwrap();
        assert!(i.spans.iter().all(|s| s.link.as_deref() == Some("https://x.y")));
        let i = inline("[a [b] c](https://x.y/w_(b)) d");
        assert_eq!(i.text, "a [b] c d");
        assert_eq!(i.spans[0].link.as_deref(), Some("https://x.y/w_(b)"));
        let i = inline("[a\nb](https://x.y)");
        assert!(i.spans.iter().all(|s| s.link.is_some()));
    }
}
