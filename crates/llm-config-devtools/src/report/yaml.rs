//! YAML report generator.

use crate::error::Result;
use crate::security::SecurityReport;

/// Generate a YAML report.
///
/// # Errors
/// Returns an error when the report cannot be serialized as YAML.
pub fn generate(report: &SecurityReport) -> Result<String> {
    let yaml = serde_yaml::to_string(report)?;
    Ok(yaml)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_yaml_generation() {
        let report = SecurityReport::new(PathBuf::from("."), vec![]);
        let yaml = generate(&report).unwrap();
        assert!(yaml.contains("timestamp"));
        assert!(yaml.contains("findings"));
    }
}
