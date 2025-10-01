use std::fs::read;

use quil_rs::quil::Quil;
use qcs::RegisterData;
use qcs_sdk_qir::{
    output::{self, DebugOutputFormat},
    transpile_qir_to_quil,
};

#[test]
fn transpile_qir_to_quil_bell_state() {
    let data = read("tests/fixtures/programs/bell_state.bc").unwrap();
    let output = transpile_qir_to_quil(&data).unwrap();
    insta::assert_snapshot!(output.program.to_quil_or_debug());
    insta::assert_snapshot!(output.shot_count);
}

#[test]
fn capture_recorded_output_and_convert() {
    let data = read("tests/fixtures/programs/record_output.bc").unwrap();
    let output = transpile_qir_to_quil(&data).unwrap();
    insta::assert_json_snapshot!(&output.recorded_output);

    let debug_format = output::try_format::<DebugOutputFormat>(
        &RegisterData::I8(vec![vec![1, 2], vec![2, 4], vec![3, 6]]),
        &output.recorded_output,
    )
    .unwrap();
    insta::assert_snapshot!(debug_format);
}
