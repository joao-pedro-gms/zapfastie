//! One mail, parsed: what it says, who wrote it, and what came with it.
//!
//! The reader shows text. A message that carries a plain part is read from
//! it; one that only carries HTML is boiled down to its words by [`html_to_text`],
//! which keeps the shape of the document, its paragraphs and its lines, and
//! none of its styling, scripts or remote pictures. That is the promise the
//! guide makes: this client draws the words, and a message that needs a layout
//! opens in the browser.

use mail_parser::MessageParser;

/// A file that came with the message, left on the server until asked for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MailPart {
    /// The file's own name, which senders are free to lie about.
    pub name: String,
    pub mime: String,
    /// How many bytes it holds once decoded.
    pub size: usize,
    /// The part's index in the message, for fetching one attachment alone.
    pub index: usize,
}

/// A message, read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedMail {
    pub subject: String,
    /// Who it says it is from, one entry, `Name <address>`.
    pub from: String,
    /// Everyone it is addressed to, `Name <address>` and comma separated.
    pub to: String,
    /// The `Date` header in seconds since the epoch, zero when it had none.
    pub date: i64,
    /// What the reader sees.
    pub text: String,
    /// The HTML part as it arrived, empty when the sender wrote none. The
    /// reader opens it in the browser when the message needs more layout than
    /// this client draws.
    pub html: String,
    /// Whether [`Self::text`] came from an HTML part, which the reader offers
    /// to open in the browser.
    pub from_html: bool,
    pub parts: Vec<MailPart>,
    /// `Message-ID`, which threads the message with its replies.
    pub message_id: String,
    /// The `In-Reply-To` and `References` chain, oldest first.
    pub references: Vec<String>,
}

/// Reads the bytes of a message. `None` when they are not a message at all.
pub fn parse(raw: &[u8]) -> Option<ParsedMail> {
    let message = MessageParser::new().parse(raw)?;
    // The plain part when the sender wrote one, the HTML boiled down when they
    // did not, and the same answer from both sides about which it was. The
    // parts are asked, not the joined bodies: a message that carries only HTML
    // has an HTML part in the text list too, and its source is not words.
    let plain = message
        .text_body()
        .filter(|part| part.content_type().is_none_or(|content| !is_html(&content)))
        .find_map(|part| part.text())
        .filter(|text| !text.trim().is_empty());
    let html = message
        .html_body()
        .find_map(|part| part.text())
        .map(|html| html.into_owned())
        .unwrap_or_default();
    let (text, from_html) = match plain {
        Some(text) => (text.into_owned(), false),
        None if !html.is_empty() => (html_to_text(&html), true),
        None => (String::new(), false),
    };
    let from = message
        .from()
        .and_then(|list| list.mailboxes().next())
        .map(address_of)
        .unwrap_or_default();
    let to = message
        .to()
        .map(|list| {
            list.mailboxes()
                .map(address_of)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let parts = message
        .attachments()
        .enumerate()
        .map(|(index, part)| MailPart {
            name: part.attachment_name().unwrap_or_default().to_owned(),
            mime: part
                .content_type()
                .map(|content| match content.subtype() {
                    Some(subtype) => format!("{}/{subtype}", content.ctype()),
                    None => content.ctype().to_owned(),
                })
                .unwrap_or_else(|| "application/octet-stream".to_owned()),
            size: part.decoded_len(),
            index,
        })
        .collect();
    let references = message
        .references()
        .map(|list| list.iter().map(str::to_owned).collect())
        .unwrap_or_default();
    Some(ParsedMail {
        subject: message.subject().unwrap_or_default().to_owned(),
        from,
        to,
        date: message.date().map(|date| date.to_timestamp()).unwrap_or(0),
        text: collapse_blank_lines(&text),
        html,
        from_html,
        parts,
        message_id: message.message_id().unwrap_or_default().to_owned(),
        references,
    })
}

/// Whether a part is the HTML half of a message.
fn is_html(content: &mail_parser::ContentType<'_>) -> bool {
    content.ctype().eq_ignore_ascii_case("text")
        && content
            .subtype()
            .is_some_and(|subtype| subtype.eq_ignore_ascii_case("html"))
}

/// One address as `Name <address>`, or the address alone when there is no
/// name. The display name is quoted when it holds punctuation that would
/// otherwise read as part of the address.
fn address_of(address: mail_parser::Mailbox<'_>) -> String {
    let email = address.address().unwrap_or_default();
    match address.name() {
        Some(name) if !name.trim().is_empty() => {
            let name = name.trim();
            if name.contains([',', '<', '>', '@', '"']) {
                format!("\"{}\" <{email}>", name.replace('"', "'"))
            } else {
                format!("{name} <{email}>")
            }
        }
        _ => email.to_owned(),
    }
}

/// Turns an HTML part into the words it holds.
///
/// Blocks become lines, `<br>` a line, list items and table rows a line each,
/// and the links keep their text. Nothing is styled, nothing is fetched, and
/// `<script>`, `<style>` and `<head>` are dropped whole: what is left is what a
/// reader can read.
pub fn html_to_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len() / 2);
    let bytes = html.as_bytes();
    let mut index = 0;
    let mut skip_until: Option<&str> = None;
    while index < bytes.len() {
        if let Some(tag) = skip_until {
            match find_ascii_case_insensitive(html, index, tag) {
                Some(end) => {
                    // Past the closing tag's own '>', which is not text.
                    index = end + tag.len();
                    if let Some(close) = html[index..].find('>') {
                        index += close + 1;
                    }
                    skip_until = None;
                    continue;
                }
                None => break,
            }
        }
        if bytes[index] == b'<' {
            // A comment is not markup the reader wants.
            if html[index..].starts_with("<!--") {
                match html[index..].find("-->") {
                    Some(end) => {
                        index += end + 3;
                        continue;
                    }
                    None => break,
                }
            }
            let Some(close) = html[index..].find('>') else {
                // A lone '<' with nothing closing it is the character itself,
                // as in "2 < 3".
                text.push('<');
                index += 1;
                continue;
            };
            let raw = &html[index + 1..index + close];
            let closing = raw.trim_start().starts_with('/');
            let name = tag_name(raw);
            match name.as_str() {
                "br" => text.push('\n'),
                "p" | "div" | "tr" | "li" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                | "blockquote" | "table" | "ul" | "ol" | "hr" | "pre" => {
                    if !text.ends_with('\n') && !text.is_empty() {
                        text.push('\n');
                    }
                    if name == "li" && !closing {
                        text.push_str("- ");
                    }
                }
                "td" | "th" => {
                    if !text.ends_with([' ', '\n']) && !text.is_empty() {
                        text.push(' ');
                    }
                }
                "script" => skip_until = Some("</script"),
                "style" => skip_until = Some("</style"),
                "head" => skip_until = Some("</head"),
                _ => {}
            }
            index += close + 1;
            continue;
        }
        if bytes[index] == b'&'
            && let Some((decoded, len)) = entity_at(&html[index..])
        {
            text.push_str(&decoded);
            index += len;
            continue;
        }
        let ch = html[index..].chars().next().expect("a character");
        text.push(ch);
        index += ch.len_utf8();
    }
    squeeze_spaces(&collapse_blank_lines(&text))
}

/// HTML lays out its words whatever whitespace the source holds, so a run of
/// spaces in a line is one space here, as a browser would draw it.
fn squeeze_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let mut previous = false;
        for ch in line.trim().chars() {
            let space = ch == ' ' || ch == '\t';
            if space && previous {
                continue;
            }
            out.push(if space { ' ' } else { ch });
            previous = space;
        }
        out.push('\n');
    }
    out
}

/// The element name of an opening or closing tag, lowercased.
fn tag_name(tag: &str) -> String {
    let tag = tag.trim_start_matches('/').trim_start();
    tag.split(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .next()
        .unwrap_or_default()
        .to_lowercase()
}

fn find_ascii_case_insensitive(haystack: &str, from: usize, needle: &str) -> Option<usize> {
    let haystack = &haystack[from..];
    let lower = haystack.to_lowercase();
    lower.find(needle).map(|at| at + from)
}

/// One HTML entity at the start of `text`, and how many bytes it took.
fn entity_at(text: &str) -> Option<(String, usize)> {
    let end = text.find(';')?;
    if end > 12 {
        return None;
    }
    let name = &text[1..end];
    let decoded = match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" | "#39" => "'",
        "nbsp" => " ",
        "mdash" => "-",
        "ndash" => "-",
        "hellip" => "...",
        "rsquo" | "#8217" => "'",
        "lsquo" | "#8216" => "'",
        "ldquo" | "#8220" => "\"",
        "rdquo" | "#8221" => "\"",
        // The money, signs and accented letters a receipt or a letter from
        // Portugal, Spain, France or Germany carries.
        "pound" => "£",
        "euro" => "€",
        "cent" => "¢",
        "yen" => "¥",
        "copy" => "©",
        "reg" => "®",
        "trade" => "™",
        "deg" => "°",
        "plusmn" => "±",
        "times" => "×",
        "divide" => "÷",
        "frac12" => "½",
        "sect" => "§",
        "para" => "¶",
        "bull" => "-",
        "middot" => "·",
        "laquo" => "«",
        "raquo" => "»",
        "aacute" => "á",
        "agrave" => "à",
        "acirc" => "â",
        "atilde" => "ã",
        "ccedil" => "ç",
        "eacute" => "é",
        "egrave" => "è",
        "ecirc" => "ê",
        "iacute" => "í",
        "oacute" => "ó",
        "ocirc" => "ô",
        "otilde" => "õ",
        "uacute" => "ú",
        "uuml" => "ü",
        "auml" => "ä",
        "ouml" => "ö",
        "szlig" => "ß",
        "ntilde" => "ñ",
        "Aacute" => "Á",
        "Atilde" => "Ã",
        "Ccedil" => "Ç",
        "Eacute" => "É",
        "Otilde" => "Õ",
        _ => {
            // Numeric entities beyond the few above: decimal or hexadecimal.
            let number = name.strip_prefix('#')?;
            let code = if let Some(hex) = number.strip_prefix(['x', 'X']) {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                number.parse::<u32>().ok()?
            };
            return char::from_u32(code).map(|ch| (ch.to_string(), end + 1));
        }
    };
    Some((decoded.to_owned(), end + 1))
}

/// Trims every line, drops the empty ones at both ends, and keeps at most one
/// empty line between paragraphs.
fn collapse_blank_lines(text: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    let mut blank = false;
    for line in text.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            if lines.is_empty() || blank {
                continue;
            }
            blank = true;
            lines.push("");
        } else {
            blank = false;
            lines.push(line);
        }
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    let mut out = lines.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_message_gives_its_words_and_headers() {
        let raw = b"From: Ann Lee <ann@example.com>\r\n\
                    To: Ada <ada@example.com>, Bob <bob@example.com>\r\n\
                    Subject: The lease\r\n\
                    Date: Fri, 13 Nov 2026 10:00:00 +0000\r\n\
                    Message-ID: <one@example.com>\r\n\
                    \r\n\
                    It is signed.\r\nSee you Thursday.\r\n";
        let mail = parse(raw).expect("a message");
        assert_eq!(mail.subject, "The lease");
        assert_eq!(mail.from, "Ann Lee <ann@example.com>");
        assert_eq!(mail.to, "Ada <ada@example.com>, Bob <bob@example.com>");
        assert_eq!(mail.text, "It is signed.\nSee you Thursday.\n");
        assert!(!mail.from_html);
        assert_eq!(mail.message_id, "one@example.com");
        assert!(mail.date > 1_700_000_000);
    }

    #[test]
    fn a_message_without_a_plain_part_is_read_from_its_html() {
        let raw = b"From: ann@example.com\r\nSubject: Sale\r\n\
                    Content-Type: text/html; charset=utf-8\r\n\r\n\
                    <html><head><style>p{color:red}</style></head>\
                    <body><p>Hello <b>Ada</b>,</p><p>the price is &pound;5 &amp; up.</p>\
                    <ul><li>one</li><li>two</li></ul></body></html>";
        let mail = parse(raw).expect("a message");
        assert!(mail.from_html, "the words came from HTML");
        assert!(
            mail.html.contains("<b>Ada</b>"),
            "the source is kept for the browser: {:?}",
            mail.html
        );
        assert!(mail.text.contains("Hello Ada,"), "{:?}", mail.text);
        assert!(
            mail.text.contains("the price is £5 & up."),
            "{:?}",
            mail.text
        );
        assert!(mail.text.contains("- one"), "{:?}", mail.text);
        assert!(
            !mail.text.contains("color:red"),
            "a style is not words: {:?}",
            mail.text
        );
    }

    #[test]
    fn a_plain_part_wins_over_the_html_one() {
        let raw = b"From: ann@example.com\r\nSubject: Both\r\n\
                    Content-Type: multipart/alternative; boundary=b\r\n\r\n\
                    --b\r\nContent-Type: text/plain\r\n\r\nplain words\r\n\
                    --b\r\nContent-Type: text/html\r\n\r\n<p>html words</p>\r\n--b--\r\n";
        let mail = parse(raw).expect("a message");
        assert_eq!(mail.text, "plain words\n");
        assert!(!mail.from_html);
    }

    #[test]
    fn an_attachment_is_named_sized_and_counted() {
        let raw = b"From: ann@example.com\r\nSubject: Files\r\n\
                    Content-Type: multipart/mixed; boundary=b\r\n\r\n\
                    --b\r\nContent-Type: text/plain\r\n\r\nthe invoice\r\n\
                    --b\r\nContent-Type: application/pdf; name=invoice.pdf\r\n\
                    Content-Disposition: attachment; filename=invoice.pdf\r\n\
                    Content-Transfer-Encoding: base64\r\n\r\n\
                    aGVsbG8=\r\n--b--\r\n";
        let mail = parse(raw).expect("a message");
        assert_eq!(mail.parts.len(), 1);
        assert_eq!(mail.parts[0].name, "invoice.pdf");
        assert_eq!(mail.parts[0].size, 5, "the decoded bytes, not the base64");
        assert!(mail.parts[0].mime.contains("pdf"), "{:?}", mail.parts[0]);
        assert_eq!(mail.text, "the invoice\n");
    }

    #[test]
    fn html_becomes_lines_without_its_markup() {
        let html = "<div>One<br>Two</div><div>Three</div>";
        assert_eq!(html_to_text(html), "One\nTwo\nThree\n");
        let html = "<p>a&nbsp;&nbsp;b</p><script>var x = 1 < 2;</script><p>c</p>";
        assert_eq!(html_to_text(html), "a b\nc\n");
        assert_eq!(html_to_text("<p>&#65;&#x42;</p>"), "AB\n");
        // A stray '<' that opens no tag is kept as the character it is.
        assert_eq!(html_to_text("2 < 3"), "2 < 3\n");
        assert_eq!(html_to_text(""), "");
    }

    #[test]
    fn blank_lines_are_collapsed_and_the_ends_trimmed() {
        assert_eq!(collapse_blank_lines("\n\na\n\n\n\nb\n\n"), "a\n\nb\n");
        assert_eq!(collapse_blank_lines("   \n\t\n"), "");
    }

    #[test]
    fn a_message_that_is_not_one_is_refused() {
        assert!(parse(b"").is_none());
        assert!(parse(&[0xff, 0xfe, 0x00]).is_none());
    }
}
