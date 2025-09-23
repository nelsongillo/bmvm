#![feature(macro_metavar_expr_concat)]
#![no_std]
#![no_main]
extern crate core;

#[cfg(feature = "links1")]
seq_macro::seq!(N in 0..1 {
    #[unsafe(no_mangle)]
    pub extern "C" fn up~N() -> i32 {
        unsafe { hyper~N() + 1 }
    }
});

#[cfg(feature = "links8")]
seq_macro::seq!(N in 0..8 {
    #[unsafe(no_mangle)]
     pub extern "C" fn up~N() -> i32 {
        unsafe { hyper~N() + 1 }
    }
});

#[cfg(feature = "links16")]
seq_macro::seq!(N in 0..16 {
    #[unsafe(no_mangle)]
     pub extern "C" fn up~N() -> i32 {
        unsafe { hyper~N() + 1 }
    }
});

#[cfg(feature = "links32")]
seq_macro::seq!(N in 0..32 {
    #[unsafe(no_mangle)]
     pub extern "C" fn up~N() -> i32 {
        unsafe { hyper~N() + 1 }
    }
});

#[cfg(feature = "links64")]
seq_macro::seq!(N in 0..64 {
    #[unsafe(no_mangle)]
     pub extern "C" fn up~N() -> i32 {
        unsafe { hyper~N() + 1 }
    }
});

#[cfg(feature = "links128")]
seq_macro::seq!(N in 0..128 {
    #[unsafe(no_mangle)]
     pub extern "C" fn up~N() -> i32 {
        unsafe { hyper~N() + 1 }
    }
});

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

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unreachable!();
}
