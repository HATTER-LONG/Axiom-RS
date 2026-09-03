//! Validated correlation identifiers.

use std::fmt;
use std::hash::Hash;

/// Maximum byte length of a [`CorrelationId`].
pub const MAX_LEN: usize = 128;

/// Stable identifier that cannot be mixed with an arbitrary string.
///
/// Allowed characters are ASCII letters, digits, `.`, `_`, and `-`. Values are
/// stored exactly as supplied; the constructor never trims or case-folds input.
///
/// # Errors
///
/// [`CorrelationId::parse`] fails when the input is empty, longer than
/// [`crate::MAX_LEN`] bytes, or contains a disallowed character.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CorrelationId(String);

/// Why identifier construction failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvalidIdentifier {
    /// The input was empty.
    Empty,
    /// The input exceeded [`crate::MAX_LEN`].
    TooLong {
        /// Observed length in bytes.
        length: usize,
    },
    /// A character outside the allowed set was found.
    InvalidCharacter {
        /// UTF-8 character index from the start of the input.
        index: usize,
        /// Rejected character.
        found: char,
    },
}

impl CorrelationId {
    /// Parse `raw` as a correlation identifier.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentifier`] when `raw` violates the identifier rules.
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, InvalidIdentifier> {
        let raw = raw.as_ref();
        if raw.is_empty() {
            return Err(InvalidIdentifier::Empty);
        }
        if raw.len() > MAX_LEN {
            return Err(InvalidIdentifier::TooLong { length: raw.len() });
        }
        for (index, found) in raw.chars().enumerate() {
            if !is_allowed(found) {
                return Err(InvalidIdentifier::InvalidCharacter { index, found });
            }
        }
        Ok(Self(raw.to_owned()))
    }

    /// Borrow the validated identifier text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_allowed(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-')
}

impl AsRef<str> for CorrelationId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for InvalidIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("identifier must not be empty"),
            Self::TooLong { length } => {
                write!(f, "identifier length {length} exceeds {MAX_LEN}")
            }
            Self::InvalidCharacter { index, found } => {
                write!(f, "invalid identifier character {found:?} at index {index}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeSet, HashSet};
    use std::hash::{Hash, Hasher};

    fn hash_of<T: Hash>(value: &T) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn accepts_legal_identifiers() {
        for input in ["a", "A0._-", "request-1.child_2"] {
            let id = CorrelationId::parse(input).unwrap();
            assert_eq!(id.as_str(), input);
            assert_eq!(id.to_string(), input);
        }
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(
            CorrelationId::parse("").unwrap_err(),
            InvalidIdentifier::Empty
        );
    }

    #[test]
    fn rejects_too_long() {
        let input = "a".repeat(MAX_LEN + 1);
        assert_eq!(
            CorrelationId::parse(&input).unwrap_err(),
            InvalidIdentifier::TooLong {
                length: MAX_LEN + 1
            }
        );
    }

    #[test]
    fn accepts_max_length() {
        let input = "a".repeat(MAX_LEN);
        assert_eq!(CorrelationId::parse(&input).unwrap().as_str(), input);
    }

    #[test]
    fn rejects_illegal_characters() {
        let cases = [
            (" ", 0, ' '),
            ("ab c", 2, ' '),
            ("id/1", 2, '/'),
            ("ä", 0, 'ä'),
        ];
        for (input, index, found) in cases {
            assert_eq!(
                CorrelationId::parse(input).unwrap_err(),
                InvalidIdentifier::InvalidCharacter { index, found }
            );
        }
    }

    #[test]
    fn does_not_trim_or_fold() {
        assert!(CorrelationId::parse(" A").is_err());
        let id = CorrelationId::parse("Ab").unwrap();
        assert_ne!(id, CorrelationId::parse("ab").unwrap());
    }

    #[test]
    fn eq_ord_hash_and_display_agree() {
        let left = CorrelationId::parse("a-1").unwrap();
        let right = CorrelationId::parse("a-1").unwrap();
        assert_eq!(left, right);
        assert_eq!(hash_of(&left), hash_of(&right));
        assert_eq!(left.cmp(&right), std::cmp::Ordering::Equal);
        assert!(CorrelationId::parse("a").unwrap() < CorrelationId::parse("b").unwrap());
        let mut set = HashSet::new();
        set.insert(left.clone());
        assert!(set.contains(&right));
        let ordered = BTreeSet::from([
            CorrelationId::parse("b").unwrap(),
            CorrelationId::parse("a").unwrap(),
        ]);
        let names: Vec<_> = ordered.iter().map(CorrelationId::as_str).collect();
        assert_eq!(names, ["a", "b"]);
    }
}
