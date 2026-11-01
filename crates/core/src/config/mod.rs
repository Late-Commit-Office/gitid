//! Reading and writing git config through `git config`.
//! Config files are never parsed or written directly.

mod layers;
mod model;
mod read;
mod write;

pub use layers::{is_multi_valued, layers, resolve, EntryRef, KeyLayers, Layer, Status};
pub use model::{normalize_key, parse_bool, ConfigSnapshot, Entry, Origin, Scope};
pub use read::{parse_z, read_all, read_all_in, read_target};
pub use write::{add, escape_regex, replace_value, set, unset, unset_all, unset_value, Target};
