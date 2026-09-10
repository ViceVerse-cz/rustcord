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
