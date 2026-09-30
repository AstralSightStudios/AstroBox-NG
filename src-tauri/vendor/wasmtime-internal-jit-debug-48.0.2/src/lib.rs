//! > **⚠️ Warning ⚠️**: this crate is an internal-only crate for the Wasmtime
//! > project and is not intended for general use. APIs are not strictly
//! > reviewed for safety and usage outside of Wasmtime may have bugs. If
//! > you're interested in using this feel free to file an issue on the
//! > Wasmtime repository to start a discussion about doing so, but otherwise
//! > be aware that your usage of this crate is not supported.
#![no_std]

#[cfg(feature = "std")]
extern crate std;

#[cfg(all(feature = "gdb_jit_int", target_os = "windows"))]
pub mod gdb_jit_int {
    // Share the canonical GDB/LLDB JIT descriptor and registration lock with
    // Wasmtime 38 so both versions' code images remain visible to the debugger.
    pub use jit_debug_v38::gdb_jit_int::GdbJitImageRegistration;
}

#[cfg(all(feature = "gdb_jit_int", not(target_os = "windows")))]
pub mod gdb_jit_int;

#[cfg(all(feature = "perf_jitdump", target_os = "linux"))]
pub mod perf_jitdump;

#[cfg(all(test, feature = "gdb_jit_int", target_os = "windows"))]
mod tests {
    use super::gdb_jit_int::GdbJitImageRegistration;

    #[test]
    fn both_wasmtime_versions_register_jit_images() {
        let legacy = jit_debug_v38::gdb_jit_int::GdbJitImageRegistration::register(std::vec![1]);
        let v4 = GdbJitImageRegistration::register(std::vec![2]);

        assert_eq!(legacy.file(), &[1]);
        assert_eq!(v4.file(), &[2]);

        drop(legacy);
        drop(v4);
    }
}
