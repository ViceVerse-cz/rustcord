//! Bounded native text formatting. No HTML renderer, image loader, or automatic URL access.
use egui::{FontId, Stroke, TextFormat, text::LayoutJob};
use model::Id;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::collections::VecDeque;

const MAX_INPUT: usize = 8192;
const MAX_EVENTS: usize = 512;
const MAX_DEPTH: usize = 16;
const MAX_LINKS: usize = 16;

#[derive(Clone, Copy, Default)]
struct Style {
    strong: bool,
    italic: bool,
    code: bool,
    strike: bool,
    quote: bool,
    link: bool,
}
pub struct Formatted {
    spans: Vec<(String, Style)>,
    pub links: Vec<String>,
    pub limited: bool,
    pub spoilers: bool,
}

#[derive(Default)]
pub struct FormatCache {
    entries: VecDeque<(Id, String, Formatted)>,
    bytes: usize,
}
impl FormatCache {
    pub fn retain(&mut self, mut keep: impl FnMut(Id) -> bool) {
        self.entries.retain(|(id, _, _)| keep(*id));
        self.bytes = self
            .entries
            .iter()
            .map(|(_, source, parsed)| source.capacity() + parsed.bytes())
            .sum();
    }
    pub fn get(&mut self, id: Id, source: &str) -> &Formatted {
        let existing = self.entries.iter().position(|(cached, _, _)| *cached == id);
        let entry = existing.and_then(|index| self.entries.remove(index));
        let entry = match entry {
            Some((id, cached, parsed)) if cached == source => (id, cached, parsed),
            _ => {
                let mut end = source.len().min(64 * 1024);
                while !source.is_char_boundary(end) {
                    end -= 1;
                }
                (id, source[..end].to_owned(), Formatted::parse(source))
            }
        };
        self.entries.push_back(entry);
        self.retain(|_| true);
        while self.entries.len() > 64 || self.bytes > 1024 * 1024 {
            let (_, source, parsed) = self.entries.pop_front().expect("cache over budget");
            self.bytes -= source.capacity() + parsed.bytes();
        }
        &self.entries.back().expect("one bounded message fits").2
    }
}

/// The URL passed to the OS is exactly the normalized target displayed for confirmation.
pub fn external_url(input: &str) -> Option<String> {
    if input.len() > 2048 || input.chars().any(|c| c.is_control() || c == '\\') {
        return None;
    }
    let url = url::Url::parse(input).ok()?;
    (matches!(url.scheme(), "https" | "http")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| url.to_string())
}

impl Formatted {
    pub fn parse(source: &str) -> Self {
        let mut end = source.len().min(MAX_INPUT);
        while !source.is_char_boundary(end) {
            end -= 1;
        }
        if let Some((line_end, _)) = source[..end].match_indices('\n').nth(127) {
            end = line_end;
        }
        let input = &source[..end];
        let mut output = Self {
            spans: Vec::new(),
            links: Vec::new(),
            limited: end < source.len(),
            // ponytail: conceal the entire message for spoiler syntax, including code literals;
            // replace with per-span concealment when Discord-specific parsing is implemented.
            spoilers: source.contains("||"),
        };
        let mut stack = Vec::new();
        let mut style = Style::default();
        for (count, event) in Parser::new_ext(input, Options::ENABLE_STRIKETHROUGH).enumerate() {
            if count >= MAX_EVENTS || stack.len() > MAX_DEPTH {
                // Complexity overflow displays bounded literal text, never a partial misleading parse.
                output.spans = vec![(input.to_owned(), Style::default())];
                output.links.clear();
                output.limited = true;
                return output;
            }
            match event {
                Event::Start(tag) => {
                    stack.push(style);
                    match tag {
                        Tag::Strong | Tag::Heading { .. } => style.strong = true,
                        Tag::Emphasis => style.italic = true,
                        Tag::Strikethrough => style.strike = true,
                        Tag::CodeBlock(_) => style.code = true,
                        Tag::BlockQuote(_) => {
                            output.push("│ ", style);
                            style.quote = true;
                        }
                        Tag::Item => output.push("• ", style),
                        Tag::Link { dest_url, .. } => {
                            if let Some(url) = external_url(&dest_url)
                                && output.links.len() < MAX_LINKS
                            {
                                if !output.links.contains(&url) {
                                    output.links.push(url);
                                }
                                style.link = true;
                            }
                        }
                        Tag::Image { .. } => output.push("[image: ", style),
                        _ => {}
                    }
                }
                Event::End(tag) => {
                    match tag {
                        TagEnd::Paragraph
                        | TagEnd::Heading(_)
                        | TagEnd::CodeBlock
                        | TagEnd::BlockQuote(_)
                        | TagEnd::Item => output.push("\n", style),
                        TagEnd::Image => output.push("]", style),
                        _ => {}
                    }
                    style = stack.pop().unwrap_or_default();
                }
                Event::Text(text) => {
                    if !style.code {
                        for word in text.split_whitespace() {
                            let target = word.trim_end_matches(['.', ',', ';', '!', '?', ')', ']']);
                            if (target.starts_with("https://") || target.starts_with("http://"))
                                && output.links.len() < MAX_LINKS
                                && let Some(url) = external_url(target)
                                && !output.links.contains(&url)
                            {
                                output.links.push(url);
                            }
                        }
                    }
                    output.push(&text, style);
                }
                Event::Html(text) | Event::InlineHtml(text) => {
                    output.push(&text, style);
                }
                Event::Code(text) => output.push(
                    &text,
                    Style {
                        code: true,
                        ..style
                    },
                ),
                Event::SoftBreak | Event::HardBreak => output.push("\n", style),
                Event::Rule => output.push("────────\n", style),
                _ => {}
            }
        }
        output
    }
    fn push(&mut self, text: &str, style: Style) {
        if !text.is_empty() {
            self.spans.push((text.to_owned(), style));
        }
    }
    pub fn bytes(&self) -> usize {
        self.spans.capacity() * size_of::<(String, Style)>()
            + self.spans.iter().map(|(s, _)| s.capacity()).sum::<usize>()
            + self.links.capacity() * size_of::<String>()
            + self.links.iter().map(String::capacity).sum::<usize>()
    }
    pub fn layout(&self, ui: &egui::Ui) -> LayoutJob {
        let mut job = LayoutJob::default();
        let visuals = ui.visuals();
        let body = egui::TextStyle::Body.resolve(ui.style());
        for (text, style) in &self.spans {
            let color = if style.link {
                visuals.hyperlink_color
            } else if style.strong {
                visuals.strong_text_color()
            } else if style.quote {
                visuals.weak_text_color()
            } else {
                visuals.text_color()
            };
            job.append(
                text,
                0.0,
                TextFormat {
                    font_id: if style.code {
                        FontId::monospace(body.size)
                    } else {
                        body.clone()
                    },
                    color,
                    background: if style.code {
                        visuals.code_bg_color
                    } else {
                        egui::Color32::TRANSPARENT
                    },
                    italics: style.italic,
                    strikethrough: if style.strike {
                        Stroke::new(1.0, color)
                    } else {
                        Stroke::NONE
                    },
                    underline: if style.link {
                        Stroke::new(1.0, color)
                    } else {
                        Stroke::NONE
                    },
                    ..Default::default()
                },
            );
        }
        job
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_formatting_and_inert_external_content() {
        let parsed = Formatted::parse(
            "**strong** *em* ~~gone~~ `code`\n> quote\n\n[site](https://example.com/a) ![alt](https://example.com/image) <script>inert</script>",
        );
        assert!(parsed.spans.iter().any(|(s, f)| s == "strong" && f.strong));
        assert!(parsed.spans.iter().any(|(s, f)| s == "em" && f.italic));
        assert!(parsed.spans.iter().any(|(s, f)| s == "gone" && f.strike));
        assert!(parsed.spans.iter().any(|(s, f)| s == "code" && f.code));
        assert_eq!(parsed.links, ["https://example.com/a"]);
        assert!(parsed.spans.iter().any(|(s, _)| s.contains("<script>")));
        for unsafe_url in [
            "javascript:alert(1)",
            "file:///tmp/test",
            "data:text/html,x",
            "https://owner:secret@example.com",
            "https://example.com\n",
            "https:\\example.com",
        ] {
            assert!(external_url(unsafe_url).is_none());
        }
        assert_eq!(
            external_url("https://例え.jp"),
            Some("https://xn--r8jz45g.jp/".into())
        );
        let deep = Formatted::parse(&format!("{}text", "> ".repeat(100)));
        assert!(deep.limited && deep.links.is_empty());
        let huge = Formatted::parse(&"日本語".repeat(10_000));
        assert!(huge.limited && huge.bytes() < 16 * 1024);
        assert!(Formatted::parse("||**concealed**||").spoilers);
        assert_eq!(
            Formatted::parse("https://example.com/a, `https://example.com/private`").links,
            ["https://example.com/a"]
        );
        let mut cache = FormatCache::default();
        for id in 0..500 {
            cache.get(Id(id), &format!("{id} {}", "日本語".repeat(2000)));
        }
        assert!(cache.entries.len() <= 64 && cache.bytes <= 1024 * 1024);
        assert!(cache.get(Id(499), "||changed||").spoilers);
        cache.retain(|_| false);
        assert!(cache.entries.is_empty() && cache.bytes == 0);
    }
}
