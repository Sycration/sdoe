// Emscripten JavaScript library for the WebAssembly build (see build.rs).
//
// A few QtCore files of the prebuilt Qt for WebAssembly catch C++
// exceptions with emscripten's JavaScript-based handling. The link disables
// exception catching (C++ throws abort), so their landing pads, which call
// these two functions, are never reached. They abort if that ever changes.
addToLibrary({
  __resumeException: (ptr) => abort('C++ exception catching is disabled (__resumeException)'),
  llvm_eh_typeid_for: (type) => abort('C++ exception catching is disabled (llvm_eh_typeid_for)'),
});
