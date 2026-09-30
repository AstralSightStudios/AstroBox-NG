This is the `wasmtime-jit-debug` crate, which contains JIT debug interfaces support for Wasmtime.

AstroBox carries a patch of Wasmtime 48.0.2's internal crate. On Windows,
Wasmtime 38 and 48 both export the canonical `__jit_debug_*` symbols, which
causes LNK2005 when both are linked into the app. The 48.x registration type
therefore re-exports 38.x's implementation on Windows, sharing its debugger
descriptor and registration mutex. This keeps native JIT image registration
for both runtimes. The original 48.x implementation is used elsewhere.

When upgrading either Wasmtime version, check that the GDB JIT registration
API and image format are still compatible before carrying this patch forward.
