# Standard BrainFuck of NanJing (SBF/NJ)

```sh
sbfnj        prog.b  # naive interpreting
sbfnj --o1   prog.b  # interpret with optimizations
sbfnj --o2   prog.b  # more optimizations (TODO)
sbfnj --jit  prog.b  # (TODO)
sbfnj --llvm prog.b  # emit llvm ir, call clang and execute
```
