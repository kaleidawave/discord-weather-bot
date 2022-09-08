mod weather;
use serenity::model::prelude::command::CommandOptionType;
use weather::get_forecast;

use log::info;
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use serenity::{async_trait, model::prelude::GuildId};
use shuttle_service::error::CustomError;
use shuttle_service::SecretStore;
use sqlx::PgPool;

struct Bot {
    weather_api_key: String,
    client: reqwest::Client,
    discord_guild_id: GuildId,
}

#[shuttle_service::main]
async fn serenity(#[shared::Postgres] pool: PgPool) -> shuttle_service::ShuttleSerenity {
    // Get the discord token set in `Secrets.toml` from the shared Postgres database
    let discord_token = pool
        .get_secret("DISCORD_TOKEN")
        .await
        .map_err(CustomError::new)?;

    let weather_api_key = pool
        .get_secret("WEATHER_API_KEY")
        .await
        .map_err(CustomError::new)?;

    let discord_guild_id = pool
        .get_secret("DISCORD_GUILD_ID")
        .await
        .map_err(CustomError::new)?;

    let client = get_client(
        &discord_token,
        &weather_api_key,
        discord_guild_id.parse().unwrap(),
    )
    .await;

    Ok(client)
}

pub async fn get_client(
    discord_token: &str,
    weather_api_key: &str,
    discord_guild_id: u64,
) -> Client {
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::empty();

    Client::builder(discord_token, intents)
        .event_handler(Bot {
            weather_api_key: weather_api_key.to_owned(),
            client: reqwest::Client::new(),
            discord_guild_id: GuildId(discord_guild_id),
        })
        .await
        .expect("Err creating client")
}

const OPTION_NAME: &str = "place";

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let commands =
            GuildId::set_application_commands(&self.discord_guild_id, &ctx.http, |commands| {
                commands
                    .create_application_command(|command| {
                        command.name("hello").description("Say hello")
                    })
                    .create_application_command(|command| {
                        command
                            .name("weather")
                            .description("Display the weather")
                            .create_option(|option| {
                                option
                                    .name(OPTION_NAME)
                                    .description("City to lookup forecast")
                                    .kind(CommandOptionType::String)
                                    .required(true)
                            })
                    })
            })
            .await
            .unwrap();

        info!("{:#?}", commands);
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let response_content = match command.data.name.as_str() {
                "hello" => "hello".to_string(),
                "weather" => {
                    let argument = command
                        .data
                        .options
                        .iter()
                        .find(|opt| opt.name == OPTION_NAME)
                        .cloned();

                    let value = argument.unwrap().value.unwrap();
                    let place = value.as_str().unwrap();

                    match get_forecast(place, &self.weather_api_key, &self.client).await {
                        Ok((location, forecast)) => {
                            format!("Forecast: {} in {}", forecast.headline.overview, location)
                        }
                        Err(err) => {
                            format!("Err: {}", err)
                        }
                    }
                }
                command => unreachable!("Unknown command: {}", command),
            };

            let create_interaction_response =
                command.create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(response_content))
                });

            if let Err(why) = create_interaction_response.await {
                println!("Cannot respond to slash command: {}", why);
            }
        }
    }
}
