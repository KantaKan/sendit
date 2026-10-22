pub mod error;
pub mod identity;
pub mod protocol;
pub mod sanitize;

// Replaced by the real events module in Task 6.
pub mod events {
    #[derive(Clone, Debug, PartialEq)]
    pub enum FailReason {}
}

pub use error::{Error, Result};
