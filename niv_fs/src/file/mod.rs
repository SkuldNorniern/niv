//! File operations for loading and saving with proper encoding handling.
//!
//! This module provides:
//! - Streaming file loading with encoding detection
//! - EOL detection and normalization
//! - Binary/huge file guards
//! - Atomic saving with transcoding
//! - File identity tracking for renames/moves
//! - Cross-platform permission preservation

pub mod eol;
pub mod identity;
pub mod load;
pub mod save;

pub use eol::{EolType, normalize_eol, restore_eol};
pub use identity::{FileIdentity, FileIdentityConfig};
pub use load::{FileLoadConfig, FileLoadResult, load_file, load_file_with_config};
pub use save::{FileSaveConfig, FileSaveResult, SaveContext, save_file, save_file_with_config};
