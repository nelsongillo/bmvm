#![no_std]
#![no_main]

mod alloc;
mod host_calls;
mod panic;
mod setup;

pub use bmvm_macros::{entry, expose_guest as expose, host};
