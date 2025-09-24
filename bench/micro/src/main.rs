use bmvm_host::mem::{AlignedNonZeroUsize, AlignedUsize};
use bmvm_host::{ConfigBuilder, ModuleBuilder, Upcall, expose, linker};
use const_format::formatcp;
use std::hint::black_box;

#[cfg(feature = "links1")]
const LINKS: usize = 1;
#[cfg(feature = "links8")]
const LINKS: usize = 8;
#[cfg(feature = "links16")]
const LINKS: usize = 16;
#[cfg(feature = "links32")]
const LINKS: usize = 32;
#[cfg(feature = "links64")]
const LINKS: usize = 64;
#[cfg(feature = "links128")]
const LINKS: usize = 128;

pub fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).unwrap();
    let stack = AlignedNonZeroUsize::new_ceil(1).unwrap();
    let mut calls: Vec<Upcall<(), i32>> = Vec::with_capacity(LINKS);

    let mut linker = linker::ConfigBuilder::new();
    #[cfg(feature = "links1")]
    {
        linker = linker.register_guest_function::<(), i32>("up0");
    }

    #[cfg(feature = "links8")]
    loop_code::repeat!(INDEX 8 {
        linker = linker.register_guest_function::<(), i32>(formatcp!("up{}", INDEX));
    });

    #[cfg(feature = "links16")]
    loop_code::repeat!(INDEX 16 {
            linker = linker.register_guest_function::<(), i32>(formatcp!("up{}", INDEX));
    });

    #[cfg(feature = "links32")]
    loop_code::repeat!(INDEX 32 {
        linker = linker.register_guest_function::<(), i32>(formatcp!("up{}", INDEX));
    });

    #[cfg(feature = "links64")]
    loop_code::repeat!(INDEX 64 {
        linker = linker.register_guest_function::<(), i32>(formatcp!("up{}", INDEX));
    });

    #[cfg(feature = "links128")]
    loop_code::repeat!(INDEX 128 {
        linker = linker.register_guest_function::<(), i32>(formatcp!("up{}", INDEX));
    });

    let mut module = black_box({
        ModuleBuilder::new()
            .configure_vm(
                ConfigBuilder::new()
                    .stack_size(stack)
                    .shared_memory(AlignedUsize::zero()),
            )
            .configure_linker(linker)
            .with_path(path.as_ref())
            .build()?
    });

    #[cfg(feature = "links1")]
    {
        calls.push(module.get_upcall::<(), i32>("up0")?);
    }

    #[cfg(feature = "links8")]
    loop_code::repeat!(INDEX 8 {
        calls.push(module.get_upcall::<(), i32>(formatcp!("up{}", INDEX))?);
    });

    #[cfg(feature = "links16")]
    loop_code::repeat!(INDEX 16 {
        calls.push(module.get_upcall::<(), i32>(formatcp!("up{}", INDEX))?);
    });

    #[cfg(feature = "links32")]
    loop_code::repeat!(INDEX 32 {
            calls.push(module.get_upcall::<(), i32>(formatcp!("up{}", INDEX))?);
    });

    #[cfg(feature = "links64")]
    loop_code::repeat!(INDEX 64 {
        calls.push(module.get_upcall::<(), i32>(formatcp!("up{}", INDEX))?);
    });

    #[cfg(feature = "links128")]
    loop_code::repeat!(INDEX 128 {
        calls.push(module.get_upcall::<(), i32>(formatcp!("up{}", INDEX))?);
    });

    std::mem::drop(module);
    std::mem::drop(calls);
    Ok(())
}

#[cfg(feature = "links1")]
seq_macro::seq!(N in 0..1 {
    #[expose]
    pub extern "C" fn hyper~N() -> i32 {
        N
    }

    static UP~N: &'static str = "up~N";
});

#[cfg(feature = "links8")]
seq_macro::seq!(N in 0..8 {
    #[expose]
    pub extern "C" fn hyper~N() -> i32 {
        N
    }
});

#[cfg(feature = "links16")]
seq_macro::seq!(N in 0..16 {
    #[expose]
    pub extern "C" fn hyper~N() -> i32 {
        N
    }
});

#[cfg(feature = "links32")]
seq_macro::seq!(N in 0..32 {
    #[expose]
    pub extern "C" fn hyper~N() -> i32 {
        N
    }
});

#[cfg(feature = "links64")]
seq_macro::seq!(N in 0..64 {
    #[expose]
    pub extern "C" fn hyper~N() -> i32 {
        N
    }
});

#[cfg(feature = "links128")]
seq_macro::seq!(N in 0..128 {
    #[expose]
    pub extern "C" fn hyper~N() -> i32 {
        N
    }
});
