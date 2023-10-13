use serde::de::DeserializeOwned;

pub fn from_json<T: DeserializeOwned>(
    name: &'static str,
    json: &serde_json::Value,
) -> anyhow::Result<T> {
    serde_json::from_value(json.clone())
        .map_err(|e| anyhow::format_err!("Failed to deserialize {name}: {e}; {json}"))
}
