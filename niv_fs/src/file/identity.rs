//! File identity tracking for detecting renames and moves.
//!
//! Uses file system metadata to create stable identities that persist
//! across file renames and moves within the same volume.

use std::path::Path;
use std::time::SystemTime;

/// File identity configuration
#[derive(Debug, Clone)]
pub struct FileIdentityConfig {
    /// Whether to use fast rolling hash for content sampling
    pub use_fast_hash: bool,
    /// Size of content sample for hashing (in bytes)
    pub hash_sample_size: usize,
}

impl Default for FileIdentityConfig {
    fn default() -> Self {
        FileIdentityConfig {
            use_fast_hash: true,
            hash_sample_size: 8192, // 8KB sample
        }
    }
}

/// Stable file identity that persists across renames and moves.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileIdentity {
    /// Device ID (Unix) or Volume ID (Windows)
    pub device_id: u64,
    /// Inode number (Unix) or File ID (Windows)
    pub inode: u64,
    /// File size in bytes
    pub size: u64,
    /// Last modification time
    pub mtime: SystemTime,
    /// Fast rolling hash of file content (optional)
    pub content_hash: Option<u64>,
}

impl FileIdentity {
    /// Create a new file identity from a path.
    pub fn from_path<P: AsRef<Path>>(
        path: P,
        config: &FileIdentityConfig,
    ) -> std::io::Result<Self> {
        Self::from_path_with_hash(path, config, config.use_fast_hash)
    }

    /// Create identity with explicit hash control.
    pub fn from_path_with_hash<P: AsRef<Path>>(
        path: P,
        config: &FileIdentityConfig,
        compute_hash: bool,
    ) -> std::io::Result<Self> {
        let metadata = std::fs::metadata(&path)?;
        let mtime = metadata.modified()?;

        let content_hash = if compute_hash {
            Self::compute_fast_hash(&path, config.hash_sample_size)?
        } else {
            None
        };

        // Get device and inode information
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(FileIdentity {
                device_id: metadata.dev(),
                inode: metadata.ino(),
                size: metadata.size(),
                mtime,
                content_hash,
            })
        }

        #[cfg(windows)]
        {
            // Windows implementation would need winapi or similar
            // For now, use placeholder values
            Ok(FileIdentity {
                device_id: 0, // Would need winapi to get volume ID
                inode: 0,     // Would need winapi to get file ID
                size: metadata.len(),
                mtime,
                content_hash,
            })
        }

        #[cfg(not(any(unix, windows)))]
        {
            // Fallback for other platforms
            Ok(FileIdentity {
                device_id: 0,
                inode: 0,
                size: metadata.len(),
                mtime,
                content_hash,
            })
        }
    }

    /// Check if two identities represent the same file (allowing for content changes).
    pub fn is_same_file(&self, other: &FileIdentity) -> bool {
        self.device_id == other.device_id && self.inode == other.inode
    }

    /// Check if the file has been modified since this identity was created.
    pub fn is_modified(&self, current: &FileIdentity) -> bool {
        !self.is_same_file(current) || self.mtime != current.mtime || self.size != current.size
    }

    /// Check if content has changed (requires hash to be computed).
    pub fn content_changed(&self, current: &FileIdentity) -> Option<bool> {
        match (&self.content_hash, &current.content_hash) {
            (Some(a), Some(b)) => Some(a != b),
            _ => None, // Cannot determine without hash
        }
    }

    /// Compute a fast rolling hash of the file content.
    fn compute_fast_hash<P: AsRef<Path>>(
        path: P,
        sample_size: usize,
    ) -> std::io::Result<Option<u64>> {
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(path)?;
        let mut buffer = vec![0u8; sample_size.min(8192)]; // Cap at 8KB
        let bytes_read = file.read(&mut buffer)?;

        if bytes_read == 0 {
            return Ok(None);
        }

        // Simple rolling hash - not cryptographically secure but fast
        let mut hash = 0u64;
        let mut rolling = 0u64;
        let base = 257u64;
        let modulos = 1_000_000_007u64;

        // Initialize with first few bytes
        for i in 0..bytes_read.min(8) {
            rolling = (rolling * base + buffer[i] as u64) % modulos;
        }
        hash ^= rolling;

        // Rolling hash for remaining bytes
        for i in 8..bytes_read {
            rolling = (rolling * base + buffer[i] as u64) % modulos;
            rolling =
                (rolling + modulos - (buffer[i - 8] as u64 * base.pow(7) % modulos)) % modulos;
            hash ^= rolling;
        }

        Ok(Some(hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn create_temp_file(content: &[u8]) -> std::path::PathBuf {
        let temp_dir = env::temp_dir();
        let file_name = format!(
            "test_identity_{}.txt",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let temp_path = temp_dir.join(file_name);
        std::fs::write(&temp_path, content).unwrap();
        temp_path
    }

    fn cleanup_temp_file(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_file_identity_creation() {
        let temp_file = create_temp_file(b"Hello, world!");

        let config = FileIdentityConfig::default();
        let identity = FileIdentity::from_path(&temp_file, &config).unwrap();

        assert!(identity.size > 0);
        assert!(identity.device_id >= 0);
        assert!(identity.inode > 0);

        cleanup_temp_file(&temp_file);
    }

    #[test]
    fn test_same_file_detection() {
        let temp_file = create_temp_file(b"Hello, world!");

        let config = FileIdentityConfig::default();
        let identity1 = FileIdentity::from_path(&temp_file, &config).unwrap();

        // Small delay to ensure different timestamps if filesystem supports it
        std::thread::sleep(std::time::Duration::from_millis(10));

        let identity2 = FileIdentity::from_path(&temp_file, &config).unwrap();

        assert!(identity1.is_same_file(&identity2));

        cleanup_temp_file(&temp_file);
    }

    #[test]
    fn test_content_hash() {
        let temp_file = create_temp_file(b"Hello, world!");

        let config = FileIdentityConfig {
            use_fast_hash: true,
            hash_sample_size: 1024,
        };
        let identity = FileIdentity::from_path(&temp_file, &config).unwrap();

        assert!(identity.content_hash.is_some());

        cleanup_temp_file(&temp_file);
    }

    #[test]
    fn test_content_change_detection() {
        let temp_file = create_temp_file(b"Hello, world!");

        let config = FileIdentityConfig {
            use_fast_hash: true,
            hash_sample_size: 1024,
        };
        let identity1 = FileIdentity::from_path(&temp_file, &config).unwrap();

        // Modify content
        std::fs::write(&temp_file, b"Hello, universe!").unwrap();
        let identity2 = FileIdentity::from_path(&temp_file, &config).unwrap();

        assert!(identity1.is_same_file(&identity2)); // Same file
        assert!(identity1.is_modified(&identity2)); // But modified

        if let Some(changed) = identity1.content_changed(&identity2) {
            assert!(changed); // Content changed
        } else {
            panic!("Content hash should be available");
        }

        cleanup_temp_file(&temp_file);
    }
}
