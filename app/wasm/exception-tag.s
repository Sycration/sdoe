# The wasm exception tag (see build.rs). LLVM defines it in objects that
# throw; the precompiled Rust standard library only catches.
.globl __cpp_exception
.tagtype __cpp_exception i32
__cpp_exception:
