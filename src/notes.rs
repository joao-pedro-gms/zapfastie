//! Notes: markdown kept on this computer, beside the chats and, later, the
//! mail.
//!
//! A note is text the reader wrote. It can cite what it was written about: a
//! message in a chat, and one day a mail. The citation is a typed reference,
//! kept in `note_links`, so a note survives its subject (a deleted message
//! leaves a reference that no longer resolves, and the note still reads).
//!
//! The store is the app's own, not an account's: it lives in the state folder
//! under a key of its own in the OS keyring, and unlinking a WhatsApp number
//! does not touch it. It is one small SQLCipher database, opened the same way
//! the chat archive is.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use crate::util::now;

/// The tables of a notes database, created on open.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS notes (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL DEFAULT '',
    pinned INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS note_links (
    note INTEGER NOT NULL,
    chat TEXT NOT NULL DEFAULT '',
    message TEXT NOT NULL DEFAULT '',
    mail_account TEXT NOT NULL DEFAULT '',
    mail_uid TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS note_links_note ON note_links (note);
CREATE INDEX IF NOT EXISTS note_links_message ON note_links (chat, message);
";

/// What a note was written about.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NoteRef {
    /// A message of a chat, by the chat's canonical id and the message's.
    Message { chat: String, message: String },
    /// A mail, by the account it arrived on and the message's uid there. No
    /// mail is read yet: the shape exists so notes written before mail ships
    /// need no migration after it.
    Mail { account: String, uid: String },
}

/// A note, whole.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Note {
    /// The row's id; zero for a note that has not been saved yet.
    pub id: i64,
    /// What the list shows. The first line of the body, unless the reader
    /// renamed it.
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Note {
    /// A new, empty note, titled by the first line of its body.
    pub fn new() -> Self {
        let at = now();
        Self {
            created_at: at,
            updated_at: at,
            ..Default::default()
        }
    }

    /// The title a note takes from its body: its first line with something in
    /// it, trimmed, and cut to [`TITLE_LIMIT`] characters so a pasted
    /// paragraph does not become a row three screens tall.
    pub fn title_from(body: &str) -> String {
        let first = body
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or_default();
        let title = first.trim_start_matches('#').trim();
        title.chars().take(TITLE_LIMIT).collect()
    }

    /// Whether the note holds nothing worth keeping.
    pub fn is_blank(&self) -> bool {
        self.body.trim().is_empty() && self.title.trim().is_empty()
    }
}

/// How much of a note's first line a list row shows.
pub const TITLE_LIMIT: usize = 80;

/// A note as the list needs it, without its body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteSummary {
    pub id: i64,
    pub title: String,
    pub pinned: bool,
    pub updated_at: i64,
    /// The first words of the body under the title, for the row's second line.
    pub excerpt: String,
}

/// How much of a note a list row quotes under its title.
const EXCERPT_CHARS: usize = 120;

/// The notes of this computer.
pub struct NoteStore {
    connection: Connection,
}

impl NoteStore {
    /// Opens the notes database at `path`, creating it and its keyring key on
    /// first use.
    pub fn open(path: &Path) -> Result<Self> {
        Self::prepare(crate::archive::open_own_database(path)?)
    }

    /// A store for tests and the demo: nothing touches the disk.
    pub fn in_memory() -> Result<Self> {
        Self::prepare(Connection::open_in_memory()?)
    }

    fn prepare(connection: Connection) -> Result<Self> {
        connection.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        connection.execute_batch(SCHEMA)?;
        Ok(Self { connection })
    }

    /// Every note, pinned first and then the most recently written.
    pub fn list(&self) -> Result<Vec<NoteSummary>> {
        self.summaries(
            "SELECT id, title, body, pinned, updated_at FROM notes
             ORDER BY pinned DESC, updated_at DESC, id DESC",
            [],
        )
    }

    /// Runs a list query whose first five columns are a summary's.
    fn summaries(&self, sql: &str, params: impl rusqlite::Params) -> Result<Vec<NoteSummary>> {
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement.query_map(params, |row| {
            let body: String = row.get(2)?;
            Ok(NoteSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                pinned: row.get(3)?,
                updated_at: row.get(4)?,
                excerpt: body
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .skip(1)
                    .collect::<Vec<_>>()
                    .join(" ")
                    .chars()
                    .take(EXCERPT_CHARS)
                    .collect(),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// One note, body and all.
    pub fn get(&self, id: i64) -> Result<Option<Note>> {
        Ok(self
            .connection
            .query_row(
                "SELECT id, title, body, pinned, created_at, updated_at FROM notes WHERE id = ?1",
                [id],
                |row| {
                    Ok(Note {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        body: row.get(2)?,
                        pinned: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    /// Writes a note, inserting it when it has no id yet and returning the id
    /// it was given. The title follows the body unless it was set by hand.
    pub fn save(&self, note: &mut Note) -> Result<()> {
        let at = now();
        note.updated_at = at;
        note.created_at = if note.created_at == 0 {
            at
        } else {
            note.created_at
        };
        if note.title.trim().is_empty() {
            note.title = Note::title_from(&note.body);
        }
        if note.id == 0 {
            self.connection.execute(
                "INSERT INTO notes (title, body, pinned, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    note.title,
                    note.body,
                    note.pinned,
                    note.created_at,
                    note.updated_at
                ],
            )?;
            note.id = self.connection.last_insert_rowid();
        } else {
            self.connection.execute(
                "UPDATE notes SET title = ?2, body = ?3, pinned = ?4, updated_at = ?5
                 WHERE id = ?1",
                params![note.id, note.title, note.body, note.pinned, note.updated_at],
            )?;
        }
        Ok(())
    }

    /// Removes a note and what it cited.
    pub fn delete(&self, id: i64) -> Result<()> {
        self.connection
            .execute("DELETE FROM notes WHERE id = ?1", [id])?;
        self.connection
            .execute("DELETE FROM note_links WHERE note = ?1", [id])?;
        Ok(())
    }

    /// Notes whose title or body holds `query`, in the list's order. An empty
    /// query is the whole list.
    pub fn search(&self, query: &str) -> Result<Vec<NoteSummary>> {
        let query = query.trim();
        if query.is_empty() {
            return self.list();
        }
        let needle = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
        self.summaries(
            "SELECT id, title, body, pinned, updated_at FROM notes
             WHERE title LIKE ?1 ESCAPE '\\' OR body LIKE ?1 ESCAPE '\\'
             ORDER BY pinned DESC, updated_at DESC, id DESC",
            [&needle],
        )
    }

    /// What a note cites, oldest citation first.
    pub fn links(&self, id: i64) -> Result<Vec<NoteRef>> {
        let mut statement = self.connection.prepare(
            "SELECT chat, message, mail_account, mail_uid FROM note_links
             WHERE note = ?1 ORDER BY rowid",
        )?;
        let rows = statement.query_map([id], |row| {
            let (chat, message, account, uid): (String, String, String, String) =
                (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?);
            Ok(if !account.is_empty() || !uid.is_empty() {
                NoteRef::Mail { account, uid }
            } else {
                NoteRef::Message { chat, message }
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Notes that cite one message, newest first: the same message can be in
    /// more than one note.
    pub fn notes_citing(&self, chat: &str, message: &str) -> Result<Vec<i64>> {
        let mut statement = self.connection.prepare(
            "SELECT note FROM note_links WHERE chat = ?1 AND message = ?2
             ORDER BY rowid DESC",
        )?;
        let rows = statement.query_map([chat, message], |row| row.get::<_, i64>(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Records that a note cites `reference`, once.
    pub fn link(&self, note: i64, reference: &NoteRef) -> Result<()> {
        let (chat, message, account, uid) = match reference {
            NoteRef::Message { chat, message } => (chat.as_str(), message.as_str(), "", ""),
            NoteRef::Mail { account, uid } => ("", "", account.as_str(), uid.as_str()),
        };
        self.connection.execute(
            "INSERT INTO note_links (note, chat, message, mail_account, mail_uid)
             SELECT ?1, ?2, ?3, ?4, ?5
             WHERE NOT EXISTS (
                SELECT 1 FROM note_links
                WHERE note = ?1 AND chat = ?2 AND message = ?3
                  AND mail_account = ?4 AND mail_uid = ?5
             )",
            params![note, chat, message, account, uid],
        )?;
        Ok(())
    }

    /// Drops a citation, and the note's other citations stay.
    pub fn unlink(&self, note: i64, reference: &NoteRef) -> Result<()> {
        let (chat, message, account, uid) = match reference {
            NoteRef::Message { chat, message } => (chat.as_str(), message.as_str(), "", ""),
            NoteRef::Mail { account, uid } => ("", "", account.as_str(), uid.as_str()),
        };
        self.connection.execute(
            "DELETE FROM note_links
             WHERE note = ?1 AND chat = ?2 AND message = ?3
               AND mail_account = ?4 AND mail_uid = ?5",
            params![note, chat, message, account, uid],
        )?;
        Ok(())
    }

    /// How many notes there are, for the rail's badge and the empty state.
    pub fn count(&self) -> Result<usize> {
        let count: i64 = self
            .connection
            .query_row("SELECT count(*) FROM notes", [], |row| row.get(0))
            .context("counting the notes")?;
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> NoteStore {
        NoteStore::in_memory().expect("an in-memory store")
    }

    fn written(store: &NoteStore, title: &str, body: &str) -> i64 {
        let mut note = Note {
            title: title.to_owned(),
            body: body.to_owned(),
            ..Note::new()
        };
        store.save(&mut note).expect("saves");
        note.id
    }

    #[test]
    fn a_note_is_written_read_back_and_removed() {
        let store = store();
        let id = written(&store, "Groceries", "milk\nbread");
        let note = store.get(id).expect("reads").expect("the note");
        assert_eq!(note.title, "Groceries");
        assert_eq!(note.body, "milk\nbread");
        assert!(!note.pinned);
        assert!(note.created_at > 0);
        assert_eq!(store.count().expect("counts"), 1);
        store.delete(id).expect("deletes");
        assert!(store.get(id).expect("reads").is_none());
        assert_eq!(store.count().expect("counts"), 0);
    }

    #[test]
    fn a_note_without_a_title_takes_the_first_line_of_its_body() {
        assert_eq!(Note::title_from("  # Buy milk  \nand bread"), "Buy milk");
        assert_eq!(Note::title_from("\n\ntext"), "text");
        assert_eq!(Note::title_from(""), "");
        let long = "x".repeat(TITLE_LIMIT + 40);
        assert_eq!(Note::title_from(&long).chars().count(), TITLE_LIMIT);
    }

    #[test]
    fn the_list_puts_pinned_notes_first_and_the_newest_of_the_rest_next() {
        let store = store();
        let older = written(&store, "Older", "one");
        let newer = written(&store, "Newer", "two");
        let pinned = written(&store, "Pinned", "three");
        let mut note = store.get(pinned).expect("reads").expect("the note");
        note.pinned = true;
        // The store writes `updated_at` itself, so pinning an older note still
        // sorts it first by its flag and not by the clock.
        store.save(&mut note).expect("saves");
        let listed: Vec<String> = store
            .list()
            .expect("lists")
            .into_iter()
            .map(|note| note.title)
            .collect();
        assert_eq!(listed, ["Pinned", "Newer", "Older"]);
        assert!(older < newer, "the ids follow the order they were written");
    }

    #[test]
    fn a_note_row_quotes_the_body_below_its_first_line() {
        let store = store();
        written(&store, "Title", "Title\nthe rest of it\nand more");
        let summary = &store.list().expect("lists")[0];
        assert_eq!(summary.title, "Title");
        assert_eq!(summary.excerpt, "the rest of it and more");
    }

    #[test]
    fn a_search_finds_a_word_in_the_title_or_the_body() {
        let store = store();
        written(&store, "Groceries", "milk");
        written(&store, "Ideas", "the milk price");
        written(&store, "Other", "nothing");
        let found: Vec<String> = store
            .search("milk")
            .expect("searches")
            .into_iter()
            .map(|note| note.title)
            .collect();
        assert_eq!(found.len(), 2, "both notes naming milk");
        assert_eq!(store.search("").expect("searches").len(), 3);
        // A wildcard typed by the reader is a character, not a pattern.
        assert!(store.search("%").expect("searches").is_empty());
    }

    #[test]
    fn a_note_remembers_what_it_cites_and_a_message_finds_its_notes() {
        let store = store();
        let id = written(&store, "About that", "");
        let cited = NoteRef::Message {
            chat: "1@s.whatsapp.net".into(),
            message: "abc".into(),
        };
        store.link(id, &cited).expect("links");
        store.link(id, &cited).expect("linking twice keeps one");
        store
            .link(
                id,
                &NoteRef::Mail {
                    account: "me@example.com".into(),
                    uid: "42".into(),
                },
            )
            .expect("links a mail");
        assert_eq!(
            store.links(id).expect("reads"),
            vec![
                cited.clone(),
                NoteRef::Mail {
                    account: "me@example.com".into(),
                    uid: "42".into()
                }
            ]
        );
        assert_eq!(
            store
                .notes_citing("1@s.whatsapp.net", "abc")
                .expect("finds"),
            vec![id]
        );
        store.unlink(id, &cited).expect("unlinks");
        assert_eq!(store.links(id).expect("reads").len(), 1);
        store.delete(id).expect("deletes");
        assert!(
            store
                .notes_citing("1@s.whatsapp.net", "abc")
                .expect("finds")
                .is_empty(),
            "a deleted note cites nothing"
        );
    }

    #[test]
    fn a_citation_survives_the_message_it_names() {
        let store = store();
        let id = written(&store, "Quoted", "");
        store
            .link(
                id,
                &NoteRef::Message {
                    chat: "gone@s.whatsapp.net".into(),
                    message: "deleted".into(),
                },
            )
            .expect("links");
        // The note reads with its citation and without the message in sight.
        assert_eq!(store.get(id).expect("reads").expect("note").title, "Quoted");
        assert_eq!(store.links(id).expect("reads").len(), 1);
    }

    #[test]
    fn the_store_keeps_notes_across_opens_on_disk() {
        let directory = tempfile::tempdir().expect("a temporary folder");
        let path = directory.path().join("notes").join("notes.db");
        // The platform keyring is not there on a build machine, so the test
        // carries its own: the file is still opened and closed the same way.
        let keyring = keyring_core::mock::Store::new().expect("a keyring");
        let open = |path: &Path| {
            NoteStore::prepare(
                crate::archive::open_own_database_in(path, &*keyring).expect("opens"),
            )
            .expect("prepares")
        };
        let id = {
            let store = open(&path);
            written(&store, "Kept", "body")
        };
        let store = open(&path);
        assert_eq!(store.get(id).expect("reads").expect("note").title, "Kept");
        // The file is encrypted, not a plain SQLite database.
        let bytes = std::fs::read(&path).expect("reads the file");
        assert!(
            !bytes
                .windows(b"SQLite format 3".len())
                .any(|window| window == b"SQLite format 3"),
            "the notes database is not plaintext"
        );
    }
}
