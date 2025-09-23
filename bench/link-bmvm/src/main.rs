#![feature(macro_metavar_expr_concat)]
#![no_std]
#![no_main]
extern crate core;
use bmvm_guest::{expose, host};

#[cfg(feature = "links1")]
seq_macro::seq!(N in 0..1 {
    #[expose]
    pub extern "C" fn up~N() -> i32 {
        N
    }
});

#[cfg(feature = "links8")]
seq_macro::seq!(N in 0..8 {
    #[expose]
    pub fn up~N() -> i32 {
        N
    }
});

#[cfg(feature = "links16")]
seq_macro::seq!(N in 0..16 {
    #[expose]
    pub fn up~N() -> i32 {
        N
    }
});

#[cfg(feature = "links32")]
seq_macro::seq!(N in 0..32 {
    #[expose]
    pub fn up~N() -> i32 {
        N
    }
});

#[cfg(feature = "links64")]
seq_macro::seq!(N in 0..64 {
    #[expose]
    pub fn up~N() -> i32 {
        N
    }
});

#[cfg(feature = "links128")]
seq_macro::seq!(N in 0..128 {
    #[expose]
    pub fn up~N() -> i32 {
        N
    }
});

#[host]
unsafe extern "C" {
    #[cfg(feature = "links1")]
    seq_macro::seq!(N in 0..1 {
        fn hyper~N() -> i32;
    });

    #[cfg(feature = "links8")]
    seq_macro::seq!(N in 0..8 {
        fn hyper~N() -> i32;
    });

    #[cfg(feature = "links16")]
    seq_macro::seq!(N in 0..16 {
        fn hyper~N() -> i32;
    });

    #[cfg(feature = "links32")]
    seq_macro::seq!(N in 0..32 {
        fn hyper~N() -> i32;
    });

    #[cfg(feature = "links64")]
    seq_macro::seq!(N in 0..64 {
        fn hyper~N() -> i32;
    });

    #[cfg(feature = "links128")]
    seq_macro::seq!(N in 0..128 {
        fn hyper~N() -> i32;
    });
}
