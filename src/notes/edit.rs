//! The plain text edits behind a note's toolbar and its typing: wrapping a
//! selection in markers, giving lines a prefix, carrying a list on to the next
//! line, and moving a list item one level.
//!
//! Every function takes the note's text and a selection counted in characters,
//! as egui counts a cursor, and returns the new text with where the caret goes.
//! Nothing here touches the database, egui or the clock, so all of it is
//! checked by tests that read like the sentence the reader typed.

/// A run of characters selected in a note, `from` before `to`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub from: usize,
    pub to: usize,
}

impl Selection {
    /// A selection between two caret positions, in either order.
    pub fn new(from: usize, to: usize) -> Self {
        Self {
            from: from.min(to),
            to: from.max(to),
        }
    }

    /// A caret with nothing selected.
    pub fn caret(at: usize) -> Self {
        Self { from: at, to: at }
    }

    /// Whether nothing is selected.
    pub fn is_empty(&self) -> bool {
        self.from == self.to
    }

    /// How many characters the selection spans.
    pub fn len(&self) -> usize {
        self.to - self.from
    }
}

/// A note after an edit: what it now says, and where the caret goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub text: String,
    pub caret: Selection,
}

/// Wraps the selection in `marker`, or takes the marker off when the text
/// around it already carries one. With nothing selected, the pair is written
/// and the caret lands between them.
pub fn wrap_selection(text: &str, selection: Selection, marker: &str) -> Edit {
    let start = byte_index(text, selection.from);
    let end = byte_index(text, selection.to);
    let width = marker.chars().count();
    if selection.is_empty() {
        let mut edited = String::with_capacity(text.len() + marker.len() * 2);
        edited.push_str(&text[..start]);
        edited.push_str(marker);
        edited.push_str(marker);
        edited.push_str(&text[start..]);
        return Edit {
            text: edited,
            caret: Selection::caret(selection.from + width),
        };
    }
    if text[..start].ends_with(marker) && text[end..].starts_with(marker) {
        let mut edited = String::with_capacity(text.len());
        edited.push_str(&text[..start - marker.len()]);
        edited.push_str(&text[start..end]);
        edited.push_str(&text[end + marker.len()..]);
        let from = selection.from - width;
        return Edit {
            text: edited,
            caret: Selection::new(from, from + selection.len()),
        };
    }
    let mut edited = String::with_capacity(text.len() + marker.len() * 2);
    edited.push_str(&text[..start]);
    edited.push_str(marker);
    edited.push_str(&text[start..end]);
    edited.push_str(marker);
    edited.push_str(&text[end..]);
    Edit {
        text: edited,
        caret: Selection::new(selection.from + width, selection.to + width),
    }
}

/// Wraps the selection in a link, leaving the address for the reader to
/// type: `[the words](https://)` with the caret before the closing bracket.
pub fn wrap_link(text: &str, selection: Selection) -> Edit {
    let start = byte_index(text, selection.from);
    let end = byte_index(text, selection.to);
    let mut edited = String::with_capacity(text.len() + 12);
    edited.push_str(&text[..start]);
    edited.push('[');
    edited.push_str(&text[start..end]);
    edited.push_str("](https://)");
    edited.push_str(&text[end..]);
    // Just inside the pair, before the closing bracket, with the address
    // selected so typing replaces it.
    let caret = selection.to + 2 + 1 + "https://".chars().count();
    Edit {
        text: edited,
        caret: Selection::caret(caret),
    }
}

/// Gives every line the selection touches `prefix`, or takes it off when the
/// lines carry it already.
pub fn prefix_lines(text: &str, selection: Selection, prefix: &str) -> Edit {
    let (line_start, line_end) = lines_touched(text, selection);
    let block = &text[line_start..line_end];
    let width = prefix.chars().count();
    let lines: Vec<&str> = block.split('\n').collect();
    let carried = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .all(|line| line.starts_with(prefix));
    let mut edited = String::new();
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            edited.push('\n');
        }
        match (carried, line.trim().is_empty()) {
            (true, _) if line.starts_with(prefix) => edited.push_str(&line[prefix.len()..]),
            (false, false) => {
                edited.push_str(prefix);
                edited.push_str(line);
            }
            _ => edited.push_str(line),
        }
    }
    let start_char = char_index(text, line_start);
    let caret = if selection.is_empty() {
        // The caret stays with the word it was in, moved by what the line
        // gained or lost before it.
        let after_line_start = selection.from.saturating_sub(start_char);
        let moved = if carried {
            after_line_start.saturating_sub(width)
        } else {
            after_line_start + width
        };
        Selection::caret(start_char + moved.min(edited.chars().count()))
    } else {
        let end = start_char + edited.chars().count();
        Selection::new(start_char, end)
    };
    let mut whole = String::with_capacity(text.len() + edited.len());
    whole.push_str(&text[..line_start]);
    whole.push_str(&edited);
    whole.push_str(&text[line_end..]);
    Edit { text: whole, caret }
}

/// What Enter does inside a list: a new line with the same marker, one number
/// further for an ordered list, and nothing at all on an item that is empty.
/// `None` when the caret is on a line that is not a list item.
pub fn continue_list(text: &str, caret: usize) -> Option<Edit> {
    let at = byte_index(text, caret);
    let line_start = text[..at].rfind('\n').map(|index| index + 1).unwrap_or(0);
    let line = &text[line_start..at];
    let (marker, ordered) = list_marker(line)?;
    // An item with nothing in it but its marker ends the list.
    if line.trim_end() == marker.trim_end() {
        let mut edited = String::with_capacity(text.len() - marker.len());
        edited.push_str(&text[..line_start]);
        edited.push_str(&text[line_start + marker.len()..]);
        let caret = char_index(text, line_start);
        return Some(Edit {
            text: edited,
            caret: Selection::caret(caret),
        });
    }
    let next = match ordered {
        Some(number) => format!("{}. ", number + 1),
        // A checklist starts its next line unchecked, whatever this one says.
        None => marker.replace("[x] ", "[ ] ").replace("[X] ", "[ ] "),
    };
    let mut edited = String::with_capacity(text.len() + next.len() + 1);
    edited.push_str(&text[..at]);
    edited.push('\n');
    edited.push_str(&next);
    edited.push_str(&text[at..]);
    let caret = char_index(text, at) + 1 + next.chars().count();
    Some(Edit {
        text: edited,
        caret: Selection::caret(caret),
    })
}

/// Moves the selected list lines one level in, or back out.
pub fn indent(text: &str, selection: Selection, steps: usize, outdent: bool) -> Edit {
    if steps == 0 {
        return Edit {
            text: text.to_owned(),
            caret: selection,
        };
    }
    let (line_start, line_end) = lines_touched(text, selection);
    let block = &text[line_start..line_end];
    let pad: String = "  ".repeat(steps);
    let mut edited = String::new();
    for (index, line) in block.split('\n').enumerate() {
        if index > 0 {
            edited.push('\n');
        }
        if line.trim().is_empty() {
            edited.push_str(line);
        } else if outdent {
            let taken = line.len() - line.trim_start_matches(' ').len();
            edited.push_str(&line[taken.min(pad.len())..]);
        } else {
            edited.push_str(&pad);
            edited.push_str(line);
        }
    }
    let mut whole = String::with_capacity(text.len() + edited.len());
    whole.push_str(&text[..line_start]);
    whole.push_str(&edited);
    whole.push_str(&text[line_end..]);
    let start_char = char_index(text, line_start);
    let end = start_char + edited.chars().count();
    Edit {
        text: whole,
        caret: Selection::new(start_char, end),
    }
}

/// The marker a list line opens with, and the number when it is ordered.
fn list_marker(line: &str) -> Option<(String, Option<u64>)> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    let text = &line[indent..];
    for bullet in ["- ", "* ", "+ "] {
        if let Some(rest) = text.strip_prefix(bullet) {
            // A checklist item carries the box in its marker.
            for task in ["[ ] ", "[x] ", "[X] "] {
                if let Some(after) = rest.strip_prefix(task) {
                    let marker = format!("{bullet}{task}");
                    let _ = after;
                    return Some((marker, None));
                }
            }
            return Some((bullet.to_owned(), None));
        }
    }
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    if !digits.is_empty() {
        let rest = &text[digits.len()..];
        for dot in [". ", ") "] {
            if let Some(after) = rest.strip_prefix(dot) {
                let marker = format!("{digits}{dot}");
                let _ = after;
                return Some((marker, digits.parse().ok()));
            }
        }
    }
    None
}

/// The bytes from the start of the line holding the selection's first end,
/// through the end of the line holding its last.
fn lines_touched(text: &str, selection: Selection) -> (usize, usize) {
    let start = byte_index(text, selection.from);
    let end = byte_index(text, selection.to);
    let line_start = text[..start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let line_end = text[end..]
        .find('\n')
        .map(|index| end + index)
        .unwrap_or(text.len());
    (line_start, line_end)
}

/// The byte at which the `chars`-th character starts, or the text's end.
fn byte_index(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

/// How many characters sit before that byte.
fn char_index(text: &str, bytes: usize) -> usize {
    text[..bytes.min(text.len())].chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrap_bolds_the_selection_and_keeps_the_caret_on_it() {
        let text = "400 g flour";
        // "400" is the first three characters.
        let edit = wrap_selection(text, Selection::new(0, 3), "**");
        assert_eq!(edit.text, "**400** g flour");
        assert_eq!(edit.caret, Selection::new(2, 5));
        // The same marker over the same words takes it off again.
        let back = wrap_selection(&edit.text, edit.caret, "**");
        assert_eq!(back.text, text);
        assert_eq!(back.caret, Selection::new(0, 3));
    }

    #[test]
    fn a_wrap_with_nothing_selected_writes_the_pair_and_sits_between() {
        let edit = wrap_selection("milk", Selection::caret(0), "**");
        assert_eq!(edit.text, "****milk");
        assert_eq!(edit.caret, Selection::caret(2));
    }

    #[test]
    fn a_prefix_toggles_every_line_the_selection_touches() {
        let text = "who signs the lease\nthe budget";
        let edit = prefix_lines(text, Selection::new(0, text.chars().count()), "- ");
        assert_eq!(edit.text, "- who signs the lease\n- the budget");
        let back = prefix_lines(&edit.text, edit.caret, "- ");
        assert_eq!(back.text, text);
        // An empty line in the middle stays empty.
        let spaced = prefix_lines("one\n\ntwo", Selection::new(0, 8), "- ");
        assert_eq!(spaced.text, "- one\n\n- two");
    }

    #[test]
    fn a_prefix_on_a_caret_gives_its_own_line_the_marker() {
        let edit = prefix_lines("milk", Selection::caret(4), "- ");
        assert_eq!(edit.text, "- milk");
        assert_eq!(edit.caret, Selection::caret(6));
    }

    #[test]
    fn enter_continues_a_bullet_list() {
        let text = "- milk";
        let edit = continue_list(text, 6).expect("a list line");
        assert_eq!(edit.text, "- milk\n- ");
        assert_eq!(edit.caret, Selection::caret(9));
    }

    #[test]
    fn enter_continues_a_numbered_list_one_further() {
        let edit = continue_list("1. milk", 7).expect("an ordered list");
        assert_eq!(edit.text, "1. milk\n2. ");
        let edit = continue_list("9. milk", 7).expect("an ordered list");
        assert_eq!(edit.text, "9. milk\n10. ");
    }

    #[test]
    fn enter_on_an_empty_item_ends_the_list() {
        let edit = continue_list("- milk\n- ", 9).expect("a list line");
        assert_eq!(edit.text, "- milk\n");
        assert_eq!(edit.caret, Selection::caret(7));
        assert!(continue_list("plain words", 4).is_none());
    }

    #[test]
    fn enter_continues_a_checklist_unchecked() {
        let text = "- [x] the budget";
        let edit = continue_list(text, text.chars().count()).expect("a checklist");
        assert_eq!(edit.text, "- [x] the budget\n- [ ] ");
        assert_eq!(edit.caret, Selection::caret(23));
    }

    #[test]
    fn indent_nests_the_selected_lines_and_takes_them_back_out() {
        let text = "- one\n- two";
        let edit = indent(text, Selection::new(0, text.chars().count()), 1, false);
        assert_eq!(edit.text, "  - one\n  - two");
        let back = indent(&edit.text, edit.caret, 1, true);
        assert_eq!(back.text, text);
    }

    #[test]
    fn a_wrap_keeps_a_right_to_left_line_intact() {
        let text = "المجموعة الثانية";
        let end = text.chars().count();
        let edit = wrap_selection(text, Selection::new(0, end), "**");
        assert_eq!(edit.text, format!("**{text}**"));
        assert_eq!(
            edit.text.chars().count(),
            text.chars().count() + 4,
            "no byte is lost or doubled"
        );
        let back = wrap_selection(&edit.text, edit.caret, "**");
        assert_eq!(back.text, text);
        assert_eq!(back.caret, Selection::new(0, end));
    }

    #[test]
    fn a_wrap_keeps_a_joined_emoji_intact() {
        // A family emoji is one grapheme of several scalars.
        let text = "family 👨‍👩‍👧 and a flag 🇧🇷";
        let end = text.chars().count();
        let edit = wrap_selection(text, Selection::new(0, end), "**");
        assert_eq!(edit.text, format!("**{text}**"));
        let back = wrap_selection(&edit.text, edit.caret, "**");
        assert_eq!(back.text, text);
        assert_eq!(back.caret, Selection::new(0, end));
        assert!(back.text.ends_with("🇧🇷"), "the flag is still whole");
    }
}
