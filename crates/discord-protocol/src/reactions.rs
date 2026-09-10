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
