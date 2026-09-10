use crate::Id;
use serde::{Deserialize, Serialize};

pub const MAX_REACTIONS: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReactionEmoji {
    pub id: Option<Id>,
    pub name: Option<String>,
}
impl ReactionEmoji {
    pub fn valid(&self) -> bool {
        self.name.as_ref().is_none_or(|name| {
            !name.is_empty() && name.len() <= 128 && !name.chars().any(char::is_control)
        }) && (self.id.is_some() || self.name.is_some())
    }
    pub fn label(&self) -> String {
        match (self.id, self.name.as_deref()) {
            (Some(_), Some(name)) => format!(":{name}:"),
            (Some(_), None) => "Deleted emoji".into(),
            (_, name) => name.unwrap_or("Emoji").into(),
        }
    }
    pub fn same(&self, other: &Self) -> bool {
        if self.id.is_some() || other.id.is_some() {
            self.id == other.id
        } else {
            self.name == other.name
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Reaction {
    pub emoji: ReactionEmoji,
    pub count: u32,
    pub me: bool,
    #[serde(default)]
    pub me_burst: bool,
}
pub fn reaction_bytes(reactions: &[Reaction]) -> usize {
    std::mem::size_of_val(reactions)
        + reactions
            .iter()
            .map(|r| r.emoji.name.as_ref().map_or(0, String::capacity))
            .sum::<usize>()
}
pub fn valid_reactions(reactions: &[Reaction]) -> bool {
    reactions.len() <= MAX_REACTIONS
        && reactions.iter().enumerate().all(|(index, r)| {
            r.emoji.valid()
                && r.count > 0
                && !reactions[..index]
                    .iter()
                    .any(|other| r.emoji.same(&other.emoji))
        })
}

/// Compact guild catalog; unknown role metadata never grants picker eligibility.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct CustomEmoji {
    pub id: Id,
    pub name: String,
    #[serde(default)]
    pub animated: bool,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub managed: bool,
    #[serde(default)]
    pub roles: Option<Vec<Id>>,
}
pub const MAX_GUILD_EMOJIS: usize = 1000;
pub const MAX_GUILD_EMOJI_BYTES: usize = 256 * 1024;
impl CustomEmoji {
    pub fn valid(&self) -> bool {
        self.id.0 != 0
            && (2..=32).contains(&self.name.len())
            && self
                .name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
            && self
                .roles
                .as_ref()
                .is_none_or(|roles| roles.len() <= 256 && roles.iter().all(|id| id.0 != 0))
    }
    pub fn usable(&self) -> bool {
        self.valid()
            && self.available
            && !self.managed
            && self.roles.as_ref().is_some_and(Vec::is_empty)
    }
    pub fn markup(&self) -> String {
        format!(
            "<{}:{}:{}>",
            if self.animated { "a" } else { "" },
            self.name,
            self.id
        )
    }
    pub fn heap_bytes(&self) -> usize {
        self.name.capacity()
            + self
                .roles
                .as_ref()
                .map_or(0, |roles| roles.capacity() * std::mem::size_of::<Id>())
    }
}
pub fn custom_emoji_bytes(emojis: &Vec<CustomEmoji>) -> usize {
    emojis.capacity() * std::mem::size_of::<CustomEmoji>()
        + emojis.iter().map(CustomEmoji::heap_bytes).sum::<usize>()
}
pub fn valid_custom_emojis(emojis: &Vec<CustomEmoji>) -> bool {
    let mut ids = std::collections::BTreeSet::new();
    emojis.len() <= MAX_GUILD_EMOJIS
        && custom_emoji_bytes(emojis) <= MAX_GUILD_EMOJI_BYTES
        && emojis
            .iter()
            .all(|emoji| emoji.valid() && ids.insert(emoji.id))
}
