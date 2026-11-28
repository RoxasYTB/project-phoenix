use gpui::SharedString;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Id(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Presence {
    Online,
    Idle,
    Dnd,
    #[default]
    Offline,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImageRef {
    Default(u8),
    Url(SharedString),
}

#[derive(Clone, Debug)]
pub struct User {
    pub id: Id,
    pub name: SharedString,
    pub display_name: SharedString,
    pub avatar: ImageRef,
    pub bot: bool,
    pub verified_bot: bool,
    pub tag: Option<(SharedString, Option<SharedString>)>,
    pub tag_guild: Option<Id>,
    pub nameplate: Option<(SharedString, Option<u32>)>,
    pub name_style: Option<(u32, u32, Vec<u32>)>,
    pub decoration: Option<SharedString>,
    pub flags: u64,
}

impl User {
    pub const PROVISIONAL_ACCOUNT: u64 = 1 << 23;

    pub fn is_provisional(&self) -> bool {
        self.flags & Self::PROVISIONAL_ACCOUNT != 0
    }
}

#[derive(Clone, Debug)]
pub struct Guild {
    pub id: Id,
    pub name: SharedString,
    pub acronym: SharedString,
    pub icon: ImageRef,
    pub unread: bool,
    pub mentions: u32,
    pub banner_color: Option<u32>,
    pub owner: Option<Id>,
    pub discoverable: bool,
    pub community: bool,
    pub boosts: u32,
    pub boost_tier: u8,
    pub afk_channel: Option<Id>,
    pub system_channel: Option<Id>,
    pub rules_channel: Option<Id>,
    pub member_count: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelKind {
    Text,
    Voice,
    Announcement,
    Forum,
    Stage,
    Category,
}

#[derive(Clone, Debug)]
pub struct Channel {
    pub id: Id,
    pub guild: Option<Id>,
    pub parent: Option<Id>,
    pub name: SharedString,
    pub topic: Option<SharedString>,
    pub kind: ChannelKind,
    pub position: i32,
    pub unread: bool,
    pub muted: bool,
    pub mentions: u32,
    pub voice_users: Vec<Id>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EmbedMedia {
    pub url: SharedString,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Embed {
    pub color: Option<u32>,
    pub provider: Option<SharedString>,
    pub title: Option<SharedString>,
    pub description: Option<SharedString>,
    pub url: Option<SharedString>,
    pub kind: Option<SharedString>,
    pub thumbnail: Option<EmbedMedia>,
    pub image: Option<EmbedMedia>,
    pub images: Vec<EmbedMedia>,
    pub video: Option<EmbedMedia>,
    pub author: Option<EmbedAuthor>,
    pub fields: Vec<EmbedField>,
    pub footer: Option<EmbedFooter>,
    pub timestamp: Option<i64>,
}

pub fn merge_embeds_on_url(embeds: Vec<Embed>) -> Vec<Embed> {
    let mut out: Vec<Embed> = Vec::with_capacity(embeds.len());
    let mut first: std::collections::HashMap<SharedString, usize> = Default::default();
    for e in embeds {
        let Some(url) = e.url.clone() else {
            out.push(e);
            continue;
        };
        match first.get(&url) {
            None => {
                first.insert(url, out.len());
                out.push(e);
            }
            Some(&i) => {
                if let Some(img) = e.image {
                    let head = &mut out[i];
                    if head.images.is_empty() {
                        head.images.extend(head.image.clone());
                    }
                    head.images.push(img);
                }
            }
        }
    }
    out
}

pub fn hides_simple_embed_content(content: &str, embeds: &[Embed]) -> bool {
    let [e] = embeds else { return false };
    let kind = e.kind.as_deref();
    matches!(kind, Some("image" | "gifv"))
        && (e.image.is_some() || e.video.is_some() || (kind == Some("image") && e.thumbnail.is_some()))
        && (kind == Some("gifv") || (e.author.is_none() && e.title.is_none()))
        && crate::markdown::is_single_link(content)
}

#[derive(Clone, Debug, Default)]
pub struct EmbedAuthor {
    pub name: SharedString,
    pub url: Option<SharedString>,
    pub icon: Option<SharedString>,
}

#[derive(Clone, Debug, Default)]
pub struct EmbedField {
    pub name: SharedString,
    pub value: SharedString,
    pub inline: bool,
}

#[derive(Clone, Debug, Default)]
pub struct EmbedFooter {
    pub text: SharedString,
    pub icon: Option<SharedString>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AttachmentKind {
    #[default]
    File,
    Image,
    Video,
    Sticker(crate::media::StickerFormat),
    Audio,
}

#[derive(Clone, Debug, Default)]
pub struct Attachment {
    pub filename: SharedString,
    pub url: SharedString,
    pub width: u32,
    pub height: u32,
    pub is_image: bool,
    pub size: u64,
    pub kind: AttachmentKind,
    pub original: SharedString,
    pub spoiler: bool,
    pub description: Option<SharedString>,
    pub voice: Option<VoiceMeta>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VoiceMeta {
    pub duration_secs: f64,
    pub waveform: SharedString,
}

impl Attachment {
    pub fn is_spoiler(&self) -> bool {
        self.spoiler || self.filename.starts_with("SPOILER_")
    }
}

#[derive(Clone, Debug, Default)]
pub struct Reaction {
    pub emoji: SharedString,
    pub count: u32,
    pub me: bool,
    pub emoji_id: Option<Id>,
    pub burst_count: u32,
    pub me_burst: bool,
    pub burst_colors: Vec<SharedString>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MessageKind {
    #[default]
    Default,
    Reply,
    Join,
    Pin,
    Boost(u8),
    ThreadCreated,
    RecipientAdd(Option<Id>),
    RecipientRemove(Option<Id>),
    Call,
    ChannelNameChange,
    StageStart,
    StageEnd,
    StageSpeaker,
    StageTopic,
    PollResult(Option<Id>),
    OtherSystem,
}

#[derive(Clone, Debug, Default)]
pub struct ThreadRef {
    pub id: Id,
    pub name: SharedString,
    pub message_count: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThreadInfo {
    pub parent: Option<Id>,
    pub owner: Option<Id>,
    pub private: bool,
    pub forum_post: bool,
    pub joined: bool,
    pub member_flags: u8,
    pub member_muted: bool,
    pub mute_end: Option<i64>,
    pub archived: bool,
    pub locked: bool,
    pub invitable: bool,
    pub auto_archive: u32,
    pub slowmode: u32,
    pub flags: u64,
    pub unread: bool,
    pub mentions: u32,
    pub member_count: u32,
    pub member_ids_preview: Vec<Id>,
    pub message_count: u32,
    pub last_message: Option<Id>,
}

impl ThreadInfo {
    pub fn notify_level(&self) -> u8 {
        if self.member_flags & 8 != 0 {
            8
        } else if self.member_flags & 4 != 0 {
            4
        } else {
            2
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Message {
    pub id: Id,
    pub channel: Id,
    pub author: Id,
    pub timestamp: i64,
    pub edited: bool,
    pub edited_at: i64,
    pub content: SharedString,
    pub reply_to: Option<Id>,
    pub reply_mentions_author: bool,
    pub reply_preview: Option<(Id, Option<SharedString>, SharedString)>,
    pub reply_voice: bool,
    pub forward: Option<Forward>,
    pub author_nick: Option<SharedString>,
    pub embeds: Vec<Embed>,
    pub attachments: Vec<Attachment>,
    pub soundboard_sounds: Vec<SoundboardSound>,
    pub reactions: Vec<Reaction>,
    pub kind: MessageKind,
    pub pinned: bool,
    pub mention_everyone: bool,
    pub thread: Option<ThreadRef>,
    pub nonce: Option<u64>,
    pub send_state: SendState,
    pub flags: u64,
    pub type_id: u8,
    pub webhook_name: Option<SharedString>,
    pub application_id: Option<Id>,
    pub has_mentions: bool,
    pub tts: bool,
    pub interaction: Option<CommandUse>,
    pub call: Option<CallInfo>,
    pub sys: Option<Box<SysData>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SysData {
    pub app_name: Option<SharedString>,
    pub tier_name: Option<SharedString>,
    pub months: u32,
    pub renewal: bool,
    pub product: Option<SharedString>,
    pub ref_channel: Option<Id>,
    pub ref_guild: Option<Id>,
    pub mention: Option<Id>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CallInfo {
    pub participants: Vec<Id>,
    pub duration_ms: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandUse {
    pub id: Id,
    pub user: Id,
    pub name: SharedString,
    pub command_type: u8,
    pub target: Option<Id>,
}

pub const FLAG_LOADING: u64 = 1 << 7;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SendState {
    #[default]
    Sent,
    Sending,
    Failed,
}

pub const FLAG_EPHEMERAL: u64 = 1 << 6;

#[derive(Clone, Debug, Default)]
pub struct SoundboardSound {
    pub id: Id,
    pub guild: Option<Id>,
    pub name: Option<SharedString>,
    pub emoji_id: Option<Id>,
    pub emoji_name: Option<SharedString>,
    pub volume: f32,
    pub available: bool,
    pub user: Option<Id>,
}

#[derive(Clone, Debug, Default)]
pub struct Forward {
    pub guild: Option<Id>,
    pub channel: Id,
    pub timestamp: i64,
}

#[derive(Clone, Debug)]
pub struct Role {
    pub id: Id,
    pub name: SharedString,
    pub color: Option<u32>,
    pub position: i32,
    pub hoist: bool,
    pub icon: Option<SharedString>,
    pub managed: bool,
    pub connection: bool,
}

#[derive(Clone, Debug)]
pub struct Member {
    pub user: Id,
    pub roles: Vec<Id>,
    pub presence: Presence,
    pub activity: Option<SharedString>,
    pub nick: Option<SharedString>,
    pub avatar: Option<SharedString>,
    pub decoration: Option<SharedString>,
    pub nameplate: Option<(SharedString, Option<u32>)>,
    pub joined_at: Option<i64>,
    pub rejoined: bool,
    pub premium_since: Option<i64>,
}

pub fn acronym(name: &str) -> SharedString {
    let name = name.replace("'s ", " ");
    let mut s = String::new();
    let mut in_word = false;
    for ch in name.chars() {
        let w = ch.is_ascii_alphanumeric() || ch == '_';
        if (w && !in_word) || (!w && !ch.is_whitespace()) {
            s.push(ch);
        }
        in_word = w;
    }
    s.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acronyms() {
        assert_eq!(acronym("Rust Programming Language").as_ref(), "RPL");
        assert_eq!(acronym("gpui").as_ref(), "g");
        assert_eq!(acronym("La Retaudière").as_ref(), "LRèr");
        assert_eq!(acronym("Roxas's World").as_ref(), "RW");
    }

    #[test]
    fn simple_media_embed_hides_its_link() {
        let media = || Some(EmbedMedia { url: "https://media.tenor.com/a.mp4".into(), width: 10, height: 10 });
        let gif = Embed { kind: Some("gifv".into()), url: Some("https://tenor.com/view/a".into()), video: media(), ..Default::default() };
        let tenor = "https://tenor.com/view/a";
        assert!(hides_simple_embed_content(tenor, std::slice::from_ref(&gif)));
        assert!(hides_simple_embed_content("https://klipy.com/gifs/b", std::slice::from_ref(&gif)));
        assert!(!hides_simple_embed_content("trop bien https://tenor.com/view/a", std::slice::from_ref(&gif)));
        assert!(!hides_simple_embed_content("https://tenor.com/view/a https://tenor.com/view/b", &[gif.clone(), gif.clone()]));
        assert!(!hides_simple_embed_content("https://tenor.com/view/a https://tenor.com/view/b", std::slice::from_ref(&gif)));
        let image = Embed { kind: Some("image".into()), image: media(), ..Default::default() };
        assert!(hides_simple_embed_content("https://i.imgur.com/a.png", std::slice::from_ref(&image)));
        let titled = Embed { title: Some("Titre".into()), ..image.clone() };
        assert!(!hides_simple_embed_content("https://i.imgur.com/a.png", &[titled]));
        let bare = Embed { kind: Some("gifv".into()), ..Default::default() };
        assert!(!hides_simple_embed_content(tenor, &[bare]));
        let rich = Embed { kind: Some("rich".into()), image: media(), ..Default::default() };
        assert!(!hides_simple_embed_content(tenor, &[rich]));
        assert!(!hides_simple_embed_content(tenor, &[]));
        let attachment_gif = Embed { kind: Some("image".into()), thumbnail: media(), ..Default::default() };
        let cdn = "https://cdn.discordapp.com/attachments/1571655566340915312/1571672456920432791/togif.gif";
        assert!(hides_simple_embed_content(cdn, std::slice::from_ref(&attachment_gif)));
        let klipy = Embed { kind: Some("gifv".into()), title: Some("Scuba Cat".into()), thumbnail: media(), video: media(), ..Default::default() };
        assert!(hides_simple_embed_content("https://klipy.com/gifs/scuba-scuba-cat", &[klipy]));
        let video_thumb = Embed { kind: Some("video".into()), thumbnail: media(), ..Default::default() };
        assert!(!hides_simple_embed_content(cdn, &[video_thumb]));
    }

    #[test]
    fn embeds_sharing_a_url_merge_their_images() {
        let media = |u: &str| EmbedMedia { url: u.to_string().into(), width: 10, height: 10 };
        let post = |img: &str, title: Option<&str>| Embed {
            url: Some("https://x.com/a/status/1".into()),
            title: title.map(|t| t.to_string().into()),
            image: Some(media(img)),
            ..Default::default()
        };
        let other = Embed { url: Some("https://example.com".into()), image: Some(media("o")), ..Default::default() };
        let bare = Embed { title: Some("no url".into()), ..Default::default() };
        let out = merge_embeds_on_url(vec![post("1", Some("post")), bare, post("2", None), other, post("3", None)]);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].images.iter().map(|m| m.url.as_ref()).collect::<Vec<_>>(), ["1", "2", "3"]);
        assert_eq!(out[1].title.as_deref(), Some("no url"));
        assert!(out[2].images.is_empty(), "a lone embed keeps its single image");
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VoiceFlags {
    pub self_mute: bool,
    pub self_deaf: bool,
    pub server_mute: bool,
    pub server_deaf: bool,
    pub stream: bool,
    pub video: bool,
    pub suppress: bool,
    pub undiscoverable: bool,
    pub hand: Option<i64>,
}
