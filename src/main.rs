mod jit;
mod llvm;
mod o0;
mod o1;
mod o2;

use std::fs::File;

use clap::Parser;
use eyre::Result;

/// Standard BrainFuck of NanJing
#[derive(Parser, Debug)]
struct Args {
    /// Disable optimization (default)
    #[arg(long, group = "opt")]
    o0: bool,
    /// Enable optimizations
    #[arg(long, group = "opt")]
    o1: bool,
    /// More optimizations
    #[arg(long, group = "opt")]
    o2: bool,
    /// JIT
    #[arg(long, group = "opt")]
    jit: bool,
    /// Emit LLVM IR and call LLVM
    #[arg(long, group = "opt")]
    llvm: bool,
    /// Input filename
    input: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let f = File::open(args.input)?;
    if args.o1 {
        o1::main(f)
    } else if args.o2 {
        o2::main(f)
    } else if args.jit {
        jit::main(f)
    } else if args.llvm {
        llvm::main(f)
    } else {
        o0::main(f)
    }
}
