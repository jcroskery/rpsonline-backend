use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;

use chrono::Utc;
use rand::distributions::{Alphanumeric, Distribution, Uniform};
use rand::{thread_rng, Rng};
use serde_json::json;

macro_rules! get_one_cell {
    ($table:expr, $value:expr, $where_name:expr, $where_value:expr, $type:ty) => {
        mysql::from_value::<$type>(
            mysql::get_some_like($table, $value, $where_name, $where_value).await[0][0].clone(),
        )
        .to_string()
    };
}

macro_rules! try_get_one_cell {
    ($table:expr, $value:expr, $where_name:expr, $where_value:expr, $type:ty) => {
        mysql::try_from_value::<$type>(
            mysql::get_some_like($table, $value, $where_name, $where_value).await[0][0].clone(),
        )
    };
}

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
        "/get_status_of_human_game" => get_status_of_human_game(body).await,
        "/search_for_human_game" => search_for_human_game(body).await,
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

async fn update_last_contact(game_id: &str, player: i32) {
    mysql::change_row_where(
        "games",
        "id",
        game_id,
        &format!("player_{}_time", player),
        &now(),
    )
    .await;
}

async fn check_quit_game(game_id: &str, player: i32) -> bool {
    if let Some(player_time) = try_get_one_cell!(
        "games",
        &format!("player_{}_time", player),
        "id",
        game_id,
        i64
    ) {
        if player_time + 60 < now().parse().unwrap() {
            return true;
        }
    }
    false
}

async fn search_for_human_game(body: HashMap<&str, &str>) -> String {
    if mysql::row_exists("game_users", "id", body["id"]).await {
        let games = mysql::get_some_like_null("games", "id", "player_2_id").await;
        if games.len() != 0 {
            let id: i64 = mysql::from_value(games[0][0].clone());
            let player_1_id = get_one_cell!("games", "player_1_id", "id", &id.to_string(), String);
            if player_1_id != body["id"] {
                update_last_contact(&id.to_string(), 2).await;
                mysql::change_row_where("games", "id", &id.to_string(), "player_2_id", body["id"])
                    .await;
                mysql::change_row_where("game_users", "id", body["id"], "game", &id.to_string())
                    .await;
                return json!({ "id": id }).to_string();
            }
        }
    }
    return json!( { "failed": true } ).to_string();
}

async fn quit_game(id: &str, body: HashMap<&str, &str>) -> String {
    mysql::change_row_where("games", "id", id, "status", "1").await;
    String::from("Game over")
}

async fn get_status_of_human_game(body: HashMap<&str, &str>) -> String {
    if mysql::row_exists("game_users", "id", body["id"]).await {
        let game_id = get_one_cell!("game_users", "game", "id", body["id"], i32);
        let game = &mysql::get_some_like("games", "player_1_id, status", "id", &game_id).await[0];
        if mysql::from_value::<i64>(game[1].clone()) != 1 {
            if mysql::from_value::<String>(game[0].clone()) == body["id"] {
                update_last_contact(&game_id, 1).await;
                if check_quit_game(&game_id, 2).await {
                    return quit_game(&game_id, body).await;
                }
            } else {
                update_last_contact(&game_id, 2).await;
                if check_quit_game(&game_id, 1).await {
                    return quit_game(&game_id, body).await;
                }
            }
        } else {
            return json!( {"status": 1} ).to_string();
        }
    }
    String::new()
}

async fn new_game(body: HashMap<&str, &str>) -> String {
    if mysql::row_exists("game_users", "id", body["id"]).await {
        let old_game = mysql::get_some_like("game_users", "game", "id", body["id"]).await;
        if old_game[0][0] != mysql::NULL {
            mysql::change_row_where(
                "games",
                "id",
                &mysql::from_value::<i64>(old_game[0][0].clone()).to_string(),
                "status",
                "1",
            )
            .await;
        }
        let type_of_opponent = if body["type"] == "human" { "0" } else { "1" };
        let mut id: u32;
        let mut num = 2;
        loop {
            id = Uniform::new_inclusive(1, 10u32.pow(num)).sample(&mut thread_rng());
            if !mysql::row_exists("games", "id", &id.to_string()).await {
                break;
            } else {
                num += 1;
            }
        }
        mysql::change_row_where("game_users", "id", body["id"], "game", &id.to_string()).await;
        mysql::insert_row(
            "games",
            vec![
                "id",
                "status",
                "type",
                "player_1_id",
                "player_2_id",
                "round",
                "score_1",
                "score_2",
                "player_1_time",
                "player_2_time",
                "log",
            ],
            vec![
                &id.to_string(),
                "0",
                type_of_opponent,
                body["id"],
                "",
                "1",
                "0",
                "0",
                &now(),
                "",
                "",
            ],
        )
        .await
        .unwrap();
        json!({ "id": id }).to_string()
    } else {
        String::new()
    }
}

fn now() -> String {
    Utc::now().timestamp().to_string()
}
