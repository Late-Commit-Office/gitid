//! Reading and writing git config through `git config`.
//! Config files are never parsed or written directly.

mod model;
mod read;
mod write;

pub use model::{normalize_key, parse_bool, ConfigSnapshot, Entry, Origin, Scope};
pub use read::{parse_z, read_all, read_all_in, read_target};
pub use write::{add, escape_regex, set, unset, unset_all, unset_value, Target};
