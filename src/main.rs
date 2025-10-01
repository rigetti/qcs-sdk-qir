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

use std::{path::PathBuf, str::FromStr};

use clap::{Parser, ValueEnum};
use eyre::{Report, Result};

use qcs_sdk_qir::{ExecutionTarget, PatchOptions};

#[cfg(not(feature = "serde_support"))]
use quil_rs::quil::Quil;

#[derive(Parser, Debug)]
#[clap(
    name = "QCS SDK QIR Command Line Tool",
    about = "Transform & translate QIR programs to target Rigetti systems."
)]
enum QcsQirCli {
    #[clap(
        name = "transform",
        about = "Given an LLVM bitcode file, replace quantum intrinsics with calls to execute equivalent Quil on Rigetti QCS"
    )]
    Transform {
        #[clap(long, default_value = "shot-count")]
        format: QirFormat,

        llvm_bitcode_path: PathBuf,

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
    #[clap(
        name = "transpile-to-quil",
        about = "Given an LLVM bitcode file, output the equivalent Quil program"
    )]
    TranspileToQuil {
        #[clap(long, default_value = "shot-count")]
        format: QirFormat,

        llvm_bitcode_path: PathBuf,
    },
}

#[derive(Debug, Copy, Clone, ValueEnum)]
enum QirFormat {
    ShotCount,
    Unitary,
}

impl FromStr for QirFormat {
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "shot-count" => Ok(QirFormat::ShotCount),
            "unitary" => Ok(QirFormat::Unitary),
            _ => Err(eyre::eyre!("unrecognized QIR format")),
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let opt = QcsQirCli::parse();
    match opt {
        QcsQirCli::Transform {
            format,
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
            let module = match format {
                QirFormat::ShotCount => {
                    qcs_sdk_qir::patch_qir_with_qcs(options, &bitcode, &context)?
                }
                QirFormat::Unitary => {
                    qcs_sdk_qir::patch_unitary_qir_with_qcs(options, &bitcode, &context)?
                }
            };
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
        QcsQirCli::TranspileToQuil {
            format,
            llvm_bitcode_path,
        } => {
            let data = std::fs::read(llvm_bitcode_path)?;

            match format {
                QirFormat::ShotCount => {
                    let output = qcs_sdk_qir::transpile_qir_to_quil(&data)?;

                    #[cfg(feature = "serde_support")]
                    println!("{}", serde_json::to_string_pretty(&output)?);

                    #[cfg(not(feature = "serde_support"))]
                    {
                        println!("shot count: {}\n", output.shot_count);
                        println!("quil:\n{}", output.program.to_quil()?);
                        println!("recorded output:\n{:#?}", output.recorded_output);
                    }
                }
                QirFormat::Unitary => {
                    let output = qcs_sdk_qir::transpile_unitary_qir_to_quil(&data)?;

                    #[cfg(feature = "serde_support")]
                    println!("{}", serde_json::to_string_pretty(&output)?);

                    #[cfg(not(feature = "serde_support"))]
                    {
                        println!("quil:\n{}", output.program.to_quil()?);
                        println!("recorded output:\n{:#?}", output.recorded_output);
                    }
                }
            }

            Ok(())
        }
    }
}
