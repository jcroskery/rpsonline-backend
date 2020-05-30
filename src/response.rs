use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;

use serde_json::json;

fn message(message: &str) -> String {
    json!({ "message": message }).to_string()
}

async fn log_game(game_id: &str) {
    let game_string = String::from("hi\n");
    let mut file = OpenOptions::new().append(true).create(true).open("/var/log/olmmcc/games.log").unwrap();
    file.write_all(game_string.as_bytes()).unwrap();
}

pub async fn formulate_response(url: &str, body: HashMap<&str, &str>) -> String {
    match url {
        "/new_game" => new_game(body).await,
        _ => message(&format!("The provided url {} could not be resolved.", url))
    }
}

async fn new_game(body: HashMap<&str, &str>) -> String {
    if body["type"] == "human" {

    } else {

    }
    log_game("").await;
    String::new()
}
