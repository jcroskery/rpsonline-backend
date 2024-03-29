use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;

use self::Outcomes::*;
use chrono::Utc;
use rand::distributions::{Alphanumeric, Distribution, Uniform};
use rand::{thread_rng, Rng};
use serde_json::json;

#[derive(Debug, PartialEq)]
enum Outcomes {
    WIN1,
    WIN2,
    TIE,
    WAITING,
    UNDEFINED,
}

impl Outcomes {
    fn get_outcome(global_move: &str) -> Self {
        if global_move == "0" || global_move == "5" || global_move == "a" {
            TIE
        } else if global_move == "1" || global_move == "6" || global_move == "8" {
            WIN1
        } else if global_move == "2" || global_move == "4" || global_move == "9" {
            WIN2
        } else if global_move == " " {
            UNDEFINED
        } else {
            WAITING
        }
    }
}

fn get_global_move(player_1_move: &str, player_2_move: &str) -> String {
    format!(
        "{:x}",
        (player_2_move.parse::<i32>().unwrap() * 4) + player_1_move.parse::<i32>().unwrap()
    )
}

fn get_player_1_move(global_move: &str) -> &str {
    if global_move == "0" || global_move == "4" || global_move == "8" || global_move == "c" {
        "0"
    } else if global_move == "1" || global_move == "5" || global_move == "9" || global_move == "d" {
        "1"
    } else if global_move == "2" || global_move == "6" || global_move == "a" || global_move == "e" {
        "2"
    } else {
        "3"
    }
}

fn get_player_2_move(global_move: &str) -> &str {
    if global_move == "0" || global_move == "1" || global_move == "2" || global_move == "3" {
        "0"
    } else if global_move == "4" || global_move == "5" || global_move == "6" || global_move == "7" {
        "1"
    } else if global_move == "8" || global_move == "9" || global_move == "a" || global_move == "b" {
        "2"
    } else {
        "3"
    }
}

fn update_global_move(
    global_move: &str,
    player_1_move: Option<&str>,
    player_2_move: Option<&str>,
) -> String {
    let mut new_player_1_move = get_player_1_move(global_move);
    let mut new_player_2_move = get_player_2_move(global_move);
    if let Some(player_move) = player_1_move {
        new_player_1_move = player_move;
    }
    if let Some(player_move) = player_2_move {
        new_player_2_move = player_move;
    }
    get_global_move(new_player_1_move, new_player_2_move)
}

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
    let game_string = format!("{}\n", get_one_cell!("games", "log", "id", game_id, String));
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
        "/get_status_of_game" => get_status_of_game(body).await,
        "/search_for_human_game" => search_for_human_game(body).await,
        "/make_move" => make_move(body).await,
        _ => message(&format!("The provided url {} could not be resolved.", url)),
    }
}

async fn get_scoring_info(id: &str) -> (i32, i32) {
    let info = &mysql::get_some_like("games", "score_1, score_2", "id", id).await[0];
    (
        mysql::from_value(info[0].clone()),
        mysql::from_value(info[1].clone()),
    )
}

async fn new_id() -> String {
    let mut id: String;
    loop {
        id = thread_rng().sample_iter(&Alphanumeric).take(32).map(|u| u as char).collect();
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
        if player_time + 30 < now().parse().unwrap() {
            return true;
        }
    }
    false
}

async fn search_for_human_game(body: HashMap<&str, &str>) -> String {
    if mysql::row_exists("game_users", "id", body["id"]).await {
        let games = mysql::get_some_like_null("games", "id", "player_2_id").await;
        if games.len() != 0 {
            for game in games {
                let id: i64 = mysql::from_value(game[0].clone());
                let player_1_id =
                    get_one_cell!("games", "player_1_id", "id", &id.to_string(), String);
                if player_1_id != body["id"] && !check_quit_game(&id.to_string(), 1).await {
                    update_last_contact(&id.to_string(), 2).await;
                    mysql::change_row_where(
                        "games",
                        "id",
                        &id.to_string(),
                        "player_2_id",
                        body["id"],
                    )
                    .await;
                    mysql::change_row_where(
                        "game_users",
                        "id",
                        body["id"],
                        "game",
                        &id.to_string(),
                    )
                    .await;
                    return json!({ "success": true, "id": id }).to_string();
                }
            }
        }
    }
    return json!( { "success": false } ).to_string();
}

async fn quit_game(id: &str) {
    mysql::change_row_where("games", "id", id, "status", "1").await;
    log_game(id).await;
}

async fn get_status(id: &str) -> Option<i32> {
    if mysql::row_exists("game_users", "id", id).await {
        let game_id = get_one_cell!("game_users", "game", "id", id, i32);
        if get_one_cell!("games", "status", "id", &game_id, i64) != "1" {
            if get_player_number(&game_id, &id).await == 1 {
                update_last_contact(&game_id, 1).await;
                if check_quit_game(&game_id, 2).await {
                    return Some(1);
                }
            } else {
                update_last_contact(&game_id, 2).await;
                if check_quit_game(&game_id, 1).await {
                    return Some(1);
                }
            }
            return Some(0);
        } else {
            return Some(1);
        }
    }
    None
}

async fn get_current_move(game_id: &str) -> String {
    let global_move = get_one_cell!("games", "log", "id", game_id, String);
    global_move.chars().rev().take(1).collect()
}

async fn get_status_of_game(body: HashMap<&str, &str>) -> String {
    if let Some(status) = get_status(body["id"]).await {
        let game_id = get_one_cell!("game_users", "game", "id", body["id"], i32);
        let player_number = get_player_number(&game_id, body["id"]).await;
        if player_number == 1 {
            let null_games = mysql::get_some_like_null("games", "id", "player_2_id").await;
            for game in null_games {
                if game_id == mysql::from_value::<i64>(game[0].clone()).to_string()
                    && get_one_cell!("games", "type", "id", &game_id, i64) == "0"
                {
                    return json!({"opponent_found": false}).to_string();
                }
            }
        }
        let global_move = get_current_move(&game_id).await;
        if get_one_cell!("games", "score_1", "id", &game_id, i64) == "3"
            || "3" == get_one_cell!("games", "score_2", "id", &game_id, i64)
        {
            let opponent_move = if player_number == 1 {
                get_player_2_move(&global_move)
            } else {
                get_player_1_move(&global_move)
            };
            let your_move = if player_number == 1 {
                get_player_1_move(&global_move)
            } else {
                get_player_2_move(&global_move)
            };
            quit_game(&game_id).await;
            let round = get_one_cell!("games", "round", "id", &game_id, i64);
            return json!({"opponent_found": true, "status": 2, "waiting": 0, "opponent_move": opponent_move, "your_move": your_move, "round": round}).to_string();
        }
        if (player_number == 1 && get_player_1_move(&global_move) == "3")
            || (player_number == 2 && get_player_2_move(&global_move) == "3")
        {
            json!({ "opponent_found": true, "status": status, "waiting": 0}).to_string()
        } else if Outcomes::get_outcome(&global_move) == WAITING {
            let your_move = if player_number == 1 {
                get_player_1_move(&global_move)
            } else {
                get_player_2_move(&global_move)
            };
            json!({ "opponent_found": true, "status": status, "waiting": 1, "your_move": your_move})
                .to_string()
        } else {
            let opponent_move = if player_number == 1 {
                get_player_2_move(&global_move)
            } else {
                get_player_1_move(&global_move)
            };
            let your_move = if player_number == 1 {
                get_player_1_move(&global_move)
            } else {
                get_player_2_move(&global_move)
            };
            let round = get_one_cell!("games", "round", "id", &game_id, i64);
            json!({ "opponent_found": true, "status": status, "waiting": 0, "opponent_move": opponent_move, "your_move": your_move, "round": round}).to_string()
        }
    } else {
        json!({"success": false}).to_string()
    }
}

async fn get_player_number(game_id: &str, user_id: &str) -> i32 {
    if user_id == get_one_cell!("games", "player_1_id", "id", game_id, String) {
        1
    } else {
        2
    }
}

async fn store_global_move(game_id: &str, global_move: &str) {
    let mut global_moves = get_one_cell!("games", "log", "id", &game_id, String);
    global_moves = global_moves.chars().take(global_moves.len() - 1).collect();
    mysql::change_row_where(
        "games",
        "id",
        game_id,
        "log",
        &format!("{}{}", global_moves, global_move),
    )
    .await;
}
async fn add_new_move(game_id: &str) {
    mysql::change_row_where(
        "games",
        "id",
        game_id,
        "log",
        &format!("{}f", get_one_cell!("games", "log", "id", &game_id, String)),
    )
    .await;
}

async fn make_move(body: HashMap<&str, &str>) -> String {
    let status = get_status(body["id"]).await;
    if status == Some(0) {
        let game_id = get_one_cell!("game_users", "game", "id", body["id"], i32);
        let player_number = get_player_number(&game_id, body["id"]).await;
        let mut scoring_info = get_scoring_info(&game_id).await;
        let outcome = Outcomes::get_outcome(&get_current_move(&game_id).await);
        if outcome != WAITING && outcome != UNDEFINED {
            add_new_move(&game_id).await;
            let round: i32 = get_one_cell!("games", "round", "id", &game_id, i64)
                .parse()
                .unwrap();
            mysql::change_row_where("games", "id", &game_id, "round", &(round + 1).to_string())
                .await;
        }
        let mut global_move = if player_number == 1 {
            update_global_move(&get_current_move(&game_id).await, Some(body["move"]), None)
        } else {
            update_global_move(&get_current_move(&game_id).await, None, Some(body["move"]))
        };
        if get_one_cell!("games", "type", "id", &game_id, i64) == "1" {
            global_move = update_global_move(
                &global_move,
                None,
                Some(&rand::thread_rng().gen_range(0..3).to_string()),
            );
        }
        match Outcomes::get_outcome(&global_move) {
            WIN1 => {
                scoring_info.0 += 1;
            }
            WIN2 => {
                scoring_info.1 += 1;
            }
            WAITING => {
                store_global_move(&game_id, &global_move).await;
                return json!({}).to_string();
            }
            _ => {}
        }
        mysql::change_row_where(
            "games",
            "id",
            &game_id,
            "score_1",
            &scoring_info.0.to_string(),
        )
        .await;
        mysql::change_row_where(
            "games",
            "id",
            &game_id,
            "score_2",
            &scoring_info.1.to_string(),
        )
        .await;
        store_global_move(&game_id, &global_move).await;
    }
    json!({}).to_string()
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
                "score_1",
                "score_2",
                "player_1_time",
                "player_2_time",
                "log",
                "round",
            ],
            vec![
                &id.to_string(),
                "0",
                type_of_opponent,
                body["id"],
                "",
                "0",
                "0",
                &now(),
                "",
                " ",
                "1",
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
