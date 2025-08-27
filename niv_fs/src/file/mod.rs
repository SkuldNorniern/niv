//! File operations for loading and saving with proper encoding handling.
//!
//! This module provides:
//! - Streaming file loading with encoding detection
//! - EOL detection and normalization
//! - Binary/huge file guards
//! - Atomic saving with transcoding
//! - File identity tracking for renames/moves
//! - Cross-platform permission preservation

pub mod load;
pub mod save;
pub mod identity;
pub mod eol;

pub use load::{FileLoadResult, FileLoadConfig, load_file, load_file_with_config};
pub use save::{FileSaveResult, FileSaveConfig, save_file, save_file_with_config, SaveContext};
pub use identity::{FileIdentity, FileIdentityConfig};
pub use eol::{EolType, normalize_eol, restore_eol};
