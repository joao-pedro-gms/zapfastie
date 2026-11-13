//! Mail: the accounts the reader configures, read over IMAP and written over
//! SMTP, drawn by this client's own reader.
//!
//! Nothing here talks to a server of ours, because there is none. Each account
//! names the host the reader already has, keeps its password in the OS keyring,
//! and its messages in an encrypted database of its own beside the chats. A
//! message is shown as text: the plain part when the sender wrote one, and the
//! HTML part boiled down to its words otherwise, with the sender's pictures
//! left unloaded until asked for and "open in browser" for what needs a layout
//! this reader does not draw.

pub mod account;
pub mod message;
pub mod store;

pub use account::{MailAccount, MailAccountId, MailSecurity};
pub use message::{MailPart, ParsedMail};
pub use store::{MailFolder, MailStore, MailSummary};
