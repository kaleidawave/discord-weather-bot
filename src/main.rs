use std::{collections::HashMap, fs::read_to_string};

use weather_discord_bot::get_client;

#[tokio::main]
async fn main() {
    let secrets_toml = read_to_string("./Secrets.toml").expect("Could not find 'Secrets.toml'");
    let secrets = secrets_toml
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            line.split_once('=')
                .expect(&format!("Invalid 'Secrets.toml' line '{}'", line))
        })
        .map(|(key, value)| {
            (
                key,
                value
                    .strip_prefix('"')
                    .and_then(|value| value.strip_suffix('"'))
                    .unwrap_or(value),
            )
        })
        .collect::<HashMap<_, _>>();

    let discord_token = secrets.get("DISCORD_TOKEN").unwrap();
    let weather_api_key = secrets.get("WEATHER_API_KEY").unwrap();
    let discord_guild_id = secrets.get("DISCORD_GUILD_ID").unwrap().parse().unwrap();

    let mut client = get_client(discord_token, weather_api_key, discord_guild_id).await;
    if let Err(why) = client.start().await {
        println!("Err with client: {:?}", why);
    }
}
