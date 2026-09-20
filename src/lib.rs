//! Shared controller ownership; domain authority and checks remain in adapters.
//! See THIRD_PARTY_NOTICES.md for Conary's demonstrated controller invariants.
pub mod contract;
pub mod controller;
pub mod evidence;
pub mod process;
pub use contract::*;
pub use controller::run;
pub use tokio_util::sync::CancellationToken;
