//! Talking to the git executable. Nothing outside this module spawns processes.

mod locate;
mod runner;
mod version;

pub use locate::{candidates, locate};
pub use runner::{Git, Output, SystemGit};
pub use version::GitVersion;
