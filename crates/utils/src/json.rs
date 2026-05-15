use serde::de::DeserializeOwned;

pub fn from_json<T: DeserializeOwned>(
    name: &'static str,
    json: &serde_json::Value,
) -> anyhow::Result<T> {
    serde_json::from_value(json.clone())
        .map_err(|e| anyhow::format_err!("Failed to deserialize {name}: {e}; {json}"))
}

pub fn get_field<T: DeserializeOwned>(
    json: &mut serde_json::Value,
    error_sink: &mut Vec<(String, serde_json::Error)>,
    field: &'static str,
    default: impl FnOnce() -> T,
) -> T {
    // check alias first, to work around the VS Code where it pre-fills the
    // defaults instead of sending an empty object.
    let mut pointer = field.replace('_', "/");
    pointer.insert(0, '/');
    json.pointer_mut(&pointer)
        .and_then(|it| {
            serde_json::from_value(it.take()).map_or_else(
                |e| {
                    tracing::warn!("Failed to deserialize config field at {}: {:?}", pointer, e);
                    error_sink.push((pointer, e));
                    None
                },
                Some,
            )
        })
        .unwrap_or_else(default)
}
