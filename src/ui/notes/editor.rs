//! The note being written: its markdown laid out as it will read, with the
//! markers kept in the text and painted quietly.
//!
//! The layout keeps every byte the reader typed, in order, because the caret
//! egui draws is an index into that same text: styling a character must never
//! move one.

use std::ops::Range;

use egui::text::{CCursor, CCursorRange, LayoutJob};
use egui::{FontId, Stroke, TextFormat};

use crate::app::App;
use crate::i18n::{Locale, gettext};
use crate::model::Action;
use crate::notes::edit;
use crate::notes::markdown::{self, Kind, Style};
use crate::theme::{self, Palette};

/// The size a note is written in.
const BODY: f32 = 14.0;

/// Draws the note's body, styled as it will read, and saves it as it is typed.
pub fn show(app: &mut App, ui: &mut egui::Ui, palette: &Palette, locale: Locale) -> egui::Response {
    let id = super::editor_id();
    if ui.memory(|memory| memory.has_focus(id)) {
        continue_the_list(app, ui, id);
    }
    let hint = gettext(locale, "Write a note");
    let mut body = app.note_draft.clone();
    let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, _wrap: f32| {
        let job = layout_job(buffer.as_str(), palette);
        ui.fonts_mut(|fonts| fonts.layout_job(job))
    };
    let response = ui.add(
        egui::TextEdit::multiline(&mut body)
            .id(id)
            .font(theme::regular(BODY))
            .text_color(palette.text)
            .desired_width(f32::INFINITY)
            .desired_rows(18)
            .margin(egui::vec2(2.0, 2.0))
            .hint_text(egui::RichText::new(hint.as_ref()).color(palette.secondary))
            .layouter(&mut layouter),
    );
    if response.changed() {
        app.note_draft = body;
        app.actions.push(Action::SaveNote);
    }
    response
}

/// Enter inside a list carries its marker to the next line, and an empty item
/// ends the list. The key is taken before the editor sees it, which would
/// otherwise only break the line.
fn continue_the_list(app: &mut App, ui: &egui::Ui, id: egui::Id) {
    let pressed = ui.input_mut(|input| {
        let mut pressed = false;
        input.events.retain(|event| {
            let egui::Event::Key {
                key: egui::Key::Enter,
                pressed: down,
                modifiers,
                ..
            } = event
            else {
                return true;
            };
            if !*down || modifiers.shift {
                return true;
            }
            pressed = true;
            false
        });
        pressed
    });
    if !pressed {
        return;
    }
    let Some(state) = egui::text_edit::TextEditState::load(ui.ctx(), id) else {
        return;
    };
    let Some(range) = state.cursor.char_range() else {
        return;
    };
    let Some(edit) = edit::continue_list(&app.note_draft, range.primary.index.into()) else {
        return;
    };
    app.note_draft = edit.text;
    app.actions.push(Action::SaveNote);
    let mut state = state;
    state
        .cursor
        .set_char_range(Some(CCursorRange::one(CCursor::new(edit.caret.from))));
    state.store(ui.ctx(), id);
}

/// The note's text laid out block by block: headings at their size, styled
/// runs as they will read, and every marker the reader typed in the quiet
/// colour, so the text stays exactly what it was.
pub fn layout_job(text: &str, palette: &Palette) -> LayoutJob {
    let mut spans: Vec<(Range<usize>, Look)> = Vec::new();
    for block in markdown::parse(text) {
        let heading = match block.kind {
            Kind::Heading(level) => Some(level),
            _ => None,
        };
        if matches!(block.kind, Kind::Code { .. }) {
            spans.push((block.span.clone(), Look::CodeBlock));
            continue;
        }
        // A list's bullet sits before the block that holds its words.
        if matches!(block.kind, Kind::List { .. } | Kind::Task { .. }) {
            let line_start = text[..block.span.start]
                .rfind('\n')
                .map(|index| index + 1)
                .unwrap_or(0);
            if line_start < block.span.start {
                spans.push((line_start..block.span.start, Look::Marker));
            }
        }
        let end = block.span.end.min(text.len());
        let mut covered = block.span.start;
        for inline in &block.inlines {
            if inline.span.start > covered {
                spans.push((covered..inline.span.start, Look::Marker));
            }
            spans.push((
                inline.span.clone(),
                Look::Styled(inline.style.clone(), heading),
            ));
            covered = inline.span.end.max(covered);
        }
        if covered < end {
            spans.push((covered..end, Look::Marker));
        }
    }
    spans.sort_by_key(|(span, _)| span.start);
    let mut job = LayoutJob::default();
    let mut at = 0;
    for (span, look) in spans {
        // Blocks can name ranges that overlap at their edges; the first one
        // to reach a byte keeps it, so no character is written twice.
        if span.start < at || span.end > text.len() || span.start >= span.end {
            continue;
        }
        if span.start > at {
            job.append(&text[at..span.start], 0.0, look.format(palette, false));
        }
        job.append(&text[span.start..span.end], 0.0, look.format(palette, true));
        at = span.end;
    }
    if at < text.len() {
        job.append(&text[at..], 0.0, Look::Marker.format(palette, false));
    }
    job
}

/// How one run of the note is painted.
#[derive(Clone, Debug)]
enum Look {
    /// A marker the note is written with: the `#`, the `**`, the dash.
    Marker,
    /// A fenced code block's body.
    CodeBlock,
    /// A run the reader styled, inside a heading or a paragraph.
    Styled(Style, Option<u8>),
}

impl Look {
    /// The format of a run. `words` says whether the run is text the reader
    /// reads, as opposed to a marker: a marker is quiet whatever the block is.
    fn format(&self, palette: &Palette, words: bool) -> TextFormat {
        let plain = TextFormat {
            font_id: theme::regular(BODY),
            color: palette.text,
            ..Default::default()
        };
        match self {
            Look::Marker => TextFormat {
                color: palette.dim,
                ..plain
            },
            Look::CodeBlock => TextFormat {
                font_id: FontId::monospace(BODY),
                color: palette.secondary,
                background: palette.surface,
                ..Default::default()
            },
            Look::Styled(style, heading) => {
                let size = heading.map_or(BODY, heading_size);
                let heading_font = heading.map(|_| theme::semibold(size));
                let mut format = TextFormat {
                    font_id: heading_font.unwrap_or_else(|| theme::regular(size)),
                    ..plain
                };
                if !words {
                    format.color = palette.dim;
                    return format;
                }
                match style {
                    Style::Plain => {}
                    Style::Strong => format.font_id = theme::semibold(size),
                    Style::Emphasis => {
                        format.italics = true;
                    }
                    Style::Strikethrough => {
                        format.strikethrough = Stroke::new(1.0, palette.secondary);
                    }
                    Style::Code => {
                        format.font_id = FontId::monospace(size);
                        format.background = palette.surface;
                    }
                    Style::Link(_) => {
                        format.color = palette.link;
                        format.underline = Stroke::new(1.0, palette.link);
                    }
                }
                format
            }
        }
    }
}

/// The size a heading is drawn at, by its level.
fn heading_size(level: u8) -> f32 {
    match level {
        1 => 22.0,
        2 => 18.0,
        3 => 16.5,
        _ => 15.0,
    }
}

/// Where the reader is in the body, as a selection of characters.
pub fn selection_of(ui: &egui::Ui, id: egui::Id) -> Option<edit::Selection> {
    let state = egui::text_edit::TextEditState::load(ui.ctx(), id)?;
    let range = state.cursor.char_range()?;
    Some(edit::Selection::new(
        range.primary.index.into(),
        range.secondary.index.into(),
    ))
}

/// Puts the caret on a selection of characters, and keeps the keys there.
pub fn select(ui: &egui::Ui, id: egui::Id, selection: edit::Selection) {
    let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
    let range = if selection.is_empty() {
        CCursorRange::one(CCursor::new(selection.from))
    } else {
        CCursorRange::two(CCursor::new(selection.from), CCursor::new(selection.to))
    };
    state.cursor.set_char_range(Some(range));
    state.store(ui.ctx(), id);
    ui.memory_mut(|memory| memory.request_focus(id));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section_at(job: &LayoutJob, byte: usize) -> &TextFormat {
        job.sections
            .iter()
            .find(|section| section.byte_range.start.0 <= byte && byte < section.byte_range.end.0)
            .map(|section| &section.format)
            .unwrap_or_else(|| panic!("no section covers byte {byte}"))
    }

    #[test]
    fn layout_keeps_the_source() {
        let palette = Palette::dark();
        let source = "# Notes\n\nA **bold** word and `code`.\n\n- [ ] a task\n";
        let job = layout_job(source, &palette);
        assert_eq!(
            job.text, source,
            "every byte the caret counts is still there, in order"
        );
        // The heading is drawn at its own size, and its marker is quiet.
        let heading = job.text.find("Notes").expect("the heading words");
        assert_eq!(section_at(&job, heading).font_id.size, 22.0);
        let hash = job.text.find('#').expect("the heading marker");
        assert_eq!(section_at(&job, hash).color, palette.dim);
        // Bold is bolder than the body around it.
        let bold = job.text.find("bold").expect("the bold words");
        assert_eq!(section_at(&job, bold).font_id, theme::semibold(BODY));
        let plain = job.text.find("word").expect("the plain words");
        assert_eq!(section_at(&job, plain).font_id, theme::regular(BODY));
        // The markers of the bold run stay in the text, quiet.
        let star = job.text.find("**").expect("a bold marker");
        assert_eq!(section_at(&job, star).color, palette.dim);
    }

    #[test]
    fn a_checklist_line_and_its_box_are_drawn() {
        let palette = Palette::dark();
        let source = "- [ ] who signs the lease\n";
        let job = layout_job(source, &palette);
        assert_eq!(job.text, source);
        let box_start = job.text.find('[').expect("the box");
        assert_eq!(section_at(&job, box_start).color, palette.dim);
        let words = job.text.find("who").expect("the words");
        assert_eq!(section_at(&job, words).color, palette.text);
    }

    #[test]
    fn a_fenced_block_is_monospace_and_keeps_its_fences() {
        let palette = Palette::dark();
        let source = "```rust\nlet x = 1;\n```\n";
        let job = layout_job(source, &palette);
        assert_eq!(job.text, source);
        let code = job.text.find("let x").expect("the code");
        assert_eq!(section_at(&job, code).font_id, FontId::monospace(BODY));
    }
}
