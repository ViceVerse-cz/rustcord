use crate::Id;

pub const SEARCH_PAGE_SIZE: usize = 25;
pub const MAX_SEARCH_BYTES: usize = 64 * 1024;
pub fn valid_search_query(query: &str) -> bool {
    !query.trim().is_empty()
        && query.len() <= 1024
        && query.chars().count() <= 256
        && !query.chars().any(char::is_control)
}
pub struct SearchHit {
    pub id: Id,
    pub channel: Id,
    pub author: String,
    pub excerpt: String,
}
pub struct SearchPage {
    pub hits: Vec<SearchHit>,
    pub total: u64,
    pub partial: bool,
}
impl SearchPage {
    pub fn bytes(&self) -> usize {
        self.hits.capacity() * size_of::<SearchHit>()
            + self
                .hits
                .iter()
                .map(|h| h.author.capacity() + h.excerpt.capacity())
                .sum::<usize>()
    }
    pub fn valid(&self, channel: Id, before: Option<Id>) -> bool {
        self.hits.len() <= SEARCH_PAGE_SIZE
            && self.bytes() <= MAX_SEARCH_BYTES
            && self.hits.iter().all(|h| {
                h.id.0 > 0
                    && h.channel == channel
                    && before.is_none_or(|b| h.id < b)
                    && h.author.len() <= 512
                    && h.excerpt.len() <= 1024
            })
            && self.hits.windows(2).all(|w| w[0].id > w[1].id)
    }
}
