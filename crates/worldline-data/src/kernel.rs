//! A reader for SPICE text kernels, NASA NAIF's format for planetary
//! constants.
//!
//! Only text between `\begindata` and `\begintext` is data; everything else
//! is commentary. Data is a list of assignments, `NAME = value` or
//! `NAME = ( value value … )`, which may span lines. Numbers may use
//! Fortran-style `D` exponents.

use std::collections::HashMap;

use crate::ParseError;

/// Parses a text kernel into its numeric assignments.
pub fn parse(text: &str) -> Result<HashMap<String, Vec<f64>>, ParseError> {
    let mut data = String::new();
    let mut in_data = false;
    for line in text.lines() {
        match line.trim() {
            "\\begindata" => in_data = true,
            "\\begintext" => in_data = false,
            _ if in_data => {
                data.push_str(line);
                data.push('\n');
            }
            _ => {}
        }
    }

    let spaced = data
        .replace('(', " ( ")
        .replace(')', " ) ")
        .replace('=', " = ")
        .replace(',', " ");
    let mut tokens = spaced.split_whitespace();
    let mut values = HashMap::new();
    while let Some(name) = tokens.next() {
        if tokens.next() != Some("=") {
            return Err(ParseError::new(0, format!("expected `=` after `{name}`")));
        }
        let list = match tokens.next() {
            Some("(") => {
                let mut list = Vec::new();
                loop {
                    match tokens.next() {
                        Some(")") => break,
                        Some(token) => list.push(number(token)?),
                        None => {
                            return Err(ParseError::new(0, format!("unclosed `(` in `{name}`")));
                        }
                    }
                }
                list
            }
            Some(token) => vec![number(token)?],
            None => return Err(ParseError::new(0, format!("missing value for `{name}`"))),
        };
        values.insert(name.to_string(), list);
    }
    Ok(values)
}

fn number(token: &str) -> Result<f64, ParseError> {
    token
        .replace(['D', 'd'], "E")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| ParseError::new(0, format!("`{token}` is not a finite number")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_data_blocks() {
        let text = "\
BODY1_X = ( 9 9 )   this is commentary and must be ignored
\\begindata
   BODY399_PM = (  190.147  360.9856235
                     -1.4D-12 )
   BODY4_MAX_PHASE_DEGREE = 2
   BODY3_ANGLES = ( +125.045 -1935.5364525000 )
\\begintext
BODY399_PM = ( 1 2 3 )
";
        let values = parse(text).unwrap();
        assert_eq!(values["BODY399_PM"], [190.147, 360.985_623_5, -1.4e-12]);
        assert_eq!(values["BODY4_MAX_PHASE_DEGREE"], [2.0]);
        assert_eq!(values["BODY3_ANGLES"], [125.045, -1_935.536_452_5]);
        assert!(!values.contains_key("BODY1_X"));
    }

    #[test]
    fn reports_malformed_data() {
        assert!(parse("\\begindata\nBODY1 = ( 1 2\n\\begintext\n").is_err());
        assert!(parse("\\begindata\nBODY1 = ( 1 x )\n").is_err());
    }
}
