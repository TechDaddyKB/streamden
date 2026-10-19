//! StreamDen engine core (Spec 03), independent of the desktop shell.

pub use den_protocol;

pub mod auth;
pub mod bus;
pub mod clock;
pub mod commands;
pub mod config;
pub mod context;
pub mod engine;
pub mod error;
pub mod exec;
pub mod history;
pub mod matcher;
pub mod ops;
pub mod queues;
pub mod schedule;
pub mod scheduler;
pub mod shutdown;
pub mod signals;
pub mod storage;
pub mod template;
pub mod vars;
