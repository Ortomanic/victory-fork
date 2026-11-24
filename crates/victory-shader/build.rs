//! `VICTORY_SHADER_SOURCE`: a hash of everything the translation is made
//! from (this crate's sources and manifest, and the workspace's lock file,
//! which pins its dependencies). The host's shader cache keys on it instead
//! of on the whole executable, so a rebuild that leaves the translator alone
//! (most of them: the GPU, HLE and loader crates) keeps its translations.

use std::path::{Path, PathBuf};

/// FNV-1a, 128 bits: no dependency, and the same on every host.
struct Fnv(u128);

impl Fnv {
    fn update(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u128::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013B);
        }
    }
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn main() {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let lock = root.join("../../Cargo.lock");
    let mut all = Vec::new();
    files(&root.join("src"), &mut all);
    all.push(root.join("Cargo.toml"));
    all.push(root.join("build.rs"));
    all.push(lock.clone());
    all.sort();
    let mut h = Fnv(0x6c62_272e_07bb_0142_62b8_2175_6295_c58d);
    for path in &all {
        let name = path.strip_prefix(&root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        let bytes = std::fs::read(path).unwrap_or_default();
        h.update(name.as_bytes());
        h.update(&(bytes.len() as u64).to_le_bytes());
        h.update(&bytes);
    }
    println!("cargo:rustc-env=VICTORY_SHADER_SOURCE={:032x}", h.0);
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", lock.display());
}
