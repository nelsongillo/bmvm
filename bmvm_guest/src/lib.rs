#![no_std]
#![no_main]

mod alloc;
mod host_calls;
mod setup;
mod panic;

pub use bmvm_macros::{entry, expose_guest as expose, host};
