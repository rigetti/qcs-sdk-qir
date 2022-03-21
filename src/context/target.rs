use std::str::FromStr;

#[derive(Debug)]
pub(crate) enum ExecutionTarget {
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
