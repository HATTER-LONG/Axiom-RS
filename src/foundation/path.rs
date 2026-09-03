//! Structured diagnostic paths into nested values.

use std::fmt;

/// One segment of a [`Path`].
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PathSegment {
    /// Object field name.
    Field(String),
    /// Zero-based list index.
    Index(usize),
}

/// Location of a nested input error.
///
/// Paths are immutable. Appending a segment returns a new path. Display uses
/// dotted fields and `[index]` list segments, for example
/// `options.mesh.faces[3].size`. Field names that are not Rust-like identifiers
/// are rendered as quoted bracket segments.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Path {
    segments: Vec<PathSegment>,
}

impl Path {
    /// Empty root path.
    #[must_use]
    pub fn root() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Borrow the segments from the root.
    #[must_use]
    pub fn segments(&self) -> &[PathSegment] {
        &self.segments
    }

    /// Return a path with `name` appended. The original path is unchanged.
    #[must_use]
    pub fn field(&self, name: impl Into<String>) -> Self {
        self.append(PathSegment::Field(name.into()))
    }

    /// Return a path with `index` appended. The original path is unchanged.
    #[must_use]
    pub fn index(&self, index: usize) -> Self {
        self.append(PathSegment::Index(index))
    }

    fn append(&self, segment: PathSegment) -> Self {
        let mut segments = self.segments.clone();
        segments.push(segment);
        Self { segments }
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_path(f, &self.segments)
    }
}

fn write_path(f: &mut fmt::Formatter<'_>, segments: &[PathSegment]) -> fmt::Result {
    let mut first = true;
    for segment in segments {
        match segment {
            PathSegment::Field(name) if ident_field(name) => {
                if !first {
                    f.write_str(".")?;
                }
                f.write_str(name)?;
            }
            PathSegment::Field(name) => write!(f, "[{}]", quote_field(name))?,
            PathSegment::Index(index) => write!(f, "[{index}]")?,
        }
        first = false;
    }
    Ok(())
}

fn ident_field(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn quote_field(name: &str) -> String {
    let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::hash::{Hash, Hasher};

    fn hash_of<T: Hash>(value: &T) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn root_displays_empty() {
        assert_eq!(Path::root().to_string(), "");
        assert!(Path::root().segments().is_empty());
    }

    #[test]
    fn mixed_path_display() {
        let path = Path::root()
            .field("options")
            .field("mesh")
            .field("faces")
            .index(3)
            .field("size");
        assert_eq!(path.to_string(), "options.mesh.faces[3].size");
    }

    #[test]
    fn special_field_names() {
        assert_eq!(Path::root().field("has.dot").to_string(), "[\"has.dot\"]");
        assert_eq!(Path::root().field("a b").to_string(), "[\"a b\"]");
        assert_eq!(Path::root().field("").to_string(), "[\"\"]");
        assert_eq!(Path::root().field("3").to_string(), "[\"3\"]");
        assert_eq!(
            Path::root().field("quote\"slash\\").to_string(),
            "[\"quote\\\"slash\\\\\"]"
        );
        let mixed = Path::root().field("options").field("has.dot").index(0);
        assert_eq!(mixed.to_string(), "options[\"has.dot\"][0]");
    }

    #[test]
    fn append_does_not_mutate() {
        let root = Path::root();
        let child = root.field("a");
        assert_eq!(root.to_string(), "");
        assert_eq!(child.to_string(), "a");
        let with_index = child.index(1);
        assert_eq!(child.to_string(), "a");
        assert_eq!(with_index.to_string(), "a[1]");
    }

    #[test]
    fn segments_preserve_kind() {
        let path = Path::root().field("a").index(2);
        assert_eq!(
            path.segments(),
            &[PathSegment::Field("a".into()), PathSegment::Index(2)]
        );
    }

    #[test]
    fn eq_and_hash() {
        let left = Path::root().field("a").index(1);
        let right = Path::root().field("a").index(1);
        assert_eq!(left, right);
        assert_eq!(hash_of(&left), hash_of(&right));
        let mut set = HashSet::new();
        set.insert(left);
        assert!(set.contains(&right));
        assert_ne!(Path::root().field("a"), Path::root().index(0));
    }
}
