#![feature(path_add_extension)]

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
    /// Emit IR and exit
    #[arg(long)]
    text: bool,
    /// Disable optimization (default)
    #[arg(long, group = "opt")]
    o0: bool,
    /// Enable optimizations
    #[arg(long, group = "opt")]
    o1: bool,
    /// More optimizations
    #[arg(long, group = "opt")]
    o2: bool,
    /// JIT (TDOO)
    #[arg(long, group = "opt")]
    jit: bool,
    /// Emit LLVM IR and call clang
    #[arg(long, group = "opt")]
    llvm: bool,
    /// Input filename
    input: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let f = File::open(&args.input)?;
    if args.o1 {
        o1::main(args, f)
    } else if args.o2 {
        o2::main(args, f)
    } else if args.jit {
        jit::main(args, f)
    } else if args.llvm {
        llvm::main(args, f)
    } else {
        o0::main(args, f)
    }
}
