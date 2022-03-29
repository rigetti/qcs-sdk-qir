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

use std::str::FromStr;

#[derive(Debug)]
pub enum ExecutionTarget {
    Qpu(String),
    Qvm,
}

impl Default for ExecutionTarget {
    fn default() -> Self {
        Self::Qvm
    }
}

impl FromStr for ExecutionTarget {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "qvm" => Self::Qvm,
            _ => Self::Qpu(String::from(s)),
        })
    }
}
