use std::{fs::File, process::Command};

use eyre::{OptionExt, Result, eyre};
use inkwell::{
    AddressSpace, IntPredicate,
    attributes::{Attribute, AttributeLoc},
    builder::Builder,
    context::Context,
    module::{Linkage, Module},
    values::{FunctionValue, PointerValue},
};

use crate::o1;

#[derive(Debug, Clone)]
pub enum Stmt {
    PtrInc(i32),
    ValInc(i32),
    Loop(Vec<Stmt>),
    Output,
    Input,
}

pub fn parse(prog: Vec<o1::Inst>) -> Vec<Stmt> {
    parse_rec(&mut prog.into_iter())
}

fn parse_rec(iter: &mut impl Iterator<Item = o1::Inst>) -> Vec<Stmt> {
    let mut prog = Vec::new();
    while let Some(inst) = iter.next() {
        let stmt = match inst {
            o1::Inst::PtrInc(n) => Stmt::PtrInc(n),
            o1::Inst::ValInc(n) => Stmt::ValInc(n),
            o1::Inst::LoopStart(_target) => Stmt::Loop(parse_rec(iter)),
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

#[derive(Debug)]
pub struct Compiler<'ctx> {
    context: &'ctx Context,
    builder: Builder<'ctx>,
    module: Module<'ctx>,
    main: FunctionValue<'ctx>,
    ptr: PointerValue<'ctx>,
    memory: PointerValue<'ctx>,
    calloc: FunctionValue<'ctx>,
    putchar: FunctionValue<'ctx>,
    getchar: FunctionValue<'ctx>,
}

impl<'ctx> Compiler<'ctx> {
    fn new(context: &'ctx Context) -> Result<Self> {
        let builder = context.create_builder();
        let module = context.create_module("main");

        let i32_type = context.i32_type();
        let main_type = i32_type.fn_type(&[], false);
        let main = module.add_function("main", main_type, None);

        let entry = context.append_basic_block(main, "entry");
        builder.position_at_end(entry);
        let ptr = builder.build_alloca(i32_type, "ptr")?;
        let ptr_type = context.ptr_type(AddressSpace::default());
        let memory = builder.build_alloca(ptr_type, "memory")?;

        let i64_type = context.i64_type();
        let calloc_type = ptr_type.fn_type(&[i64_type.into()], false);
        let calloc = module.add_function("calloc", calloc_type, Some(Linkage::External));
        let noalias_kind_id = Attribute::get_named_enum_kind_id("noalias");
        let noalias = context.create_enum_attribute(noalias_kind_id, 0);
        calloc.add_attribute(AttributeLoc::Return, noalias);
        let putchar_type = i32_type.fn_type(&[i32_type.into()], false);
        let putchar = module.add_function("putchar", putchar_type, Some(Linkage::External));
        let getchar_type = i32_type.fn_type(&[], false);
        let getchar = module.add_function("getchar", getchar_type, Some(Linkage::External));

        Ok(Compiler {
            context: context,
            builder,
            module,
            main,
            ptr,
            memory,
            calloc,
            putchar,
            getchar,
        })
    }

    fn compile(&mut self, prog: Vec<Stmt>) -> Result<()> {
        let i32_type = self.context.i32_type();
        let i32_zero = i32_type.const_zero();
        self.builder.build_store(self.ptr, i32_zero)?;
        let i64_type = self.context.i64_type();
        let val = self.builder.build_direct_call(
            self.calloc,
            &[
                i64_type.const_int(30000, false).into(),
                i64_type.const_int(1, false).into(),
            ],
            "",
        )?;
        self.builder
            .build_store(self.memory, val.try_as_basic_value().left().ok_or_eyre("")?)?;

        self.compile_rec(prog)?;

        self.builder.build_return(Some(&i32_zero))?;

        self.module
            .print_to_file("prog.ll")
            .map_err(|s| eyre!(s.to_string()))?;
        Ok(())
    }

    fn compile_rec(&mut self, prog: Vec<Stmt>) -> Result<()> {
        let i32_type = self.context.i32_type();
        let ptr_type = self.context.ptr_type(AddressSpace::default());
        let i8_type = self.context.i8_type();
        for stmt in prog {
            match stmt {
                Stmt::PtrInc(n) => {
                    let ptr = self.builder.build_load(i32_type, self.ptr, "")?;
                    let result = self.builder.build_int_add(
                        ptr.into_int_value(),
                        i32_type.const_int(n as u64, true),
                        "",
                    )?;
                    self.builder.build_store(self.ptr, result)?;
                }
                Stmt::ValInc(n) => {
                    let memory = self.builder.build_load(ptr_type, self.memory, "")?;
                    let ptr = self.builder.build_load(i32_type, self.ptr, "")?;
                    let element_ptr = unsafe {
                        self.builder.build_gep(
                            i8_type,
                            memory.into_pointer_value(),
                            &[ptr.into_int_value()],
                            "",
                        )
                    }?;
                    let val = self.builder.build_load(i8_type, element_ptr, "")?;
                    let val = self.builder.build_int_add(
                        val.into_int_value(),
                        i8_type.const_int(n as i8 as u64, true),
                        "",
                    )?;
                    self.builder.build_store(element_ptr, val)?;
                }
                Stmt::Loop(stmts) => {
                    let cond_bb = self.context.append_basic_block(self.main, "");
                    self.builder.build_unconditional_branch(cond_bb)?;
                    self.builder.position_at_end(cond_bb);
                    let memory = self.builder.build_load(ptr_type, self.memory, "")?;
                    let ptr = self.builder.build_load(i32_type, self.ptr, "")?;
                    let element_ptr = unsafe {
                        self.builder.build_gep(
                            i8_type,
                            memory.into_pointer_value(),
                            &[ptr.into_int_value()],
                            "",
                        )
                    }?;
                    let val = self.builder.build_load(i8_type, element_ptr, "")?;
                    let cond = self.builder.build_int_compare(
                        IntPredicate::NE,
                        val.into_int_value(),
                        i8_type.const_zero(),
                        "",
                    )?;
                    let true_bb = self.context.append_basic_block(self.main, "");
                    let false_bb = self.context.append_basic_block(self.main, "");
                    self.builder
                        .build_conditional_branch(cond, true_bb, false_bb)?;
                    self.builder.position_at_end(true_bb);
                    self.compile_rec(stmts)?;
                    self.builder.build_unconditional_branch(cond_bb)?;
                    self.builder.position_at_end(false_bb);
                }
                Stmt::Output => {
                    let memory = self.builder.build_load(ptr_type, self.memory, "")?;
                    let ptr = self.builder.build_load(i32_type, self.ptr, "")?;
                    let element_ptr = unsafe {
                        self.builder.build_gep(
                            i8_type,
                            memory.into_pointer_value(),
                            &[ptr.into_int_value()],
                            "",
                        )
                    }?;
                    let val = self.builder.build_load(i8_type, element_ptr, "")?;
                    self.builder
                        .build_direct_call(self.putchar, &[val.into()], "")?;
                }
                Stmt::Input => {
                    let memory = self.builder.build_load(ptr_type, self.memory, "")?;
                    let ptr = self.builder.build_load(i32_type, self.ptr, "")?;
                    let element_ptr = unsafe {
                        self.builder.build_gep(
                            i8_type,
                            memory.into_pointer_value(),
                            &[ptr.into_int_value()],
                            "",
                        )
                    }?;
                    let val = self.builder.build_direct_call(self.getchar, &[], "")?;
                    self.builder.build_store(
                        element_ptr,
                        val.try_as_basic_value().left().ok_or_eyre("")?,
                    )?;
                }
            }
        }
        Ok(())
    }
}

pub fn main(f: File) -> Result<()> {
    let prog = o1::compile(f)?;
    let prog = parse(prog);
    Compiler::new(&Context::create())?.compile(prog)?;
    Command::new("clang")
        .args(["-o", "prog", "-O2", "prog.ll"])
        .status()?;
    Command::new("./prog").status()?;
    Ok(())
}
