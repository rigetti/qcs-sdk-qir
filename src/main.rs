#![deny(clippy::pedantic)]

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

use clap::Parser;
use eyre::Result;

use qcs_sdk_qir::{ExecutionTarget, PatchOptions};

#[derive(Parser, Debug)]
#[structopt(name = "QIRQuilTranslator", about = "Translate QIR to Quil")]
enum QcsQirCli {
    #[clap(
        name = "transform",
        about = "Given an LLVM bitcode file, replace quantum intrinsics with calls to execute equivalent Quil on Rigetti QCS"
    )]
    Transform {
        llvm_bitcode_path: PathBuf,

        #[clap(parse(from_os_str))]
        bitcode_out: Option<PathBuf>,

        #[clap(long)]
        add_main_entrypoint: bool,

        #[clap(
            name = "target",
            long,
            default_value = "qvm",
            help = "QPU ID to target for execution, or \"qvm\" to target a generic device on the Quil QVM"
        )]
        execution_target: ExecutionTarget,

        #[clap(long)]
        cache_executables: bool,

        #[clap(long)]
        quil_rewiring_pragma: Option<String>,
    },
    #[structopt(
        name = "transpile-to-quil",
        about = "Given an LLVM bitcode file, output the equivalent Quil program"
    )]
    TranspileToQuil { llvm_bitcode_path: PathBuf },
}

fn main() -> Result<()> {
    env_logger::init();

    let opt = QcsQirCli::parse();
    match opt {
        QcsQirCli::Transform {
            add_main_entrypoint,
            llvm_bitcode_path,
            bitcode_out,
            execution_target,
            cache_executables,
            quil_rewiring_pragma,
        } => {
            let bitcode = std::fs::read(llvm_bitcode_path)?;
            let options = PatchOptions {
                add_main_entrypoint,
                execution_target,
                cache_executables,
                quil_rewiring_pragma,
            };
            let context = inkwell::context::Context::create();
            let module = qcs_sdk_qir::patch_qir_with_qcs(options, &bitcode, &context)?;
            match bitcode_out {
                Some(path) => {
                    module.write_bitcode_to_path(&path);
                }
                None => {
                    module.print_to_stderr();
                }
            }
            Ok(())
        }
        QcsQirCli::TranspileToQuil { llvm_bitcode_path } => {
            let data = std::fs::read(llvm_bitcode_path)?;
            let output = qcs_sdk_qir::transpile_qir_to_quil(&data)?;
            println!(
                "shot count: {}\nprogram: {}\n",
                output.shot_count,
                output.program.to_string(true)
            );
            Ok(())
        }
    }
}
