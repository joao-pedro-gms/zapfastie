//! The column of notes: its name and a way to start one, a search field, then
//! the notes in groups, the ones pinned first and the newest above the older.

use egui::{Align, Layout, Rect, Sense, Vec2, pos2, vec2};

use crate::app::App;
use crate::i18n::gettext;
use crate::model::Action;
use crate::notes::NoteSummary;
use crate::theme::{self, Icon, Palette};

use super::widgets;

/// The groups the list sorts notes into, in the order they are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Group {
    Pinned,
    Today,
    Yesterday,
    ThisWeek,
    Earlier,
}

impl Group {
    const ALL: [Self; 5] = [
        Self::Pinned,
        Self::Today,
        Self::Yesterday,
        Self::ThisWeek,
        Self::Earlier,
    ];

    /// Which group a note is shown under: the pinned ones always first, the
    /// rest by how long ago they were written.
    fn of(note: &NoteSummary, today: jiff::civil::Date) -> Self {
        if note.pinned {
            return Self::Pinned;
        }
        let days = crate::util::day_key(note.updated_at)
            .and_then(|day| today.since(day).ok())
            .map(|span| span.get_days())
            .unwrap_or(i32::MAX);
        match days {
            0 => Self::Today,
            1 => Self::Yesterday,
            2..=6 => Self::ThisWeek,
            _ => Self::Earlier,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Pinned => "Pinned",
            Self::Today => "Today",
            Self::Yesterday => "Yesterday",
            Self::ThisWeek => "This week",
            Self::Earlier => "Earlier",
        }
    }
}

/// Draws the column: its header, the search field, and the notes.
pub fn show(app: &mut App, ui: &mut egui::Ui, palette: &Palette, tall: bool) {
    header(app, ui, palette);
    search(app, ui, palette);
    ui.add_space(4.0);
    ui.ctx()
        .data_mut(|data| data.insert_temp(super::list_id(), ui.available_rect_before_wrap()));
    if let Some(error) = app.notes_error.clone() {
        hint(ui, palette, &error);
        return;
    }
    if !app.notes_loaded {
        app.actions.push(Action::RefreshNotes);
        return;
    }
    if app.notes_list.is_empty() {
        empty(app, ui, palette);
        return;
    }
    let rows = app.notes_list.clone();
    let today = crate::util::day_key(crate::util::now()).unwrap_or_default();
    let open = app.open_note.as_ref().map(|note| note.id);
    egui::ScrollArea::vertical()
        .id_salt("notes-rows")
        .auto_shrink([false, tall])
        .show(ui, |ui| {
            for group in Group::ALL {
                let mut in_group = rows
                    .iter()
                    .filter(|note| Group::of(note, today) == group)
                    .peekable();
                if in_group.peek().is_none() {
                    continue;
                }
                ui.add_space(8.0);
                let index = Group::ALL
                    .iter()
                    .position(|known| *known == group)
                    .unwrap_or(0);
                ui.ctx().data_mut(|data| {
                    data.insert_temp(
                        egui::Id::new(("notes-group", index)),
                        group.name().to_owned(),
                    )
                });
                let label = gettext(app.locale, group.name());
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(label.as_ref())
                            .font(theme::semibold(11.5))
                            .color(palette.dim),
                    )
                    .selectable(false),
                );
                ui.add_space(2.0);
                for note in in_group {
                    row(app, ui, palette, note, open == Some(note.id));
                }
            }
        });
}

/// The name of the column, and the button that starts a note.
fn header(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    let locale = app.locale;
    let top = ui.max_rect();
    // The caption buttons ZapFastie draws over a corner the header reaches.
    let room = crate::ui::caption_room(ui.ctx(), top, 140.0);
    crate::ui::caption_drag(
        ui,
        Rect::from_min_size(top.min, vec2(top.width(), theme::TOP_BAR_HEIGHT + room.top)),
    );
    ui.add_space(8.0 + room.top);
    ui.horizontal(|ui| {
        ui.add_space(room.left);
        let (icon, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
        theme::paint_icon(ui, Icon::Notebook, icon, 18.0, palette.secondary);
        ui.label(
            egui::RichText::new(gettext(locale, "Notes").as_ref())
                .size(16.0)
                .strong()
                .color(palette.text),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(room.right);
            let new = gettext(locale, "New note");
            if theme::icon_button(ui, Icon::Plus, 18.0, palette.secondary, palette.text, &new)
                .clicked()
            {
                app.actions.push(Action::NewNote);
            }
        });
    });
    ui.add_space(8.0);
}

/// The field that filters the list. Typing reads the list again on the next
/// pass, with what was typed.
fn search(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    let locale = app.locale;
    if app.notes_focus {
        ui.ctx()
            .memory_mut(|memory| memory.request_focus(super::search_id()));
        app.notes_focus = false;
    }
    let placeholder = gettext(locale, "Search notes");
    let before = app.notes_search.clone();
    widgets::search_field(
        ui,
        palette,
        super::search_id(),
        &mut app.notes_search,
        &placeholder,
        ui.available_width(),
    );
    if app.notes_search != before {
        app.actions.push(Action::RefreshNotes);
    }
}

/// No notes at all, or nothing matching the search: a line that says which,
/// and the way to start writing.
fn empty(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    let locale = app.locale;
    if app.notes_search.trim().is_empty() {
        widgets::empty_state(
            ui,
            palette,
            Icon::Notebook,
            gettext(locale, "No notes yet").as_ref(),
            gettext(
                locale,
                "A note stays on this computer, where only you read it.",
            )
            .as_ref(),
        );
        let new = gettext(locale, "New note");
        if theme::soft_button(ui, palette, Some(Icon::Plus), &new, false).clicked() {
            app.actions.push(Action::NewNote);
        }
        return;
    }
    hint(
        ui,
        palette,
        gettext(locale, "No notes match your search").as_ref(),
    );
}

/// A line where the list would be: nothing matching, or why the notes could
/// not be opened.
fn hint(ui: &mut egui::Ui, palette: &Palette, text: &str) {
    ui.add_space(18.0);
    ui.label(
        egui::RichText::new(text)
            .size(13.0)
            .color(palette.secondary),
    );
}

/// One note in the list: its title, the first words under it, when it was last
/// written, and, while the pointer rests on it, the way to pin it.
fn row(app: &mut App, ui: &mut egui::Ui, palette: &Palette, note: &NoteSummary, selected: bool) {
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), super::ROW_HEIGHT),
        Sense::click(),
    );
    ui.ctx()
        .data_mut(|data| data.insert_temp(super::row_id(note.id), rect));
    let fill = if selected {
        palette.surface_hover
    } else if response.hovered() {
        palette.surface
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 8.0, fill);
    let inner = rect.shrink2(vec2(10.0, 6.0));
    let stamp = crate::util::chat_stamp(app.locale, note.updated_at);
    let stamp_width = 54.0;
    let shown = if note.pinned {
        (Icon::Pin, gettext(app.locale, "Unpin the note"))
    } else {
        (Icon::PinOff, gettext(app.locale, "Pin the note"))
    };
    let mut left = inner.left();
    if note.pinned {
        let pin = Rect::from_min_size(pos2(left, inner.top() + 3.0), Vec2::splat(13.0));
        theme::paint_icon(ui, Icon::Pin, pin, 13.0, palette.secondary);
        left += 19.0;
    }
    let title = if note.title.is_empty() {
        gettext(app.locale, "Untitled note").into_owned()
    } else {
        note.title.clone()
    };
    line(
        ui,
        Rect::from_min_size(
            pos2(left, inner.top()),
            vec2(inner.right() - left - stamp_width, 18.0),
        ),
        egui::RichText::new(title)
            .font(theme::medium(14.0))
            .color(palette.text),
    );
    line(
        ui,
        Rect::from_min_size(
            pos2(inner.right() - stamp_width, inner.top()),
            vec2(stamp_width, 18.0),
        ),
        egui::RichText::new(stamp)
            .font(theme::regular(11.0))
            .color(palette.dim),
    );
    // A note that cites a message says so beside its time.
    if note.cited {
        let cited = Rect::from_min_size(
            pos2(inner.right() - stamp_width - 16.0, inner.top() + 3.0),
            vec2(12.0, 12.0),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(cited)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        theme::icon(&mut child, Icon::MessageCircle, 12.0, palette.dim);
    }
    line(
        ui,
        Rect::from_min_size(
            pos2(inner.left(), inner.top() + 21.0),
            vec2(inner.width(), 16.0),
        ),
        egui::RichText::new(&note.excerpt)
            .font(theme::regular(12.0))
            .color(palette.secondary),
    );
    if response.hovered() {
        let (icon, tip) = shown;
        let at = Rect::from_min_size(
            pos2(inner.right() - 22.0, inner.bottom() - 18.0),
            Vec2::splat(18.0),
        );
        let button = ui.interact(at, super::row_id(note.id).with("pin"), Sense::click());
        theme::paint_icon(ui, icon, at, 14.0, palette.secondary);
        if button.clicked() {
            app.actions.push(Action::PinNote {
                id: note.id,
                pinned: !note.pinned,
            });
            return;
        }
        let _ = tip;
    }
    if response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        app.actions.push(Action::OpenNote(note.id));
    }
}

/// One truncated line of a row, left aligned in its rect.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(pinned: bool, at: i64) -> NoteSummary {
        NoteSummary {
            id: at,
            title: "A note".into(),
            pinned,
            updated_at: at,
            excerpt: String::new(),
            cited: false,
        }
    }

    #[test]
    fn the_list_groups_pinned_today_and_older() {
        let now = crate::util::now();
        let today = crate::util::day_key(now).expect("today");
        let day = 86_400;
        assert_eq!(
            Group::of(&summary(true, now - 40 * day), today),
            Group::Pinned,
            "a pinned note is first however old it is"
        );
        assert_eq!(Group::of(&summary(false, now), today), Group::Today);
        assert_eq!(
            Group::of(&summary(false, now - day), today),
            Group::Yesterday
        );
        assert_eq!(
            Group::of(&summary(false, now - 3 * day), today),
            Group::ThisWeek
        );
        assert_eq!(
            Group::of(&summary(false, now - 40 * day), today),
            Group::Earlier
        );
        // And the groups are drawn in that order.
        let order: Vec<&str> = Group::ALL.iter().map(|group| group.name()).collect();
        assert_eq!(
            order,
            ["Pinned", "Today", "Yesterday", "This week", "Earlier"]
        );
    }
}
