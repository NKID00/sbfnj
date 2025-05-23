use std::{
    fs::File,
    io::{Read, stdin},
};

use eyre::Result;

pub fn main(mut f: File) -> Result<()> {
    let mut instructions = Vec::new();
    f.read_to_end(&mut instructions)?;
    let instructions = instructions;
    let mut pc = 0;
    let mut nest_level;
    let mut memory = vec![0u8; 30000];
    let mut ptr = 0usize;
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
                memory[ptr] = memory[ptr].wrapping_add(1);
            }
            b'-' => {
                memory[ptr] = memory[ptr].wrapping_sub(1);
            }
            b'[' if memory[ptr] == 0 => {
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
            b'.' => print!("{}", memory[ptr] as char),
            b',' => memory[ptr] = input.next().and_then(Result::ok).unwrap_or(0),
            _ => {}
        }
        pc += 1;
    }
    Ok(())
}
