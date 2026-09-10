//! Credential-free, viewport-driven static avatars. No tokens enter this worker.
use eframe::egui;
use model::Id;
use sha2::{Digest, Sha256};
use std::{
    collections::BinaryHeap,
    fs::{self, OpenOptions},
    io::{self, Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::{mpsc as async_mpsc, watch};

const MAX_ENCODED: usize = 2 * 1024 * 1024;
const MAX_AVATAR_ENCODED: usize = 512 * 1024;
const MAX_DISK: u64 = 1024 * 1024 * 1024;
const MAX_FILES: usize = 4096;
const RETENTION: Duration = Duration::from_secs(90 * 24 * 60 * 60);
const CACHE_ERROR: &str = "Images could not be cached; they remain available in memory";
pub type Cleanup = mpsc::Receiver<Result<(), &'static str>>;

pub struct AvatarResult {
    pub key: String,
    pub image: Option<egui::ColorImage>,
    pub error: Option<&'static str>,
}

pub struct AvatarWorker {
    requests: async_mpsc::Sender<String>,
    results: async_mpsc::Receiver<AvatarResult>,
    cancel: watch::Sender<bool>,
    clear: Arc<AtomicBool>,
    cleanup: Option<Cleanup>,
}
impl AvatarWorker {
    pub fn start(
        runtime: &tokio::runtime::Runtime,
        account: Id,
        ctx: egui::Context,
    ) -> Result<Self, &'static str> {
        let root = dirs::data_local_dir().map(|root| {
            root.join("serein")
                .join("avatars")
                .join(account.to_string())
        });
        Self::start_at(runtime, root, ctx)
    }
    fn start_at(
        runtime: &tokio::runtime::Runtime,
        root: Option<PathBuf>,
        ctx: egui::Context,
    ) -> Result<Self, &'static str> {
        let (requests, receive) = async_mpsc::channel(128);
        let (send, results) = async_mpsc::channel(2);
        let (cancel, cancelled) = watch::channel(false);
        let clear = Arc::new(AtomicBool::new(false));
        let cleanup_flag = clear.clone();
        let (done, cleanup) = mpsc::sync_channel(1);
        let handle = runtime.handle().clone();
        std::thread::Builder::new()
            .name("avatar-cache".into())
            .spawn(move || {
                handle.block_on(run(root.as_deref(), receive, send, cancelled, &ctx));
                let result = if cleanup_flag.load(Ordering::Acquire) {
                    clear_directory(root.as_deref())
                } else {
                    Ok(())
                };
                let _ = done.send(result);
                ctx.request_repaint();
            })
            .map_err(|_| "Could not start image worker")?;
        Ok(Self {
            requests,
            results,
            cancel,
            clear,
            cleanup: Some(cleanup),
        })
    }
    pub fn request(&self, key: String) -> bool {
        cdn_url(&key).is_some() && self.requests.try_send(key).is_ok()
    }
    pub fn poll(&mut self) -> Option<AvatarResult> {
        self.results.try_recv().ok()
    }
    /// Completion follows the last decode/write. Recreate the account worker only afterwards.
    pub fn shutdown_and_clear(self) -> Cleanup {
        self.finish(true)
    }
    pub fn shutdown(self) -> Cleanup {
        self.finish(false)
    }
    fn finish(mut self, clear: bool) -> Cleanup {
        self.clear.store(clear, Ordering::Release);
        self.cancel.send_replace(true);
        self.cleanup
            .take()
            .expect("worker owns its cleanup receiver")
    }
}
impl Drop for AvatarWorker {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
    }
}

fn clear_directory(root: Option<&Path>) -> Result<(), &'static str> {
    let root = root.ok_or("Could not locate cached images for removal")?;
    match fs::remove_dir_all(root) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Could not remove cached images; files may remain on disk"),
    }
}

// Build, rather than accept, URLs. Even malformed service metadata cannot choose a host/path.
fn cdn_url(key: &str) -> Option<String> {
    if let Some(id) = key.strip_prefix("emoji-") {
        let id: Id = id.parse().ok()?;
        return Some(format!(
            "https://cdn.discordapp.com/emojis/{id}.png?size=64"
        ));
    }
    if let Some(value) = key.strip_prefix("banner-") {
        let (id, hash) = value.split_once('-')?;
        let id: Id = id.parse().ok()?;
        return model::valid_avatar_hash(hash)
            .then(|| format!("https://cdn.discordapp.com/banners/{id}/{hash}.png?size=512"));
    }
    for (prefix, kind, size) in [
        ("member-avatar-", "avatars", 128),
        ("member-banner-", "banners", 512),
    ] {
        if let Some(value) = key.strip_prefix(prefix) {
            let (guild, value) = value.split_once('-')?;
            let (user, hash) = value.split_once('-')?;
            let guild: Id = guild.parse().ok()?;
            let user: Id = user.parse().ok()?;
            return model::valid_avatar_hash(hash).then(||format!("https://cdn.discordapp.com/guilds/{guild}/users/{user}/{kind}/{hash}.png?size={size}"));
        }
    }
    if let Some(source) = key.strip_prefix("embed:") {
        return embed_url(source);
    }
    if let Some(icon) = key.strip_prefix("guild-") {
        let (id, hash) = icon.split_once('-')?;
        let id: Id = id.parse().ok()?;
        return model::valid_avatar_hash(hash)
            .then(|| format!("https://cdn.discordapp.com/icons/{id}/{hash}.png?size=128"));
    }
    if let Some(index) = key.strip_prefix("default-") {
        return (index.len() == 1 && matches!(index.as_bytes()[0], b'0'..=b'5'))
            .then(|| format!("https://cdn.discordapp.com/embed/avatars/{index}.png"));
    }
    let (id, hash) = key.split_once('-')?;
    let id: Id = id.parse().ok()?;
    let digest = hash.strip_prefix("a_").unwrap_or(hash);
    (digest.len() == 32 && digest.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| format!("https://cdn.discordapp.com/avatars/{id}/{hash}.png?size=128"))
}

// Only service-provided image objects reach this path. Never fetch an arbitrary embed source.
fn embed_url(source: &str) -> Option<String> {
    if source.len() > 2048 || source.bytes().any(|b| b.is_ascii_control() || b == b'\\') {
        return None;
    }
    let mut url = url::Url::parse(source).ok()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    let host = url.host_str()?;
    let path = url.path();
    let valid_path = if path.starts_with("/attachments/") {
        let mut parts = path.trim_start_matches('/').split('/');
        parts.next();
        parts.next()?.parse::<Id>().is_ok()
            && parts.next()?.parse::<Id>().is_ok()
            && parts.next().is_some_and(|name| !name.is_empty())
            && parts.next().is_none()
    } else if path.starts_with("/external/") {
        let mut parts = path.trim_start_matches('/').split('/');
        parts.next();
        parts.next().is_some_and(|hash| {
            (16..=256).contains(&hash.len())
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        }) && matches!(parts.next(), Some("https" | "http"))
            && parts.next().is_some_and(|domain| !domain.is_empty())
    } else {
        let parts: Vec<_> = path.trim_start_matches('/').split('/').collect();
        matches!(parts.as_slice(), ["avatars" | "icons", id, hash]
            if id.parse::<Id>().is_ok() && hash.rsplit_once('.').is_some_and(|(hash, _)| model::valid_avatar_hash(hash)))
            || matches!(parts.as_slice(), ["embed", "avatars", index]
                if matches!(*index, "0.png" | "1.png" | "2.png" | "3.png" | "4.png" | "5.png"))
    };
    if !valid_path
        || !matches!(
            host,
            "cdn.discordapp.com"
                | "media.discordapp.net"
                | "images-ext-1.discordapp.net"
                | "images-ext-2.discordapp.net"
        )
    {
        return None;
    }
    if host == "cdn.discordapp.com" {
        url.set_host(Some("media.discordapp.net")).ok()?;
    }
    // Static proxy conversion is unofficial. A rejected/unsupported format stays a placeholder;
    // do not follow redirects, contact the original host, or add animation decoders as fallback.
    let query: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| {
            !matches!(
                key.as_ref(),
                "format" | "width" | "height" | "quality" | "animated"
            )
        })
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    url.set_query(None);
    url.query_pairs_mut()
        .extend_pairs(query)
        .append_pair("format", "png")
        .append_pair("width", "512")
        .append_pair("height", "512");
    Some(url.into())
}

fn disk_key(key: &str) -> Option<String> {
    cdn_url(key)?;
    if key.starts_with("embed:") {
        Some(format!("embed-{:x}", Sha256::digest(key.as_bytes())))
    } else {
        Some(key.to_owned())
    }
}

async fn run(
    root: Option<&Path>,
    mut requests: async_mpsc::Receiver<String>,
    results: async_mpsc::Sender<AvatarResult>,
    mut cancelled: watch::Receiver<bool>,
    ctx: &egui::Context,
) {
    let mut disk = root.and_then(|root| Disk::open(root.to_owned()).ok());
    let client = reqwest::Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(5))
        .pool_max_idle_per_host(1)
        .build()
        .ok();
    let mut cooldown = Instant::now();
    loop {
        let key = tokio::select! {
            biased;
            _ = cancelled.changed() => break,
            key = requests.recv() => match key { Some(key) => key, None => break },
        };
        if *cancelled.borrow() {
            break;
        }
        let Some(url) = cdn_url(&key) else { continue };
        let mut error = disk.is_none().then_some(CACHE_ERROR);
        let cached = disk.as_mut().and_then(|disk| match disk.read(&key) {
            Ok(bytes) => bytes,
            Err(_) => {
                error = Some(CACHE_ERROR);
                None
            }
        });
        let embed = key.starts_with("embed:")
            || key.starts_with("banner-")
            || key.starts_with("member-banner-");
        let mut image = cached.as_deref().and_then(|bytes| decode(bytes, embed));
        if image.is_none()
            && Instant::now() >= cooldown
            && let Some(client) = &client
        {
            let downloaded = tokio::select! {
                biased;
                _ = cancelled.changed() => break,
                bytes = download(client, &url, &mut cooldown) => bytes,
            };
            if let Some(bytes) = downloaded {
                image = decode(&bytes, embed);
                if *cancelled.borrow() {
                    break;
                }
                if image.is_some()
                    && let Some(disk) = &mut disk
                    && disk.write(&key, &bytes).is_err()
                {
                    error = Some(CACHE_ERROR);
                }
            }
        }
        if *cancelled.borrow() {
            break;
        }
        tokio::select! {
            biased;
            _ = cancelled.changed() => break,
            result = results.send(AvatarResult { key, image, error }) => if result.is_err() { break },
        }
        ctx.request_repaint();
    }
}

async fn download(client: &reqwest::Client, url: &str, cooldown: &mut Instant) -> Option<Vec<u8>> {
    let mut response = client.get(url).send().await.ok()?;
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let seconds = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
            .unwrap_or(60.0);
        // A nonsensical delay suspends this session's CDN requests, rather than retrying early.
        *cooldown = Instant::now()
            .checked_add(Duration::try_from_secs_f64(seconds.max(1.0)).unwrap_or(Duration::MAX))
            .unwrap_or_else(|| Instant::now() + Duration::from_secs(100 * 365 * 24 * 60 * 60));
        return None;
    }
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|length| length > MAX_ENCODED as u64)
    {
        return None;
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or(4096)
            .min(MAX_ENCODED as u64) as usize,
    );
    while let Some(chunk) = response.chunk().await.ok()? {
        if bytes.len().checked_add(chunk.len())? > MAX_ENCODED {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    Some(bytes)
}

fn decode(bytes: &[u8], embed: bool) -> Option<egui::ColorImage> {
    if bytes.len()
        > if embed {
            MAX_ENCODED
        } else {
            MAX_AVATAR_ENCODED
        }
    {
        return None;
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(if embed { 1024 } else { 256 });
    limits.max_image_height = Some(if embed { 1024 } else { 256 });
    limits.max_alloc = Some(if embed { 8 * 1024 * 1024 } else { 1024 * 1024 });
    reader.limits(limits);
    let size = if embed { 512 } else { 128 };
    let image = reader.decode().ok()?.thumbnail(size, size).into_rgba8();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    ))
}

struct Disk {
    root: PathBuf,
    bytes: u64,
    files: usize,
    last_prune: Instant,
}
impl Disk {
    fn open(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        }
        let mut disk = Self {
            root,
            bytes: 0,
            files: 0,
            last_prune: Instant::now(),
        };
        disk.prune(0, 0)?;
        Ok(disk)
    }
    fn read(&mut self, key: &str) -> io::Result<Option<Vec<u8>>> {
        let name = disk_key(key).ok_or(io::ErrorKind::InvalidInput)?;
        let path = self.root.join(format!("{name}.png"));
        let file = match OpenOptions::new().read(true).write(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let metadata = file.metadata()?;
        if metadata.len() > MAX_ENCODED as u64
            || SystemTime::now()
                .duration_since(metadata.modified()?)
                .unwrap_or_default()
                >= RETENTION
        {
            return Ok(None);
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        (&file)
            .take(MAX_ENCODED as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_ENCODED {
            return Ok(None);
        }
        file.set_modified(SystemTime::now())?;
        Ok(Some(bytes))
    }
    fn write(&mut self, key: &str, bytes: &[u8]) -> io::Result<()> {
        if cdn_url(key).is_none() || bytes.len() > MAX_ENCODED {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let name = disk_key(key).ok_or(io::ErrorKind::InvalidInput)?;
        let path = self.root.join(format!("{name}.png"));
        if self.bytes + bytes.len() as u64 > MAX_DISK
            || self.files + 1 > MAX_FILES
            || self.last_prune.elapsed() >= Duration::from_secs(24 * 60 * 60)
        {
            self.prune(bytes.len() as u64, 1)?;
        }
        let previous = fs::metadata(&path).ok().map(|m| m.len());
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let temp = self.root.join(format!(
            "{}.{}.part",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temp)?;
            file.write_all(bytes)?;
            drop(file);
            #[cfg(windows)]
            if path.exists() {
                fs::remove_file(&path)?;
            }
            fs::rename(&temp, &path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result?;
        self.bytes = self.bytes.saturating_sub(previous.unwrap_or(0)) + bytes.len() as u64;
        self.files += usize::from(previous.is_none());
        Ok(())
    }
    fn prune(&mut self, reserve_bytes: u64, reserve_files: usize) -> io::Result<()> {
        // Keep only 32 eviction candidates in RAM even if a pre-existing directory is enormous.
        loop {
            let mut oldest = BinaryHeap::new();
            self.bytes = 0;
            self.files = 0;
            for entry in fs::read_dir(&self.root)? {
                let entry = entry?;
                let metadata = entry.metadata()?;
                if !metadata.is_file() {
                    continue;
                }
                let modified = metadata.modified()?;
                if entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "part")
                    || SystemTime::now()
                        .duration_since(modified)
                        .unwrap_or_default()
                        >= RETENTION
                {
                    fs::remove_file(entry.path())?;
                    continue;
                }
                self.bytes = self.bytes.saturating_add(metadata.len());
                self.files += 1;
                oldest.push((modified, entry.path(), metadata.len()));
                if oldest.len() > 32 {
                    oldest.pop();
                }
            }
            if self.bytes + reserve_bytes <= MAX_DISK && self.files + reserve_files <= MAX_FILES {
                break;
            }
            for (_, path, bytes) in oldest.into_sorted_vec() {
                fs::remove_file(path)?;
                self.bytes = self.bytes.saturating_sub(bytes);
                self.files -= 1;
                if self.bytes + reserve_bytes <= MAX_DISK && self.files + reserve_files <= MAX_FILES
                {
                    break;
                }
            }
        }
        self.last_prune = Instant::now();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn custom_emoji_urls_are_static_and_confined_to_discord_cdn() {
        assert_eq!(
            super::cdn_url("emoji-9001").as_deref(),
            Some("https://cdn.discordapp.com/emojis/9001.png?size=64")
        );
        for key in [
            "emoji-0",
            "emoji-../9001",
            "emoji-9001?size=8192",
            "emoji-https://example.com",
            "emoji-9001/foo",
        ] {
            assert!(super::cdn_url(key).is_none());
        }
    }

    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([70, 150, 120, 255]));
        let mut encoded = Cursor::new(Vec::new());
        image
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        encoded.into_inner()
    }
    #[test]
    fn bounded_images_cache_reopen_eviction_and_cancelled_cleanup() {
        assert!(cdn_url("../token").is_none());
        assert_eq!(
            cdn_url("banner-1-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
            "https://cdn.discordapp.com/banners/1/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png?size=512"
        );
        assert_eq!(
            cdn_url("member-banner-2-1-a_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
            "https://cdn.discordapp.com/guilds/2/users/1/banners/a_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png?size=512"
        );
        assert_eq!(
            cdn_url("member-avatar-2-1-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
            "https://cdn.discordapp.com/guilds/2/users/1/avatars/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png?size=128"
        );
        assert!(cdn_url("member-banner-2-0-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").is_none());
        assert!(cdn_url("banner-1-../../invalid").is_none());
        assert!(cdn_url("default-6").is_none());
        assert!(cdn_url("0-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").is_none());
        assert!(cdn_url("1-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/").is_none());
        assert_eq!(
            cdn_url("1-a_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
            "https://cdn.discordapp.com/avatars/1/a_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png?size=128"
        );
        assert_eq!(
            cdn_url("guild-1-a_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
            "https://cdn.discordapp.com/icons/1/a_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png?size=128"
        );
        for source in [
            "http://media.discordapp.net/attachments/1/2/image.png",
            "https://media.discordapp.net.evil.test/attachments/1/2/image.png",
            "https://user@media.discordapp.net/attachments/1/2/image.png",
            "https://media.discordapp.net:444/attachments/1/2/image.png",
            "https://127.0.0.1/attachments/1/2/image.png",
            "https://media.discordapp.net/attachments/1/2/image.png#fragment",
            "https://cdn.discordapp.com/api/v10/users/@me",
            "https://media.discordapp.net/attachments/../api",
            "https://images-ext-1.discordapp.net/external/short/https/example.com/a.png",
        ] {
            assert!(
                embed_url(source).is_none(),
                "Unsafe or unsupported test URL was accepted"
            );
        }
        let embed_key = "embed:https://cdn.discordapp.com/attachments/1/2/image.png?ex=abc&is=def&hm=synthetic&format=webp&width=4096";
        let transformed = cdn_url(embed_key).unwrap();
        assert!(transformed.starts_with("https://media.discordapp.net/attachments/1/2/image.png?"));
        assert!(transformed.ends_with("format=png&width=512&height=512"));
        assert!(transformed.contains("hm=synthetic"));
        assert!(!transformed.contains("4096"));
        assert_eq!(disk_key(embed_key).unwrap().len(), 70);
        assert!(
            disk_key(embed_key)
                .unwrap()
                .bytes()
                .all(|b| b.is_ascii_hexdigit() || matches!(b, b'm' | b'-'))
        );
        assert!(embed_url("https://images-ext-1.discordapp.net/external/abcdefghijklmnop/https/example.com/image.jpg").is_some());
        assert!(decode(&png(1025, 1), true).is_none());
        assert_eq!(decode(&png(1024, 512), true).unwrap().size, [512, 256]);
        assert!(decode(&vec![0; MAX_ENCODED + 1], false).is_none());
        assert!(decode(b"not an image", false).is_none());
        assert!(decode(&png(257, 1), false).is_none());
        let bytes = png(256, 256);
        assert_eq!(decode(&bytes, false).unwrap().size, [128, 128]);
        let root = std::env::temp_dir().join(format!(
            "serein-avatar-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let account_a = root.join("1");
        let account_b = root.join("2");
        let mut disk = Disk::open(account_a.clone()).unwrap();
        disk.write("default-0", &bytes).unwrap();
        disk.write(embed_key, &bytes).unwrap();
        assert!(fs::read_dir(&account_a).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("synthetic")
        }));
        drop(disk);
        let mut disk = Disk::open(account_a.clone()).unwrap();
        assert_eq!(disk.read("default-0").unwrap().unwrap(), bytes);
        assert_eq!(disk.read(embed_key).unwrap().unwrap(), bytes);
        assert!(
            Disk::open(account_b.clone())
                .unwrap()
                .read("default-0")
                .unwrap()
                .is_none()
        );
        let sparse = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(account_b.join("oversized.png"))
            .unwrap();
        sparse.set_len(MAX_DISK + 1).unwrap();
        drop(sparse);
        assert_eq!(Disk::open(account_b.clone()).unwrap().bytes, 0);
        let old = account_a.join("default-0.png");
        OpenOptions::new()
            .write(true)
            .open(old)
            .unwrap()
            .set_modified(SystemTime::now() - RETENTION - Duration::from_secs(1))
            .unwrap();
        disk.prune(0, 0).unwrap();
        assert!(disk.read("default-0").unwrap().is_none());
        fs::remove_file(account_a.join(format!("{}.png", disk_key(embed_key).unwrap()))).unwrap();
        drop(disk);
        // Eviction and full directory deletion are disk workloads, not a worker-cancellation
        // deadline: deleting 4096 files can exceed five seconds on a Windows CI filesystem.
        let eviction = root.join("eviction");
        let mut disk = Disk::open(eviction.clone()).unwrap();
        for index in 0..MAX_FILES + 1 {
            fs::write(eviction.join(format!("synthetic-{index}.png")), []).unwrap();
        }
        disk.prune(0, 0).unwrap();
        assert_eq!(disk.files, MAX_FILES);
        disk.write("default-0", &bytes).unwrap();
        assert_eq!(disk.files, MAX_FILES);
        drop(disk);
        clear_directory(Some(&eviction)).unwrap();
        assert!(!eviction.exists());
        // A valid cached image keeps the shutdown fixture entirely offline and tiny.
        let mut disk = Disk::open(account_a.clone()).unwrap();
        disk.write("default-0", &bytes).unwrap();
        assert_eq!(disk.files, 1);
        drop(disk);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut worker =
            AvatarWorker::start_at(&runtime, Some(account_a.clone()), egui::Context::default())
                .unwrap();
        assert!(worker.request("default-0".into()));
        let result = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), worker.results.recv())
                .await
                .unwrap()
                .unwrap()
        });
        assert_eq!(result.image.unwrap().size, [128, 128]);
        assert!(result.error.is_none());
        // Fill the result channel, then cancel: shutdown must not wait on the renderer.
        for _ in 0..32 {
            assert!(worker.request("default-0".into()));
        }
        runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), async {
                while worker.results.len() < 2 {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            })
            .await
            .unwrap();
        });
        worker
            .shutdown_and_clear()
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert!(!account_a.exists());
        assert!(account_b.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn synthetic_download_limits_redirects_and_retry_delay() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let bytes = png(2, 2);
        let expected = bytes.clone();
        let server = tokio::spawn(async move {
            for response in [
                [format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len()).into_bytes(), bytes].concat(),
                format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", MAX_ENCODED + 1).into_bytes(),
                [b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n".to_vec(), format!("{:x}\r\n", MAX_ENCODED + 1).into_bytes(), vec![0; MAX_ENCODED + 1], b"\r\n0\r\n\r\n".to_vec()].concat(),
                b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
                b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 123.5\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
            ] {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 4096];
                let length = stream.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..length]).to_ascii_lowercase();
                assert!(!request.contains("authorization:"));
                assert!(!request.contains("cookie:"));
                let _ = stream.write_all(&response).await;
            }
        });
        // Only this private test seam accepts HTTP; production constructs fixed HTTPS CDN URLs.
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let mut cooldown = Instant::now();
        let url = format!("http://{address}/synthetic.png");
        assert_eq!(
            download(&client, &url, &mut cooldown).await.unwrap(),
            expected
        );
        assert!(download(&client, &url, &mut cooldown).await.is_none());
        assert!(download(&client, &url, &mut cooldown).await.is_none());
        assert!(download(&client, &url, &mut cooldown).await.is_none());
        assert!(download(&client, &url, &mut cooldown).await.is_none());
        assert!(cooldown.duration_since(Instant::now()) > Duration::from_secs(120));
        server.await.unwrap();
    }
}
