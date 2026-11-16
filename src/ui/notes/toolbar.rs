//! The strip of buttons that writes a note's markdown: it wraps the selection
//! in markers, gives the selected lines a prefix, or makes a link of them, and
//! leaves the caret where the reader was, so typing carries on.

use egui::{Align, Layout, Modifiers};

use crate::app::App;
use crate::i18n::gettext;
use crate::model::Action;
use crate::notes::edit;
use crate::theme::{self, Icon, Palette};

use super::editor;
use super::editor_id;

/// One thing the toolbar can do to the note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Bold,
    Italic,
    Strike,
    Code,
    Heading,
    Bullet,
    Ordered,
    Checklist,
    Quote,
    Link,
}

impl Mark {
    /// Every button, in the order they are drawn.
    pub const ALL: [Self; 10] = [
        Self::Bold,
        Self::Italic,
        Self::Strike,
        Self::Code,
        Self::Heading,
        Self::Bullet,
        Self::Ordered,
        Self::Checklist,
        Self::Quote,
        Self::Link,
    ];

    /// What the button is called, before `gettext`.
    fn name(self) -> &'static str {
        match self {
            Self::Bold => "Bold",
            Self::Italic => "Italic",
            Self::Strike => "Strikethrough",
            Self::Code => "Code",
            Self::Heading => "Heading",
            Self::Bullet => "Bulleted list",
            Self::Ordered => "Numbered list",
            Self::Checklist => "Checklist",
            Self::Quote => "Quote",
            Self::Link => "Link",
        }
    }

    /// The keys that do the same as the button.
    fn shortcut(self) -> &'static str {
        match self {
            Self::Bold => "Ctrl+B",
            Self::Italic => "Ctrl+I",
            Self::Strike => "Ctrl+Shift+X",
            Self::Link => "Ctrl+K",
            _ => "",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::Bold => Icon::Bold,
            Self::Italic => Icon::Italic,
            Self::Strike => Icon::Strikethrough,
            Self::Code => Icon::Code,
            Self::Heading => Icon::Heading,
            Self::Bullet => Icon::List,
            Self::Ordered => Icon::ListOrdered,
            Self::Checklist => Icon::ListChecks,
            Self::Quote => Icon::Quote,
            Self::Link => Icon::Link,
        }
    }

    /// The markers or line prefix the button writes over the selection.
    fn marker(self) -> &'static str {
        match self {
            Self::Bold => "**",
            Self::Italic => "_",
            Self::Strike => "~~",
            Self::Code => "`",
            Self::Heading => "# ",
            Self::Bullet => "- ",
            Self::Ordered => "1. ",
            Self::Checklist => "- [ ] ",
            Self::Quote => "> ",
            Self::Link => "",
        }
    }

    /// Whether the mark wraps the selection rather than prefixing its lines.
    fn wraps(self) -> bool {
        matches!(self, Self::Bold | Self::Italic | Self::Strike | Self::Code)
    }
}

/// Applies one mark to a note's text over a selection, and says where the
/// caret goes.
pub fn apply(body: &str, mark: Mark, selection: edit::Selection) -> edit::Edit {
    match mark {
        Mark::Link => edit::wrap_link(body, selection),
        _ if mark.wraps() => edit::wrap_selection(body, selection, mark.marker()),
        _ => edit::prefix_lines(body, selection, mark.marker()),
    }
}

/// Draws the buttons. A press changes the draft, keeps the note's save going
/// and hands the caret back to the editor.
pub fn show(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    let locale = app.locale;
    let id = editor_id();
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 24.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for mark in Mark::ALL {
                let shortcut = mark.shortcut();
                let name = gettext(locale, mark.name());
                let tip = if shortcut.is_empty() {
                    name.to_string()
                } else {
                    format!("{name} ({shortcut})")
                };
                let button = theme::icon_button(
                    ui,
                    mark.icon(),
                    16.0,
                    palette.secondary,
                    palette.text,
                    &tip,
                );
                ui.ctx()
                    .data_mut(|data| data.insert_temp(button_id(mark), button.rect));
                if button.clicked() {
                    press(app, ui, mark, id);
                }
            }
        },
    );
}

/// The keys that format, while the editor has them. They act on the note being
/// written and nowhere else in the window.
pub fn shortcuts(app: &mut App, ui: &egui::Ui) {
    let id = editor_id();
    if !ui.memory(|memory| memory.has_focus(id)) {
        return;
    }
    let key = |modifiers: Modifiers, key: egui::Key| {
        ui.input_mut(|input| input.consume_key(modifiers, key))
    };
    // The shifted spelling is taken first: every modifier asked for must be
    // held, and Ctrl+Shift+X would otherwise pass for Ctrl+X.
    let pressed = if key(Modifiers::CTRL | Modifiers::SHIFT, egui::Key::X) {
        Some(Mark::Strike)
    } else if key(Modifiers::CTRL, egui::Key::B) {
        Some(Mark::Bold)
    } else if key(Modifiers::CTRL, egui::Key::I) {
        Some(Mark::Italic)
    } else if key(Modifiers::CTRL, egui::Key::K) {
        Some(Mark::Link)
    } else {
        None
    };
    if let Some(mark) = pressed {
        press(app, ui, mark, id);
    }
}

/// Where a button was drawn, for layout tests.
pub fn button_id(mark: Mark) -> egui::Id {
    egui::Id::new(("notes-mark", mark.name()))
}

/// Applies a mark over what the editor has selected, and puts the selection
/// back where the edit says.
fn press(app: &mut App, ui: &egui::Ui, mark: Mark, id: egui::Id) {
    let Some(selection) = editor::selection_of(ui, id) else {
        return;
    };
    let edit = apply(&app.note_draft, mark, selection);
    editor::select(ui, id, edit.caret);
    if edit.text != app.note_draft {
        app.note_draft = edit.text;
        app.actions.push(Action::SaveNote);
    }
}

/// The note toolbar's own tests: they drive a whole frame, because the caret
/// and the buttons only mean something once they are drawn.
#[cfg(all(test, feature = "demo"))]
mod tests {
    use super::*;
    use crate::notes::{Note, NoteStore};

    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 800.0),
            )),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            app.frame_ui(ui);
        });
        output.textures_delta.clear();
    }

    /// A window with one note open and the caret over `select`.
    fn open_note(select: edit::Selection) -> (App, egui::Context, i64) {
        let directory = tempfile::tempdir().expect("a temporary folder");
        let (mut app, _events) = App::headless(
            crate::paths::AppDirs::under(directory.path()),
            crate::settings::Settings::default(),
        );
        std::mem::forget(directory);
        // A window with the sample account linked, so the notes tab is what
        // the frame draws rather than the linking screen.
        crate::demo::populate(&mut app);
        let store = NoteStore::in_memory().expect("an in-memory store");
        let mut note = Note {
            body: "abc def".to_owned(),
            ..Note::new()
        };
        store.save(&mut note).expect("saves");
        app.notes = Some(store);
        app.page = crate::model::Page::Notes;
        app.notes_loaded = false;
        app.refresh_notes();
        app.actions.push(Action::OpenNote(note.id));
        let ctx = egui::Context::default();
        app.attach(&ctx);
        // One frame applies the open, the next draws the note and its tools.
        frame(&mut app, &ctx, Vec::new());
        frame(&mut app, &ctx, Vec::new());
        let id = editor_id();
        ctx.memory_mut(|memory| memory.request_focus(id));
        let mut state = egui::text_edit::TextEditState::load(&ctx, id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(select.from),
                egui::text::CCursor::new(select.to),
            )));
        state.store(&ctx, id);
        (app, ctx, note.id)
    }

    fn click(app: &mut App, ctx: &egui::Context, mark: Mark) {
        let rect = ctx
            .data(|data| data.get_temp::<egui::Rect>(button_id(mark)))
            .unwrap_or_else(|| panic!("{mark:?} was not drawn"));
        let pos = rect.center();
        let press = |pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        };
        frame(app, ctx, vec![egui::Event::PointerMoved(pos), press(true)]);
        frame(app, ctx, vec![press(false)]);
    }

    #[test]
    fn the_bold_button_wraps_the_selection_and_keeps_the_caret() {
        let (mut app, ctx, id) = open_note(edit::Selection::new(0, 3));
        click(&mut app, &ctx, Mark::Bold);
        assert_eq!(app.note_draft, "**abc** def");
        let range = egui::text_edit::TextEditState::load(&ctx, editor_id())
            .and_then(|state| state.cursor.char_range())
            .expect("a caret");
        assert_eq!(range.primary.index, egui::text::CharIndex(5));
        assert_eq!(range.secondary.index, egui::text::CharIndex(2));
        // The edit was saved through the note's usual path, not just drawn.
        let stored = app
            .notes
            .as_ref()
            .expect("a store")
            .get(id)
            .expect("reads")
            .expect("the note");
        assert_eq!(stored.body, "**abc** def");
    }

    #[test]
    fn a_line_mark_gives_every_selected_line_its_prefix() {
        let (mut app, ctx, _) = open_note(edit::Selection::new(0, 7));
        click(&mut app, &ctx, Mark::Bullet);
        assert_eq!(app.note_draft, "- abc def");
        click(&mut app, &ctx, Mark::Bullet);
        assert_eq!(app.note_draft, "abc def", "the same button takes it off");
    }

    #[test]
    fn the_link_button_points_somewhere_the_reader_can_finish() {
        let (mut app, ctx, _) = open_note(edit::Selection::new(0, 3));
        click(&mut app, &ctx, Mark::Link);
        assert_eq!(app.note_draft, "[abc](https://) def");
    }

    #[test]
    fn the_markdown_shortcuts_act_only_on_the_notes_page() {
        let (mut app, ctx, _) = open_note(edit::Selection::new(0, 3));
        // The editor has the keys: Ctrl+B is the button.
        let ctrl_b = egui::Event::Key {
            key: egui::Key::B,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        };
        frame(&mut app, &ctx, vec![ctrl_b]);
        assert_eq!(app.note_draft, "**abc** def");

        // On any other page the same keys change nothing.
        let (mut app, ctx, _) = open_note(edit::Selection::new(0, 3));
        app.page = crate::model::Page::Settings;
        let ctrl_b = egui::Event::Key {
            key: egui::Key::B,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        };
        frame(&mut app, &ctx, vec![ctrl_b]);
        assert_eq!(app.note_draft, "abc def");
    }
}
