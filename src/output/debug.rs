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

use super::{Error, OutputFormat};
use crate::RecordedOutput;

use qcs::RegisterData;

/// Formats output of QIR programs in a debug-friendly structure.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Default)]
pub struct DebugOutputFormat(pub Vec<String>);

impl OutputFormat for DebugOutputFormat {
    /// Create an [`DebugOutputFormat`].
    /// Will return [`enum@Error`] if something about the [`RegisterData`] or [`RecordedOutput`] is
    /// unsupported, or if any of the result's data is indexed out-of-range.
    ///
    /// # Arguments
    ///
    /// - `result`: The output returned from the [`qcs`] crate.
    /// - `mapping`: The [`crate::ProgramOutput::recorded_output`] field from a call to [`crate::transpile_qir_to_quil`]
    ///
    /// # Errors
    ///
    /// See [`enum@Error`].
    fn try_new(result: &RegisterData, mapping: &[RecordedOutput]) -> Result<Self, Error> {
        let mut output = vec![];
        match result {
            RegisterData::I8(shots_results) => {
                for (shot_idx, shot) in shots_results.iter().enumerate() {
                    for recorded_output in mapping {
                        let shot_id = shot_idx + 1;
                        match recorded_output {
                            RecordedOutput::ShotStart => {
                                output.push(format!("[shot:{shot_id} start]"));
                            }
                            RecordedOutput::ShotEnd => {
                                output.push(format!("[shot:{shot_id} end]"));
                                break;
                            }
                            RecordedOutput::ResultReadoutOffset(index) => {
                                #[allow(clippy::cast_possible_truncation)]
                                let index = *index as usize;
                                if let Some(result) = shot.get(index) {
                                    output.push(format!("[shot:{shot_id} result {result}]"));
                                } else {
                                    return Err(Error::NoShotDataAtIndex(shot_id, index));
                                }
                            }
                            RecordedOutput::BoolReadoutOffset(..)
                            | RecordedOutput::IntegerReadoutOffset(..)
                            | RecordedOutput::DoubleReadoutOffset(..) => {
                                return Err(Error::UnimplementedRecordType(format!(
                                    "{recorded_output:?}"
                                )))
                            }
                            RecordedOutput::TupleStart => {
                                output.push(format!("[shot:{shot_id} tuple_start]"));
                            }
                            RecordedOutput::TupleEnd => {
                                output.push(format!("[shot:{shot_id} tuple_end]"));
                            }
                            RecordedOutput::ArrayStart => {
                                output.push(format!("[shot:{shot_id} array_start]"));
                            }
                            RecordedOutput::ArrayEnd => {
                                output.push(format!("[shot:{shot_id} array_end]"));
                            }
                        }
                    }
                }
                Ok(Self(output))
            }
            RegisterData::Complex32(..) | RegisterData::F64(..) | RegisterData::I16(..) => {
                Err(Error::UnimplementedResultType(format!("{result:?}")))
            }
        }
    }
}

impl std::fmt::Display for DebugOutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.join("\n"))
    }
}

#[test]
fn test_execution_result_debug_output() {
    const EXPECTED_OUTPUT: &str = r"
[shot:1 start]
[shot:1 result 1]
[shot:1 result 2]
[shot:1 result 3]
[shot:1 end]
[shot:2 start]
[shot:2 result 10]
[shot:2 result 20]
[shot:2 result 30]
[shot:2 end]
[shot:3 start]
[shot:3 result 11]
[shot:3 result 22]
[shot:3 result 33]
[shot:3 end]
";

    let execution_result =
        RegisterData::I8(vec![vec![1, 2, 3], vec![10, 20, 30], vec![11, 22, 33]]);
    let mapping = [
        RecordedOutput::ShotStart,
        RecordedOutput::ResultReadoutOffset(0),
        RecordedOutput::ResultReadoutOffset(1),
        RecordedOutput::ResultReadoutOffset(2),
        RecordedOutput::ShotEnd,
    ];

    let output = DebugOutputFormat::try_new(&execution_result, &mapping).unwrap();
    assert_eq!(output.0.len(), 15);

    assert_eq!(
        super::try_format::<DebugOutputFormat>(&execution_result, &mapping).unwrap(),
        EXPECTED_OUTPUT.trim()
    );
}

#[test]
fn test_out_of_range_debug_output() {
    // use misaligned result data with mapping data to trigger `NoShotDataAtIndex` error
    let execution_result = RegisterData::I8(vec![vec![1, 2, 3], vec![10, 20]]);
    let mapping = [
        RecordedOutput::ShotStart,
        RecordedOutput::ResultReadoutOffset(0),
        RecordedOutput::ResultReadoutOffset(1),
        RecordedOutput::ResultReadoutOffset(2),
        RecordedOutput::ShotEnd,
    ];

    let try_output = DebugOutputFormat::try_new(&execution_result, &mapping);
    if let Some(Error::NoShotDataAtIndex(shot_id, index)) = try_output.err() {
        assert_eq!(shot_id, 2);
        assert_eq!(index, 2);
    } else {
        panic!();
    }
}
