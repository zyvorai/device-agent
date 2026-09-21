# Zyvor Device Agent Plugin SDK (Rust)

Native plugins stay separate processes. WASI plugins need a binary built with
`--features wasm` and may only declare the `none` capability.
