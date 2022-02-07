use serde::*;

pub fn deserialize_json<'a, T: de::Deserialize<'a>>(
    arg: &'a str,
) -> Result<T, veilid_core::VeilidAPIError> {
    serde_json::from_str(arg).map_err(|e| veilid_core::VeilidAPIError::ParseError {
        message: e.to_string(),
        value: String::new(),
    })
}

pub fn deserialize_opt_json<T: de::DeserializeOwned>(
    arg: Option<String>,
) -> Result<T, veilid_core::VeilidAPIError> {
    let arg = arg.ok_or_else(|| veilid_core::VeilidAPIError::ParseError {
        message: "invalid null string passed to rust".to_owned(),
        value: String::new(),
    })?;
    deserialize_json(&arg)
}

pub fn serialize_json<T: Serialize>(val: T) -> String {
    serde_json::to_string(&val).expect("failed to serialize json value")
}
