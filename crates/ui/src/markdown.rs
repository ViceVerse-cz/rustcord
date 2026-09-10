//! Bounded native text formatting. No HTML renderer, image loader, or automatic URL access.
use egui::{FontId, Stroke, TextFormat, text::LayoutJob};
use model::Id;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, TextMergeWithOffset};
use std::collections::VecDeque;
use unicode_segmentation::UnicodeSegmentation;

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
    link: Option<usize>,
    mention: Option<Id>,
    no_autolink: bool,
}
pub struct Formatted {
    spans: Vec<(String, Style)>,
    mention_count: usize,
    pub links: Vec<String>,
    pub limited: bool,
    pub spoilers: bool,
}

#[derive(Default)]
pub struct FormatCache {
    entries: VecDeque<((Id, u16), String, Formatted)>,
    bytes: usize,
}
impl FormatCache {
    pub fn retain(&mut self, mut keep: impl FnMut(Id) -> bool) {
        self.entries.retain(|((id, _), _, _)| keep(*id));
        self.bytes = self
            .entries
            .iter()
            .map(|(_, source, parsed)| source.capacity() + parsed.bytes())
            .sum();
    }
    pub fn get(&mut self, id: Id, source: &str) -> &Formatted {
        self.get_part(id, 0, source)
    }
    pub fn get_part(&mut self, message: Id, part: u16, source: &str) -> &Formatted {
        let id = (message, part);
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
            mention_count: 0,
            links: Vec::new(),
            limited: end < source.len(),
            // ponytail: conceal the entire message for spoiler syntax, including code literals;
            // replace with per-span concealment when Discord-specific parsing is implemented.
            spoilers: source.contains("||"),
        };
        let mut stack = Vec::new();
        let mut style = Style::default();
        for (count, (event, range)) in TextMergeWithOffset::new(
            Parser::new_ext(input, Options::ENABLE_STRIKETHROUGH).into_offset_iter(),
        )
        .enumerate()
        {
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
                            style.no_autolink = true;
                            style.link = output.add_link(&dest_url);
                        }
                        Tag::Image { .. } => {
                            style.no_autolink = true;
                            output.push("[image: ", style);
                        }
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
                    if style.code || style.no_autolink {
                        output.push(&text, style);
                    } else {
                        output.push_mentions(&text, style, &input[range]);
                    }
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
    fn add_link(&mut self, target: &str) -> Option<usize> {
        let url = external_url(target)?;
        if let Some(index) = self.links.iter().position(|existing| *existing == url) {
            return Some(index);
        }
        if self.links.len() >= MAX_LINKS {
            return None;
        }
        self.links.push(url);
        Some(self.links.len() - 1)
    }
    fn push_autolinks(&mut self, text: &str, style: Style) {
        let mut consumed = 0;
        let mut scanned = 0;
        for word in text.split_whitespace() {
            let start = scanned + text[scanned..].find(word).expect("word from source");
            scanned = start + word.len();
            let candidate = word.trim_start_matches(['(', '[', '{']);
            let mut target = candidate.trim_end_matches(['.', ',', ';', '!', '?', ']', '}']);
            while target.ends_with(')') && target.matches(')').count() > target.matches('(').count()
            {
                target = &target[..target.len() - 1];
            }
            if (target.starts_with("https://") || target.starts_with("http://"))
                && let Some(link) = self.add_link(target)
            {
                let link_start = start + word.len() - candidate.len();
                self.push(&text[consumed..link_start], style);
                self.push(
                    target,
                    Style {
                        link: Some(link),
                        ..style
                    },
                );
                consumed = link_start + target.len();
            }
        }
        self.push(&text[consumed..], style);
    }
    fn push_mentions(&mut self, text: &str, style: Style, source: &str) {
        let mut consumed = 0;
        let mut raw_cursor = 0;
        for (start, _) in text.match_indices("<@") {
            let Some((id, len)) = model::user_mention_prefix(&text[start..]) else {
                continue;
            };
            if self.mention_count >= model::MAX_MENTIONS {
                self.limited = true;
                break;
            }
            let token = &text[start..start + len];
            let Some(raw_start) = source[raw_cursor..].find(token).map(|i| i + raw_cursor) else {
                continue;
            };
            raw_cursor = raw_start + len;
            if source[..raw_start]
                .bytes()
                .rev()
                .take_while(|b| *b == b'\\')
                .count()
                % 2
                == 1
            {
                continue;
            }
            self.push_autolinks(&text[consumed..start], style);
            self.push(
                token,
                Style {
                    mention: Some(id),
                    ..style
                },
            );
            self.mention_count += 1;
            consumed = start + len;
        }
        self.push_autolinks(&text[consumed..], style);
    }
    #[cfg(test)]
    pub fn show(&self, ui: &mut egui::Ui, opening: &mut Option<String>) {
        self.show_mentions(ui, opening, &[], &mut None);
    }
    #[cfg(test)]
    pub fn show_mentions(
        &self,
        ui: &mut egui::Ui,
        opening: &mut Option<String>,
        users: &[model::User],
        profile: &mut Option<model::User>,
    ) {
        self.show_with_images(
            ui,
            opening,
            users,
            profile,
            &mut crate::avatars::Avatars::default(),
            true,
        );
    }
    pub fn show_with_images(
        &self,
        ui: &mut egui::Ui,
        opening: &mut Option<String>,
        users: &[model::User],
        profile: &mut Option<model::User>,
        images: &mut crate::avatars::Avatars,
        demo: bool,
    ) {
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 0.0),
            egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
            |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                let mut start = 0;
                while start < self.spans.len() {
                    if let Some(id) = self.spans[start].1.mention {
                        let user = users.iter().find(|user| user.id == id);
                        let label = format!(
                            "@{}",
                            user.map_or_else(|| id.to_string(), |u| u.name.clone())
                        );
                        let response = ui
                            .add(egui::Link::new(egui::RichText::new(&label).strong()))
                            .on_hover_text("Open user profile");
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Link,
                                ui.is_enabled(),
                                format!("{label}, user profile"),
                            )
                        });
                        if response.clicked() {
                            *profile = Some(user.cloned().unwrap_or(model::User {
                                id,
                                name: format!("User {id}"),
                                avatar: None,
                                discriminator: 0,
                            }));
                        }
                        start += 1;
                        continue;
                    }
                    let target = self.spans[start].1.link;
                    let count = self.spans[start..]
                        .iter()
                        .take_while(|(_, style)| style.link == target && style.mention.is_none())
                        .count();
                    let spans = &self.spans[start..start + count];
                    if let Some(index) = target {
                        let url = &self.links[index];
                        let label: String = spans.iter().map(|(text, _)| text.as_str()).collect();
                        let response =
                            Self::show_emoji(spans, ui, true, images, demo).on_hover_text(url);
                        // Text selection in egui's Link overwrites its accessibility role.
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Link,
                                ui.is_enabled(),
                                &label,
                            )
                        });
                        if response.clicked() {
                            *opening = Some(url.clone());
                        }
                    } else {
                        Self::show_emoji(spans, ui, false, images, demo);
                    }
                    start += count;
                }
            },
        );
    }
    fn show_emoji(
        spans: &[(String, Style)],
        ui: &mut egui::Ui,
        link: bool,
        images: &mut crate::avatars::Avatars,
        demo: bool,
    ) -> egui::Response {
        let mut response: Option<egui::Response> = None;
        let mut pending = Vec::new();
        let size = egui::TextStyle::Body.resolve(ui.style()).size * 1.25;
        let flush = |pending: &mut Vec<(String, Style)>, ui: &mut egui::Ui| {
            let job = Self::layout(pending, ui);
            pending.clear();
            if link {
                ui.add(egui::Link::new(job))
            } else {
                ui.add(egui::Label::new(job).wrap().selectable(true))
            }
        };
        for (text, style) in spans {
            let mut start = 0;
            let mut offset = 0;
            while offset < text.len() {
                let custom = (!style.code)
                    .then(|| crate::emoji::custom_prefix(&text[offset..]))
                    .flatten();
                let len = custom.map_or_else(
                    || {
                        text[offset..]
                            .graphemes(true)
                            .next()
                            .expect("remaining text")
                            .len()
                    },
                    |(_, len)| len,
                );
                let cluster = &text[offset..offset + len];
                let image = if custom.is_none() && !style.code {
                    crate::emoji::image(ui.ctx(), cluster, size)
                } else {
                    None
                };
                if image.is_none() && custom.is_none() {
                    offset += len;
                    continue;
                }
                if offset > start {
                    pending.push((text[start..offset].to_owned(), *style));
                }
                if !pending.is_empty() {
                    let next = flush(&mut pending, ui);
                    response = Some(response.map_or(next.clone(), |r| r.union(next)));
                }
                let next = crate::emoji::selectable(
                    ui,
                    cluster,
                    |ui| {
                        if let Some((id, _)) = custom {
                            images.custom_image(ui.ctx(), id, size, demo)
                        } else {
                            image
                        }
                    },
                    size,
                    link,
                );
                response = Some(response.map_or(next.clone(), |r| r.union(next)));
                offset += len;
                start = offset;
            }
            if start < text.len() {
                pending.push((text[start..].to_owned(), *style));
            }
        }
        if !pending.is_empty() || response.is_none() {
            let next = flush(&mut pending, ui);
            response = Some(response.map_or(next.clone(), |r| r.union(next)));
        }
        response.expect("text or emoji response")
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
    fn layout(spans: &[(String, Style)], ui: &egui::Ui) -> LayoutJob {
        let mut job = LayoutJob::default();
        let visuals = ui.visuals();
        let body = egui::TextStyle::Body.resolve(ui.style());
        for (text, style) in spans {
            let color = if style.link.is_some() {
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
                    underline: if style.link.is_some() {
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
    fn selecting_across_images_copies_unicode_and_custom_markup() {
        let ctx = egui::Context::default();
        crate::emoji::install(&ctx).unwrap();
        let source = "A 👩🏽‍💻 ❤️ <:serein_wave:9001> Z";
        let parsed = Formatted::parse(source);
        let mut avatars = crate::avatars::Avatars::default();
        let mut clock = 0.0;
        let mut run = |events| {
            clock += 1.0;
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 200.0),
                    )),
                    events,
                    time: Some(clock),
                    ..Default::default()
                },
                |ui| parsed.show_with_images(ui, &mut None, &[], &mut None, &mut avatars, true),
            )
        };
        let mut output = run(vec![]);
        let mut start = egui::Pos2::ZERO;
        let mut end = egui::Pos2::ZERO;
        let mut custom_rect = egui::Rect::NOTHING;
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape {
                if text.galley.text() == "A " {
                    start = text.pos + egui::vec2(0.0, 5.0);
                }
                if text.galley.text() == "<:serein_wave:9001>" {
                    custom_rect = egui::Rect::from_min_size(text.pos, text.galley.size());
                }
                if text.galley.text().starts_with(" Z") {
                    end = text.pos + egui::vec2(text.galley.size().x, 5.0);
                }
            }
        }
        output.textures_delta.clear();
        assert!(end.x > start.x);
        for events in [
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            vec![egui::Event::PointerMoved(end)],
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        ] {
            run(events).drop_without_applying_deltas();
        }
        let mut output = run(vec![egui::Event::Copy]);
        output.textures_delta.clear();
        let copied = output
            .platform_output
            .commands
            .iter()
            .find_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.as_str()),
                _ => None,
            });
        assert_eq!(copied.map(str::trim_end), Some(source));
        for fraction in [0.25, 0.75] {
            let start = custom_rect.left_top() + egui::vec2(custom_rect.width() * fraction, 5.0);
            for events in [
                vec![
                    egui::Event::PointerMoved(start),
                    egui::Event::PointerButton {
                        pos: start,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                vec![egui::Event::PointerMoved(end)],
                vec![egui::Event::PointerButton {
                    pos: end,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            ] {
                run(events).drop_without_applying_deltas();
            }
            let mut output = run(vec![egui::Event::Copy]);
            output.textures_delta.clear();
            let copied = output
                .platform_output
                .commands
                .iter()
                .find_map(|command| match command {
                    egui::OutputCommand::CopyText(text) => Some(text.as_str()),
                    _ => None,
                });
            assert!(
                matches!(
                    copied.map(str::trim_end),
                    Some("<:serein_wave:9001> Z" | " Z")
                ),
                "partial emoji copied: {copied:?}"
            );
        }
    }
    #[test]
    fn emoji_render_as_whole_images_but_code_and_source_stay_literal() {
        let ctx = egui::Context::default();
        crate::emoji::install(&ctx).unwrap();
        let source = "👩🏽‍💻 `😀` 🇨🇿";
        let parsed = Formatted::parse(source);
        assert!(
            parsed
                .spans
                .iter()
                .any(|(text, style)| style.code && text == "😀")
        );
        let output = ctx.run_ui(Default::default(), |ui| {
            parsed.show(ui, &mut None);
        });
        let images = output
            .shapes
            .iter()
            .filter(|shape| {
                matches!(
                    &shape.shape, egui::Shape::Rect(rect) if rect.brush.is_some()
                )
            })
            .count();
        output.drop_without_applying_deltas();
        assert_eq!(images, 2, "one image per complete grapheme, none in code");
    }
    #[test]
    fn user_mentions_preserve_literals_and_open_native_profiles() {
        let parsed = Formatted::parse(
            "Hello **<@42>** <@!42> `<@43>` \\<@44> &lt;@45&gt; [<@46>](https://example.com) <@&47>",
        );
        assert_eq!(
            parsed
                .spans
                .iter()
                .filter_map(|(_, style)| style.mention)
                .collect::<Vec<_>>(),
            vec![Id(42), Id(42)]
        );
        assert!(
            parsed
                .spans
                .iter()
                .any(|(_, style)| style.mention == Some(Id(42)) && style.strong)
        );
        let parsed = Formatted::parse("<@42>");
        let ctx = egui::Context::default();
        let mut profile = None;
        let mut opening = None;
        let users = vec![model::User {
            id: Id(42),
            name: "Synthetic Robin".into(),
            avatar: None,
            discriminator: 0,
        }];
        for key in [egui::Key::Tab, egui::Key::Enter] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ui| parsed.show_mentions(ui, &mut opening, &users, &mut profile),
            );
            assert!(output.platform_output.commands.is_empty());
            output.textures_delta.clear();
        }
        assert_eq!(profile.unwrap().name, "Synthetic Robin");
        assert!(opening.is_none());
    }
    #[test]
    fn inline_links_preserve_markdown_and_activate_their_own_destinations() {
        let source = "Before [**Markdown** *label*](https://example.com/masked) then (https://example.org/a_(b)). `https://code.test` [https://label.test](javascript:bad)";
        let parsed = Formatted::parse(source);
        assert_eq!(
            parsed.links,
            ["https://example.com/masked", "https://example.org/a_(b)"]
        );
        assert!(
            parsed
                .spans
                .iter()
                .any(|(s, f)| s == "Markdown" && f.strong && f.link == Some(0))
        );
        assert!(
            parsed
                .spans
                .iter()
                .any(|(s, f)| s == "label" && f.italic && f.link == Some(0))
        );
        assert!(
            parsed
                .spans
                .iter()
                .any(|(s, f)| s == "https://example.org/a_(b)" && f.link == Some(1))
        );
        let repeated = Formatted::parse("nothttps://example.org https://example.org");
        assert_eq!(repeated.spans[0].0, "nothttps://example.org ");
        assert!(repeated.spans[0].1.link.is_none());

        let ctx = egui::Context::default();
        let mut opening = None;
        let mut render = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(220.0, 500.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| parsed.show(ui, &mut opening),
            );
            assert!(
                output.platform_output.commands.is_empty(),
                "Links must use confirmation, never open while rendering or on first activation"
            );
            output.textures_delta.clear();
            opening.take()
        };
        assert!(render(vec![]).is_none());
        // Native links participate in keyboard focus; each targets its own normalized URL.
        for target in &parsed.links {
            assert!(
                render(vec![egui::Event::Key {
                    key: egui::Key::Tab,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }])
                .is_none()
            );
            assert_eq!(
                render(vec![egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }]),
                Some(target.clone())
            );
            render(vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
        }
    }
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
