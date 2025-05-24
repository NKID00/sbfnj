use std::{fs::File, io::Write, path::PathBuf, process::Command, str::FromStr};

use eyre::{OptionExt, Result};
use inkwell::{
    AddressSpace, IntPredicate,
    attributes::{Attribute, AttributeLoc},
    builder::Builder,
    context::Context,
    module::{Linkage, Module},
    targets::TargetTriple,
    values::{FunctionValue, IntValue, PointerValue},
};

use crate::{
    Args, o1,
    o2::{self, Stmt},
};

#[derive(Debug)]
pub struct Compiler<'ctx> {
    context: &'ctx Context,
    builder: Builder<'ctx>,
    module: Module<'ctx>,
    main: FunctionValue<'ctx>,
    ptr: IntValue<'ctx>,
    mem: PointerValue<'ctx>,
    putchar: FunctionValue<'ctx>,
    getchar: FunctionValue<'ctx>,
}

impl<'ctx> Compiler<'ctx> {
    fn new(context: &'ctx Context) -> Result<Self> {
        let builder = context.create_builder();
        let module = context.create_module("main");
        #[cfg(target_arch = "x86_64")]
        module.set_triple(&TargetTriple::create("x86_64-pc-linux-gnu"));

        let i32_type = context.i32_type();
        let main_type = i32_type.fn_type(&[], false);
        let main = module.add_function("main", main_type, None);

        let ptr_type = context.ptr_type(AddressSpace::default());
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

        let entry = context.append_basic_block(main, "entry");
        builder.position_at_end(entry);
        let ptr = i32_type.const_zero();
        let mem = builder
            .build_direct_call(
                calloc,
                &[
                    i64_type.const_int(30000, false).into(),
                    i64_type.const_int(1, false).into(),
                ],
                "mem",
            )?
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_pointer_value();

        Ok(Compiler {
            context,
            builder,
            module,
            main,
            ptr,
            mem,
            putchar,
            getchar,
        })
    }

    fn compile(&mut self, prog: Vec<Stmt>) -> Result<String> {
        self.compile_rec(prog)?;

        self.builder
            .build_return(Some(&self.context.i32_type().const_zero()))?;

        Ok(self.module.print_to_string().to_string())
    }

    fn compile_rec(&mut self, prog: Vec<Stmt>) -> Result<()> {
        let i32_type = self.context.i32_type();
        let i8_type = self.context.i8_type();
        for stmt in prog {
            match stmt {
                Stmt::PtrInc(n) => {
                    self.ptr = self.builder.build_int_add(
                        self.ptr,
                        i32_type.const_int(n as u64, true),
                        "ptr",
                    )?;
                }
                Stmt::ValInc(n) => {
                    let element_ptr =
                        unsafe { self.builder.build_gep(i8_type, self.mem, &[self.ptr], "") }?;
                    let val = self.builder.build_load(i8_type, element_ptr, "")?;
                    let val = self.builder.build_int_add(
                        val.into_int_value(),
                        i8_type.const_int(n as i8 as u64, true),
                        "",
                    )?;
                    self.builder.build_store(element_ptr, val)?;
                }
                Stmt::Loop(stmts) => {
                    let current_bb = self.builder.get_insert_block().unwrap();
                    let cond_bb = self.context.append_basic_block(self.main, "cond");
                    self.builder.build_unconditional_branch(cond_bb)?;
                    self.builder.position_at_end(cond_bb);
                    let phi = self.builder.build_phi(i32_type, "ptr")?;
                    phi.add_incoming(&[(&self.ptr, current_bb)]);
                    self.ptr = phi.as_basic_value().into_int_value();

                    let element_ptr =
                        unsafe { self.builder.build_gep(i8_type, self.mem, &[self.ptr], "") }?;
                    let val = self.builder.build_load(i8_type, element_ptr, "")?;
                    let cond = self.builder.build_int_compare(
                        IntPredicate::NE,
                        val.into_int_value(),
                        i8_type.const_zero(),
                        "",
                    )?;

                    let true_bb = self.context.append_basic_block(self.main, "t");
                    let false_bb = self.context.append_basic_block(self.main, "f");
                    self.builder
                        .build_conditional_branch(cond, true_bb, false_bb)?;
                    self.builder.position_at_end(true_bb);

                    self.compile_rec(stmts)?;

                    self.builder.build_unconditional_branch(cond_bb)?;
                    phi.add_incoming(&[(&self.ptr, self.builder.get_insert_block().unwrap())]);
                    self.builder.position_at_end(false_bb);
                    self.ptr = phi.as_basic_value().into_int_value();
                }
                Stmt::Output => {
                    let element_ptr =
                        unsafe { self.builder.build_gep(i8_type, self.mem, &[self.ptr], "") }?;
                    let val = self.builder.build_load(i8_type, element_ptr, "")?;
                    self.builder
                        .build_direct_call(self.putchar, &[val.into()], "")?;
                }
                Stmt::Input => {
                    let val = self.builder.build_direct_call(self.getchar, &[], "")?;
                    let element_ptr =
                        unsafe { self.builder.build_gep(i8_type, self.mem, &[self.ptr], "") }?;
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

fn compile(prog: Vec<Stmt>) -> Result<String> {
    Compiler::new(&Context::create())?.compile(prog)
}

pub fn main(args: Args, f: File) -> Result<()> {
    let prog = o1::compile(f)?;
    let prog = o2::compile(prog);
    let ir = compile(prog)?;
    if args.text {
        print!("{ir}");
        return Ok(());
    }
    let path = PathBuf::from_str(&args.input).unwrap();
    let ir_path = path.with_added_extension("ll");
    let exe_path = path.with_added_extension("out");
    let exe_path = if exe_path.is_relative() {
        let mut temp = PathBuf::from_str("./").unwrap();
        temp.push(&exe_path);
        temp
    } else {
        exe_path
    };
    File::create(&ir_path)?.write_all(ir.as_bytes())?;
    Command::new("clang")
        .args([
            "-o".as_ref(),
            exe_path.as_os_str(),
            "-O2".as_ref(),
            ir_path.as_os_str(),
        ])
        .status()?;
    Command::new(exe_path).status()?;
    Ok(())
}
