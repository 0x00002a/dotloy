use fs_err as fs;
use serde::{Deserialize, Serialize};
use std::{
    io,
    ops::Deref,
    path::{Path, PathBuf},
};

/// It's an absolute file path, what more could you ask for
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[serde(transparent)]
pub struct AbsPathBuf {
    path: PathBuf,
}

impl AbsPathBuf {
    pub fn new(path: impl AsRef<Path>) -> io::Result<Self> {
        let p = fs::canonicalize(path)?;
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
