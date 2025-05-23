use std::{
    fs::File,
    io::{BufReader, Read, stdin},
};

use eyre::{Result, eyre};

#[derive(Debug, Clone, Copy)]
pub enum Inst {
    PtrInc(i32),
    ValInc(i32),
    LoopStart(usize),
    LoopEnd(usize),
    Output,
    Input,
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

pub fn main(f: File) -> Result<()> {
    use Inst::*;

    let instructions = compile(f)?;

    let mut pc = 0;
    let mut memory = vec![0u8; 30000];
    let mut ptr = 0usize;
    let lock = stdin().lock();
    let mut input = lock.bytes().fuse();
    while pc < instructions.len() {
        match instructions[pc] {
            PtrInc(n) => {
                ptr = ptr.wrapping_add_signed(n as isize);
                pc += 1;
            }
            ValInc(n) => {
                memory[ptr] = memory[ptr].wrapping_add_signed(n as i8);
                pc += 1;
            }
            LoopStart(target) if memory[ptr] == 0 => pc = target,
            LoopEnd(target) if memory[ptr] != 0 => pc = target,
            Output => {
                print!("{}", memory[ptr] as char);
                pc += 1;
            }
            Input => {
                memory[ptr] = input.next().and_then(Result::ok).unwrap_or(0);
                pc += 1;
            }
            _ => pc += 1,
        }
    }
    Ok(())
}
