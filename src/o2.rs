use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
    fs::File,
    io::{Bytes, Read, StdinLock, StdoutLock, Write, stdin, stdout},
    iter::Fuse,
    mem::take,
    ops::{Add, AddAssign},
};

use eyre::{Result, eyre};

use crate::{Args, o1};

#[derive(Debug, Clone)]
pub enum Stmt {
    PtrInc(i32),
    ValInc(i32),
    Loop(Vec<Stmt>),
    Output,
    Input,
}

impl Display for Stmt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl Stmt {
    pub fn pure(&self) -> bool {
        match self {
            Stmt::PtrInc(_) | Stmt::ValInc(_) => true,
            Stmt::Loop(stmts) => stmts.iter().all(Stmt::pure),
            Stmt::Output | Stmt::Input => false,
        }
    }
}

pub fn compile(prog: Vec<o1::Inst>) -> Vec<Stmt> {
    compile_rec(&mut prog.into_iter())
}

fn compile_rec(iter: &mut impl Iterator<Item = o1::Inst>) -> Vec<Stmt> {
    let mut prog = Vec::new();
    while let Some(inst) = iter.next() {
        let stmt = match inst {
            o1::Inst::PtrInc(n) => Stmt::PtrInc(n),
            o1::Inst::ValInc(n) => Stmt::ValInc(n),
            o1::Inst::LoopStart(_target) => Stmt::Loop(compile_rec(iter)),
            o1::Inst::LoopEnd(_target) => {
                return prog;
            }
            o1::Inst::Output => Stmt::Output,
            o1::Inst::Input => Stmt::Input,
        };
        prog.push(stmt);
    }
    prog
}

#[derive(Debug, Clone)]
enum SymExVal {
    Const(i32),
    Cell(i32),
    Add(Box<SymExVal>, Box<SymExVal>),
    // Mul(Box<SymExVal>, Box<SymExVal>),
}

impl SymExVal {
    fn is_const(&self) -> bool {
        matches!(self, SymExVal::Const(_))
    }
    fn const_val(&self) -> Option<i32> {
        if let SymExVal::Const(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    fn simplify(self) -> Self {
        use SymExVal::*;
        match self {
            Add(lhs, rhs) => {
                let lhs = lhs.simplify();
                let rhs = rhs.simplify();
                match (lhs, rhs) {
                    (Const(lv), Const(rv)) => Const(lv.wrapping_add(rv)),
                    (Cell(n), Const(rv)) => todo!(),
                    (Add(llhs, lrhs), Const(rv)) => match *lrhs {
                        Const(lv) => Add(llhs, Box::new(Const(lv.wrapping_add(rv)))),
                        _ => Add(Box::new(Add(llhs, lrhs)), Box::new(Const(rv))),
                    },
                    _ => todo!(),
                }
            }
            _ => self,
        }
    }
}

impl Default for SymExVal {
    fn default() -> Self {
        Self::Const(0)
    }
}

impl Add for SymExVal {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        SymExVal::Add(Box::new(self), Box::new(rhs)).simplify()
    }
}

impl AddAssign for SymExVal {
    fn add_assign(&mut self, rhs: Self) {
        let lhs = take(self);
        *self = lhs + rhs;
    }
}

#[derive(Debug)]
struct SymExInfo {
    ptr_delta: i32,
    memory_delta: BTreeMap<i32, SymExVal>,
}

fn symbolic_execution(prog: &Vec<Stmt>) -> Result<SymExInfo> {
    use Stmt::*;
    use SymExVal::*;

    let mut ptr_delta = 0;
    let mut memory_delta = BTreeMap::new();
    for stmt in prog {
        match stmt {
            PtrInc(n) => ptr_delta += n,
            ValInc(n) => match memory_delta.get_mut(&ptr_delta) {
                Some(delta) => *delta += Const(*n),
                None => {
                    memory_delta.insert(ptr_delta, Const(*n));
                }
            },
            Loop(_) => Err(eyre!("nested loop is not implemented"))?,
            Output | Input => Err(eyre!("not pure, env model is not implemented"))?,
        }
    }
    Ok(SymExInfo {
        ptr_delta,
        memory_delta,
    })
}

fn optimize(prog: Vec<Stmt>) -> Vec<Stmt> {
    prog.into_iter()
        .flat_map(|stmt| match stmt {
            Stmt::Loop(stmts) => optimize_loop(stmts),
            _ => vec![stmt],
        })
        .collect()
}

fn optimize_loop(body: Vec<Stmt>) -> Vec<Stmt> {
    use SymExVal::*;

    match symbolic_execution(&body) {
        Ok(SymExInfo {
            ptr_delta,
            memory_delta,
        }) => {
            if ptr_delta == 0 {
                // memory[ptr] is loop index
                let step = match memory_delta.get(&0) {
                    Some(Const(0)) => unimplemented!("diverge: dead loop"),
                    Some(Const(v)) => *v,
                    _ => return body,
                };
                if step != 1 {
                    return body;
                }
                println!("step = {step}, body = {body:?}");
                body
            } else {
                body
            }
        }
        Err(_) => body,
    }
}

#[derive(Debug)]
struct Interpreter<'a, 'b> {
    output: StdoutLock<'a>,
    input: Fuse<Bytes<StdinLock<'a>>>,
    prog: &'b Vec<Stmt>,
    memory: Vec<u8>,
    ptr: usize,
}

impl<'a, 'b> Interpreter<'a, 'b> {
    fn new(prog: &'b Vec<Stmt>) -> Self {
        Self {
            output: stdout().lock(),
            input: stdin().lock().bytes().fuse(),
            prog,
            memory: vec![0u8; 30000],
            ptr: 0,
        }
    }

    fn interpret(&mut self) -> Result<()> {
        self.interpret_rec(self.prog)
    }

    fn interpret_rec(&mut self, prog: &Vec<Stmt>) -> Result<()> {
        for stmt in prog {
            match stmt {
                Stmt::PtrInc(n) => self.ptr = self.ptr.wrapping_add_signed(*n as isize),
                Stmt::ValInc(n) => {
                    self.memory[self.ptr] = self.memory[self.ptr].wrapping_add_signed(*n as i8)
                }
                Stmt::Loop(body) => {
                    while self.memory[self.ptr] != 0 {
                        self.interpret_rec(body)?
                    }
                }
                Stmt::Output => {
                    self.output.write_all(&[self.memory[self.ptr]])?;
                }
                Stmt::Input => {
                    self.memory[self.ptr] = self.input.next().and_then(Result::ok).unwrap_or(0)
                }
                _ => unimplemented!(),
            }
        }
        Ok(())
    }
}

pub fn main(args: Args, f: File) -> Result<()> {
    let prog = o1::compile(f)?;
    let prog = compile(prog);
    let prog = optimize(prog);
    if args.text {
        // print!("{}", Prog(prog.clone()));
        // return Ok(());
        todo!()
    }
    Interpreter::new(&prog).interpret()
}
