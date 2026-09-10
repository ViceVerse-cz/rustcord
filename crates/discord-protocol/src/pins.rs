use crate::{
    Timestamp,
    search::{Hit, list},
};
use model::{Id, SearchPage};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Reply {
    #[serde(deserialize_with = "list::<_,_,25>")]
    items: Vec<Pin>,
    has_more: bool,
}
#[derive(Deserialize)]
struct Pin {
    #[serde(rename = "pinned_at")]
    _pinned_at: Timestamp,
    message: Hit,
}
impl Reply {
    pub fn into_page(self, channel: Id) -> Result<SearchPage, &'static str> {
        let page = SearchPage {
            hits: self
                .items
                .into_iter()
                .map(|pin| pin.message.into_hit())
                .collect(),
            // Pins have no service-provided total. The UI uses the page length only.
            total: 0,
            partial: self.has_more,
        };
        if !page.valid_pins(channel) {
            return Err("Invalid pinned messages");
        }
        Ok(page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn pins_preserve_pin_order_and_reject_wrong_scope_duplicates_and_capacity() {
        let pin = |id: u64, channel: u64| json!({"pinned_at":"2026-09-10T12:00:00Z","message":{"id":id.to_string(),"channel_id":channel.to_string(),"author":{"id":"7","username":"Synthetic"},"content":"hidden ||synthetic secret||"}});
        let decode =
            |value: serde_json::Value| crate::decode::<Reply>(&serde_json::to_vec(&value).unwrap());
        let page = decode(json!({"items":[pin(2,1),pin(9,1)],"has_more":true}))
            .unwrap()
            .into_page(Id(1))
            .unwrap();
        assert_eq!(
            page.hits.iter().map(|h| h.id).collect::<Vec<_>>(),
            vec![Id(2), Id(9)]
        );
        assert!(page.partial);
        assert!(!page.hits[0].excerpt.contains("synthetic secret"));
        for items in [vec![pin(2, 2)], vec![pin(2, 1), pin(2, 1)]] {
            assert!(
                decode(json!({"items":items,"has_more":false}))
                    .unwrap()
                    .into_page(Id(1))
                    .is_err()
            );
        }
        assert!(decode(json!({"items":vec![pin(2,1);26],"has_more":true})).is_err());
        assert!(
            decode(json!({"items":[],"has_more":false}))
                .unwrap()
                .into_page(Id(1))
                .unwrap()
                .hits
                .is_empty()
        );
        assert!(decode(json!({"items":[]})).is_err());
    }
}
