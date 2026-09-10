//! Account-isolated bounded SQLite cache. This is not Discord's authoritative state.
use model::{Id, Message, User};
use rusqlite::{Connection, OptionalExtension, params};
use std::{collections::BTreeMap, path::Path};

const MAX_EMBED_JSON: usize = 256 * 1024;
const MAX_WINDOW_BYTES: usize = 4 * 1024 * 1024;
pub struct LocalStore(Connection);
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreError {
    Unavailable,
    Capacity,
    Incompatible,
}
type Result<T> = std::result::Result<T, StoreError>;
impl From<rusqlite::Error> for StoreError {
    fn from(_: rusqlite::Error) -> Self {
        Self::Unavailable
    }
}
/// Reject excess entries during parsing, before allocating a whole malformed array.
struct CachedEmbeds(Vec<model::Embed>);
impl<'de> serde::Deserialize<'de> for CachedEmbeds {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = CachedEmbeds;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("at most ten cached embeds")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut embeds = Vec::new();
                for _ in 0..model::MAX_EMBEDS {
                    match sequence.next_element()? {
                        Some(embed) => embeds.push(embed),
                        None => return Ok(CachedEmbeds(embeds)),
                    }
                }
                if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                    return Err(serde::de::Error::custom("cached embed limit"));
                }
                Ok(CachedEmbeds(embeds))
            }
        }
        deserializer.deserialize_seq(Visitor)
    }
}
impl LocalStore {
    pub fn open_default() -> Result<Self> {
        let root = dirs::data_local_dir()
            .ok_or(StoreError::Unavailable)?
            .join("serein");
        std::fs::create_dir_all(&root).map_err(|_| StoreError::Unavailable)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| StoreError::Unavailable)?;
        }
        Self::open(&root.join("client.sqlite3"))
    }
    pub fn open(path: &Path) -> Result<Self> {
        Self::initialize(Connection::open(path)?)
    }
    fn initialize(connection: Connection) -> Result<Self> {
        connection.busy_timeout(std::time::Duration::from_secs(2))?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > 4 {
            return Err(StoreError::Incompatible);
        }
        connection.execute_batch("PRAGMA page_size=4096; PRAGMA max_page_count=16384; PRAGMA cache_size=-2048; PRAGMA temp_store=MEMORY; PRAGMA journal_mode=DELETE; PRAGMA secure_delete=ON; PRAGMA auto_vacuum=FULL;
            CREATE TABLE IF NOT EXISTS messages(account TEXT NOT NULL,channel TEXT NOT NULL,id TEXT NOT NULL,author TEXT NOT NULL,name TEXT NOT NULL,content TEXT NOT NULL,edited INTEGER NOT NULL,reply TEXT,unsupported INTEGER NOT NULL,PRIMARY KEY(account,channel,id));
            CREATE TABLE IF NOT EXISTS channels(account TEXT NOT NULL,channel TEXT NOT NULL,touched INTEGER NOT NULL,PRIMARY KEY(account,channel));
            CREATE TABLE IF NOT EXISTS drafts(account TEXT NOT NULL,channel TEXT NOT NULL,content TEXT NOT NULL,PRIMARY KEY(account,channel));
            CREATE TABLE IF NOT EXISTS appearance(singleton INTEGER PRIMARY KEY CHECK(singleton=1),theme TEXT NOT NULL CHECK(theme IN ('light','dark')));
            ")?;
        let has_avatar: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('messages') WHERE name='avatar')",
            [],
            |row| row.get(0),
        )?;
        if !has_avatar {
            connection.execute_batch("BEGIN; ALTER TABLE messages ADD COLUMN avatar TEXT; ALTER TABLE messages ADD COLUMN discriminator INTEGER NOT NULL DEFAULT 0; PRAGMA user_version=3; COMMIT;")?;
        }
        let has_embeds: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('messages') WHERE name='embeds')",
            [],
            |row| row.get(0),
        )?;
        if !has_embeds {
            connection.execute_batch("BEGIN; ALTER TABLE messages ADD COLUMN embeds TEXT NOT NULL DEFAULT '[]'; ALTER TABLE messages ADD COLUMN embeds_suppressed INTEGER NOT NULL DEFAULT 0; PRAGMA user_version=4; COMMIT;")?;
        } else {
            connection.pragma_update(None, "user_version", 4)?;
        }
        Ok(Self(connection))
    }
    /// One account-independent application preference. System deletes the override.
    pub fn appearance(&self) -> Result<Appearance> {
        match self
            .0
            .query_row("SELECT theme FROM appearance WHERE singleton=1", [], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
            .as_deref()
        {
            None => Ok(Appearance::System),
            Some("light") => Ok(Appearance::Light),
            Some("dark") => Ok(Appearance::Dark),
            Some(_) => Err(StoreError::Incompatible),
        }
    }
    pub fn save_appearance(&self, appearance: Appearance) -> Result<()> {
        match appearance {
            Appearance::System => {
                self.0.execute("DELETE FROM appearance", [])?;
            }
            Appearance::Light | Appearance::Dark => {
                let theme = if appearance == Appearance::Light {
                    "light"
                } else {
                    "dark"
                };
                self.0.execute("INSERT INTO appearance VALUES(1,?1) ON CONFLICT(singleton) DO UPDATE SET theme=excluded.theme", [theme])?;
            }
        }
        Ok(())
    }
    pub fn save_channel(&mut self, account: Id, channel: Id, messages: &[Message]) -> Result<()> {
        if messages.len() > 500
            || messages.iter().map(Message::bytes).sum::<usize>() > MAX_WINDOW_BYTES
            || messages
                .iter()
                .any(|m| m.channel != channel || !model::valid_embeds(&m.embeds))
        {
            return Err(StoreError::Capacity);
        }
        // ponytail: replace one <=500-row window transactionally; switch to mutation UPSERTs if write cost is measured to matter.
        let transaction = self.0.transaction()?;
        let account = account.to_string();
        let channel = channel.to_string();
        transaction.execute(
            "DELETE FROM messages WHERE account=?1 AND channel=?2",
            params![account, channel],
        )?;
        for message in messages {
            let embeds =
                serde_json::to_string(&message.embeds).map_err(|_| StoreError::Incompatible)?;
            if embeds.len() > MAX_EMBED_JSON {
                return Err(StoreError::Capacity);
            }
            transaction.execute(
                "INSERT INTO messages(account,channel,id,author,name,content,edited,reply,unsupported,avatar,discriminator,embeds,embeds_suppressed) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                params![
                    account,
                    channel,
                    message.id.to_string(),
                    message.author.id.to_string(),
                    message.author.name,
                    message.content,
                    message.edited,
                    message.reply_to.map(|id| id.to_string()),
                    message.unsupported,
                    message.author.avatar,
                    message.author.discriminator,
                    embeds,
                    message.embeds_suppressed
                ],
            )?;
        }
        transaction.execute("INSERT INTO channels VALUES(?1,?2,unixepoch('subsec')*1000) ON CONFLICT(account,channel) DO UPDATE SET touched=excluded.touched",params![account,channel])?;
        // Global limit: 20 channel windows, 10000 messages AND 48 MiB content, below the 64 MiB database page ceiling.
        loop {
            let (channels,bytes):(i64,i64)=transaction.query_row("SELECT (SELECT count(*) FROM channels),(SELECT coalesce(sum(length(CAST(content AS BLOB))+length(CAST(name AS BLOB))+length(CAST(embeds AS BLOB))+256),0) FROM messages)",[],|r|Ok((r.get(0)?,r.get(1)?)))?;
            if channels <= 20 && bytes <= 48 * 1024 * 1024 {
                break;
            }
            let (a, c): (String, String) = transaction.query_row(
                "SELECT account,channel FROM channels ORDER BY touched,account,channel LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            transaction.execute(
                "DELETE FROM messages WHERE account=?1 AND channel=?2",
                params![a, c],
            )?;
            transaction.execute(
                "DELETE FROM channels WHERE account=?1 AND channel=?2",
                params![a, c],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
    pub fn load_channel(&self, account: Id, channel: Id) -> Result<Vec<Message>> {
        let mut query = self.0.prepare("SELECT id,author,name,content,edited,reply,unsupported,avatar,discriminator,embeds,embeds_suppressed FROM messages WHERE account=?1 AND channel=?2 ORDER BY length(id),id LIMIT 500")?;
        let mut rows = query.query(params![account.to_string(), channel.to_string()])?;
        let mut messages = Vec::new();
        let mut bytes = 0;
        while let Some(row) = rows.next()? {
            // Inspect borrowed SQLite fields before allocating attacker-controlled cache strings.
            for (column, maximum) in [
                (0, 20),
                (1, 20),
                (2, 512),
                (3, 64 * 1024),
                (9, MAX_EMBED_JSON),
            ] {
                if row
                    .get_ref(column)?
                    .as_str()
                    .map_err(|_| StoreError::Incompatible)?
                    .len()
                    > maximum
                {
                    return Err(StoreError::Capacity);
                }
            }
            for (column, maximum) in [(5, 20), (7, 34)] {
                if !matches!(row.get_ref(column)?, rusqlite::types::ValueRef::Null)
                    && row
                        .get_ref(column)?
                        .as_str()
                        .map_err(|_| StoreError::Incompatible)?
                        .len()
                        > maximum
                {
                    return Err(StoreError::Capacity);
                }
            }
            let embeds = serde_json::from_str::<CachedEmbeds>(
                row.get_ref(9)?
                    .as_str()
                    .map_err(|_| StoreError::Incompatible)?,
            )
            .map_err(|_| StoreError::Incompatible)?
            .0;
            if !model::valid_embeds(&embeds) {
                return Err(StoreError::Capacity);
            }
            let parse = |value: String| value.parse::<Id>().map_err(|_| StoreError::Incompatible);
            let message = Message {
                id: parse(row.get(0)?)?,
                channel,
                author: User {
                    id: parse(row.get(1)?)?,
                    name: row.get(2)?,
                    avatar: row.get(7)?,
                    discriminator: row.get(8)?,
                },
                content: row.get(3)?,
                edited: row.get(4)?,
                edited_at: None,
                reply_to: row.get::<_, Option<String>>(5)?.map(parse).transpose()?,
                unsupported: row.get(6)?,
                nonce: None,
                revision: 0,
                embeds,
                embeds_suppressed: row.get(10)?,
            };
            bytes += message.bytes();
            if bytes > MAX_WINDOW_BYTES {
                return Err(StoreError::Capacity);
            }
            messages.push(message);
        }
        Ok(messages)
    }
    pub fn save_draft(&mut self, account: Id, channel: Id, content: &str) -> Result<()> {
        if content.len() > 8192 {
            return Err(StoreError::Capacity);
        }
        let transaction = self.0.transaction()?;
        transaction.execute(
            "DELETE FROM drafts WHERE account=?1 AND channel=?2",
            params![account.to_string(), channel.to_string()],
        )?;
        if !content.is_empty() {
            transaction.execute(
                "INSERT INTO drafts VALUES(?1,?2,?3)",
                params![account.to_string(), channel.to_string(), content],
            )?;
        }
        let (count, bytes): (i64, i64) = transaction.query_row(
            "SELECT count(*),coalesce(sum(length(CAST(content AS BLOB))),0) FROM drafts",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if count > 64 || bytes > 2 * 1024 * 1024 {
            return Err(StoreError::Capacity);
        }
        transaction.commit()?;
        Ok(())
    }
    pub fn load_drafts(&self, account: Id) -> Result<BTreeMap<Id, String>> {
        let mut query = self
            .0
            .prepare("SELECT channel,content FROM drafts WHERE account=?1 LIMIT 64")?;
        let rows = query.query_map([account.to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut drafts = BTreeMap::new();
        let mut bytes = 0;
        for row in rows {
            let (id, content) = row?;
            bytes += content.len();
            if bytes > 2 * 1024 * 1024 || content.len() > 8192 {
                return Err(StoreError::Capacity);
            }
            drafts.insert(id.parse().map_err(|_| StoreError::Incompatible)?, content);
        }
        Ok(drafts)
    }
    pub fn clear_history(&mut self, account: Id) -> Result<()> {
        let transaction = self.0.transaction()?;
        transaction.execute(
            "DELETE FROM messages WHERE account=?1",
            [account.to_string()],
        )?;
        transaction.execute(
            "DELETE FROM channels WHERE account=?1",
            [account.to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }
    pub fn forget_account(&mut self, account: Id) -> Result<()> {
        let transaction = self.0.transaction()?;
        for table in ["messages", "channels", "drafts"] {
            transaction.execute(
                &format!("DELETE FROM {table} WHERE account=?1"),
                [account.to_string()],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schema_three_and_untrusted_cached_embeds_remain_bounded() {
        let store = LocalStore::initialize(Connection::open_in_memory().unwrap()).unwrap();
        store.0.execute_batch("ALTER TABLE messages DROP COLUMN embeds; ALTER TABLE messages DROP COLUMN embeds_suppressed; PRAGMA user_version=3;").unwrap();
        store.0.execute("INSERT INTO messages(account,channel,id,author,name,content,edited,unsupported) VALUES('1','2','3','4','Synthetic','body',0,0)", []).unwrap();
        let store = LocalStore::initialize(store.0).unwrap();
        let loaded = store.load_channel(Id(1), Id(2)).unwrap();
        assert_eq!(loaded[0].content, "body");
        assert!(loaded[0].embeds.is_empty());
        assert!(!loaded[0].embeds_suppressed);
        let version: u32 = store
            .0
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 4);
        for (json, error) in [
            ("broken JSON".to_owned(), StoreError::Incompatible),
            (
                serde_json::to_string(&vec![model::Embed::default(); 11]).unwrap(),
                StoreError::Incompatible,
            ),
            (
                serde_json::json!([{"fields": vec![model::EmbedField::default(); 26]}]).to_string(),
                StoreError::Incompatible,
            ),
            (" ".repeat(MAX_EMBED_JSON + 1), StoreError::Capacity),
        ] {
            store
                .0
                .execute("UPDATE messages SET embeds=?1", [json])
                .unwrap();
            assert!(matches!(store.load_channel(Id(1), Id(2)), Err(actual) if actual == error));
        }
        store
            .0
            .execute(
                "UPDATE messages SET embeds='[]',content=?1",
                ["x".repeat(64 * 1024 + 1)],
            )
            .unwrap();
        assert!(matches!(
            store.load_channel(Id(1), Id(2)),
            Err(StoreError::Capacity)
        ));
    }
    #[test]
    fn account_isolation_draft_reopen_eviction_and_logout() {
        let root =
            std::env::temp_dir().join(format!("serein-synthetic-store-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("test.sqlite3");
        let legacy = Connection::open(&path).unwrap();
        legacy.execute_batch("CREATE TABLE IF NOT EXISTS messages(account TEXT NOT NULL,channel TEXT NOT NULL,id TEXT NOT NULL,author TEXT NOT NULL,name TEXT NOT NULL,content TEXT NOT NULL,edited INTEGER NOT NULL,reply TEXT,unsupported INTEGER NOT NULL,PRIMARY KEY(account,channel,id)); PRAGMA user_version=1;").unwrap();
        legacy.execute("INSERT OR REPLACE INTO messages(account,channel,id,author,name,content,edited,reply,unsupported) VALUES('9','90','900','9','Legacy','Synthetic',0,NULL,0)", []).unwrap();
        drop(legacy);
        let mut store = LocalStore::open(&path).unwrap();
        let upgraded = store.load_channel(Id(9), Id(90)).unwrap();
        assert!(upgraded[0].author.avatar.is_none());
        assert_eq!(upgraded[0].author.discriminator, 0);
        assert!(upgraded[0].embeds.is_empty());
        assert!(!upgraded[0].embeds_suppressed);
        assert_eq!(store.appearance().unwrap(), Appearance::System);
        store.save_appearance(Appearance::Light).unwrap();
        store.save_draft(Id(1), Id(20), "synthetic draft").unwrap();
        // The prior schema remains readable and is upgraded without losing drafts.
        store.0.pragma_update(None, "user_version", 1).unwrap();
        drop(store);
        let mut store = LocalStore::open(&path).unwrap();
        assert_eq!(store.appearance().unwrap(), Appearance::Light);
        store.save_appearance(Appearance::Dark).unwrap();
        store.0.execute_batch("PRAGMA query_only=ON;").unwrap();
        assert_eq!(
            store.save_appearance(Appearance::System),
            Err(StoreError::Unavailable)
        );
        assert_eq!(store.appearance().unwrap(), Appearance::Dark);
        store.0.execute_batch("PRAGMA query_only=OFF;").unwrap();
        assert_eq!(
            store.load_drafts(Id(1)).unwrap()[&Id(20)],
            "synthetic draft"
        );
        assert!(store.load_drafts(Id(2)).unwrap().is_empty());
        for channel in 1..=63 {
            store
                .save_draft(Id(3), Id(channel), "other synthetic draft")
                .unwrap();
        }
        assert_eq!(
            store.save_draft(Id(1), Id(21), "over capacity"),
            Err(StoreError::Capacity)
        );
        assert_eq!(store.load_drafts(Id(1)).unwrap().len(), 1);
        assert_eq!(
            store.load_drafts(Id(1)).unwrap()[&Id(20)],
            "synthetic draft"
        );
        for channel in 1..=30 {
            let message = Message {
                id: Id(100),
                channel: Id(channel),
                author: User {
                    id: Id(1),
                    name: "Synthetic".into(),
                    avatar: Some("0123456789abcdef0123456789abcdef".into()),
                    discriminator: 1234,
                },
                content: "synthetic".into(),
                edited: false,
                edited_at: None,
                reply_to: None,
                unsupported: false,
                embeds: vec![model::Embed {
                    title: Some("Cached synthetic embed".into()),
                    ..Default::default()
                }],
                embeds_suppressed: true,
                nonce: None,
                revision: 0,
            };
            store.save_channel(Id(1), Id(channel), &[message]).unwrap();
        }
        let count: i64 = store
            .0
            .query_row("SELECT count(*) FROM channels", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 20);
        assert!(store.load_channel(Id(2), Id(30)).unwrap().is_empty());
        drop(store);
        let mut store = LocalStore::open(&path).unwrap();
        let cached = store.load_channel(Id(1), Id(30)).unwrap();
        assert_eq!(
            cached[0].embeds[0].title.as_deref(),
            Some("Cached synthetic embed")
        );
        assert!(cached[0].embeds_suppressed);
        let cached_author = &cached[0].author;
        assert_eq!(
            cached_author.avatar.as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
        assert_eq!(cached_author.discriminator, 1234);
        // A failed logout must leave the whole account transaction intact.
        store.0.execute_batch("CREATE TRIGGER reject_draft_delete BEFORE DELETE ON drafts BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END;").unwrap();
        assert_eq!(store.forget_account(Id(1)), Err(StoreError::Unavailable));
        assert!(!store.load_channel(Id(1), Id(30)).unwrap().is_empty());
        assert!(!store.load_drafts(Id(1)).unwrap().is_empty());
        store
            .0
            .execute_batch("DROP TRIGGER reject_draft_delete;")
            .unwrap();
        store.forget_account(Id(1)).unwrap();
        assert!(store.load_drafts(Id(1)).unwrap().is_empty());
        assert!(store.load_channel(Id(1), Id(30)).unwrap().is_empty());
        assert_eq!(store.appearance().unwrap(), Appearance::Dark);
        store.save_appearance(Appearance::System).unwrap();
        drop(store);
        let store = LocalStore::open(&path).unwrap();
        assert_eq!(store.appearance().unwrap(), Appearance::System);
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
}
