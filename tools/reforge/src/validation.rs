use super::*;

pub(crate) fn validate_config(value: &toml::Value) -> Result<()> {
    Config::parse_toml(&toml::to_string(value)?)?;
    Ok(())
}
