use std::collections::VecDeque;

pub(super) struct Token<'a> {
    pub text: &'a str,
    pub class: &'static str,
}

// Conservative shell tokens preserve every byte, including unfinished edits and heredocs.
pub(super) fn highlight(source: &str) -> Vec<Token<'_>> {
    if source.len() > 128 * 1024 {
        return vec![Token {
            text: source,
            class: "",
        }];
    }
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut heredocs = VecDeque::new();
    let mut offset = 0;
    while offset < bytes.len() {
        if tokens.len() >= 8192 {
            return vec![Token {
                text: source,
                class: "",
            }];
        }
        let start = offset;
        let byte = bytes[offset];
        let class = match byte {
            b'\n' => {
                offset += 1;
                tokens.push(Token {
                    text: &source[start..offset],
                    class: "",
                });
                while let Some((delimiter, strip_tabs)) = heredocs.pop_front() {
                    let body = offset;
                    while offset < bytes.len() {
                        let end = source[offset..]
                            .find('\n')
                            .map_or(bytes.len(), |index| offset + index);
                        let line = &source[offset..end];
                        offset = (end + 1).min(bytes.len());
                        if (if strip_tabs {
                            line.trim_start_matches('\t')
                        } else {
                            line
                        }) == delimiter
                        {
                            break;
                        }
                    }
                    tokens.push(Token {
                        text: &source[body..offset],
                        class: "shell-string",
                    });
                }
                continue;
            }
            b' ' | b'\t' | b'\r' => {
                offset += 1;
                while bytes
                    .get(offset)
                    .is_some_and(|next| matches!(next, b' ' | b'\t' | b'\r'))
                {
                    offset += 1;
                }
                ""
            }
            b'#' if start == 0
                || bytes[start - 1].is_ascii_whitespace()
                || b";|&()".contains(&bytes[start - 1]) =>
            {
                offset = source[offset..]
                    .find('\n')
                    .map_or(bytes.len(), |index| offset + index);
                "shell-comment"
            }
            b'\'' | b'"' => {
                offset = quoted_end(bytes, offset, byte);
                "shell-string"
            }
            b'$' => {
                offset += 1;
                if bytes.get(offset) == Some(&b'{') {
                    offset = source[offset..]
                        .find('}')
                        .map_or(bytes.len(), |index| offset + index + 1);
                } else if bytes
                    .get(offset)
                    .is_some_and(|next| b"@*#?$!-0123456789".contains(next))
                {
                    offset += 1;
                } else {
                    while bytes
                        .get(offset)
                        .is_some_and(|next| next.is_ascii_alphanumeric() || *next == b'_')
                    {
                        offset += 1;
                    }
                }
                "shell-variable"
            }
            b'\\' => {
                offset += 1;
                if offset < bytes.len() {
                    offset += source[offset..].chars().next().unwrap().len_utf8();
                }
                ""
            }
            b';' | b'|' | b'&' | b'<' | b'>' | b'(' | b')' => {
                if byte == b'<'
                    && bytes.get(offset + 1) == Some(&b'<')
                    && bytes.get(offset + 2) == Some(&b'<')
                {
                    offset += 3;
                } else if byte == b'<' && bytes.get(offset + 1) == Some(&b'<') {
                    if let Some(delimiter) = heredoc_delimiter(source, offset + 2) {
                        heredocs.push_back(delimiter);
                    }
                    offset += 2;
                } else {
                    offset += 1;
                }
                "shell-operator"
            }
            _ => {
                offset += source[offset..].chars().next().unwrap().len_utf8();
                while offset < bytes.len()
                    && !bytes[offset].is_ascii_whitespace()
                    && !b"'\"$\\;|&<>()".contains(&bytes[offset])
                {
                    offset += source[offset..].chars().next().unwrap().len_utf8();
                }
                if matches!(
                    &source[start..offset],
                    "if" | "then"
                        | "else"
                        | "elif"
                        | "fi"
                        | "for"
                        | "while"
                        | "until"
                        | "do"
                        | "done"
                        | "case"
                        | "esac"
                        | "in"
                        | "function"
                        | "export"
                        | "return"
                        | "exit"
                ) {
                    "shell-keyword"
                } else {
                    ""
                }
            }
        };
        tokens.push(Token {
            text: &source[start..offset],
            class,
        });
    }
    tokens
}

fn quoted_end(bytes: &[u8], start: usize, quote: u8) -> usize {
    let mut offset = start + 1;
    while offset < bytes.len() {
        if bytes[offset] == quote {
            return offset + 1;
        }
        if quote == b'"' && bytes[offset] == b'\\' {
            offset += 1;
        }
        offset += 1;
    }
    bytes.len()
}

fn heredoc_delimiter(source: &str, start: usize) -> Option<(String, bool)> {
    let tail = &source[start..];
    let strip_tabs = tail.starts_with('-');
    let tail = if strip_tabs { &tail[1..] } else { tail };
    let tail = tail.trim_start_matches([' ', '\t']);
    let first = *tail.as_bytes().first()?;
    let delimiter = if matches!(first, b'\'' | b'"') {
        let end = quoted_end(tail.as_bytes(), 0, first);
        if tail.as_bytes().get(end - 1) != Some(&first) || end == 1 {
            return None;
        }
        &tail[1..end - 1]
    } else {
        let end = tail
            .find(|ch: char| ch.is_whitespace() || ";|&<>()".contains(ch))
            .unwrap_or(tail.len());
        &tail[..end]
    };
    if delimiter.is_empty() {
        None
    } else {
        Some((delimiter.into(), strip_tabs))
    }
}
