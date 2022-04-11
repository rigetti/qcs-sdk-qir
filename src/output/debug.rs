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

use qcs::ExecutionResult;

/// Formats output of QIR programs in a debug-friendly structure.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Default)]
pub struct DebugOutputFormat(pub Vec<String>);

impl OutputFormat for DebugOutputFormat {
    /// Create an [`DebugOutputFormat`].
    /// Will return [`enum@Error`] if something about the [`ExecutionResult`] or [`RecordedOutput`] is
    /// unsupported, or if any of the result's data is indexed out-of-range.
    ///
    /// # Errors
    ///
    /// See [`enum@Error`].
    fn try_new(result: &ExecutionResult, mapping: &[RecordedOutput]) -> Result<Self, Error> {
        let mut output = vec![];
        match result {
            ExecutionResult::I8(shots_results) => {
                let mut shot_idx = 0;
                for recorded_output in mapping {
                    let shot_id = shot_idx + 1;
                    match recorded_output {
                        RecordedOutput::ShotStart => {
                            output.push(format!("[shot:{} start]", shot_id));
                        }
                        RecordedOutput::ShotEnd => {
                            output.push(format!("[shot:{} end]", shot_id));
                            shot_idx += 1;
                        }
                        RecordedOutput::ReadoutOffset(index) => {
                            // TODO: determine if we need the upstream `index` to be 64-bit.
                            #[allow(clippy::cast_possible_truncation)]
                            let index = *index as usize;
                            if let Some(result) = shots_results
                                .get(shot_idx)
                                .and_then(|shot_results| shot_results.get(index))
                            {
                                output.push(format!("[shot:{} result {}]", shot_id, result));
                            } else {
                                return Err(Error::NoShotDataAtIndex(shot_id, index));
                            }
                        }
                        RecordedOutput::TupleStart => {
                            output.push(format!("[shot:{} tuple_start]", shot_id));
                        }
                        RecordedOutput::TupleEnd => {
                            output.push(format!("[shot:{} tuple_end]", shot_id));
                        }
                        RecordedOutput::ArrayStart => {
                            output.push(format!("[shot:{} array_start]", shot_id));
                        }
                        RecordedOutput::ArrayEnd => {
                            output.push(format!("[shot:{} array_end]", shot_id));
                        }
                    }
                }
                Ok(Self(output))
            }
            ExecutionResult::Complex32(..)
            | ExecutionResult::F64(..)
            | ExecutionResult::I16(..) => {
                Err(Error::UnimplementedResultType(format!("{:?}", result)))
            }
        }
    }
}

impl From<DebugOutputFormat> for String {
    fn from(output: DebugOutputFormat) -> Self {
        output.0.join("\n")
    }
}

#[test]
fn test_execution_result_debug_output() {
    let execution_result = ExecutionResult::I8(vec![
        vec![1, 2, 3],
        vec![10, 20, 30, 40],
        vec![11, 22, 33, 44, 55],
    ]);
    let mapping = [
        RecordedOutput::ShotStart,
        RecordedOutput::ReadoutOffset(0),
        RecordedOutput::ReadoutOffset(1),
        RecordedOutput::ReadoutOffset(2),
        RecordedOutput::ShotEnd,
        RecordedOutput::ShotStart,
        RecordedOutput::ReadoutOffset(0),
        RecordedOutput::ReadoutOffset(1),
        RecordedOutput::ReadoutOffset(2),
        RecordedOutput::ReadoutOffset(3),
        RecordedOutput::ShotEnd,
        RecordedOutput::ShotStart,
        RecordedOutput::ReadoutOffset(0),
        RecordedOutput::ReadoutOffset(1),
        RecordedOutput::ReadoutOffset(2),
        RecordedOutput::ReadoutOffset(3),
        RecordedOutput::ReadoutOffset(4),
        RecordedOutput::ShotEnd,
    ];

    let output = DebugOutputFormat::try_new(&execution_result, &mapping).unwrap();
    assert_eq!(output.0.len(), 18);

    const EXPECTED_OUTPUT: &str = r#"
[shot:1 start]
[shot:1 result 1]
[shot:1 result 2]
[shot:1 result 3]
[shot:1 end]
[shot:2 start]
[shot:2 result 10]
[shot:2 result 20]
[shot:2 result 30]
[shot:2 result 40]
[shot:2 end]
[shot:3 start]
[shot:3 result 11]
[shot:3 result 22]
[shot:3 result 33]
[shot:3 result 44]
[shot:3 result 55]
[shot:3 end]
"#;

    assert_eq!(
        super::try_format::<DebugOutputFormat>(&execution_result, &mapping).unwrap(),
        EXPECTED_OUTPUT.trim()
    )
}

#[test]
fn test_out_of_range_debug_output() {
    // use misaligned result data with mapping data to trigger `NoShotDataAtIndex` error
    let execution_result = ExecutionResult::I8(vec![vec![1, 2, 3], vec![10, 20, 30]]);
    let mapping = [
        RecordedOutput::ShotStart,
        RecordedOutput::ReadoutOffset(0),
        RecordedOutput::ReadoutOffset(1),
        RecordedOutput::ReadoutOffset(2),
        RecordedOutput::ShotEnd,
        RecordedOutput::ShotStart,
        RecordedOutput::ReadoutOffset(0),
        RecordedOutput::ReadoutOffset(1),
        RecordedOutput::ReadoutOffset(2),
        RecordedOutput::ReadoutOffset(3),
        RecordedOutput::ShotEnd,
        RecordedOutput::ShotStart,
        RecordedOutput::ReadoutOffset(0),
        RecordedOutput::ReadoutOffset(1),
        RecordedOutput::ReadoutOffset(2),
        RecordedOutput::ReadoutOffset(3),
        RecordedOutput::ReadoutOffset(4),
        RecordedOutput::ShotEnd,
    ];

    let try_output = DebugOutputFormat::try_new(&execution_result, &mapping);
    if let Some(Error::NoShotDataAtIndex(shot_id, index)) = try_output.err() {
        assert_eq!(shot_id, 2);
        assert_eq!(index, 3)
    } else {
        assert!(false);
    }
}