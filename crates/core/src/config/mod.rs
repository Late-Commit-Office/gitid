//! Reading git config through `git config`. Files are never parsed directly.
//! (Writing arrives in step 2 as `write.rs`.)

mod model;
mod read;

pub use model::{normalize_key, parse_bool, ConfigSnapshot, Entry, Origin, Scope};
pub use read::{parse_z, read_all, read_all_in};
