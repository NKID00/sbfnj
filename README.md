# Standard BrainFuck of NanJing (SBF/NJ)

Tiered optimizing JIT (TODO) interpreter and compiler for Brainfuck, targets WebAssembly (TODO) and LLVM.

```sh
sbfnj        prog.b  # Naive interpreter
sbfnj --o1   prog.b  # Optimizing interpreter
sbfnj --o2   prog.b  # More optimizations (symbolic execution on loops, etc.)
sbfnj --jit  prog.b  # Tiered JIT (TODO)
sbfnj --llvm prog.b  # Emit LLVM IR, call clang and execute
```

```sh
Standard BrainFuck of NanJing

Usage: sbfnj [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Input filename

Options:
      --text  Emit IR and exit
      --o0    Disable optimization (default)
      --o1    Enable optimizations
      --o2    More optimizations
      --jit   JIT
      --llvm  Emit LLVM IR and call clang
  -h, --help  Print help
```
