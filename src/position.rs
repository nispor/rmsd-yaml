// SPDX-License-Identifier: Apache-2.0

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct RmsdPosition {
    line: usize,
    column: usize,
}

impl std::fmt::Display for RmsdPosition {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> Result<(), std::fmt::Error> {
        write!(f, "line {} column {}", self.line, self.column)
    }
}

impl From<&str> for RmsdPosition {
    fn from(s: &str) -> Self {
        // It is OK to unwrap as we are sure the regex express is valid
        let re = regex::Regex::new("^line ([0-9]+) column ([0-9]+)").unwrap();
        if let Some(captures) = re.captures(s) {
            if let (Some(line_str), Some(column_str)) = (
                captures.get(1).map(|c| c.as_str()),
                captures.get(2).map(|c| c.as_str()),
            ) {
                if let (Ok(line), Ok(column)) =
                    (line_str.parse::<usize>(), column_str.parse::<usize>())
                {
                    return Self { line, column };
                }
            }
        }

        log::debug!(
            "Invalid string for RmsdPosition: {s}, \
             should be like 'line 92 column 28'"
        );

        Self::default()
    }
}
