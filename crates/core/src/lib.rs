//! gitid-core: which git identity applies to a folder, and why.
//!
//! The one rule: every read and write goes through the `git` executable.
//! Config files are never parsed or written directly.
//!
//! - Step 1: locating git, reading config with scope and origin, and
//!   explaining the effective author/committer identity.
//! - Step 2: writing config with an explicit target, and identity rules
//!   (`includeIf` per folder or per server) in [`rules`].

pub mod config;
pub mod error;
pub mod git;
pub mod identity;
pub mod paths;
pub mod platform;
pub mod repo;
pub mod rules;
pub mod secrets;

pub use config::{ConfigSnapshot, Entry, Origin, Scope};
pub use error::{Error, Result};
pub use git::{Git, GitVersion, SystemGit};
pub use identity::{
    effective_identity, Field, Identity, Part, Role, RoleIdentity, Severity, Source, Warning,
};
pub use repo::RepoLocation;
