//! A note's markdown, read into the blocks the reader and the editor draw.
//!
//! Every block and every styled run keeps the range of bytes it came from in
//! the note's own text, so a click on a checkbox can write back to the exact
//! line that drew it, and the editor can dim a marker without moving a
//! character.

use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// The style of a run of text inside a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Style {
    Plain,
    Strong,
    Emphasis,
    Strikethrough,
    Code,
    /// A link, with where it points.
    Link(String),
}

impl Style {
    /// Whether the run is one the reader typed as a marked span.
    pub fn marked(&self) -> bool {
        !matches!(self, Self::Plain)
    }
}

/// A run of text with one style, and where it sits in the note's source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inline {
    pub span: Range<usize>,
    pub style: Style,
    pub text: String,
}

/// What a block is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A heading, by its level from 1 to 6.
    Heading(u8),
    Paragraph,
    /// A list item that is not a task; `depth` counts the lists around it.
    List {
        ordered: bool,
        depth: usize,
    },
    /// A checklist item, with the range of its `[ ]` or `[x]`.
    Task {
        checked: bool,
        marker: Range<usize>,
        depth: usize,
    },
    /// A quoted paragraph.
    Quote,
    /// A fenced or indented code block, with the language it names.
    Code {
        language: String,
    },
    /// A horizontal rule.
    Rule,
}

/// One block of a note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub kind: Kind,
    /// The bytes of the source this block was read from.
    pub span: Range<usize>,
    pub inlines: Vec<Inline>,
    /// The block's words: a code block's body, a paragraph's text.
    pub text: String,
}

/// Reads a note's markdown into blocks, in the order they are written.
pub fn parse(source: &str) -> Vec<Block> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(source, options).into_offset_iter();

    let mut blocks: Vec<Block> = Vec::new();
    let mut draft: Option<Draft> = None;
    let mut styles: Vec<Style> = Vec::new();
    let mut quotes = 0usize;
    let mut lists: Vec<bool> = Vec::new();
    // One entry per open list item: the task marker it carries, if any.
    let mut items: Vec<Option<(bool, Range<usize>)>> = Vec::new();
    let mut code: Option<(String, Range<usize>, String)> = None;

    for (event, span) in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                draft = Some(Draft::new(span.clone(), Kind::Heading(level as u8)));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(draft) = draft.take() {
                    blocks.push(draft.finish());
                }
            }
            Event::Start(Tag::Paragraph) => {
                draft = Some(Draft::new(span.clone(), block_kind(&items, &lists, quotes)));
            }
            Event::End(TagEnd::Paragraph) => {
                close(&mut draft, &mut blocks);
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let language = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(language) => {
                        language.split_whitespace().next().unwrap_or("").to_owned()
                    }
                    pulldown_cmark::CodeBlockKind::Indented => String::new(),
                };
                code = Some((language, span.clone(), String::new()));
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((language, span, text)) = code.take() {
                    blocks.push(Block {
                        kind: Kind::Code { language },
                        span,
                        inlines: Vec::new(),
                        text,
                    });
                }
            }
            Event::Rule => blocks.push(Block {
                kind: Kind::Rule,
                span,
                inlines: Vec::new(),
                text: String::new(),
            }),
            Event::Start(Tag::BlockQuote(_)) => quotes += 1,
            Event::End(TagEnd::BlockQuote(_)) => quotes = quotes.saturating_sub(1),
            Event::Start(Tag::List(start)) => lists.push(start.is_some()),
            Event::End(TagEnd::List(_)) => {
                lists.pop();
            }
            Event::Start(Tag::Item) => items.push(None),
            Event::End(TagEnd::Item) => {
                // A tight list item holds its words with no paragraph around
                // them, so the block it opened is closed here.
                close(&mut draft, &mut blocks);
                items.pop();
            }
            Event::TaskListMarker(checked) => {
                if let Some(item) = items.last_mut() {
                    *item = Some((checked, span.clone()));
                }
            }
            Event::Start(Tag::Strong) => styles.push(Style::Strong),
            Event::End(TagEnd::Strong) => {
                pop_style(&mut styles, |style| matches!(style, Style::Strong))
            }
            Event::Start(Tag::Emphasis) => styles.push(Style::Emphasis),
            Event::End(TagEnd::Emphasis) => {
                pop_style(&mut styles, |style| matches!(style, Style::Emphasis))
            }
            Event::Start(Tag::Strikethrough) => styles.push(Style::Strikethrough),
            Event::End(TagEnd::Strikethrough) => {
                pop_style(&mut styles, |style| matches!(style, Style::Strikethrough))
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                styles.push(Style::Link(dest_url.to_string()))
            }
            Event::End(TagEnd::Link) => {
                pop_style(&mut styles, |style| matches!(style, Style::Link(_)))
            }
            Event::Text(text) => {
                if let Some((_, _, body)) = code.as_mut() {
                    body.push_str(&text);
                } else {
                    // A tight list item arrives with no paragraph around its
                    // words; the block starts with the first of them.
                    if draft.is_none() {
                        draft = Some(Draft::new(span.clone(), block_kind(&items, &lists, quotes)));
                    }
                    if let Some(draft) = draft.as_mut() {
                        draft.push(span.clone(), styles.last().cloned(), &text);
                    }
                }
            }
            Event::Code(text) => {
                if draft.is_none() {
                    draft = Some(Draft::new(span.clone(), block_kind(&items, &lists, quotes)));
                }
                // The inline-code span covers its backticks; the words it
                // styles are what the reader sees.
                let inner = trim_fence(&source[span.clone()], &span);
                if let Some(draft) = draft.as_mut() {
                    draft.push(inner, Some(Style::Code), &text);
                }
            }
            // A line broken inside a paragraph reads as a space; two spaces
            // before the newline ask for a line of its own.
            Event::SoftBreak => {
                if let Some(draft) = draft.as_mut() {
                    draft.push(span.clone(), None, " ");
                }
            }
            Event::HardBreak => {
                if let Some(draft) = draft.as_mut() {
                    draft.push(span.clone(), None, "\n");
                }
            }
            // Images, inline HTML, footnotes and math stay outside what a
            // note draws: their text, if any, arrives as other events.
            _ => {}
        }
    }
    blocks
}

/// The blocks of a note, as one paragraph of reading text: no markers, no
/// fences, one line per block.
pub fn plain(source: &str) -> String {
    parse(source)
        .iter()
        .map(|block| block.text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The first words of a note under its title, for a list row. The line that
/// names the note is left out, as the row shows it above.
pub fn excerpt(source: &str, title: &str, limit: usize) -> String {
    // The excerpt reads on from under a first line that already says the
    // title, whether as a heading or in the same words; a note that starts
    // with other words keeps them all.
    let first = source.split_inclusive('\n').next().unwrap_or(source);
    let line = first.trim().trim_start_matches('#').trim();
    let rest = &source[first.len().min(source.len())..];
    let body = if first.trim_start().starts_with('#') || line == title.trim() {
        rest
    } else {
        source
    };
    let text = plain(body);
    let mut excerpt = String::new();
    for word in text.split_whitespace() {
        if !excerpt.is_empty() {
            excerpt.push(' ');
        }
        if excerpt.chars().count() + word.chars().count() > limit {
            break;
        }
        excerpt.push_str(word);
    }
    excerpt
}

/// Flips one checklist item in the note's source: `[ ]` becomes `[x]` and the
/// other way round, and nothing else moves.
pub fn toggle_task(source: &str, marker: &Range<usize>) -> String {
    let mut text = source.to_owned();
    if marker.start >= marker.end || marker.end > text.len() {
        return text;
    }
    let marker = marker.clone();
    if !text.is_char_boundary(marker.start) || !text.is_char_boundary(marker.end) {
        return text;
    }
    let checked = text[marker.clone()].contains('x') || text[marker.clone()].contains('X');
    let replacement = if checked { "[ ]" } else { "[x]" };
    text.replace_range(marker, replacement);
    text
}

/// Drops the innermost open style of one kind, and anything opened inside it.
fn pop_style(styles: &mut Vec<Style>, matches: impl Fn(&Style) -> bool) {
    if let Some(index) = styles.iter().rposition(&matches) {
        styles.truncate(index);
    }
}

/// The span of an inline-code run without the backticks around it.
fn trim_fence(slice: &str, span: &Range<usize>) -> Range<usize> {
    let lead = slice.len() - slice.trim_start_matches('`').len();
    let tail = slice.len() - slice.trim_end_matches('`').len();
    let start = span.start + lead;
    let end = span.end.saturating_sub(tail).max(start);
    start..end
}

/// The kind a block takes from the lists, items and quotes around it.
fn block_kind(items: &[Option<(bool, Range<usize>)>], lists: &[bool], quotes: usize) -> Kind {
    match items.last().and_then(|item| item.clone()) {
        Some((checked, marker)) => Kind::Task {
            checked,
            marker,
            depth: lists.len().saturating_sub(1),
        },
        None if !lists.is_empty() => Kind::List {
            ordered: *lists.last().unwrap_or(&false),
            depth: lists.len() - 1,
        },
        None if quotes > 0 => Kind::Quote,
        None => Kind::Paragraph,
    }
}

/// Keeps the block being read, unless it holds nothing.
fn close(draft: &mut Option<Draft>, blocks: &mut Vec<Block>) {
    let Some(draft) = draft.take() else {
        return;
    };
    if !draft.text.trim().is_empty() {
        blocks.push(draft.finish());
    }
}

/// The block being read.
struct Draft {
    kind: Kind,
    span: Range<usize>,
    inlines: Vec<Inline>,
    text: String,
}

impl Draft {
    fn new(span: Range<usize>, kind: Kind) -> Self {
        Self {
            kind,
            span,
            inlines: Vec::new(),
            text: String::new(),
        }
    }

    fn push(&mut self, span: Range<usize>, style: Option<Style>, text: &str) {
        self.text.push_str(text);
        let style = style.unwrap_or(Style::Plain);
        match self.inlines.last_mut() {
            // Runs of one style that touch are painted as one.
            Some(last) if last.style == style && last.span.end == span.start => {
                last.span.end = span.end;
                last.text.push_str(text);
            }
            _ => self.inlines.push(Inline {
                span,
                style,
                text: text.to_owned(),
            }),
        }
    }

    fn finish(self) -> Block {
        Block {
            kind: self.kind,
            span: self.span,
            inlines: self.inlines,
            text: self.text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heading_carries_its_level_and_its_bytes() {
        let source = "# Rust Berlin\n\nThursday at c-base.";
        let blocks = parse(source);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind, Kind::Heading(1));
        assert_eq!(&source[blocks[0].span.clone()], "# Rust Berlin\n");
        assert_eq!(blocks[0].text, "Rust Berlin");
        assert_eq!(blocks[1].kind, Kind::Paragraph);
        assert_eq!(blocks[1].text, "Thursday at c-base.");
    }

    #[test]
    fn both_list_kinds_are_told_apart() {
        let blocks = parse("- milk\n- bread\n\n1. one\n2. two\n");
        assert_eq!(blocks.len(), 4);
        assert_eq!(
            blocks[0].kind,
            Kind::List {
                ordered: false,
                depth: 0
            }
        );
        assert_eq!(
            blocks[2].kind,
            Kind::List {
                ordered: true,
                depth: 0
            }
        );
        assert_eq!(blocks[3].text, "two");
    }

    #[test]
    fn a_task_item_keeps_its_marker_range() {
        let source = "- [ ] who signs the lease\n- [x] the budget\n";
        let blocks = parse(source);
        assert_eq!(blocks.len(), 2);
        let (checked, marker) = match blocks[0].kind.clone() {
            Kind::Task {
                checked, marker, ..
            } => (checked, marker),
            other => panic!("a task item, not {other:?}"),
        };
        assert!(!checked);
        assert_eq!(&source[marker.clone()], "[ ]");
        assert_eq!(blocks[0].text, "who signs the lease");
        let flipped = toggle_task(source, &marker);
        assert_eq!(
            flipped, "- [x] who signs the lease\n- [x] the budget\n",
            "only the marker the block named changes"
        );
        assert!(matches!(blocks[1].kind, Kind::Task { checked: true, .. }));
    }

    #[test]
    fn toggling_a_task_flips_only_its_marker() {
        let source = "- [x] done\n- [ ] not yet\n";
        let blocks = parse(source);
        let marker = match blocks[1].kind.clone() {
            Kind::Task { marker, .. } => marker,
            other => panic!("a task item, not {other:?}"),
        };
        assert_eq!(toggle_task(source, &marker), "- [x] done\n- [x] not yet\n");
        // A range that is not a marker leaves the note as it was.
        assert_eq!(toggle_task(source, &(0..0)), source);
    }

    #[test]
    fn a_quote_is_a_block() {
        let blocks = parse("> the lease is signed\n");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].kind, Kind::Quote);
        assert_eq!(blocks[0].text, "the lease is signed");
    }

    #[test]
    fn a_fenced_block_keeps_its_language_and_text() {
        let source = "```rust\nlet x = 1;\n```\n";
        let blocks = parse(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].kind,
            Kind::Code {
                language: "rust".into()
            }
        );
        assert_eq!(blocks[0].text, "let x = 1;\n");
    }

    #[test]
    fn a_rule_is_a_block() {
        let blocks = parse("one\n\n---\n\ntwo\n");
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[1].kind, Kind::Rule);
        assert_eq!(blocks[1].text, "");
    }

    #[test]
    fn inline_styles_keep_their_ranges() {
        let source =
            "a **bold** and _soft_ and ~~gone~~ and `code` and [link](https://example.com)";
        let blocks = parse(source);
        let block = &blocks[0];
        let styled = |wanted: &str| {
            block
                .inlines
                .iter()
                .find(|inline| {
                    matches!(
                        (&inline.style, wanted),
                        (Style::Strong, "strong")
                            | (Style::Emphasis, "emphasis")
                            | (Style::Strikethrough, "strike")
                            | (Style::Code, "code")
                            | (Style::Link(_), "link")
                    )
                })
                .map(|inline| (inline.text.clone(), inline.span.clone()))
        };
        for (wanted, text) in [
            ("strong", "bold"),
            ("emphasis", "soft"),
            ("strike", "gone"),
            ("code", "code"),
            ("link", "link"),
        ] {
            let (found, span) = styled(wanted).unwrap_or_else(|| panic!("no {wanted} run"));
            assert_eq!(found, text, "{wanted} text");
            assert_eq!(
                &source[span.clone()],
                text,
                "the {wanted} range names the words, not the markers"
            );
        }
        assert_eq!(
            styled("link").map(|_| ()),
            Some(()),
            "a link keeps where it points"
        );
        let link = block.inlines.iter().find_map(|inline| match &inline.style {
            Style::Link(url) => Some(url.as_str()),
            _ => None,
        });
        assert_eq!(link, Some("https://example.com"));
    }

    #[test]
    fn plain_text_holds_no_markup() {
        let source = "# Title\n\n- one\n- **two**\n\n> quoted\n\n```rust\nlet x = 1;\n```\n";
        assert_eq!(
            plain(source),
            "Title\none\ntwo\nquoted\nlet x = 1;",
            "no heading marker, bullet, emphasis or fence"
        );
    }

    #[test]
    fn an_excerpt_reads_the_words_under_the_title() {
        let source =
            "# Rust Berlin\n\nThursday, 19:00, at c-base.\n\nThings to ask:\n- who signs the lease";
        assert_eq!(
            excerpt(source, "Rust Berlin", 80),
            "Thursday, 19:00, at c-base. Things to ask: who signs the lease"
        );
        assert!(excerpt(source, "Rust Berlin", 20).chars().count() <= 20);
        assert!(!excerpt(source, "Rust Berlin", 80).contains('#'));
        // A note that starts with its own words keeps the first of them.
        let list = "- The Soul of a New Machine\n- A Philosophy of Software Design";
        assert_eq!(
            excerpt(list, "Reading list", 80),
            "The Soul of a New Machine A Philosophy of Software Design"
        );
    }
}
