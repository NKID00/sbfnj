use std::{
    fmt::{Display, Formatter},
    fs::File,
    io::{BufReader, Read, Write, stdin, stdout},
};

use eyre::{Result, eyre};

use crate::Args;

#[derive(Debug, Clone, Copy)]
pub enum Inst {
    PtrInc(i32),
    ValInc(i32),
    LoopStart(usize),
    LoopEnd(usize),
    Output,
    Input,
}

impl Display for Inst {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use Inst::*;

        match self {
            PtrInc(n) => write!(f, "add ptr, {n}"),
            ValInc(n) => write!(f, "add val, {n}"),
            LoopStart(target) => write!(f, "jz {target}"),
            LoopEnd(target) => write!(f, "jnz {target}"),
            Output => write!(f, "out"),
            Input => write!(f, "in"),
        }
    }
}

#[derive(Debug, Clone)]
struct Prog(Vec<Inst>);

impl Display for Prog {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use Inst::*;

        let lines = self.0.len();
        let line_number_width = lines.to_string().len().max(2);
        let mut tabs = 0;
        for (line, inst) in self.0.iter().enumerate() {
            if let LoopEnd(_) = inst {
                tabs -= 1
            }
            writeln!(
                f,
                "{0:>1$}  {2}{inst}",
                line,
                line_number_width,
                " ".repeat(tabs * 2)
            )?;
            if let LoopStart(_) = inst {
                tabs += 1
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
enum State {
    PtrArithm(i32),
    ValArithm(i32),
    None,
}

pub fn compile(f: File) -> Result<Vec<Inst>> {
    use Inst::*;

    let bytes = BufReader::new(f).bytes().map_while(Result::ok);
    let mut prog: Vec<Inst> = Vec::new();
    let mut state = State::None;
    let mut stack: Vec<usize> = Vec::new();
    for c in bytes {
        match c {
            b'>' => match state {
                State::PtrArithm(n) => state = State::PtrArithm(n + 1),
                State::ValArithm(n) => {
                    prog.push(ValInc(n));
                    state = State::PtrArithm(1);
                }
                State::None => state = State::PtrArithm(1),
            },
            b'<' => match state {
                State::PtrArithm(n) => state = State::PtrArithm(n - 1),
                State::ValArithm(n) => {
                    prog.push(ValInc(n));
                    state = State::PtrArithm(-1);
                }
                State::None => state = State::PtrArithm(-1),
            },
            b'+' => match state {
                State::ValArithm(n) => state = State::ValArithm(n + 1),
                State::PtrArithm(n) => {
                    prog.push(PtrInc(n));
                    state = State::ValArithm(1);
                }
                State::None => state = State::ValArithm(1),
            },
            b'-' => match state {
                State::ValArithm(n) => state = State::ValArithm(n - 1),
                State::PtrArithm(n) => {
                    prog.push(PtrInc(n));
                    state = State::ValArithm(-1);
                }
                State::None => state = State::ValArithm(-1),
            },
            b'[' | b']' | b'.' | b',' => {
                match state {
                    State::ValArithm(n) => {
                        prog.push(ValInc(n));
                    }
                    State::PtrArithm(n) => {
                        prog.push(PtrInc(n));
                    }
                    State::None => {}
                }
                state = State::None;
                match c {
                    b'[' => {
                        stack.push(prog.len());
                        prog.push(LoopStart(0));
                    }
                    b']' => {
                        let start = stack
                            .pop()
                            .ok_or_else(|| eyre!("Orphan ']' should be matched with '['"))?;
                        prog.push(LoopEnd(start + 1));
                        prog[start] = LoopStart(prog.len());
                    }
                    b'.' => prog.push(Output),
                    b',' => prog.push(Input),
                    _ => unreachable!(),
                }
            }
            _ => {}
        }
    }
    if !stack.is_empty() {
        Err(eyre!("Orphan '[' should be matched with ']'"))?;
    }
    match state {
        State::ValArithm(n) => prog.push(ValInc(n)),
        State::PtrArithm(n) => prog.push(PtrInc(n)),
        State::None => {}
    }
    Ok(prog)
}

pub fn main(args: Args, f: File) -> Result<()> {
    use Inst::*;

    let prog = compile(f)?;
    if args.text {
        print!("{}", Prog(prog.clone()));
        return Ok(());
    }

    let mut pc = 0;
    let mut mem = vec![0u8; 30000];
    let mut ptr = 0usize;
    let mut output = stdout().lock();
    let lock = stdin().lock();
    let mut input = lock.bytes().fuse();
    while pc < prog.len() {
        match prog[pc] {
            PtrInc(n) => {
                ptr = ptr.wrapping_add_signed(n as isize);
                pc += 1;
            }
            ValInc(n) => {
                mem[ptr] = mem[ptr].wrapping_add_signed(n as i8);
                pc += 1;
            }
            LoopStart(target) if mem[ptr] == 0 => pc = target,
            LoopEnd(target) if mem[ptr] != 0 => pc = target,
            Output => {
                output.write_all(&[mem[ptr]])?;
                pc += 1;
            }
            Input => {
                mem[ptr] = input.next().and_then(Result::ok).unwrap_or(0);
                pc += 1;
            }
            _ => pc += 1,
        }
    }
    Ok(())
}
