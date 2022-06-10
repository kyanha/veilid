use super::*;
use core::fmt::Debug;

#[instrument(level = "trace", ret, err)]
pub fn deserialize_json<'a, T: de::Deserialize<'a> + Debug>(
    arg: &'a str,
) -> Result<T, VeilidAPIError> {
    serde_json::from_str(arg).map_err(|e| VeilidAPIError::ParseError {
        message: e.to_string(),
        value: format!(
            "deserialize_json:\n---\n{}\n---\n to type {}",
            arg,
            std::any::type_name::<T>()
        ),
    })
}

#[instrument(level = "trace", ret, err)]
pub fn deserialize_opt_json<T: de::DeserializeOwned + Debug>(
    arg: Option<String>,
) -> Result<T, VeilidAPIError> {
    let arg = arg.as_ref().ok_or_else(|| VeilidAPIError::ParseError {
        message: "invalid null string".to_owned(),
        value: format!(
            "deserialize_json_opt: null to type {}",
            std::any::type_name::<T>()
        ),
    })?;
    deserialize_json(arg)
}

#[instrument(level = "trace", ret)]
pub fn serialize_json<T: Serialize + Debug>(val: T) -> String {
    match serde_json::to_string(&val) {
        Ok(v) => v,
        Err(e) => {
            panic!("failed to serialize json value: {}\nval={:?}", e, val);
        }
    }
}
