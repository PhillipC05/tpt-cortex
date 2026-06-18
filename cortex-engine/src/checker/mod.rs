mod checker;
mod manifest;
mod error;
mod scope;

pub use checker::check;
pub use manifest::PermissionManifest;
pub use error::CheckError;
