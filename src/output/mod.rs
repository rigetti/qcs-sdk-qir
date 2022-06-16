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

pub mod debug;
pub use debug::DebugOutputFormat;

use std::fmt::Display;

use crate::RecordedOutput;

use qcs::RegisterData;
use thiserror::Error;

/// All errors that may be returned from [`try_format`].
#[derive(Error, Debug)]
pub enum Error {
    /// The [`RegisterData`] is a type that is not implemented.
    #[error("the execution result type `{0}` is unimplemented")]
    UnimplementedResultType(String),

    /// The [`RecordedOutput`] is a type that is not implemented.
    #[error("the record type `{0}` is unimplemented")]
    UnimplementedRecordType(String),

    /// Encountered when [`RegisterData`] data was indexed out-of-range.
    #[error("No data was available in the `RegisterData` for shot ID {0} at index {1}")]
    NoShotDataAtIndex(usize, usize),
}

#[allow(clippy::module_name_repetitions)]
/// An [`OutputFormat`] describes the behavior required to translate QCS [`RegisterData`] values
/// into an environment-specific output format.
pub trait OutputFormat: Display {
    /// While some [`RecordedOutput`] and [`RegisterData`] variants may be unimplemented for
    /// various output formats, this provides an interface that can fail. Once all variants can be
    /// implemented, a `new` function can be added and this deprecated.
    ///
    /// # Errors
    ///
    /// See [`enum@Error`].
    fn try_new(result: &RegisterData, mapping: &[RecordedOutput]) -> Result<Self, Error>
    where
        Self: Sized;
}

/// A generic function over `F`: [`OutputFormat`], which attempts to format program output based on
/// the `&RegisterData` and `&[RecordedOutput]` provided. Caller must specify the concrete
/// implementation of the `OutputFormat`, e.g. using `DebugOutputFormat` in this crate.
///
/// While some [`RecordedOutput`] and [`RegisterData`] variants may be unimplemented for various
/// output formats, this provides an interface that can fail. Once all variants can be
/// impelemented, a `format` function can be added and this deprecated.
///
/// # Errors
///
/// See `Error`.
///
/// ```
/// pub use qcs_sdk_qir::output::{try_format, DebugOutputFormat, Error};
/// use qcs::RegisterData;
/// use qcs_sdk_qir::RecordedOutput;
///
/// fn format_output() -> Result<String, Error> {
///     // in practice, `result` and `mapping` would be provided to you from other QCS SDK
///     // function calls, not constructed manually as done here for demonstration purposes.
///     let result = &RegisterData::I8(vec![vec![1]]);
///     let mapping: &[RecordedOutput] = &[
///         RecordedOutput::ShotStart, RecordedOutput::ResultReadoutOffset(0), RecordedOutput::ShotEnd
///     ];
///
///     let output = try_format::<DebugOutputFormat>(result, mapping)?;
///     assert_eq!(output.lines().count(), 3);
///     Ok(output)
/// }
/// ```
pub fn try_format<F>(result: &RegisterData, mapping: &[RecordedOutput]) -> Result<String, Error>
where
    F: OutputFormat,
{
    F::try_new(result, mapping).map(|output| output.to_string())
}
