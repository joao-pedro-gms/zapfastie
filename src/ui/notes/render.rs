//! A note read as it was written: headings at their size, lists with their
//! bullets, quotes behind a bar, code in its own room, and a checklist whose
//! boxes are pressed to tick them.

use std::ops::Range;

use egui::text::LayoutJob;
use egui::{Align, Frame, Layout, Margin, Sense, Stroke, TextFormat, vec2};

use crate::app::App;
use crate::i18n::gettext;
use crate::model::Action;
use crate::notes::markdown::{self, Block, Kind, Style};
use crate::theme::{self, Icon, Palette};

/// A note read, rather than written.
pub fn show(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    let body = app.note_draft.clone();
    let note = app.open_note.as_ref().map(|note| note.id).unwrap_or(0);
    let locale = app.locale;
    ui.ctx()
        .data_mut(|data| data.insert_temp(read_id(), ui.available_rect_before_wrap()));
    egui::ScrollArea::vertical()
        .id_salt("note-read")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            for block in markdown::parse(&body) {
                block_ui(app, ui, palette, locale, note, &block);
            }
        });
}

/// Where the reading view was drawn last, for layout tests.
pub fn read_id() -> egui::Id {
    egui::Id::new("notes-read")
}

/// One block of the note, drawn and, when it is a task, clickable.
fn block_ui(
    app: &mut App,
    ui: &mut egui::Ui,
    palette: &Palette,
    locale: crate::i18n::Locale,
    note: i64,
    block: &Block,
) {
    let job = job_for(block, palette);
    match block.kind.clone() {
        Kind::Heading(level) => {
            ui.add_space(if level <= 2 { 10.0 } else { 6.0 });
            ui.add(egui::Label::new(job).wrap().selectable(true));
            ui.add_space(2.0);
        }
        Kind::Rule => {
            ui.add_space(8.0);
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(rect, 0.0, palette.outline);
            ui.add_space(8.0);
        }
        Kind::Code { language } => {
            ui.add_space(4.0);
            let copy = gettext(locale, "Copy the code");
            Frame::new()
                .fill(palette.surface)
                .corner_radius(egui::CornerRadius::same(theme::RADIUS_SMALL))
                .inner_margin(Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            if theme::icon_button(
                                ui,
                                Icon::Copy,
                                14.0,
                                palette.secondary,
                                palette.text,
                                &copy,
                            )
                            .clicked()
                            {
                                ui.ctx().copy_text(block.text.clone());
                            }
                        });
                        if !language.is_empty() {
                            ui.label(
                                egui::RichText::new(&language)
                                    .font(theme::regular(11.0))
                                    .color(palette.dim),
                            );
                        }
                    });
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(block.text.trim_end())
                                .font(egui::FontId::monospace(12.5))
                                .color(palette.text),
                        )
                        .wrap()
                        .selectable(true),
                    );
                });
            ui.add_space(4.0);
        }
        Kind::Quote => {
            ui.add_space(4.0);
            Frame::new()
                .inner_margin(Margin {
                    left: 12,
                    right: 0,
                    top: 2,
                    bottom: 2,
                })
                .show(ui, |ui| {
                    let bar = ui.max_rect();
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(
                            bar.min - vec2(10.0, 0.0),
                            vec2(3.0, bar.height()),
                        ),
                        egui::CornerRadius::same(2),
                        palette.outline,
                    );
                    let mut quoted = job;
                    for section in &mut quoted.sections {
                        section.format.color = palette.secondary;
                    }
                    ui.add(egui::Label::new(quoted).wrap().selectable(true));
                });
            ui.add_space(4.0);
        }
        Kind::List { ordered, depth } => {
            let marker = if ordered { "1." } else { "•" };
            list_row(ui, palette, marker, depth, job, None);
        }
        Kind::Task {
            checked, marker, ..
        } => {
            let boxed = task_box(ui, palette, &checked, &job);
            if boxed {
                app.actions.push(Action::ToggleNoteTask {
                    id: note,
                    marker: marker.clone(),
                });
            }
        }
        Kind::Paragraph => {
            // A paragraph whose words hold a link stays clickable through the
            // galley: the point of the click names the run under it.
            let (response, galley) = selectable(ui, job, palette.text);
            if let Some(url) = link_under(block, &response, &galley) {
                app.actions.push(Action::OpenUrl(url));
            }
            ui.add_space(4.0);
        }
    }
}

/// A list item: its bullet, then its words, indented by how deep the list
/// sits. The bullet of a numbered list counts from one in the order drawn.
fn list_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    marker: &str,
    depth: usize,
    job: LayoutJob,
    _checked: Option<bool>,
) {
    ui.horizontal_top(|ui| {
        ui.add_space(depth as f32 * 18.0);
        ui.add_space(2.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new(marker)
                    .font(theme::regular(13.5))
                    .color(palette.secondary),
            )
            .selectable(false),
        );
        ui.add(egui::Label::new(job).wrap().selectable(true));
    });
}

/// A checklist item: a box the reader presses, then its words, struck through
/// once it is ticked.
fn task_box(ui: &mut egui::Ui, palette: &Palette, checked: &bool, job: &LayoutJob) -> bool {
    let mut pressed = false;
    ui.horizontal_top(|ui| {
        ui.add_space(2.0);
        let (rect, response) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
        let box_rect = egui::Rect::from_min_size(rect.min, vec2(15.0, 15.0));
        ui.painter().rect(
            box_rect,
            egui::CornerRadius::same(3),
            if *checked {
                palette.accent
            } else {
                egui::Color32::TRANSPARENT
            },
            Stroke::new(
                1.0,
                if *checked {
                    palette.accent
                } else {
                    // `outline` sits a shade off the surface and left the box
                    // invisible; a dim rule reads as an empty box.
                    palette.dim
                },
            ),
            egui::StrokeKind::Inside,
        );
        if *checked {
            theme::paint_icon(ui, Icon::Check, box_rect, 12.0, palette.on_accent);
        }
        let mut words = job.clone();
        if *checked {
            for section in &mut words.sections {
                section.format.strikethrough = Stroke::new(1.0, palette.secondary);
                section.format.color = palette.secondary;
            }
        }
        let text = ui.add(egui::Label::new(words).wrap().selectable(true));
        if response.clicked() {
            pressed = true;
        }
        // The words tick the box too, which is what a checklist invites.
        if text.clicked() {
            pressed = true;
        }
    });
    pressed
}

/// Lays a block's words out over the window's width, paints them so they can
/// be swept and copied, and hands back what a click needs to find a link.
fn selectable(
    ui: &mut egui::Ui,
    mut job: LayoutJob,
    color: egui::Color32,
) -> (egui::Response, std::sync::Arc<egui::Galley>) {
    job.wrap.max_width = ui.available_width().max(1.0);
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::CLICK | Sense::DRAG);
    egui::text_selection::LabelSelectionState::label_text_selection(
        ui,
        &response,
        rect.min,
        galley.clone(),
        color,
        Stroke::NONE,
    );
    (response, galley)
}

/// The link under a click in `block`, if the click landed on one.
fn link_under(block: &Block, response: &egui::Response, galley: &egui::Galley) -> Option<String> {
    if !response.clicked() {
        return None;
    }
    let pointer = response.interact_pointer_pos()?;
    let cursor = galley.cursor_from_pos(pointer - response.rect.min);
    link_at(block, cursor.index.into())
}

/// The address a character of the block's reading text belongs to.
fn link_at(block: &Block, index: usize) -> Option<String> {
    block.inlines.iter().find_map(|inline| match &inline.style {
        Style::Link(url) if inline_char_range(block, inline).contains(&index) => Some(url.clone()),
        _ => None,
    })
}

/// The characters an inline run covers inside the block's own text.
fn inline_char_range(block: &Block, inline: &markdown::Inline) -> Range<usize> {
    let before: usize = block
        .inlines
        .iter()
        .take_while(|run| run.span.start < inline.span.start)
        .map(|run| run.text.chars().count())
        .sum();
    before..before + inline.text.chars().count()
}

/// The layout of one block, with every marker left out: the reading view shows
/// the words, not how they were written.
pub fn job_for(block: &Block, palette: &Palette) -> LayoutJob {
    let heading = match block.kind {
        Kind::Heading(level) => Some(level),
        _ => None,
    };
    let mut job = LayoutJob::default();
    for inline in &block.inlines {
        job.append(
            &inline.text,
            0.0,
            format_of(&inline.style, heading, palette),
        );
    }
    job
}

/// How one run reads.
fn format_of(style: &Style, heading: Option<u8>, palette: &Palette) -> TextFormat {
    let size = heading.map_or(14.0, heading_size);
    let text = TextFormat {
        font_id: theme::regular(size),
        color: palette.text,
        ..Default::default()
    };
    match style {
        Style::Plain => text,
        Style::Strong => TextFormat {
            font_id: theme::semibold(size),
            ..text
        },
        Style::Emphasis => TextFormat {
            italics: true,
            ..text
        },
        Style::Strikethrough => TextFormat {
            strikethrough: Stroke::new(1.0, palette.secondary),
            ..text
        },
        Style::Code => TextFormat {
            font_id: egui::FontId::monospace(size - 0.5),
            background: palette.surface,
            ..text
        },
        Style::Link(_) => TextFormat {
            color: palette.link,
            underline: Stroke::new(1.0, palette.link),
            ..text
        },
    }
}

/// The size a heading reads at, by its level.
fn heading_size(level: u8) -> f32 {
    match level {
        1 => 23.0,
        2 => 19.0,
        3 => 16.5,
        _ => 15.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_read_view_draws_a_heading_larger() {
        let palette = Palette::dark();
        let blocks = markdown::parse("# Rust Berlin\n\nplain words\n");
        let heading = job_for(&blocks[0], &palette);
        assert!(heading.sections[0].format.font_id.size >= 20.0);
        let body = job_for(&blocks[1], &palette);
        assert_eq!(body.sections[0].format.font_id.size, 14.0);
        // No marker is drawn: the heading reads as its words alone.
        assert_eq!(heading.text, "Rust Berlin");
        assert!(!heading.text.contains('#'));
    }

    #[test]
    fn a_read_paragraph_keeps_its_styles_without_its_markers() {
        let palette = Palette::dark();
        let blocks = markdown::parse("a **bold** and `code` word\n");
        let job = job_for(&blocks[0], &palette);
        assert_eq!(job.text, "a bold and code word");
        let bold = job.text.find("bold").expect("the bold words");
        let section = job
            .sections
            .iter()
            .find(|section| section.byte_range.start.0 <= bold && bold < section.byte_range.end.0)
            .expect("a section");
        assert_eq!(section.format.font_id, theme::semibold(14.0));
    }

    #[test]
    fn a_click_inside_a_link_names_the_address() {
        let blocks = markdown::parse("see [the lease](https://example.com) first\n");
        let block = &blocks[0];
        // "see " is four characters, so "the lease" runs 4..13.
        assert_eq!(
            link_at(block, 5).as_deref(),
            Some("https://example.com"),
            "a click inside the link names it"
        );
        assert_eq!(
            link_at(block, 0),
            None,
            "a click on the plain words does not"
        );
        assert_eq!(link_at(block, 20), None, "nor one past the link");
    }

    #[test]
    fn a_link_run_covers_its_own_characters() {
        let blocks = markdown::parse("see [the lease](https://example.com) first\n");
        let block = &blocks[0];
        let link = block
            .inlines
            .iter()
            .find(|inline| matches!(inline.style, Style::Link(_)))
            .expect("a link");
        let range = inline_char_range(block, link);
        assert_eq!(&block.text[range], "the lease");
    }
}
