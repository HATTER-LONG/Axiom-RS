//! Validated capability name and category labels.

use std::fmt;

use super::label::{InvalidLabel, parse_label};

/// Maximum byte length of a [`CapabilityName`].
pub const CAPABILITY_NAME_MAX_LEN: usize = 128;

/// Maximum byte length of a [`CapabilityCategory`].
pub const CAPABILITY_CATEGORY_MAX_LEN: usize = 64;

/// Capability identity that cannot be mixed with an arbitrary string or
/// [`crate::CorrelationId`].
///
/// Allowed characters are ASCII letters, digits, `.`, `_`, and `-`. Values are
/// stored exactly as supplied; the constructor never trims or case-folds input.
/// Equality, hashing, and ordering use the stored bytes, so names are
/// case-sensitive.
///
/// # Errors
///
/// [`CapabilityName::parse`] fails when the input is empty, longer than
/// [`CAPABILITY_NAME_MAX_LEN`] bytes, or contains a disallowed character.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CapabilityName(String);

/// Capability category tag. Rules match [`CapabilityName`] except the maximum
/// length is [`CAPABILITY_CATEGORY_MAX_LEN`].
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CapabilityCategory(String);

/// Why [`CapabilityName`] construction failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvalidCapabilityName {
    /// The input was empty.
    Empty,
    /// The input exceeded [`CAPABILITY_NAME_MAX_LEN`].
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

/// Why [`CapabilityCategory`] construction failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvalidCapabilityCategory {
    /// The input was empty.
    Empty,
    /// The input exceeded [`CAPABILITY_CATEGORY_MAX_LEN`].
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

impl CapabilityName {
    /// Parse `raw` as a capability name.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidCapabilityName`] when `raw` violates the name rules.
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, InvalidCapabilityName> {
        match parse_label(raw.as_ref(), CAPABILITY_NAME_MAX_LEN) {
            Ok(raw) => Ok(Self(raw.to_owned())),
            Err(InvalidLabel::Empty) => Err(InvalidCapabilityName::Empty),
            Err(InvalidLabel::TooLong { length }) => Err(InvalidCapabilityName::TooLong { length }),
            Err(InvalidLabel::InvalidCharacter { index, found }) => {
                Err(InvalidCapabilityName::InvalidCharacter { index, found })
            }
        }
    }

    /// Borrow the validated name text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CapabilityCategory {
    /// Parse `raw` as a capability category.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidCapabilityCategory`] when `raw` violates the category rules.
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, InvalidCapabilityCategory> {
        match parse_label(raw.as_ref(), CAPABILITY_CATEGORY_MAX_LEN) {
            Ok(raw) => Ok(Self(raw.to_owned())),
            Err(InvalidLabel::Empty) => Err(InvalidCapabilityCategory::Empty),
            Err(InvalidLabel::TooLong { length }) => {
                Err(InvalidCapabilityCategory::TooLong { length })
            }
            Err(InvalidLabel::InvalidCharacter { index, found }) => {
                Err(InvalidCapabilityCategory::InvalidCharacter { index, found })
            }
        }
    }

    /// Borrow the validated category text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for CapabilityName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for CapabilityCategory {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for CapabilityName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for CapabilityCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for InvalidCapabilityName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("capability name must not be empty"),
            Self::TooLong { length } => {
                write!(
                    f,
                    "capability name length {length} exceeds {CAPABILITY_NAME_MAX_LEN}"
                )
            }
            Self::InvalidCharacter { index, found } => {
                write!(
                    f,
                    "invalid capability name character {found:?} at index {index}"
                )
            }
        }
    }
}

impl fmt::Display for InvalidCapabilityCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("capability category must not be empty"),
            Self::TooLong { length } => {
                write!(
                    f,
                    "capability category length {length} exceeds {CAPABILITY_CATEGORY_MAX_LEN}"
                )
            }
            Self::InvalidCharacter { index, found } => {
                write!(
                    f,
                    "invalid capability category character {found:?} at index {index}"
                )
            }
        }
    }
}

impl std::error::Error for InvalidCapabilityName {}
impl std::error::Error for InvalidCapabilityCategory {}

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
    fn name_accepts_legal_values() {
        for input in ["a", "A0._-", "math.add"] {
            let name = CapabilityName::parse(input).unwrap();
            assert_eq!(name.as_str(), input);
            assert_eq!(name.as_ref(), input);
            assert_eq!(name.to_string(), input);
        }
    }

    #[test]
    fn category_accepts_legal_values() {
        for input in ["tool", "Tool.v1", "a_b-c"] {
            let category = CapabilityCategory::parse(input).unwrap();
            assert_eq!(category.as_str(), input);
            assert_eq!(category.as_ref(), input);
            assert_eq!(category.to_string(), input);
        }
    }

    #[test]
    fn name_and_category_reject_empty() {
        assert_eq!(
            CapabilityName::parse("").unwrap_err(),
            InvalidCapabilityName::Empty
        );
        assert_eq!(
            CapabilityCategory::parse("").unwrap_err(),
            InvalidCapabilityCategory::Empty
        );
        let _: &dyn std::error::Error = &InvalidCapabilityName::Empty;
        let _: &dyn std::error::Error = &InvalidCapabilityCategory::Empty;
    }

    #[test]
    fn name_length_boundaries() {
        let max = "a".repeat(CAPABILITY_NAME_MAX_LEN);
        assert_eq!(CapabilityName::parse(&max).unwrap().as_str(), max);
        let too_long = "a".repeat(CAPABILITY_NAME_MAX_LEN + 1);
        assert_eq!(
            CapabilityName::parse(&too_long).unwrap_err(),
            InvalidCapabilityName::TooLong {
                length: CAPABILITY_NAME_MAX_LEN + 1
            }
        );
    }

    #[test]
    fn category_length_boundaries() {
        let max = "a".repeat(CAPABILITY_CATEGORY_MAX_LEN);
        assert_eq!(CapabilityCategory::parse(&max).unwrap().as_str(), max);
        let too_long = "a".repeat(CAPABILITY_CATEGORY_MAX_LEN + 1);
        assert_eq!(
            CapabilityCategory::parse(&too_long).unwrap_err(),
            InvalidCapabilityCategory::TooLong {
                length: CAPABILITY_CATEGORY_MAX_LEN + 1
            }
        );
    }

    #[test]
    fn rejects_illegal_characters() {
        let cases = [
            (" ", 0, ' '),
            ("ab c", 2, ' '),
            ("id/1", 2, '/'),
            ("!", 0, '!'),
        ];
        for (input, index, found) in cases {
            assert_eq!(
                CapabilityName::parse(input).unwrap_err(),
                InvalidCapabilityName::InvalidCharacter { index, found }
            );
            assert_eq!(
                CapabilityCategory::parse(input).unwrap_err(),
                InvalidCapabilityCategory::InvalidCharacter { index, found }
            );
        }
    }

    #[test]
    fn does_not_trim_or_fold() {
        assert!(CapabilityName::parse(" A").is_err());
        assert_ne!(
            CapabilityName::parse("Ab").unwrap(),
            CapabilityName::parse("ab").unwrap()
        );
        assert_ne!(
            CapabilityCategory::parse("Tool").unwrap(),
            CapabilityCategory::parse("tool").unwrap()
        );
    }

    #[test]
    fn name_eq_ord_hash_and_display_agree() {
        let left = CapabilityName::parse("a-1").unwrap();
        let right = CapabilityName::parse("a-1").unwrap();
        assert_eq!(left, right);
        assert_eq!(hash_of(&left), hash_of(&right));
        assert_eq!(left.cmp(&right), std::cmp::Ordering::Equal);
        assert!(CapabilityName::parse("a").unwrap() < CapabilityName::parse("b").unwrap());
        let mut set = HashSet::new();
        set.insert(left.clone());
        assert!(set.contains(&right));
        let ordered = BTreeSet::from([
            CapabilityName::parse("b").unwrap(),
            CapabilityName::parse("a").unwrap(),
        ]);
        let names: Vec<_> = ordered.iter().map(CapabilityName::as_str).collect();
        assert_eq!(names, ["a", "b"]);
    }

    #[test]
    fn construction_error_display_is_structured() {
        assert!(!InvalidCapabilityName::Empty.to_string().is_empty());
        assert!(
            InvalidCapabilityName::TooLong { length: 129 }
                .to_string()
                .contains("exceeds")
        );
        assert!(
            InvalidCapabilityName::InvalidCharacter {
                index: 0,
                found: '/'
            }
            .to_string()
            .contains("invalid capability name")
        );
        assert!(!InvalidCapabilityCategory::Empty.to_string().is_empty());
        assert!(
            InvalidCapabilityCategory::TooLong { length: 65 }
                .to_string()
                .contains("exceeds")
        );
        assert!(
            InvalidCapabilityCategory::InvalidCharacter {
                index: 1,
                found: ' '
            }
            .to_string()
            .contains("invalid capability category")
        );
    }
}
