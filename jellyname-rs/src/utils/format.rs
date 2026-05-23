use std::collections::HashMap;
use std::path::PathBuf;

use color_eyre::eyre::Result;

pub fn apply_format(template: &str, ctx: &HashMap<String, String>) -> Result<PathBuf> {
    let formatted = strfmt::strfmt(template, ctx)?;
    Ok(PathBuf::from(formatted))
}
