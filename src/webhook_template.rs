use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{error, info, warn};

/// Webhook template engine for payload transformation.
/// Supports variable interpolation and basic transformations.
#[derive(Debug, Clone)]
pub struct WebhookTemplate {
    /// The template string - supports {{variable}} syntax
    pub template: String,
}

impl WebhookTemplate {
    /// Create a new template
    pub fn new(template: String) -> Self {
        Self { template }
    }

    /// Validate template syntax
    pub fn validate(&self) -> Result<(), String> {
        // Check for balanced braces
        let open_count = self.template.matches("{{").count();
        let close_count = self.template.matches("}}").count();

        if open_count != close_count {
            return Err("Unbalanced template braces".to_string());
        }

        // Check for valid variable references
        let re = regex::Regex::new(r"\{\{([a-zA-Z0-9_\.]+)\}\}").map_err(|e| e.to_string())?;

        for cap in re.captures_iter(&self.template) {
            let var_path = &cap[1];
            // Validate that the path contains only valid characters
            if !var_path.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') {
                return Err(format!("Invalid variable reference: {{{{{}}}}}", var_path));
            }
        }

        Ok(())
    }

    /// Transform event data using the template
    pub fn transform(&self, event: &Value) -> Result<Value, String> {
        // Get all variables from the event
        let context = extract_context(event)?;
        interpolate_template(&self.template, &context)
    }

    /// Transform to a pretty-printed JSON string
    pub fn transform_to_string(&self, event: &Value) -> Result<String, String> {
        let result = self.transform(event)?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }
}

/// Extract context from the event for variable interpolation
fn extract_context(event: &Value) -> Result<HashMap<String, String>, String> {
    let mut context = HashMap::new();

    // Flatten the event JSON into dot-notation keys
    flatten_json("", event, &mut context);

    Ok(context)
}

/// Recursively flatten JSON into dot-notation keys
fn flatten_json(prefix: &str, value: &Value, context: &mut HashMap<String, String>) {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                let new_key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };
                flatten_json(&new_key, val, context);
            }
        }
        Value::Array(arr) => {
            for (idx, val) in arr.iter().enumerate() {
                let new_key = format!("{}[{}]", prefix, idx);
                flatten_json(&new_key, val, context);
            }
        }
        Value::String(s) => {
            context.insert(prefix.to_string(), s.clone());
        }
        Value::Number(n) => {
            context.insert(prefix.to_string(), n.to_string());
        }
        Value::Bool(b) => {
            context.insert(prefix.to_string(), b.to_string());
        }
        Value::Null => {
            context.insert(prefix.to_string(), "null".to_string());
        }
    }
}

/// Interpolate template variables with context values
fn interpolate_template(template: &str, context: &HashMap<String, String>) -> Result<Value, String> {
    let re = regex::Regex::new(r"\{\{([a-zA-Z0-9_\.\[\]]+)\}\}").map_err(|e| e.to_string())?;

    let mut result = template.to_string();

    for cap in re.captures_iter(template) {
        let var_path = &cap[1];
        let value = context.get(var_path).cloned().unwrap_or_else(|| "".to_string());
        let placeholder = format!("{{{{{}}}}}", var_path);
        result = result.replace(&placeholder, &value);
    }

    // Try to parse as JSON first (for complex payloads), otherwise return as string
    serde_json::from_str(&result)
        .or_else(|_| {
            // If it fails, wrap in a JSON object with a result field
            Ok(json!({ "result": result }))
        })
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_validation() {
        let template = WebhookTemplate::new("{{event_data}}".to_string());
        assert!(template.validate().is_ok());

        let bad_template = WebhookTemplate::new("{{event_data}".to_string());
        assert!(bad_template.validate().is_err());
    }

    #[test]
    fn test_template_transform() {
        let template = WebhookTemplate::new(r#"{"contract":"{{contract_id}}","type":"{{event_type}}"}"#.to_string());
        let event = json!({
            "contract_id": "CA123",
            "event_type": "contract"
        });

        let result = template.transform(&event);
        assert!(result.is_ok());
    }

    #[test]
    fn test_flatten_json() {
        let mut context = HashMap::new();
        let event = json!({
            "contract_id": "CA123",
            "nested": {
                "field": "value"
            }
        });

        flatten_json("", &event, &mut context);

        assert_eq!(context.get("contract_id").map(|s| s.as_str()), Some("CA123"));
        assert_eq!(context.get("nested.field").map(|s| s.as_str()), Some("value"));
    }
}
