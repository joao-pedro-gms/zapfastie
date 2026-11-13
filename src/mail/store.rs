//! One account's messages, kept on this computer in an encrypted database of
//! their own.
//!
//! The store holds what the list needs as soon as the account syncs (sender,
//! subject, date, unread) and the body only when the reader asks for it, so a
//! mailbox opens from disk before the server is reached and a message that was
//! never opened costs nothing.
//!
//! It lives beside the chats under the state folder and is opened the same way
//! they are: SQLCipher, with a key in the OS keyring.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

/// The tables of a mail database, created on open.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS mail (
    folder TEXT NOT NULL,
    uid INTEGER NOT NULL,
    subject TEXT NOT NULL DEFAULT '',
    sender TEXT NOT NULL DEFAULT '',
    recipient TEXT NOT NULL DEFAULT '',
    date INTEGER NOT NULL DEFAULT 0,
    message_id TEXT NOT NULL DEFAULT '',
    references_ TEXT NOT NULL DEFAULT '',
    seen INTEGER NOT NULL DEFAULT 0,
    flagged INTEGER NOT NULL DEFAULT 0,
    has_parts INTEGER NOT NULL DEFAULT 0,
    preview TEXT NOT NULL DEFAULT '',
    text TEXT,
    from_html INTEGER NOT NULL DEFAULT 0,
    parts TEXT NOT NULL DEFAULT '[]',
    PRIMARY KEY (folder, uid)
);
CREATE INDEX IF NOT EXISTS mail_by_date ON mail (folder, date DESC);
CREATE TABLE IF NOT EXISTS folders (
    name TEXT PRIMARY KEY,
    unread INTEGER NOT NULL DEFAULT 0,
    total INTEGER NOT NULL DEFAULT 0,
    synced_at INTEGER NOT NULL DEFAULT 0
);
";

/// A folder as the sidebar lists it: what the server said the last time the
/// mailbox was read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MailFolder {
    pub name: String,
    pub unread: usize,
    pub total: usize,
    /// When the folder last synced, in seconds since the epoch.
    pub synced_at: i64,
}

/// What a list row needs: the headers, and the first words once the message
/// has been opened.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MailSummary {
    pub uid: u32,
    pub folder: String,
    pub subject: String,
    pub sender: String,
    pub date: i64,
    pub seen: bool,
    pub flagged: bool,
    pub has_parts: bool,
    pub preview: String,
    /// Whether the body is here, so opening it costs no network.
    pub body_here: bool,
}

impl MailSummary {
    /// What the row says the message is, when the sender left its subject
    /// empty.
    pub fn title(&self) -> &str {
        if self.subject.trim().is_empty() {
            "(no subject)"
        } else {
            &self.subject
        }
    }
}

/// A message's body, as it was fetched and parsed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MailBody {
    pub text: String,
    /// Whether the words came from an HTML part.
    pub from_html: bool,
    /// The attachments, as JSON the parser wrote, read back when shown.
    pub parts: String,
}

/// One message as the sync hands it over: headers and flags only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MailHeader {
    pub uid: u32,
    pub subject: String,
    pub sender: String,
    pub recipient: String,
    pub date: i64,
    pub message_id: String,
    pub seen: bool,
    pub flagged: bool,
    pub has_parts: bool,
    pub preview: String,
}

/// The messages of one account.
pub struct MailStore {
    connection: Connection,
}

impl MailStore {
    /// Opens the database at `path`, creating it and its keyring key on first
    /// use.
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

    /// Writes the headers of a folder's messages, keeping the bodies already
    /// fetched and the flags the reader set here.
    pub fn upsert(&mut self, folder: &str, headers: &[MailHeader]) -> Result<()> {
        let transaction = self.connection.transaction()?;
        {
            let mut statement = transaction.prepare(
                "INSERT INTO mail (folder, uid, subject, sender, recipient, date, message_id,
                                   seen, flagged, has_parts, preview)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(folder, uid) DO UPDATE SET
                    subject = excluded.subject,
                    sender = excluded.sender,
                    recipient = excluded.recipient,
                    date = excluded.date,
                    message_id = excluded.message_id,
                    seen = MAX(seen, excluded.seen),
                    flagged = MAX(flagged, excluded.flagged),
                    has_parts = excluded.has_parts,
                    preview = CASE WHEN preview = '' THEN excluded.preview ELSE preview END",
            )?;
            for header in headers {
                statement.execute(params![
                    folder,
                    header.uid,
                    header.subject,
                    header.sender,
                    header.recipient,
                    header.date,
                    header.message_id,
                    header.seen,
                    header.flagged,
                    header.has_parts,
                    header.preview,
                ])?;
            }
        }
        transaction.execute(
            "INSERT INTO folders (name, total, synced_at)
             SELECT ?1, count(*), ?2 FROM mail WHERE folder = ?1
             ON CONFLICT(name) DO UPDATE SET total = excluded.total,
                                             synced_at = excluded.synced_at",
            params![folder, crate::util::now()],
        )?;
        transaction.execute(
            "UPDATE folders SET unread = (SELECT count(*) FROM mail WHERE folder = ?1 AND seen = 0)
             WHERE name = ?1",
            params![folder],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// The messages of `folder`, newest first, without their bodies.
    pub fn list(&self, folder: &str, limit: usize) -> Result<Vec<MailSummary>> {
        let mut statement = self.connection.prepare(
            "SELECT uid, folder, subject, sender, date, seen, flagged, has_parts, preview,
                    text IS NOT NULL
             FROM mail WHERE folder = ?1
             ORDER BY date DESC, uid DESC
             LIMIT ?2",
        )?;
        let rows = statement.query_map(params![folder, limit as i64], |row| {
            Ok(MailSummary {
                uid: row.get(0)?,
                folder: row.get(1)?,
                subject: row.get(2)?,
                sender: row.get(3)?,
                date: row.get(4)?,
                seen: row.get(5)?,
                flagged: row.get(6)?,
                has_parts: row.get(7)?,
                preview: row.get(8)?,
                body_here: row.get(9)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// One message's headers, for the reader's heading.
    pub fn summary(&self, folder: &str, uid: u32) -> Result<Option<MailSummary>> {
        Ok(self
            .connection
            .query_row(
                "SELECT uid, folder, subject, sender, date, seen, flagged, has_parts, preview,
                        text IS NOT NULL
                 FROM mail WHERE folder = ?1 AND uid = ?2",
                params![folder, uid],
                |row| {
                    Ok(MailSummary {
                        uid: row.get(0)?,
                        folder: row.get(1)?,
                        subject: row.get(2)?,
                        sender: row.get(3)?,
                        date: row.get(4)?,
                        seen: row.get(5)?,
                        flagged: row.get(6)?,
                        has_parts: row.get(7)?,
                        preview: row.get(8)?,
                        body_here: row.get(9)?,
                    })
                },
            )
            .optional()?)
    }

    /// The body as it was fetched, and `None` when it never was.
    pub fn body(&self, folder: &str, uid: u32) -> Result<Option<MailBody>> {
        Ok(self
            .connection
            .query_row(
                "SELECT text, from_html, parts FROM mail WHERE folder = ?1 AND uid = ?2",
                params![folder, uid],
                |row| {
                    Ok(row.get::<_, Option<String>>(0)?.map(|text| MailBody {
                        text,
                        from_html: row.get::<_, bool>(1).unwrap_or(false),
                        parts: row.get::<_, String>(2).unwrap_or_else(|_| "[]".to_owned()),
                    }))
                },
            )
            .optional()?
            .flatten())
    }

    /// Keeps a fetched body, and marks the message read.
    pub fn save_body(&mut self, folder: &str, uid: u32, body: &MailBody) -> Result<()> {
        self.connection.execute(
            "UPDATE mail SET text = ?3, from_html = ?4, parts = ?5, seen = 1
             WHERE folder = ?1 AND uid = ?2",
            params![folder, uid, body.text, body.from_html, body.parts],
        )?;
        self.count_unread(folder)?;
        Ok(())
    }

    /// Marks a message read or unread here. The server is told separately.
    pub fn set_seen(&mut self, folder: &str, uid: u32, seen: bool) -> Result<()> {
        self.connection.execute(
            "UPDATE mail SET seen = ?3 WHERE folder = ?1 AND uid = ?2",
            params![folder, uid, seen],
        )?;
        self.count_unread(folder)?;
        Ok(())
    }

    /// Keeps the unread count of a folder in step with its rows.
    fn count_unread(&mut self, folder: &str) -> Result<()> {
        self.connection.execute(
            "UPDATE folders SET unread = (SELECT count(*) FROM mail WHERE folder = ?1 AND seen = 0)
             WHERE name = ?1",
            params![folder],
        )?;
        Ok(())
    }

    /// Every folder that has been read, with its counts.
    pub fn folders(&self) -> Result<Vec<MailFolder>> {
        let mut statement = self
            .connection
            .prepare("SELECT name, unread, total, synced_at FROM folders ORDER BY name")?;
        let rows = statement.query_map([], |row| {
            Ok(MailFolder {
                name: row.get(0)?,
                unread: row.get::<_, i64>(1)? as usize,
                total: row.get::<_, i64>(2)? as usize,
                synced_at: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// How many messages a folder holds here.
    pub fn count(&self, folder: &str) -> Result<usize> {
        let count: i64 = self
            .connection
            .query_row(
                "SELECT count(*) FROM mail WHERE folder = ?1",
                params![folder],
                |row| row.get(0),
            )
            .context("counting the mail")?;
        Ok(count as usize)
    }

    /// The newest uid of a folder, which the next sync asks the server for
    /// above.
    pub fn newest_uid(&self, folder: &str) -> Result<u32> {
        let uid: Option<i64> = self.connection.query_row(
            "SELECT max(uid) FROM mail WHERE folder = ?1",
            params![folder],
            |row| row.get(0),
        )?;
        Ok(uid.unwrap_or(0).max(0) as u32)
    }

    /// Empties a folder here, for the account that was removed from the server.
    pub fn clear(&mut self, folder: &str) -> Result<()> {
        self.connection
            .execute("DELETE FROM mail WHERE folder = ?1", params![folder])?;
        self.connection.execute(
            "UPDATE folders SET unread = 0, total = 0 WHERE name = ?1",
            params![folder],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> MailStore {
        MailStore::in_memory().expect("a store")
    }

    fn header(uid: u32, subject: &str, date: i64) -> MailHeader {
        MailHeader {
            uid,
            subject: subject.to_owned(),
            sender: "Ann <ann@example.com>".to_owned(),
            date,
            seen: false,
            preview: format!("{subject} preview"),
            ..MailHeader::default()
        }
    }

    #[test]
    fn headers_are_kept_in_date_order_and_counted() {
        let mut store = store();
        store
            .upsert("INBOX", &[header(1, "older", 100), header(2, "newer", 200)])
            .expect("writes");
        let list = store.list("INBOX", 10).expect("reads");
        assert_eq!(
            list.iter()
                .map(|row| row.subject.as_str())
                .collect::<Vec<_>>(),
            ["newer", "older"]
        );
        assert!(list.iter().all(|row| !row.body_here && !row.seen));
        let folders = store.folders().expect("reads");
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].name, "INBOX");
        assert_eq!(folders[0].total, 2);
        assert_eq!(folders[0].unread, 2);
        assert_eq!(store.newest_uid("INBOX").expect("reads"), 2);
        assert_eq!(list[0].title(), "newer");
    }

    #[test]
    fn a_second_sync_keeps_the_body_and_the_reader_s_flags() {
        let mut store = store();
        store
            .upsert("INBOX", &[header(1, "one", 100)])
            .expect("writes");
        store
            .save_body(
                "INBOX",
                1,
                &MailBody {
                    text: "the words".to_owned(),
                    from_html: true,
                    parts: "[]".to_owned(),
                },
            )
            .expect("saves");
        store.set_seen("INBOX", 1, false).expect("unreads");
        // The server still calls it unseen; the body must survive.
        store
            .upsert("INBOX", &[header(1, "one", 100)])
            .expect("writes");
        let body = store.body("INBOX", 1).expect("reads").expect("a body");
        assert_eq!(body.text, "the words");
        assert!(body.from_html);
        assert_eq!(store.folders().expect("reads")[0].unread, 1);
        assert!(store.list("INBOX", 1).expect("reads")[0].body_here);
    }

    #[test]
    fn reading_a_message_marks_it_read_and_counts_again() {
        let mut store = store();
        store
            .upsert("INBOX", &[header(1, "one", 100)])
            .expect("writes");
        assert_eq!(store.folders().expect("reads")[0].unread, 1);
        store
            .save_body(
                "INBOX",
                1,
                &MailBody {
                    text: "read now".to_owned(),
                    ..MailBody::default()
                },
            )
            .expect("saves");
        assert_eq!(store.folders().expect("reads")[0].unread, 0);
        store.set_seen("INBOX", 1, false).expect("unreads");
        assert_eq!(store.folders().expect("reads")[0].unread, 1);
    }

    #[test]
    fn a_message_that_was_never_opened_has_no_body() {
        let mut store = store();
        store
            .upsert("INBOX", &[header(7, "unread", 10)])
            .expect("writes");
        assert!(store.body("INBOX", 7).expect("reads").is_none());
        assert!(store.body("INBOX", 8).expect("reads").is_none());
        assert!(store.summary("INBOX", 7).expect("reads").is_some());
        assert!(store.summary("INBOX", 8).expect("reads").is_none());
    }

    #[test]
    fn folders_keep_their_own_counts() {
        let mut store = store();
        store
            .upsert("INBOX", &[header(1, "one", 100)])
            .expect("writes");
        store
            .upsert("Archive", &[header(1, "old", 50)])
            .expect("writes");
        let folders = store.folders().expect("reads");
        assert_eq!(folders.len(), 2);
        assert_eq!(
            folders
                .iter()
                .map(|folder| folder.name.as_str())
                .collect::<Vec<_>>(),
            ["Archive", "INBOX"]
        );
        assert_eq!(store.count("INBOX").expect("counts"), 1);
        store.clear("Archive").expect("clears");
        assert_eq!(store.count("Archive").expect("counts"), 0);
        assert_eq!(store.count("INBOX").expect("counts"), 1);
    }

    #[test]
    fn a_message_without_a_subject_still_has_a_title() {
        let summary = MailSummary::default();
        assert_eq!(summary.title(), "(no subject)");
        let mut named = MailSummary {
            subject: "  ".to_owned(),
            ..MailSummary::default()
        };
        assert_eq!(named.title(), "(no subject)");
        named.subject = "Here".to_owned();
        assert_eq!(named.title(), "Here");
    }
}
