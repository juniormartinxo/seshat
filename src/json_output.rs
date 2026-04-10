use serde_json::{json, Map, Value};

pub fn message_ready(message: &str) {
    emit_with_fields("message_ready", [("message", json!(message))]);
}

pub fn committed(summary: &str, date: Option<&str>) {
    let mut fields = vec![("summary", json!(summary))];
    if let Some(date) = date {
        fields.push(("date", json!(date)));
    }
    emit_with_fields("committed", fields);
}

pub fn cancelled(reason: &str) {
    emit_with_fields("cancelled", [("reason", json!(reason))]);
}

pub fn error(message: &str) {
    emit_with_fields("error", [("message", json!(message))]);
}

fn emit_with_fields<I>(event: &str, fields: I)
where
    I: IntoIterator<Item = (&'static str, Value)>,
{
    let mut object = Map::new();
    object.insert("event".to_string(), json!(event));
    for (key, value) in fields {
        object.insert(key.to_string(), value);
    }
    println!("{}", Value::Object(object));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_event_fields_always_include_event_first() {
        let mut object = Map::new();
        object.insert("event".to_string(), json!("message_ready"));
        object.insert("message".to_string(), json!("feat: test"));

        assert_eq!(
            Value::Object(object).to_string(),
            r#"{"event":"message_ready","message":"feat: test"}"#
        );
    }
}
