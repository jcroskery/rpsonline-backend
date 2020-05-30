use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;

use rand::distributions::{Alphanumeric, Uniform, Distribution};
use rand::{thread_rng, Rng};
use serde_json::json;

fn message(message: &str) -> String {
    json!({ "message": message }).to_string()
}

async fn log_game(game_id: &str) {
    let game_string = String::from("hi\n");
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("/var/log/olmmcc/games.log")
        .unwrap();
    file.write_all(game_string.as_bytes()).unwrap();
}

pub async fn formulate_response(url: &str, body: HashMap<&str, &str>) -> String {
    match url {
        "/new_game" => new_game(body).await,
        "/new_id" => new_id().await,
        _ => message(&format!("The provided url {} could not be resolved.", url)),
    }
}

async fn new_id() -> String {
    let mut id: String;
    loop {
        id = thread_rng().sample_iter(&Alphanumeric).take(32).collect();
        if !mysql::row_exists("game_users", "id", &id).await {
            break;
        }
    }
    mysql::insert_row("game_users", vec!["id", "game"], vec![&id, ""])
        .await
        .ok();
    json!({ "id": id }).to_string()
}

async fn new_game(body: HashMap<&str, &str>) -> String {
    let mut type_of_opponent = if body["type"] == "human" {
        "0"
    } else {
        "1"
    };
    let mut id: u32;
    let mut num = 2;
    loop {
        id = Uniform::new_inclusive(1, 10u32.pow(num)).sample(&mut thread_rng());
        if !mysql::row_exists("game_users", "id", &id.to_string()).await {
            break;
        } else {
            num += 1;
        }
    }
    json!({ "id": id }).to_string()
}
