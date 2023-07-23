use fs_err as fs;
use serde::{Deserialize, Serialize};
use std::{
    io,
    ops::Deref,
    path::{Path, PathBuf},
};

fn remove_midcomps(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match &comp {
            std::path::Component::ParentDir => {
                if !out.pop() {
                    out.push(comp);
                }
            }
            _ => {
                out.push(comp);
            }
        }
    }
    out
}

/// It's an absolute file path, what more could you ask for
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[serde(transparent)]
pub struct AbsPathBuf {
    path: PathBuf,
}

impl AbsPathBuf {
    pub fn new(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let p = if !path.exists() {
            remove_midcomps(&std::env::current_dir()?.join(path))
        } else {
            fs::canonicalize(path)?
        };
        Ok(Self { path: p })
    }
}
impl Deref for AbsPathBuf {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.path.deref()
    }
}
impl AsRef<Path> for AbsPathBuf {
    fn as_ref(&self) -> &Path {
        self.deref()
    }
}
macro_rules! impl_try_from {
    ($($ts:ty),+) => {
        $(
        impl TryFrom<$ts> for AbsPathBuf {
            type Error = std::io::Error;

            fn try_from(value: $ts) -> Result<Self, Self::Error> {
                AbsPathBuf::new(value)
            }
        })+
    };
}

impl_try_from!(&str, &Path, PathBuf, String);

#[cfg(test)]
mod tests {
    use super::AbsPathBuf;
    use assert_matches::assert_matches;

    #[test]
    fn abspath_can_handle_non_existant_paths() {
        let p = AbsPathBuf::new("I do not exist");
        assert_matches!(p, Ok(_));
    }
    #[test]
    fn abspath_normalises_paths() {
        assert_eq!(
            AbsPathBuf::new("././.").unwrap(),
            AbsPathBuf::new(".").unwrap()
        );
    }
}
