use model::Reaction;
use serde::{
    Deserialize, Deserializer,
    de::{SeqAccess, Visitor},
};

#[derive(Default)]
pub struct ReactionList(pub Vec<Reaction>);
impl<'de> Deserialize<'de> for ReactionList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct List;
        impl<'de> Visitor<'de> for List {
            type Value = ReactionList;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a bounded list of reactions")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element::<Reaction>()? {
                    if items.len() >= model::MAX_REACTIONS || !item.emoji.valid() || item.count == 0
                    {
                        return Err(serde::de::Error::custom("Invalid reaction list"));
                    }
                    items.push(item);
                }
                if !model::valid_reactions(&items) {
                    return Err(serde::de::Error::custom("Duplicate reaction emoji"));
                }
                Ok(ReactionList(items))
            }
        }
        deserializer.deserialize_seq(List)
    }
}

#[derive(Deserialize)]
pub struct ReactionTarget {
    pub channel_id: model::Id,
    pub message_id: model::Id,
}

#[cfg(test)]
mod tests {
    use crate::{MessageDto, PatchDto, decode};
    use model::{Id, Patch};
    #[test]
    fn reactions_decode_bounded_counts_custom_emoji_and_partial_removals() {
        let raw = serde_json::json!({"id":"1","channel_id":"2","author":{"id":"3","username":"Test"},
            "reactions":[{"emoji":{"id":null,"name":"👍"},"count":3,"me":true,"me_burst":true},
            {"emoji":{"id":"4","name":null},"count":1,"me":false}]});
        let message = decode::<MessageDto>(&serde_json::to_vec(&raw).unwrap())
            .unwrap()
            .into_model();
        let reactions = message.reactions.unwrap();
        assert!(reactions[0].me && reactions[0].me_burst);
        assert_eq!(reactions[1].emoji.id, Some(Id(4)));
        assert!(matches!(
            decode::<PatchDto>(br#"{"id":"1","channel_id":"2"}"#)
                .unwrap()
                .into_model()
                .reactions,
            Patch::Absent
        ));
        assert!(matches!(
            decode::<PatchDto>(br#"{"id":"1","channel_id":"2","reactions":null}"#)
                .unwrap()
                .into_model()
                .reactions,
            Patch::Null
        ));
        for values in [
            vec![raw["reactions"][0].clone(); 65],
            vec![raw["reactions"][0].clone(); 2],
            vec![
                serde_json::json!({"emoji":{"id":null,"name":"x".repeat(129)},"count":1,"me":false}),
            ],
        ] {
            let mut bad = raw.clone();
            bad["reactions"] = serde_json::json!(values);
            assert!(decode::<MessageDto>(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
    }
}

/// Decode with the catalog's retained item and allocation budgets before enqueueing.
pub struct CustomEmojiList(pub Vec<model::CustomEmoji>);
impl<'de> Deserialize<'de> for CustomEmojiList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct List;
        impl<'de> Visitor<'de> for List {
            type Value = CustomEmojiList;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a bounded guild emoji catalog")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut items = Vec::new();
                let mut bytes = 0;
                let mut ids = std::collections::BTreeSet::new();
                while let Some(item) = seq.next_element::<model::CustomEmoji>()? {
                    bytes += item.heap_bytes();
                    if items.len() >= model::MAX_GUILD_EMOJIS
                        || !item.valid()
                        || !ids.insert(item.id)
                    {
                        return Err(serde::de::Error::custom("Invalid guild emoji catalog"));
                    }
                    items.push(item);
                    if bytes + items.capacity() * std::mem::size_of::<model::CustomEmoji>()
                        > model::MAX_GUILD_EMOJI_BYTES
                    {
                        return Err(serde::de::Error::custom(
                            "Guild emoji catalog exceeds capacity",
                        ));
                    }
                }
                Ok(CustomEmojiList(items))
            }
        }
        deserializer.deserialize_seq(List)
    }
}
#[derive(Deserialize)]
pub struct GuildEmojisUpdate {
    pub guild_id: model::Id,
    pub emojis: CustomEmojiList,
}

#[cfg(test)]
mod catalog_tests {
    use super::*;
    use crate::{GuildDto, Ready, decode};
    use serde_json::json;

    #[test]
    fn guild_catalog_preserves_availability_roles_and_unknown_vs_empty() {
        let emoji =
            json!({"id":"4","name":"party_parrot","animated":true,"available":true,"roles":[]});
        let mut ready: Ready = decode(&serde_json::to_vec(&json!({
            "user":{"id":"1","username":"Synthetic"},"session_id":"synthetic","resume_gateway_url":"wss://gateway.discord.gg",
            "guilds":[{"id":"2","emojis":[emoji.clone()]},{"id":"3"},{"id":"5","emojis":[]}]
        })).unwrap()).unwrap();
        let (guilds, _) = ready.navigation().unwrap();
        let custom = &guilds[0].emojis.as_ref().unwrap()[0];
        assert_eq!(custom.markup(), "<a:party_parrot:4>");
        assert!(custom.usable());
        assert!(guilds[1].emojis.is_none());
        assert_eq!(guilds[2].emojis.as_deref(), Some([].as_slice()));
        for (field, value) in [
            ("roles", json!(["6"])),
            ("roles", json!(null)),
            ("available", json!(false)),
            ("managed", json!(true)),
        ] {
            let mut restricted = emoji.clone();
            restricted[field] = value;
            let guild: GuildDto =
                decode(&serde_json::to_vec(&json!({"id":"2","emojis":[restricted]})).unwrap())
                    .unwrap();
            assert!(!guild.emojis.unwrap().0[0].usable());
        }
        let update: GuildEmojisUpdate = decode(br#"{"guild_id":"2","emojis":[]}"#).unwrap();
        assert!(update.emojis.0.is_empty());
    }

    #[test]
    fn guild_catalog_rejects_duplicates_invalid_names_items_and_bytes() {
        let emoji = json!({"id":"4","name":"wave","available":true,"roles":[]});
        let oversized: Vec<_> = (1..=model::MAX_GUILD_EMOJIS + 1)
            .map(|id| {
                let mut e = emoji.clone();
                e["id"] = json!(id.to_string());
                e
            })
            .collect();
        let large_roles: Vec<_> = (1..=256).map(|id| id.to_string()).collect();
        let over_bytes: Vec<_> = (1..=256)
            .map(|id| json!({"id":id.to_string(),"name":"wave","roles":large_roles}))
            .collect();
        for values in [
            vec![emoji.clone(), emoji],
            oversized,
            over_bytes,
            vec![json!({"id":"4","name":"../unsafe"})],
            vec![json!({"id":"4","name":"x".repeat(33)})],
            vec![json!({"id":"4","name":"wave","roles":vec!["5";257]})],
        ] {
            assert!(
                decode::<GuildEmojisUpdate>(
                    &serde_json::to_vec(&json!({"guild_id":"2","emojis":values})).unwrap()
                )
                .is_err()
            );
        }
    }
}
