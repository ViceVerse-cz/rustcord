//! Bounded, lazy native fallback. AppKit work never runs in a render callback.
use egui::{Context, Image, TextureHandle};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, mpsc},
};
use unicode_segmentation::UnicodeSegmentation;

const CAPACITY: usize = 128;
const QUEUE: usize = 16;
type Cache = Arc<Mutex<CacheState>>;
struct Entry {
    text: String,
    image: Option<TextureHandle>,
    frame: u64,
    pending: bool,
}
#[derive(Default)]
struct CacheState {
    entries: VecDeque<Entry>,
    deferred: bool,
}
impl CacheState {
    fn evictable(&self, frame: u64) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| !entry.pending && entry.frame < frame)
    }
    fn finish(&mut self, text: &str, image: Option<TextureHandle>, frame: u64) -> bool {
        let mut changed = false;
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.text == text) {
            changed = image.is_some();
            entry.image = image;
            entry.pending = false;
        }
        // Retry queue overflow once the batch drains, not every failed glyph.
        let retry = self.deferred
            && !self.entries.iter().any(|entry| entry.pending)
            && (self.entries.len() < CAPACITY || self.evictable(frame).is_some());
        if retry {
            self.deferred = false;
        }
        changed || retry
    }
}
#[derive(Clone)]
struct Native {
    cache: Cache,
    requests: mpsc::SyncSender<(String, Context)>,
}

pub(super) fn install(ctx: &Context) {
    let cache: Cache = Default::default();
    let worker_cache = cache.clone();
    let (requests, receiver) = mpsc::sync_channel::<(String, Context)>(QUEUE);
    if std::thread::Builder::new()
        .name("native-emoji".into())
        .spawn(move || {
            while let Ok((text, ctx)) = receiver.recv() {
                let image = super::native_macos::raster(&text);
                let Ok(mut cache) = worker_cache.lock() else {
                    break;
                };
                let image = image.map(|image| {
                    ctx.load_texture("native emoji", image, egui::TextureOptions::LINEAR)
                });
                let repaint = cache.finish(&text, image, ctx.cumulative_frame_nr());
                drop(cache);
                if repaint {
                    ctx.request_repaint();
                }
            }
        })
        .is_ok()
    {
        ctx.data_mut(|data| {
            data.insert_temp(egui::Id::new("native emoji"), Native { cache, requests })
        });
    }
}

fn candidate(text: &str) -> bool {
    text.len() <= 128 && !text.contains('\u{fe0e}') && text.graphemes(true).count() == 1
        && text.chars().next().is_some_and(|c| matches!(c as u32, 0x2300..=0x27ff | 0x1f000..=0x1faff | 0xa9 | 0xae | 0x203c | 0x2049 | 0x2122 | 0x2139 | 0x2194..=0x21ff | 0x2b00..=0x2bff | 0x3030 | 0x303d | 0x3297 | 0x3299))
}

pub(super) fn image(ctx: &Context, text: &str, size: f32) -> Option<Image<'static>> {
    if !candidate(text) {
        return None;
    }
    let native = ctx.data(|data| data.get_temp::<Native>(egui::Id::new("native emoji")))?;
    let frame = ctx.cumulative_frame_nr();
    let mut cache = native.cache.lock().ok()?;
    if let Some(entry) = cache.entries.iter_mut().find(|entry| entry.text == text) {
        entry.frame = frame;
        return entry
            .image
            .as_ref()
            .map(|image| Image::new((image.id(), egui::Vec2::splat(size))).alt_text(text));
    }
    let evict = if cache.entries.len() == CAPACITY {
        // Pin entries already visited this frame: a stable viewport larger than
        // the cache settles on its first 128 entries instead of evicting itself.
        Some(cache.evictable(frame)?)
    } else {
        None
    };
    match native.requests.try_send((text.to_owned(), ctx.clone())) {
        Ok(()) => {
            if let Some(index) = evict {
                cache.entries.remove(index);
            }
            cache.entries.push_back(Entry {
                text: text.to_owned(),
                image: None,
                frame,
                pending: true,
            });
        }
        Err(mpsc::TrySendError::Full(_)) => cache.deferred = true,
        Err(mpsc::TrySendError::Disconnected(_)) => {}
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_requests_keep_complete_bounded_emoji_graphemes() {
        for text in ["👍", "👩🏽‍💻", "🫪"] {
            assert!(candidate(text));
        }
        for text in [
            "a",
            "",
            "hello",
            "👍👍",
            "©\u{fe0e}",
            &"👩\u{200d}".repeat(30),
        ] {
            assert!(!candidate(text));
        }
        assert_eq!(CAPACITY * 64 * 64 * 4, 2 * 1024 * 1024);
        assert_eq!(QUEUE * 128, 2048);
    }

    #[test]
    fn oversized_visible_set_settles_and_navigation_reuses_cache() {
        let ctx = Context::default();
        let cache: Cache = Default::default();
        let (requests, receiver) = mpsc::sync_channel(QUEUE);
        ctx.data_mut(|data| {
            data.insert_temp(
                egui::Id::new("native emoji"),
                Native {
                    cache: cache.clone(),
                    requests,
                },
            )
        });
        let names: Vec<_> = (0..CAPACITY + 10)
            .map(|i| char::from_u32(0x1f600 + i as u32).unwrap().to_string())
            .collect();
        for text in &names {
            image(&ctx, text, 18.0);
        }
        assert_eq!(cache.lock().unwrap().entries.len(), QUEUE);
        let queued: Vec<_> = receiver.try_iter().collect();
        for (index, (text, _)) in queued.iter().enumerate() {
            assert_eq!(
                cache.lock().unwrap().finish(text, None, 0),
                index + 1 == QUEUE
            );
        }
        // Fill subsequent batches: each failure is memoized, never re-enqueued.
        for text in &names {
            image(&ctx, text, 18.0);
            for (text, _) in receiver.try_iter() {
                cache.lock().unwrap().finish(&text, None, 0);
            }
        }
        assert_eq!(cache.lock().unwrap().entries.len(), CAPACITY);
        for _ in 0..4 {
            ctx.run_ui(Default::default(), |ui| {
                for text in &names {
                    image(ui.ctx(), text, 18.0);
                }
            })
            .drop_without_applying_deltas();
            assert_eq!(
                receiver.try_iter().count(),
                0,
                "visible overflow must stop repaint work"
            );
        }
        // The next viewport can immediately reclaim entries from the prior frame.
        image(&ctx, names.last().unwrap(), 18.0);
        assert!(receiver.try_recv().is_ok());
        let cache = cache.lock().unwrap();
        assert_eq!(cache.entries.len(), CAPACITY);
        assert!(cache.entries.iter().all(|entry| entry.text.len() <= 128));
    }
}
