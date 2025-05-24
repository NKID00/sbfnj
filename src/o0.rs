use std::{
    fs::File,
    io::{Read, Write, stdin, stdout},
};

use eyre::{Result, eyre};

use crate::Args;

pub fn main(args: Args, mut f: File) -> Result<()> {
    if args.text {
        return Err(eyre!("o0 interpreter has no IR"));
    }

    let mut instructions = Vec::new();
    f.read_to_end(&mut instructions)?;
    let instructions = instructions;
    let mut pc = 0;
    let mut nest_level;
    let mut mem = vec![0u8; 30000];
    let mut ptr = 0usize;
    let mut output = stdout().lock();
    let lock = stdin().lock();
    let mut input = lock.bytes().fuse();
    while pc < instructions.len() {
        match instructions[pc] {
            b'>' => {
                ptr += 1;
            }
            b'<' => {
                ptr -= 1;
            }
            b'+' => {
                mem[ptr] = mem[ptr].wrapping_add(1);
            }
            b'-' => {
                mem[ptr] = mem[ptr].wrapping_sub(1);
            }
            b'[' if mem[ptr] == 0 => {
                pc += 1;
                nest_level = 1;
                while nest_level > 0 {
                    match instructions[pc] {
                        b'[' => {
                            nest_level += 1;
                        }
                        b']' => {
                            nest_level -= 1;
                        }
                        _ => {}
                    }
                    pc += 1;
                }
                pc -= 1;
            }
            b']' => {
                pc -= 1;
                nest_level = 1;
                while nest_level > 0 {
                    match instructions[pc] {
                        b'[' => {
                            nest_level -= 1;
                        }
                        b']' => {
                            nest_level += 1;
                        }
                        _ => {}
                    }
                    pc -= 1;
                }
            }
            b'.' => {
                output.write_all(&[mem[ptr]])?;
            }
            b',' => mem[ptr] = input.next().and_then(Result::ok).unwrap_or(0),
            _ => {}
        }
        pc += 1;
    }
    Ok(())
}
