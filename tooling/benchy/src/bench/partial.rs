use crate::bench::{bench, multibench};
use bmvm_host::mem::{AlignedNonZeroUsize, AlignedUsize};
use bmvm_host::{ConfigBuilder, ModuleBuilder, Upcall, expose, linker};
use const_format::formatcp;
use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::Kvm;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::Output;
use std::ptr::null_mut;
use std::time::Instant;
use wasmtime::{Engine, Instance, Linker, Module as WasmModule, Store, TypedFunc};

pub fn native<const N: usize>(
    path: &PathBuf,
    warmup: usize,
    iters: usize,
    args: String,
) -> anyhow::Result<[Vec<f64>; N]> {
    fn pre((path, args): (&PathBuf, String)) -> anyhow::Result<(PathBuf, String)> {
        Ok((path.clone(), args))
    }
    fn exec<const N: usize>((path, args): &mut (PathBuf, String)) -> anyhow::Result<[f64; N]> {
        let output: Output = std::process::Command::new(&path)
            .args(args.split_ascii_whitespace())
            .output()?;
        let out = String::from_utf8_lossy(&output.stdout);
        let vs = out
            .trim()
            .to_string()
            .split_ascii_whitespace()
            .map(|s| s.parse::<u64>().unwrap() as f64)
            .collect::<Vec<f64>>();

        if vs.len() != N {
            return Err(anyhow::anyhow!(format!("invalid output: {}", out)));
        }

        let mut result = [0f64; N];
        for i in 0..N {
            result[i] = vs[i];
        }
        Ok(result)
    }
    fn post(_: &mut (PathBuf, String)) -> anyhow::Result<()> {
        Ok(())
    }
    multibench((path, args), warmup, iters, pre, exec, post)
}
