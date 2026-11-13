//! A mail account as the reader configures it: where it lives, how it is
//! reached, and the password the OS keyring keeps for it.
//!
//! The settings hold no secret. A password goes to the keyring under an
//! identity built from the account's own id, so two accounts on the same
//! server keep their own, and removing an account removes its entry.

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// How the client reaches the account's servers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MailSecurity {
    /// IMAP over TLS from the first byte, the port 993 default.
    #[default]
    Tls,
    /// Plain connection upgraded with STARTTLS, the port 143 default, and the
    /// port 587 default for SMTP.
    StartTls,
    /// No encryption at all, for a server on this machine. Nothing else.
    None,
}

impl MailSecurity {
    /// The port the world uses for this kind of connection.
    pub fn imap_port(self) -> u16 {
        match self {
            Self::Tls => 993,
            Self::StartTls | Self::None => 143,
        }
    }

    /// The port the world uses for submission, which is a different default
    /// from IMAP's.
    pub fn smtp_port(self) -> u16 {
        match self {
            Self::Tls => 465,
            Self::StartTls | Self::None => 587,
        }
    }

    /// The choices a settings row offers, in the order it draws them.
    pub fn all() -> [Self; 3] {
        [Self::Tls, Self::StartTls, Self::None]
    }
}

/// What names an account: the address the reader gave it. Two accounts for the
/// same address on different servers are told apart by their host.
pub type MailAccountId = String;

/// One account, as the settings file stores it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailAccount {
    /// The address, which is also what the list says the account is.
    pub address: String,
    /// Where the mail is read from, `imap.example.com` or an IP.
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_security: MailSecurity,
    /// Where mail is sent through. Empty means the IMAP host.
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_security: MailSecurity,
    /// The name the server knows, when it differs from the address.
    pub user: String,
    /// What the folders of the list are: the reader's own names for them.
    pub inbox: String,
    /// A friendly name, shown beside the address.
    pub display_name: String,
}

impl MailAccount {
    /// A new account for `address`, filled with the defaults that address's
    /// provider uses, so the reader types a password and little else.
    pub fn for_address(address: &str) -> Self {
        let (host, security) = suggested_host(address);
        Self {
            address: address.trim().to_owned(),
            imap_host: host.clone(),
            imap_port: security.imap_port(),
            imap_security: security,
            smtp_host: host,
            smtp_port: security.smtp_port(),
            smtp_security: security,
            user: address.trim().to_owned(),
            inbox: "INBOX".to_owned(),
            display_name: String::new(),
        }
    }

    /// What the settings list and the mail tab call it.
    pub fn label(&self) -> String {
        if self.display_name.trim().is_empty() {
            self.address.clone()
        } else {
            format!("{} ({})", self.display_name.trim(), self.address)
        }
    }

    /// The name the server knows, the address when it was not given.
    pub fn login(&self) -> &str {
        if self.user.trim().is_empty() {
            &self.address
        } else {
            &self.user
        }
    }

    /// Where mail is sent through: the SMTP host, or the IMAP one when the
    /// reader left it empty.
    pub fn submission_host(&self) -> &str {
        if self.smtp_host.trim().is_empty() {
            &self.imap_host
        } else {
            &self.smtp_host
        }
    }

    /// The keyring identity of this account's password. It carries a digest of
    /// the address and host, never the address itself: a keyring entry is
    /// readable by anything that can ask the keyring.
    pub fn secret_identity(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut digest = Sha256::new();
        digest.update(self.address.trim().to_lowercase().as_bytes());
        digest.update(b"\\0");
        digest.update(self.imap_host.trim().to_lowercase().as_bytes());
        let bytes = digest.finalize();
        let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        format!("mail-{hex}")
    }

    /// Everything but the secret must be there before the account is used.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.address.trim().is_empty(),
            "The account needs an address"
        );
        ensure!(
            self.address.contains('@') && !self.address.contains(' '),
            "That address does not look like an address"
        );
        ensure!(
            !self.imap_host.trim().is_empty(),
            "The account needs the server it reads mail from"
        );
        ensure!(
            self.imap_port > 0 && self.smtp_port > 0,
            "A port is a number between 1 and 65535"
        );
        ensure!(
            !self.inbox.trim().is_empty(),
            "The account needs the folder it reads"
        );
        Ok(())
    }

    /// The password, as the keyring keeps it. A missing entry is `None`, which
    /// the settings ask for again.
    pub fn password(&self) -> Result<Option<String>> {
        let secret = crate::archive::stored_secret(&self.secret_identity())
            .context("The password could not be read from the OS keyring")?;
        Ok(secret.map(|bytes| String::from_utf8_lossy(bytes.as_slice()).into_owned()))
    }

    /// Keeps a password in the keyring, replacing the old one.
    pub fn set_password(&self, password: &str) -> Result<()> {
        ensure!(!password.is_empty(), "The password is empty");
        crate::archive::save_secret(&self.secret_identity(), password.as_bytes())
            .context("The password could not be saved in the OS keyring")
    }

    /// Forgets the password, when the account goes or its server changes.
    pub fn forget_password(&self) -> Result<()> {
        crate::archive::forget_secret(&self.secret_identity())
            .context("The password could not be removed from the OS keyring")
    }
}

/// The server an address's provider uses, and how it is reached, for the
/// providers most people have. Anything else starts blank, to be typed.
fn suggested_host(address: &str) -> (String, MailSecurity) {
    let domain = address
        .rsplit_once('@')
        .map(|(_, domain)| domain.trim().to_lowercase())
        .unwrap_or_default();
    match domain.as_str() {
        "gmail.com" | "googlemail.com" => ("imap.gmail.com".to_owned(), MailSecurity::Tls),
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" => {
            ("outlook.office365.com".to_owned(), MailSecurity::Tls)
        }
        "yahoo.com" | "ymail.com" => ("imap.mail.yahoo.com".to_owned(), MailSecurity::Tls),
        "icloud.com" | "me.com" | "mac.com" => ("imap.mail.me.com".to_owned(), MailSecurity::Tls),
        _ => (String::new(), MailSecurity::Tls),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_account_fills_in_what_its_provider_uses() {
        let gmail = MailAccount::for_address("ada@gmail.com");
        assert_eq!(gmail.imap_host, "imap.gmail.com");
        assert_eq!(gmail.imap_port, 993);
        assert_eq!(gmail.smtp_port, 465);
        assert_eq!(gmail.login(), "ada@gmail.com");
        assert_eq!(gmail.label(), "ada@gmail.com");
        let own = MailAccount::for_address("ada@example.com");
        assert_eq!(own.imap_host, "", "an unknown provider starts blank");
        assert_eq!(
            MailAccount::for_address("ada@icloud.com").imap_host,
            "imap.mail.me.com"
        );
    }

    #[test]
    fn the_ports_follow_the_security_the_reader_chose() {
        assert_eq!(MailSecurity::Tls.imap_port(), 993);
        assert_eq!(MailSecurity::StartTls.imap_port(), 143);
        assert_eq!(MailSecurity::Tls.smtp_port(), 465);
        assert_eq!(MailSecurity::StartTls.smtp_port(), 587);
        assert_eq!(MailSecurity::None.smtp_port(), 587);
    }

    #[test]
    fn an_account_says_what_is_missing_before_it_is_used() {
        let mut account = MailAccount::for_address("ada@example.com");
        assert!(
            account.validate().is_err(),
            "no server, so no account to read from"
        );
        account.imap_host = "imap.example.com".to_owned();
        account.validate().expect("a server is enough");
        account.address = "not an address".to_owned();
        assert!(account.validate().is_err());
        account.address = "ada@example.com".to_owned();
        account.imap_port = 0;
        assert!(account.validate().is_err());
    }

    #[test]
    fn submission_falls_back_to_the_reading_server() {
        let mut account = MailAccount::for_address("ada@example.com");
        account.imap_host = "imap.example.com".to_owned();
        assert_eq!(account.submission_host(), "imap.example.com");
        account.smtp_host = "smtp.example.com".to_owned();
        assert_eq!(account.submission_host(), "smtp.example.com");
    }

    #[test]
    fn the_keyring_identity_carries_no_address() {
        let mut account = MailAccount::for_address("ada@example.com");
        account.imap_host = "imap.example.com".to_owned();
        let identity = account.secret_identity();
        assert!(identity.starts_with("mail-"));
        assert!(!identity.contains("ada"));
        assert!(!identity.contains("example"));
        // Two accounts on one server keep their own entry, and one address on
        // two servers keeps its own too.
        let mut other = account.clone();
        other.imap_host = "other.example.com".to_owned();
        assert_ne!(identity, other.secret_identity());
        let mut second = account.clone();
        second.address = "grace@example.com".to_owned();
        assert_ne!(identity, second.secret_identity());
    }
}
