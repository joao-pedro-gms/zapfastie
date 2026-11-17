//! The notes tab: the notes of this computer in a column on the left, the one
//! the reader picked beside it on a page as wide as reading allows.
//!
//! A note is markdown kept as it was typed, so nothing is lost between this
//! window and a text editor. The list is searchable and grouped by when a note
//! was last written, pinned notes come first, and a narrow window shows one
//! column at a time: the list with nothing open, the note once one is.

use egui::{Align, CornerRadius, Layout, Rect, Sense, Vec2, pos2, vec2};

use crate::app::App;
use crate::i18n::{Locale, gettext};
use crate::model::{Action, Dialog};
use crate::notes::NoteRef;
use crate::theme::{self, Icon, Palette};

use super::widgets;

mod editor;
mod list;
mod render;
mod toolbar;

/// The list column's width at most.
const LIST_WIDTH: f32 = 320.0;
/// The narrowest the list column goes before the note keeps its own room.
const LIST_MIN: f32 = 200.0;
/// The room the note needs beside the list.
const NOTE_MIN: f32 = 420.0;
/// Below this width the tab shows one column at a time.
const SINGLE_COLUMN: f32 = 780.0;
/// The note's widest measure, so a long note stays readable on a wide window.
const READING_WIDTH: f32 = 720.0;
/// The line under the writing surface: how much was written, and when.
const STATUS_HEIGHT: f32 = 22.0;
/// One row of the list.
const ROW_HEIGHT: f32 = 56.0;

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

/// The note editor's id, for focus and tests.
pub fn editor_id() -> egui::Id {
    egui::Id::new("note-editor")
}

/// Where the writing surface was drawn, for layout tests.
pub fn surface_id() -> egui::Id {
    egui::Id::new("note-surface")
}

/// Where the open note's actions were drawn, for layout tests.
pub fn actions_id() -> egui::Id {
    egui::Id::new("note-actions")
}

/// Where the narrow window's way back to the list was drawn.
pub fn back_id() -> egui::Id {
    egui::Id::new("note-back")
}

/// Where the note pane offers to start a note.
pub fn new_note_id() -> egui::Id {
    egui::Id::new("note-new")
}

/// The column the note is written in, for layout tests.
pub fn pane_id() -> egui::Id {
    egui::Id::new("note-pane")
}

/// Draws the tab.
pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let width = ui.available_width();
    // One column at a time when there is no room for two.
    if width < SINGLE_COLUMN {
        if app.open_note.is_some() {
            note(app, ui, &palette, true);
        } else {
            list::show(app, ui, &palette, true);
        }
        return;
    }
    let list_width = LIST_WIDTH.min((width - NOTE_MIN).max(LIST_MIN));
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            vec2(list_width, ui.available_height()),
            Layout::top_down(Align::Min),
            |ui| list::show(app, ui, &palette, false),
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
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), ui.available_height()),
            Layout::top_down(Align::Min),
            |ui| note(app, ui, &palette, false),
        );
    });
}

/// The note the reader picked, or where to pick one.
fn note(app: &mut App, ui: &mut egui::Ui, palette: &Palette, narrow: bool) {
    let locale = app.locale;
    if app.open_note.is_none() {
        if app.notes_list.is_empty() {
            start_a_note(app, ui, palette);
        } else {
            pick_a_note(ui, palette, locale);
        }
        return;
    }
    header(app, ui, palette, narrow);
    if !app.note_links.is_empty() {
        ui.add_space(theme::SPACE_6);
        citations(app, ui, palette);
    }
    ui.add_space(theme::SPACE_8);
    // Ctrl+E reads the note, and again to write it.
    if ui.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::E)) {
        app.actions.push(Action::ToggleNoteRead);
    }
    let pane = ui.available_rect_before_wrap();
    ui.ctx().data_mut(|data| data.insert_temp(pane_id(), pane));
    // The page is as wide as reading allows, and centred in the column.
    let column = ui.available_width().min(READING_WIDTH);
    let left = ((ui.available_width() - column) * 0.5).max(0.0);
    ui.horizontal_top(|ui| {
        ui.add_space(left);
        ui.vertical(|ui| {
            ui.set_width(column);
            body(app, ui, palette, column, locale);
            status(app, ui, palette);
        });
    });
}

/// The writing surface, or the note read, filling the column down to the line
/// that counts it.
fn body(app: &mut App, ui: &mut egui::Ui, palette: &Palette, column: f32, locale: Locale) {
    let height = (ui.available_height() - STATUS_HEIGHT).max(120.0);
    let (rect, _response) = ui.allocate_exact_size(vec2(column, height), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(theme::RADIUS), palette.surface);
    ui.ctx()
        .data_mut(|data| data.insert_temp(surface_id(), rect));
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(14.0)), |ui| {
        if app.note_read {
            render::show(app, ui, palette);
            return;
        }
        if app.note_editing {
            ui.ctx()
                .memory_mut(|memory| memory.request_focus(editor_id()));
            app.note_editing = false;
        }
        // The keys format the note while the editor has them, wherever the
        // pointer is, and the buttons say what they do.
        toolbar::shortcuts(app, ui);
        toolbar::show(app, ui, palette);
        ui.add_space(theme::SPACE_4);
        egui::ScrollArea::vertical()
            .id_salt("note-body")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                editor::show(app, ui, palette, locale);
            });
    });
}

/// How much was written, and when it was last kept.
fn status(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    let locale = app.locale;
    let words = app.note_draft.split_whitespace().count();
    let characters = app.note_draft.chars().count();
    let edited = app
        .open_note
        .as_ref()
        .map(|note| note.updated_at)
        .unwrap_or_default();
    ui.horizontal(|ui| {
        let words = gettext(locale, "Words: {count}").replace("{count}", &words.to_string());
        let characters =
            gettext(locale, "Characters: {count}").replace("{count}", &characters.to_string());
        ui.label(
            egui::RichText::new(format!("{words} · {characters}"))
                .font(theme::regular(11.0))
                .color(palette.dim),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if edited > 0 {
                let when = crate::util::chat_stamp(locale, edited);
                let said = gettext(locale, "Edited {when}").replace("{when}", &when);
                ui.label(
                    egui::RichText::new(said)
                        .font(theme::regular(11.0))
                        .color(palette.dim),
                );
            }
        });
    });
}

/// The note's name and its actions, under the caption buttons the window
/// draws over a corner the header reaches.
fn header(app: &mut App, ui: &mut egui::Ui, palette: &Palette, narrow: bool) {
    let locale = app.locale;
    let top = ui.max_rect();
    let room = super::caption_room(ui.ctx(), top, 240.0);
    super::caption_drag(
        ui,
        Rect::from_min_size(top.min, vec2(top.width(), theme::TOP_BAR_HEIGHT + room.top)),
    );
    ui.add_space(16.0 + room.top);
    let note = app.open_note.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add_space(12.0 + room.left);
        if narrow {
            let back = gettext(locale, "Back to the notes");
            let response = theme::icon_button(
                ui,
                Icon::ArrowLeft,
                18.0,
                palette.secondary,
                palette.text,
                &back,
            );
            ui.ctx()
                .data_mut(|data| data.insert_temp(back_id(), response.rect));
            if response.clicked() {
                app.actions.push(Action::CloseNote);
            }
        }
        let mut title = app.note_title.clone();
        let placeholder = gettext(locale, "Untitled note");
        let field = ui.add(
            egui::TextEdit::singleline(&mut title)
                .font(theme::semibold(20.0))
                .text_color(palette.text)
                .hint_text(egui::RichText::new(placeholder.as_ref()).color(palette.secondary))
                .frame(egui::Frame::NONE)
                .desired_width(320.0),
        );
        if field.changed() {
            app.note_title = title.clone();
            app.actions.push(Action::RenameNote { id: note.id, title });
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(room.right);
            let close = gettext(locale, "Close the note");
            let response =
                theme::icon_button(ui, Icon::X, 20.0, palette.secondary, palette.text, &close);
            ui.ctx()
                .data_mut(|data| data.insert_temp(actions_id(), response.rect));
            if response.clicked() {
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
                app.actions
                    .push(Action::ShowDialog(Dialog::ConfirmDeleteNote(note.id)));
            }
            let more = gettext(locale, "More for this note");
            let button = theme::icon_button(
                ui,
                Icon::Ellipsis,
                20.0,
                palette.secondary,
                palette.text,
                &more,
            );
            egui::Popup::menu(&button)
                .open_memory(button.clicked().then_some(egui::SetOpenCommand::Toggle))
                .frame(widgets::menu_frame(palette))
                .show(|ui| {
                    let duplicate = gettext(locale, "Duplicate the note");
                    if widgets::menu_item(ui, palette, Some(Icon::Copy), &duplicate) {
                        app.actions.push(Action::DuplicateNote(note.id));
                    }
                    let export = gettext(locale, "Export as markdown");
                    if widgets::menu_item(ui, palette, Some(Icon::Download), &export) {
                        app.actions.push(Action::ExportNote(note.id));
                    }
                });
            let (icon, tip) = if app.note_read {
                (Icon::Pencil, gettext(locale, "Write the note"))
            } else {
                (Icon::Eye, gettext(locale, "Read the note"))
            };
            if theme::icon_button(ui, icon, 20.0, palette.secondary, palette.text, &tip).clicked() {
                app.actions.push(Action::ToggleNoteRead);
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
}

/// What the open note cites, as a chip each: a message of a chat, and, once
/// mail is read here, a mail. A chip takes the reader back to what it names.
fn citations(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    let locale = app.locale;
    let note = app.open_note.as_ref().map(|note| note.id).unwrap_or(0);
    let mut jump = None;
    ui.horizontal_wrapped(|ui| {
        ui.add_space(theme::SPACE_12);
        for reference in app.note_links.clone() {
            match reference {
                NoteRef::Message { chat, message } => {
                    let name = app
                        .chat(&chat)
                        .map(|chat| chat.name.clone())
                        .unwrap_or_else(|| chat.clone());
                    let label = gettext(locale, "Quoted from {chat}").replace("{chat}", &name);
                    let chip =
                        theme::soft_button(ui, palette, Some(Icon::MessageCircle), &label, false)
                            .on_hover_text(
                                gettext(locale, "Open the message this note cites").as_ref(),
                            );
                    let mut remove = false;
                    egui::Popup::context_menu(&chip)
                        .frame(widgets::menu_frame(palette))
                        .show(|ui| {
                            let take = gettext(locale, "Remove the citation");
                            if widgets::menu_item(ui, palette, Some(Icon::X), &take) {
                                remove = true;
                            }
                        });
                    if remove {
                        jump = Some(Action::RemoveNoteCitation {
                            note,
                            reference: NoteRef::Message {
                                chat: chat.clone(),
                                message: message.clone(),
                            },
                        });
                    }
                    if chip.clicked() {
                        jump = Some(Action::ShowNoteSource { chat, message });
                    }
                }
                NoteRef::Mail { account, .. } => {
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

/// No notes at all: the pane says so and offers to start one.
fn start_a_note(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    let locale = app.locale;
    ui.vertical_centered(|ui| {
        ui.add_space((ui.available_height() * 0.24).max(24.0));
        widgets::empty_state(
            ui,
            palette,
            Icon::Notebook,
            gettext(locale, "No notes yet").as_ref(),
            gettext(locale, "Write one, and it stays on this computer.").as_ref(),
        );
        let new = gettext(locale, "New note");
        let response = theme::soft_button(ui, palette, Some(Icon::Plus), &new, false);
        ui.ctx()
            .data_mut(|data| data.insert_temp(new_note_id(), response.rect));
        if response.clicked() {
            app.actions.push(Action::NewNote);
        }
    });
}

/// Notes exist but none is open.
fn pick_a_note(ui: &mut egui::Ui, palette: &Palette, locale: Locale) {
    ui.vertical_centered(|ui| {
        ui.add_space((ui.available_height() * 0.3).max(24.0));
        let (icon, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
        theme::paint_icon(ui, Icon::Notebook, icon, 36.0, palette.dim);
        ui.add_space(theme::SPACE_8);
        ui.label(
            egui::RichText::new(gettext(locale, "Pick a note").as_ref())
                .size(14.5)
                .color(palette.secondary),
        );
    });
}
