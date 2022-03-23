// Copyright 2022 Rigetti Computing
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::path::PathBuf;

use crate::context::QCSCompilerContext;
use crate::transform::shot_count_block;
use context::context::ContextOptions;
use context::target::ExecutionTarget;
use structopt::StructOpt;

mod context;
mod interop;
mod transform;

#[derive(StructOpt, Debug)]
#[structopt(name = "QIRQuilTranslator", about = "Translate QIR to Quil")]
enum QcsQirCli {
    #[structopt(
        name = "transform",
        about = "Given an LLVM bitcode file, replace quantum intrinsics with calls to execute equivalent Quil on Rigetti QCS"
    )]
    Transform {
        llvm_bitcode_path: PathBuf,

        #[structopt(parse(from_os_str))]
        bitcode_out: Option<PathBuf>,

        #[structopt(long)]
        add_main_entrypoint: bool,

        #[structopt(
            name = "target",
            long,
            default_value = "qvm",
            help = "QPU ID to target for execution, or \"qvm\" to target a generic device on the Quil QVM"
        )]
        execution_target: ExecutionTarget,

        #[structopt(long)]
        cache_executables: bool,

        #[structopt(long)]
        quil_rewiring_pragma: Option<String>,
    },
    TranspileToQuil {
        llvm_bitcode_path: PathBuf,
    },
}

fn main() {
    env_logger::init();

    let opt = QcsQirCli::from_args();
    match opt {
        QcsQirCli::Transform {
            add_main_entrypoint,
            llvm_bitcode_path,
            bitcode_out,
            execution_target,
            cache_executables,
            quil_rewiring_pragma,
        } => {
            let base_context = inkwell::context::Context::create();
            let context_options = ContextOptions {
                cache_executables,
                rewiring_pragma: quil_rewiring_pragma,
            };

            let mut context = QCSCompilerContext::new_from_file(
                &base_context,
                "qcs",
                llvm_bitcode_path
                    .to_str()
                    .expect("provided LLVM bitcode path is not valid"),
                execution_target,
                context_options,
            );

            shot_count_block::qir::transpile_module(&mut context).expect("transformation failed");

            if add_main_entrypoint {
                crate::interop::entrypoint::add_main_entrypoint(&mut context);
            }

            match bitcode_out {
                Some(path) => {
                    context.module.write_bitcode_to_path(&path);
                }
                None => {
                    context.module.print_to_stderr();
                }
            }
        }
        QcsQirCli::TranspileToQuil { llvm_bitcode_path } => {
            let base_context = inkwell::context::Context::create();
            let mut context = QCSCompilerContext::new_from_file(
                &base_context,
                "qcs",
                llvm_bitcode_path
                    .to_str()
                    .expect("provided LLVM bitcode path is not valid"),
                ExecutionTarget::Qvm, // TODO: make this optional
                ContextOptions::default(),
            );
            let output = shot_count_block::quil::transpile_module(&mut context)
                .expect("transpilation failed");
            println!(
                "shot count: {}\nprogram: {}\n",
                output.shot_count,
                output.program.to_string(true)
            );
        }
    }
}
