//! Shared label validation for capability name and category.

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum InvalidLabel {
    Empty,
    TooLong { length: usize },
    InvalidCharacter { index: usize, found: char },
}

pub(super) fn parse_label(raw: &str, max_len: usize) -> Result<&str, InvalidLabel> {
    if raw.is_empty() {
        return Err(InvalidLabel::Empty);
    }
    if raw.len() > max_len {
        return Err(InvalidLabel::TooLong { length: raw.len() });
    }
    for (index, found) in raw.chars().enumerate() {
        if !found.is_ascii_alphanumeric() && found != '.' && found != '_' && found != '-' {
            return Err(InvalidLabel::InvalidCharacter { index, found });
        }
    }
    Ok(raw)
}
