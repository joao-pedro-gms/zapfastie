//! The notes tab: the notes of this computer in a column on the left, the one
//! the reader picked beside it.
//!
//! A note is markdown kept as it was typed, so nothing is lost between this
//! window and a text editor. The list is searchable, pinned notes come first,
//! and a narrow window shows one column at a time: the list with nothing
//! open, the note once one is.

use egui::{Align, CornerRadius, Frame, Layout, Margin, Rect, Sense, Vec2, pos2, vec2};

use crate::app::App;
use crate::i18n::gettext;
use crate::model::Action;
use crate::notes::NoteSummary;
use crate::theme::{self, Icon, Palette};

use super::widgets;

/// The list column's width at most.
const LIST_WIDTH: f32 = 300.0;
/// The narrowest the list column goes before the reader gets a column of its
/// own and the list one is dropped instead.
const LIST_MIN: f32 = 180.0;
/// The note's widest measure, so a long note stays readable on a wide window.
const READING_WIDTH: f32 = 720.0;
/// One row of the list.
const ROW_HEIGHT: f32 = 54.0;

/// Where the notes list was drawn, for layout tests.
pub(crate) fn list_id() -> egui::Id {
    egui::Id::new("notes-list")
}

/// Stable id of a list row, by note id, for tests and the demo.
pub fn row_id(id: i64) -> egui::Id {
    egui::Id::new(("notes-row", id))
}

/// The search field's id.
pub(crate) fn search_id() -> egui::Id {
    egui::Id::new("notes-search")
}

/// Draws the tab.
pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let width = ui.available_width();
    // One column at a time when there is no room for two: the list with
    // nothing open, and the note with the way back to the list once one is.
    let single = width < LIST_MIN + 320.0;
    if single && app.open_note.is_some() {
        note(app, ui, &palette);
        return;
    }
    if single {
        list(app, ui, &palette, true);
        return;
    }
    let list_width = LIST_WIDTH.min((width - 320.0).max(LIST_MIN));
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            vec2(list_width, ui.available_height()),
            Layout::top_down(Align::Min),
            |ui| list(app, ui, &palette, false),
        );
        // A line between the two columns, as the chat list has one.
        let x = ui.cursor().left();
        ui.painter().rect_filled(
            Rect::from_min_size(
                pos2(x, ui.max_rect().top()),
                vec2(1.0, ui.max_rect().height()),
            ),
            0.0,
            palette.outline,
        );
        ui.add_space(12.0);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), ui.available_height()),
            Layout::top_down(Align::Min),
            |ui| note(app, ui, &palette),
        );
    });
}

/// The column of notes: its name, a search field, then a row each.
fn list(app: &mut App, ui: &mut egui::Ui, palette: &Palette, tall: bool) {
    let locale = app.locale;
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let (icon, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
        theme::paint_icon(ui, Icon::Notebook, icon, 18.0, palette.secondary);
        ui.label(
            egui::RichText::new(gettext(locale, "Notes").as_ref())
                .size(16.0)
                .strong()
                .color(palette.text),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let new = gettext(locale, "New note");
            if theme::icon_button(ui, Icon::Plus, 18.0, palette.secondary, palette.text, &new)
                .clicked()
            {
                app.actions.push(Action::NewNote);
            }
        });
    });
    ui.add_space(8.0);
    if app.notes_focus {
        ui.ctx()
            .memory_mut(|memory| memory.request_focus(search_id()));
        app.notes_focus = false;
    }
    let placeholder = gettext(locale, "Search notes");
    let before = app.notes_search.clone();
    widgets::search_field(
        ui,
        palette,
        search_id(),
        &mut app.notes_search,
        &placeholder,
        ui.available_width(),
    );
    // Typing reads the list again on the next pass, with what was typed.
    if app.notes_search != before {
        app.actions.push(Action::RefreshNotes);
    }
    ui.add_space(8.0);
    ui.ctx()
        .data_mut(|data| data.insert_temp(list_id(), ui.available_rect_before_wrap()));
    if let Some(error) = app.notes_error.clone() {
        hint(ui, palette, &error);
        return;
    }
    if !app.notes_loaded {
        // The first pass of the tab: read the list, and show it next frame.
        app.actions.push(Action::RefreshNotes);
        return;
    }
    if app.notes_list.is_empty() {
        let text = if app.notes_search.trim().is_empty() {
            gettext(locale, "No notes yet.")
        } else {
            gettext(locale, "No notes match your search.")
        };
        hint(ui, palette, &text);
        return;
    }
    let open = app.open_note.as_ref().map(|note| note.id);
    let rows = app.notes_list.clone();
    egui::ScrollArea::vertical()
        .id_salt("notes-rows")
        .auto_shrink([false, tall])
        .show(ui, |ui| {
            for summary in &rows {
                if row(ui, palette, summary, open == Some(summary.id), locale) {
                    app.actions.push(Action::OpenNote(summary.id));
                }
            }
        });
}

/// One note in the list: its title, the first words under it, and a pin when
/// it is pinned. Returns whether it was clicked.
fn row(
    ui: &mut egui::Ui,
    palette: &Palette,
    summary: &NoteSummary,
    selected: bool,
    locale: crate::i18n::Locale,
) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
    ui.ctx()
        .data_mut(|data| data.insert_temp(row_id(summary.id), rect));
    let fill = if selected {
        palette.surface_hover
    } else if response.hovered() {
        palette.surface
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 8.0, fill);
    let inner = rect.shrink2(vec2(10.0, 7.0));
    let mut left = inner.left();
    if summary.pinned {
        let pin = Rect::from_min_size(pos2(left, inner.top() + 2.0), Vec2::splat(14.0));
        theme::paint_icon(ui, Icon::Pin, pin, 14.0, palette.secondary);
        left += 20.0;
    }
    let title = if summary.title.is_empty() {
        gettext(locale, "Untitled note").into_owned()
    } else {
        summary.title.clone()
    };
    // Truncated rather than clipped: a row ends in an ellipsis, as the chat
    // list's rows do.
    line(
        ui,
        Rect::from_min_size(pos2(left, inner.top()), vec2(inner.right() - left, 18.0)),
        egui::RichText::new(title)
            .font(theme::medium(14.0))
            .color(palette.text),
    );
    line(
        ui,
        Rect::from_min_size(
            pos2(inner.left(), inner.top() + 21.0),
            vec2(inner.width(), 16.0),
        ),
        egui::RichText::new(&summary.excerpt)
            .font(theme::regular(12.0))
            .color(palette.secondary),
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// One truncated line of a row, left aligned in its rect. `Ui::put` centres
/// what it is given, which is not what a list row wants.
fn line(ui: &mut egui::Ui, rect: Rect, text: egui::RichText) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.add(egui::Label::new(text).truncate().selectable(false));
        },
    );
}

/// The note the reader picked, or where to pick one.
fn note(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    let locale = app.locale;
    let Some(note) = app.open_note.clone() else {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.3).max(24.0));
            let (icon, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
            theme::paint_icon(ui, Icon::Notebook, icon, 40.0, palette.secondary);
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(gettext(locale, "Pick a note").as_ref())
                    .size(15.0)
                    .color(palette.secondary),
            );
        });
        return;
    };
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let title = if note.title.is_empty() {
            gettext(locale, "Untitled note").into_owned()
        } else {
            note.title.clone()
        };
        ui.label(
            egui::RichText::new(title)
                .size(20.0)
                .strong()
                .color(palette.text),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let close = gettext(locale, "Close the note");
            if theme::icon_button(ui, Icon::X, 20.0, palette.secondary, palette.text, &close)
                .clicked()
            {
                app.actions.push(Action::CloseNote);
            }
            let delete = gettext(locale, "Delete the note");
            if theme::icon_button(
                ui,
                Icon::Trash,
                20.0,
                palette.secondary,
                palette.text,
                &delete,
            )
            .clicked()
            {
                app.actions.push(Action::DeleteNote(note.id));
            }
            let (icon, tip) = if note.pinned {
                (Icon::PinOff, gettext(locale, "Unpin the note"))
            } else {
                (Icon::Pin, gettext(locale, "Pin the note"))
            };
            if theme::icon_button(ui, icon, 20.0, palette.secondary, palette.text, &tip).clicked() {
                app.actions.push(Action::PinNote {
                    id: note.id,
                    pinned: !note.pinned,
                });
            }
        });
    });
    if !app.note_links.is_empty() {
        ui.add_space(8.0);
        citations(app, ui, palette);
    }
    ui.add_space(10.0);
    if app.note_editing {
        ui.ctx()
            .memory_mut(|memory| memory.request_focus(editor_id()));
        app.note_editing = false;
    }
    let width = ui.available_width().min(READING_WIDTH);
    Frame::new()
        .fill(palette.surface)
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(width - 28.0);
            egui::ScrollArea::vertical()
                .id_salt("note-body")
                .auto_shrink([false, true])
                .max_height(ui.available_height() - 16.0)
                .show(ui, |ui| {
                    editor(app, ui, palette, locale);
                });
        });
}

/// What the open note cites, as a chip each: a message of a chat, and,
/// once mail is read here, a mail. A chip takes the reader back to what it
/// names.
fn citations(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    let locale = app.locale;
    let mut jump = None;
    ui.horizontal_wrapped(|ui| {
        for reference in app.note_links.clone() {
            match reference {
                crate::notes::NoteRef::Message { chat, message } => {
                    let name = app
                        .chat(&chat)
                        .map(|chat| chat.name.clone())
                        .unwrap_or_else(|| chat.clone());
                    let label = gettext(locale, "Quoted from {chat}").replace("{chat}", &name);
                    if theme::soft_button(ui, palette, Some(Icon::MessageCircle), &label, false)
                        .on_hover_text(gettext(locale, "Open the message this note cites").as_ref())
                        .clicked()
                    {
                        jump = Some(Action::ShowNoteSource { chat, message });
                    }
                }
                crate::notes::NoteRef::Mail { account, .. } => {
                    // No mail is read here yet, so the chip names the account
                    // and goes nowhere.
                    theme::soft_button(ui, palette, Some(Icon::FileText), &account, false);
                }
            }
        }
    });
    if let Some(action) = jump {
        app.actions.push(action);
    }
}

/// The note's text, edited where it is read. A change is written as it is
/// typed: a note is a few hundred bytes, and losing the last sentence to a
/// crash would cost more than the write.
fn editor(app: &mut App, ui: &mut egui::Ui, palette: &Palette, locale: crate::i18n::Locale) {
    let hint = gettext(locale, "Write a note");
    let mut body = app.note_draft.clone();
    let response = ui.add(
        egui::TextEdit::multiline(&mut body)
            .id(editor_id())
            .font(theme::regular(14.0))
            .text_color(palette.text)
            .desired_width(f32::INFINITY)
            .desired_rows(14)
            .margin(vec2(2.0, 2.0))
            .hint_text(egui::RichText::new(hint.as_ref()).color(palette.secondary)),
    );
    if response.changed() {
        app.note_draft = body;
        app.actions.push(Action::SaveNote);
    }
}

/// The note editor's id, for focus and tests.
pub fn editor_id() -> egui::Id {
    egui::Id::new("note-editor")
}

/// A line where the list would be: no notes yet, nothing matching, or why the
/// notes could not be opened.
fn hint(ui: &mut egui::Ui, palette: &Palette, text: &str) {
    ui.add_space(18.0);
    ui.label(
        egui::RichText::new(text)
            .size(13.0)
            .color(palette.secondary),
    );
}
