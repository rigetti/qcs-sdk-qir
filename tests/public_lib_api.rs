use qcs_sdk_qir::transpile_qir_to_quil;
use std::fs::read;

#[test]
fn transpile_qir_to_quil_bell_state() {
    let data = read("tests/fixtures/programs/bell_state.bc").unwrap();
    let output = transpile_qir_to_quil(&data).unwrap();
    insta::assert_snapshot!(output.program.to_string(true));
    insta::assert_display_snapshot!(output.shot_count);
}
