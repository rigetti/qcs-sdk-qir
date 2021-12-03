use std::str::FromStr;

#[derive(Debug)]
pub(crate) enum ExecutionTarget {
    QPU(String),
    QVM,
}

impl Default for ExecutionTarget {
    fn default() -> Self {
        Self::QVM
    }
}

impl FromStr for ExecutionTarget {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "qvm" => Self::QVM,
            _ => Self::QPU(String::from(s)),
        })
    }
}
