use std::{
    error::Error,
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr,
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use windows_sys::Win32::Storage::FileSystem::{
    MoveFileExW, ReplaceFileW, MOVEFILE_WRITE_THROUGH, REPLACEFILE_WRITE_THROUGH,
};

const STORAGE_VERSION: u64 = 1;
static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, PartialEq)]
pub enum LoadResult<T> {
    Missing,
    Loaded(T),
    Invalid(String),
    UnsupportedVersion(u64),
}

#[derive(Debug)]
pub enum StorageError {
    Io(io::Error),
    Serialization(serde_json::Error),
    InvalidPath,
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "storage I/O failed: {error}"),
            Self::Serialization(error) => write!(formatter, "serialization failed: {error}"),
            Self::InvalidPath => formatter.write_str("storage path has no parent directory"),
        }
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Serialization(error) => Some(error),
            Self::InvalidPath => None,
        }
    }
}

impl From<io::Error> for StorageError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

#[derive(Deserialize, Serialize)]
struct Envelope<T> {
    version: u64,
    data: T,
}

pub struct JsonStorage {
    path: PathBuf,
}

impl JsonStorage {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load<T: DeserializeOwned>(&self) -> Result<LoadResult<T>, StorageError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(LoadResult::Missing);
            }
            Err(error) => return Err(error.into()),
        };
        let value: Value = match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(error) => return Ok(LoadResult::Invalid(error.to_string())),
        };
        let Some(version) = value.get("version").and_then(Value::as_u64) else {
            return Ok(LoadResult::Invalid(
                "document is missing an unsigned version".into(),
            ));
        };
        if version != STORAGE_VERSION {
            return Ok(LoadResult::UnsupportedVersion(version));
        }

        match serde_json::from_value::<Envelope<T>>(value) {
            Ok(envelope) => Ok(LoadResult::Loaded(envelope.data)),
            Err(error) => Ok(LoadResult::Invalid(error.to_string())),
        }
    }

    pub fn write<T: Serialize>(&self, data: &T) -> Result<(), StorageError> {
        let bytes = serde_json::to_vec_pretty(&Envelope {
            version: STORAGE_VERSION,
            data,
        })?;
        let parent = self.path.parent().ok_or(StorageError::InvalidPath)?;
        fs::create_dir_all(parent)?;

        let temp_path = self.temp_path(parent);
        let result = self.write_and_replace(&temp_path, &bytes);
        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        result
    }

    fn temp_path(&self, parent: &Path) -> PathBuf {
        let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("workbreak-98.json");
        parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            sequence
        ))
    }

    fn write_and_replace(&self, temp_path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
        let mut temp = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temp_path)?;
        temp.write_all(bytes)?;
        temp.sync_all()?;
        drop(temp);

        replace_path(temp_path, &self.path).map_err(StorageError::Io)
    }
}

fn replace_path(source: &Path, destination: &Path) -> io::Result<()> {
    let source = wide_path(source);
    let destination_wide = wide_path(destination);
    let succeeded = if destination.exists() {
        // SAFETY: all pointers reference NUL-terminated buffers for the duration of the call.
        unsafe {
            ReplaceFileW(
                destination_wide.as_ptr(),
                source.as_ptr(),
                ptr::null(),
                REPLACEFILE_WRITE_THROUGH,
                ptr::null(),
                ptr::null(),
            )
        }
    } else {
        // SAFETY: both pointers reference NUL-terminated buffers for the duration of the call.
        unsafe {
            MoveFileExW(
                source.as_ptr(),
                destination_wide.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        }
    };

    if succeeded == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        fs::OpenOptions,
        os::windows::fs::OpenOptionsExt,
        sync::atomic::{AtomicU64, Ordering},
    };

    use serde::{Deserialize, Serialize, Serializer};
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

    use super::{JsonStorage, LoadResult};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct Payload {
        value: String,
    }

    struct TestDirectory(std::path::PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "workbreak-98-storage-test-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn distinguishes_missing_invalid_and_unknown_version() {
        let directory = TestDirectory::new();
        let path = directory.0.join("state.json");
        let storage = JsonStorage::new(path.clone());

        assert_eq!(storage.load::<Payload>().unwrap(), LoadResult::Missing);

        fs::write(&path, b"not json").unwrap();
        assert!(matches!(
            storage.load::<Payload>().unwrap(),
            LoadResult::Invalid(_)
        ));

        fs::write(&path, br#"{"version":99,"data":{"value":"old"}}"#).unwrap();
        assert_eq!(
            storage.load::<Payload>().unwrap(),
            LoadResult::UnsupportedVersion(99)
        );
    }

    #[test]
    fn writes_then_replaces_a_valid_document() {
        let directory = TestDirectory::new();
        let path = directory.0.join("state.json");
        let storage = JsonStorage::new(path);

        storage
            .write(&Payload {
                value: "first".into(),
            })
            .unwrap();
        storage
            .write(&Payload {
                value: "second".into(),
            })
            .unwrap();

        assert_eq!(
            storage.load::<Payload>().unwrap(),
            LoadResult::Loaded(Payload {
                value: "second".into()
            })
        );
    }

    struct FailingSerialize;

    impl Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(serde::ser::Error::custom("intentional failure"))
        }
    }

    #[test]
    fn serialization_failure_preserves_the_previous_document() {
        let directory = TestDirectory::new();
        let path = directory.0.join("state.json");
        let storage = JsonStorage::new(path);
        storage
            .write(&Payload {
                value: "safe".into(),
            })
            .unwrap();

        assert!(storage.write(&FailingSerialize).is_err());
        assert_eq!(
            storage.load::<Payload>().unwrap(),
            LoadResult::Loaded(Payload {
                value: "safe".into()
            })
        );
    }

    #[test]
    fn replacement_failure_preserves_the_locked_document() {
        let directory = TestDirectory::new();
        let path = directory.0.join("state.json");
        let storage = JsonStorage::new(path.clone());
        storage
            .write(&Payload {
                value: "safe".into(),
            })
            .unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&path)
            .unwrap();

        assert!(storage
            .write(&Payload {
                value: "lost".into()
            })
            .is_err());
        drop(lock);
        assert_eq!(
            storage.load::<Payload>().unwrap(),
            LoadResult::Loaded(Payload {
                value: "safe".into()
            })
        );
    }
}
