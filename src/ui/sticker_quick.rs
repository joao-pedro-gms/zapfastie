//! The quick sticker picker: stickers alone, over the open chat.
//!
//! The composer's picker holds emoji, GIFs and stickers behind one button and
//! a tab strip. This one is the sticker half on its own, opened from the keys
//! at a chat: a search field that takes an emoji, a word that names one, or a
//! pack's name, then one grid of every sticker that can be sent, favorites and
//! recent ones first. A click sends; Enter sends the highlighted sticker; the
//! arrows walk the grid; Escape closes. Nothing here is an emoji or a GIF.

use std::path::{Path, PathBuf};

use egui::{CornerRadius, Frame, Margin, Sense, Stroke, Vec2, vec2};

use crate::app::App;
use crate::i18n::gettext;
use crate::model::Action;
use crate::theme::{self, Palette};

use super::widgets;

/// The card's width at most; a narrow window shrinks it.
const WIDTH: f32 = 600.0;
/// The card's height at most; a short window shrinks it.
const HEIGHT: f32 = 520.0;
/// Sticker columns. The arrows move by a row, so the keys and the grid agree
/// on it.
pub(crate) const COLUMNS: usize = 6;
/// Gap between tiles.
const GAP: f32 = 8.0;
/// The search field's height, which the grid's room is measured without.
const FIELD_HEIGHT: f32 = 34.0;

/// The picker's `Modal`, the layer it is drawn on.
pub(crate) fn modal_id() -> egui::Id {
    egui::Id::new("quick-stickers")
}

/// The search field's id, which takes focus as the picker opens.
fn search_id() -> egui::Id {
    egui::Id::new("quick-sticker-search")
}

/// Where the grid drew the tile of `path`, for interaction tests.
pub fn tile_id(path: &Path) -> egui::Id {
    egui::Id::new(("quick-sticker-tile", path))
}

/// The stickers the picker lists: the whole library, favorites and recent
/// ones first, or what `query` matches. The keys list them again for the text
/// the field holds now, so Enter sends what the reader sees rather than the
/// pass before.
pub(crate) fn results(app: &App, query: &str) -> Vec<PathBuf> {
    crate::sticker_search::quick(
        &crate::sticker_search::Library {
            recent: &app.stickers,
            favorites: &app.stickers_saved,
            packs: &app.sticker_packs,
            received: &app.stickers_received,
            emojis: &app.sticker_emojis,
        },
        query,
    )
}

/// Draws the picker over the chat, if it is open.
pub fn show(app: &mut App, ctx: &egui::Context) {
    let Some(state) = app.quick_stickers.as_ref() else {
        return;
    };
    let palette = app.palette;
    let locale = app.locale;
    let animate = app.window_focused;
    let mut search = state.search.clone();
    let mut selected = state.selected;
    let mut focused = state.focused;
    let mut scroll_to = state.scroll_to;
    let list = results(app, &search);
    // A list that shrank, or never had the index, sends its last sticker.
    if selected >= list.len() {
        selected = list.len().saturating_sub(1);
    }
    let screen = ctx.content_rect();
    let width = WIDTH.min((screen.width() - 32.0).max(280.0));
    let height = HEIGHT.min((screen.height() - 48.0).max(220.0));
    // The card as tall as its rows: a reader with four favorites gets four,
    // not an empty half, and a reader with a hundred gets the rows that fit
    // and a scroll bar. Measured here rather than left to the content,
    // because a modal that sized itself keeps the height of its first pass.
    let row = cell_width(width) + GAP;
    let needed = list.len().div_ceil(COLUMNS).max(1) as f32;
    let room = (height - FIELD_HEIGHT - GAP * 2.0).max(row);
    let rows = needed.min(((room + GAP) / row).floor().max(1.0));
    let grid_height = rows * row - GAP;
    let card_height = if list.is_empty() {
        FIELD_HEIGHT + 72.0
    } else {
        FIELD_HEIGHT + GAP + grid_height
    };
    let card = Frame::new()
        .fill(palette.overlay)
        .stroke(Stroke::new(1.0, palette.outline))
        .corner_radius(CornerRadius::same(theme::RADIUS + 4))
        .inner_margin(Margin::same(16))
        .shadow(palette.modal_shadow());
    let mut chosen: Option<PathBuf> = None;
    let response = egui::Modal::new(modal_id())
        .frame(card)
        .backdrop_color(palette.shadow)
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.set_height(card_height);
            ui.spacing_mut().item_spacing = vec2(GAP, GAP);
            // The field takes the keys as the picker opens, so a sticker is a
            // word away and never a click first.
            if !focused {
                ui.ctx()
                    .memory_mut(|memory| memory.request_focus(search_id()));
                focused = true;
            }
            // The sticker tab's own hint, so every catalog already has it.
            let hint = gettext(locale, "Search by emoji, word, or pack");
            widgets::search_field(
                ui,
                &palette,
                search_id(),
                &mut search,
                &hint,
                ui.available_width(),
            );
            if list.is_empty() {
                let text = if search.trim().is_empty() {
                    gettext(locale, "Stickers you send appear here.")
                } else {
                    gettext(locale, "No stickers match your search.")
                };
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(text.as_ref())
                            .size(13.0)
                            .color(palette.secondary),
                    );
                });
                return;
            }
            egui::ScrollArea::vertical()
                .id_salt("quick-sticker-grid")
                .auto_shrink([false, true])
                .max_height(grid_height)
                .show(ui, |ui| {
                    let drawn = grid(ui, &palette, &list, selected, animate, scroll_to);
                    if let Some(index) = drawn.hovered {
                        selected = index;
                    }
                    if let Some((path, index)) = drawn.clicked {
                        selected = index;
                        chosen = Some(path);
                    }
                    if scroll_to {
                        scroll_to = false;
                    }
                });
        });
    if let Some(state) = &mut app.quick_stickers {
        // A new search starts again from the first sticker it finds.
        if state.search != search {
            selected = 0;
        }
        state.search = search;
        state.selected = selected;
        state.focused = focused;
        state.scroll_to = scroll_to;
    }
    if let Some(path) = chosen {
        app.actions.push(Action::SendSticker(path));
        app.actions.push(Action::CloseQuickStickers);
    } else if response.should_close() {
        app.actions.push(Action::CloseQuickStickers);
    }
}

/// One tile's side, from the width the grid has to fill.
fn cell_width(available: f32) -> f32 {
    (available - GAP * (COLUMNS as f32 - 1.0)) / COLUMNS as f32
}

/// What the pointer did to the grid: the tile it rests on, and the one it
/// clicked.
#[derive(Default)]
struct Drawn {
    hovered: Option<usize>,
    clicked: Option<(PathBuf, usize)>,
}

/// Draws the grid. A hovered tile becomes the selected one, so Enter sends
/// what the pointer rests on; a clicked tile is the sticker to send.
fn grid(
    ui: &mut egui::Ui,
    palette: &Palette,
    list: &[PathBuf],
    selected: usize,
    animate: bool,
    scroll_to: bool,
) -> Drawn {
    let columns = COLUMNS;
    let cell = cell_width(ui.available_width());
    ui.spacing_mut().item_spacing = vec2(GAP, GAP);
    let mut drawn = Drawn::default();
    for (row, chunk) in list.chunks(columns).enumerate() {
        ui.horizontal(|ui| {
            for (column, path) in chunk.iter().enumerate() {
                let index = row * columns + column;
                let (rect, response) = ui.allocate_exact_size(Vec2::splat(cell), Sense::click());
                ui.ctx()
                    .data_mut(|data| data.insert_temp(tile_id(path), rect));
                let current = index == selected;
                super::picker::paint_tile(
                    ui,
                    palette,
                    path,
                    rect,
                    response.hovered() || current,
                    animate,
                );
                if current {
                    ui.painter().rect_stroke(
                        rect,
                        8.0,
                        Stroke::new(2.0, palette.accent),
                        egui::StrokeKind::Inside,
                    );
                    // A key walked here: bring the sticker into view, and
                    // leave the scroll where the reader puts it after that.
                    if scroll_to {
                        ui.scroll_to_rect(rect, None);
                    }
                }
                if response.hovered() {
                    drawn.hovered = Some(index);
                }
                if response
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    drawn.clicked = Some((path.clone(), index));
                }
            }
        });
    }
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::QuickStickers;

    /// An app whose quick picker lists `stickers`, in a folder that goes when
    /// the app does.
    fn open(stickers: &[&str]) -> (tempfile::TempDir, App) {
        let root = tempfile::tempdir().expect("a temporary folder");
        let (mut app, _events) = App::headless(
            crate::paths::AppDirs::under(root.path()),
            crate::settings::Settings::default(),
        );
        let media = app.dirs.media_cache_dir();
        std::fs::create_dir_all(&media).expect("the media folder");
        let mut list = Vec::new();
        for name in stickers {
            let path = media.join(name);
            std::fs::write(&path, b"RIFF0000WEBP").expect("writes the sticker");
            list.push(path);
        }
        app.stickers = list;
        app.quick_stickers = Some(QuickStickers::default());
        app.open_chat = Some("1@s.whatsapp.net".to_owned());
        (root, app)
    }

    /// Draws one pass of the picker, as the window would.
    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 700.0),
                )),
                events,
                ..Default::default()
            },
            |ui| show(app, ui.ctx()),
        );
        output.textures_delta.clear();
    }

    fn click(pos: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    #[test]
    fn a_click_on_a_sticker_sends_it_and_closes_the_picker() {
        let (_root, mut app) = open(&["one.webp", "two.webp"]);
        let ctx = egui::Context::default();
        app.attach(&ctx);
        // A modal takes the pass that discards and the one that shows to
        // find its place, as a frame of the window does.
        frame(&mut app, &ctx, Vec::new());
        frame(&mut app, &ctx, Vec::new());
        let second = results(&app, "")[1].clone();
        let tile = ctx
            .data(|data| data.get_temp::<egui::Rect>(tile_id(&second)))
            .expect("the grid draws a tile per sticker");
        frame(&mut app, &ctx, click(tile.center()));
        assert!(
            app.actions.contains(&Action::SendSticker(second)),
            "a click sends that sticker"
        );
        assert!(
            app.actions.contains(&Action::CloseQuickStickers),
            "and closes the picker"
        );
    }

    #[test]
    fn a_search_that_empties_the_list_leaves_the_picker_open_with_no_sticker() {
        let (_root, mut app) = open(&["one.webp", "two.webp", "three.webp"]);
        let ctx = egui::Context::default();
        app.attach(&ctx);
        if let Some(state) = &mut app.quick_stickers {
            state.selected = 2;
            // Nothing is tagged, so nothing matches.
            state.search = "zzzz".to_owned();
        }
        frame(&mut app, &ctx, Vec::new());
        let state = app.quick_stickers.as_ref().expect("the picker stays open");
        assert!(results(&app, &state.search).is_empty());
        assert_eq!(
            state.selected, 0,
            "the selection is pulled back to the list"
        );
        assert!(app.actions.is_empty(), "no sticker was sent");
    }
}
