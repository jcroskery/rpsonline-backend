use std::collections::HashMap;

use serde_json::json;

fn message(message: &str) -> String {
    json!({ "message": message }).to_string()
}

pub async fn formulate_response(url: &str, body: HashMap<&str, &str>) -> String {
    match url {
        _ => message(&format!("The provided url {} could not be resolved.", url))
    }
}