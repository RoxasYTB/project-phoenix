use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use gpui::{AnyElement, App, AppContext, Entity, Global, ImageCache, ImageCacheError, ImageCacheItem, IntoElement, ParentElement, RenderImage, Resource, Styled, WeakEntity, Window, hash};

pub const BUDGET_BYTES: u64 = 80 * 1024 * 1024;
pub const RECENT_EXTRA_BYTES: u64 = 120 * 1024 * 1024;
const RECENT_CONTEXTS: usize = 14;
const KEEP_FRAMES: u64 = 2;
const KEEP_FOR: Duration = Duration::from_secs(5);
const MAX_FAILED_ITEMS: usize = 512;
const SWEEP_EVERY: u64 = 64;

static FRAME: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static CONTEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static RECENT: RefCell<VecDeque<u64>> = const { RefCell::new(VecDeque::new()) };
}

pub fn next_frame(context: u64) {
    FRAME.fetch_add(1, Ordering::Relaxed);
    if CONTEXT.replace(context) != context && context != 0 {
        RECENT.with_borrow_mut(|r| {
            r.retain(|c| *c != context);
            r.push_back(context);
            while r.len() > RECENT_CONTEXTS {
                r.pop_front();
            }
        });
    }
}

fn recent_rank(context: u64) -> Option<u64> {
    if context == 0 {
        return None;
    }
    RECENT.with_borrow(|r| r.iter().position(|c| *c == context).map(|i| i as u64 + 1))
}

fn env_mb(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().and_then(|v| v.parse::<u64>().ok()).map_or(default, |mb| mb * 1024 * 1024)
}

fn frame() -> u64 {
    FRAME.load(Ordering::Relaxed)
}

struct Entry {
    item: ImageCacheItem,
    frame: u64,
    used: Instant,
    context: u64,
    bytes: Option<u64>,
}

pub struct LruImageCache {
    items: HashMap<u64, Entry>,
    loads: u64,
    budget: u64,
    recent_extra: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Tier {
    Old,
    Recent(u64),
    Fresh,
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    tier: Tier,
    frame: u64,
    key: u64,
    bytes: u64,
}

fn bytes_of(img: &RenderImage) -> u64 {
    (0..img.frame_count())
        .map(|i| {
            let s = img.size(i);
            s.width.0.max(0) as u64 * s.height.0.max(0) as u64 * 4
        })
        .sum()
}

impl LruImageCache {
    pub fn new(budget: u64, cx: &mut App) -> Entity<Self> {
        let budget = env_mb("PHX_IMAGE_BUDGET_MB", budget);
        let recent_extra = env_mb("PHX_IMAGE_RECENT_MB", RECENT_EXTRA_BYTES);
        let e = cx.new(|_| Self { items: HashMap::new(), loads: 0, budget, recent_extra });
        cx.observe_release(&e, |c, cx| {
            for (_, e) in c.items.drain() {
                if let Some(Ok(img)) = e.item.get() {
                    cx.drop_image(img, None);
                }
            }
        })
        .detach();
        e
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn evict(&mut self, protected: u64, window: &mut Window, cx: &mut App) {
        let mut entries: Vec<Candidate> = Vec::with_capacity(self.items.len());
        let fresh = frame().saturating_sub(KEEP_FRAMES);
        for (k, e) in &mut self.items {
            if e.bytes.is_none()
                && let Some(Ok(img)) = e.item.get()
            {
                e.bytes = Some(bytes_of(&img));
            }
            if let Some(bytes) = e.bytes {
                let tier = if *k == protected || e.frame >= fresh || e.used.elapsed() < KEEP_FOR {
                    Tier::Fresh
                } else {
                    recent_rank(e.context).map_or(Tier::Old, Tier::Recent)
                };
                entries.push(Candidate { tier, frame: e.frame, key: *k, bytes });
            }
        }
        let victims = select_evictions(&mut entries, self.budget, self.recent_extra);
        for k in victims {
            if let Some(e) = self.items.remove(&k)
                && let Some(Ok(img)) = e.item.get()
            {
                cx.drop_image(img, Some(window));
            }
        }
        if self.items.len() > MAX_FAILED_ITEMS {
            self.items.retain(|k, e| *k == protected || e.frame >= fresh || !matches!(e.item.get(), Some(Err(_))));
        }
    }
}

fn select_evictions(entries: &mut [Candidate], budget: u64, recent_extra: u64) -> Vec<u64> {
    let mut used: u64 = entries.iter().map(|c| c.bytes).sum();
    if used <= budget {
        return Vec::new();
    }
    entries.sort_unstable_by_key(|c| (c.tier, c.frame, c.key));
    let mut victims = Vec::new();
    let mut pass = |tier: fn(Tier) -> bool, limit: u64, used: &mut u64| {
        let target = limit * 9 / 10;
        for c in entries.iter().filter(|c| tier(c.tier)) {
            if *used <= target {
                break;
            }
            *used = used.saturating_sub(c.bytes);
            victims.push(c.key);
        }
    };
    pass(|t| t == Tier::Old, budget, &mut used);
    if used > budget + recent_extra {
        pass(|t| matches!(t, Tier::Recent(_)), budget + recent_extra, &mut used);
    }
    victims
}

impl ImageCache for LruImageCache {
    fn load(&mut self, resource: &Resource, window: &mut Window, cx: &mut App) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        self.loads += 1;
        let now = frame();
        let key = hash(resource);
        let inserted = !self.items.contains_key(&key);
        let context = CONTEXT.get();
        let entry = self.items.entry(key).or_insert_with(|| Entry { item: ImageCacheItem::new(resource, cx), frame: now, used: Instant::now(), context, bytes: None });
        entry.frame = now;
        entry.used = Instant::now();
        entry.context = context;
        let out = entry.item.use_image(window);
        if inserted || self.loads.is_multiple_of(SWEEP_EVERY) {
            self.evict(key, window, cx);
        }
        out
    }
}

struct SharedCache(#[allow(dead_code)] Entity<LruImageCache>);
impl Global for SharedCache {}

thread_local! {
    static SHARED: RefCell<Option<WeakEntity<LruImageCache>>> = const { RefCell::new(None) };
}

pub fn init(cx: &mut App) {
    let c = LruImageCache::new(BUDGET_BYTES, cx);
    SHARED.with_borrow_mut(|s| *s = Some(c.downgrade()));
    cx.set_global(SharedCache(c));
}

pub fn chat() -> Option<Entity<LruImageCache>> {
    SHARED.with_borrow(|s| s.as_ref()?.upgrade())
}

pub fn cached(image: gpui::Img) -> gpui::Img {
    match chat() {
        Some(cache) => image.image_cache(&cache),
        None => image,
    }
}

pub fn scope(child: impl IntoElement) -> AnyElement {
    match chat() {
        Some(cache) => gpui::image_cache(cache).size_full().child(child).into_any_element(),
        None => child.into_any_element(),
    }
}

pub fn deferred(child: impl IntoElement) -> gpui::Deferred {
    match chat() {
        Some(cache) => gpui::deferred(gpui::image_cache(cache).child(child)),
        None => gpui::deferred(child),
    }
}

pub fn deferred_full(child: impl IntoElement) -> gpui::Deferred {
    match chat() {
        Some(cache) => gpui::deferred(gpui::image_cache(cache).absolute().top_0().left_0().size_full().child(child)),
        None => gpui::deferred(child),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(tier: Tier, frame: u64, key: u64, bytes: u64) -> Candidate {
        Candidate { tier, frame, key, bytes }
    }

    #[test]
    fn evicts_oldest_first_when_over_budget() {
        let mut e = vec![c(Tier::Old, 1, 10, 50), c(Tier::Old, 2, 20, 50), c(Tier::Old, 3, 30, 50)];
        assert_eq!(select_evictions(&mut e, 100, 0), vec![10, 20]);
    }

    #[test]
    fn never_evicts_fresh_images() {
        let mut e = vec![c(Tier::Fresh, 100, 1, 60), c(Tier::Fresh, 101, 2, 60), c(Tier::Fresh, 102, 3, 60)];
        assert!(select_evictions(&mut e, 100, 0).is_empty());
        let mut e = vec![c(Tier::Old, 90, 1, 60), c(Tier::Fresh, 101, 2, 60), c(Tier::Fresh, 102, 3, 60)];
        assert_eq!(select_evictions(&mut e, 100, 0), vec![1]);
    }

    #[test]
    fn keeps_recent_channels_within_their_extra_budget() {
        let mut e = vec![c(Tier::Recent(1), 1, 1, 60), c(Tier::Old, 5, 2, 60), c(Tier::Recent(2), 2, 3, 60)];
        assert_eq!(select_evictions(&mut e, 100, 100), vec![2]);
        let mut e = vec![c(Tier::Recent(1), 1, 1, 60), c(Tier::Recent(2), 2, 3, 60), c(Tier::Recent(3), 3, 4, 60)];
        assert_eq!(select_evictions(&mut e, 50, 50), vec![1, 3]);
    }

    #[test]
    fn keeps_everything_under_budget() {
        let mut e = vec![c(Tier::Old, 1, 10, 40), c(Tier::Old, 2, 20, 40)];
        assert!(select_evictions(&mut e, 100, 0).is_empty());
    }
}
