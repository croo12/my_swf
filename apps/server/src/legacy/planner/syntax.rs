use super::TYPE_KEYWORDS;

/// A server interface is named by its request: an HTTP method, a space, and a path starting with /.
pub(super) fn is_server_request(name: &str) -> bool {
    let Some((method, path)) = name.split_once(' ') else {
        return false;
    };
    ["GET", "POST", "PUT", "PATCH", "DELETE"].contains(&method)
        && path.starts_with('/')
        && !path.contains(char::is_whitespace)
}

pub(super) fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// Whether the text can be a TypeScript type expression. This is not a parser: it accepts only
/// ASCII, requires balanced brackets, and rejects two plain words in a row (as in a phrase)
/// unless one of them is a type keyword such as `keyof`. That is enough to keep prose out.
pub(super) fn is_type_expression(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() || !text.is_ascii() {
        return false;
    }
    let mut open: Vec<char> = vec![];
    let mut previous_word: Option<String> = None;
    let mut previous_char = ' ';
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let before = std::mem::replace(&mut previous_char, c);
        match c {
            // String literal types may contain anything.
            '\'' | '"' | '`' => {
                for inner in chars.by_ref() {
                    if inner == c {
                        break;
                    }
                }
                previous_word = None;
            }
            '(' | '[' | '{' | '<' => {
                open.push(c);
                previous_word = None;
            }
            // "=>" is the arrow of a function type, not a closing bracket.
            '>' if before == '=' => previous_word = None,
            ')' | ']' | '}' | '>' => {
                let expected = match c {
                    ')' => '(',
                    ']' => '[',
                    '}' => '{',
                    _ => '<',
                };
                if open.pop() != Some(expected) {
                    return false;
                }
                previous_word = None;
            }
            c if c.is_ascii_alphanumeric() || c == '_' || c == '$' => {
                let mut word = c.to_string();
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_alphanumeric() || next == '_' || next == '$' {
                        word.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                // Two words in a row are fine only around a type keyword: "keyof T", "T extends U".
                if previous_word.as_deref().is_some_and(|before| {
                    !TYPE_KEYWORDS.contains(&before) && !TYPE_KEYWORDS.contains(&word.as_str())
                }) {
                    return false;
                }
                previous_word = Some(word);
            }
            c if c.is_whitespace() => {}
            _ => previous_word = None,
        }
    }
    open.is_empty()
}

/// `name: Type`, with an optional `?` after the name.
pub(super) fn is_typed_name(entry: &str) -> bool {
    entry.split_once(':').is_some_and(|(name, ty)| {
        is_identifier(name.trim().trim_end_matches('?')) && is_type_expression(ty)
    })
}

/// One sentence: no line breaks, and no sentence end followed by more text.
pub(super) fn is_one_sentence(text: &str) -> bool {
    let text = text.trim().trim_end_matches(['.', '。']);
    !text.contains('\n') && !text.contains(". ") && !text.contains("。")
}

pub(super) fn length(text: &str) -> usize {
    text.trim().chars().count()
}

/// Inputs or outputs with whitespace removed and in a fixed order, for comparing two signatures.
pub(super) fn normalized(entries: &[String]) -> Vec<String> {
    let mut entries: Vec<String> = entries
        .iter()
        .map(|e| e.split_whitespace().collect())
        .collect();
    entries.sort();
    entries
}

pub(super) fn is_clean_name(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@'))
}
