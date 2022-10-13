mod weather;

use anyhow::Context as _;
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::gateway::Ready;
use serenity::model::prelude::command::CommandOptionType;
use serenity::prelude::*;
use serenity::{async_trait, model::prelude::GuildId};
use shuttle_secrets::SecretStore;
use tracing::{error, info};
use weather::get_forecast;

struct Bot {
    weather_api_key: String,
    client: reqwest::Client,
    discord_guild_id: GuildId,
}

#[shuttle_service::main]
async fn serenity(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> shuttle_service::ShuttleSerenity {
    // Get the discord token set in `Secrets.toml`
    let discord_token = secret_store
        .get("DISCORD_TOKEN")
        .context("'DISCORD_TOKEN' was not found")?;

    let weather_api_key = secret_store
        .get("WEATHER_API_KEY")
        .context("'WEATHER_API_KEY' was not found")?;

    let discord_guild_id = secret_store
        .get("DISCORD_GUILD_ID")
        .context("'DISCORD_GUILD_ID' was not found")?;

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
    // Set gateway intents, which decides what events the bot will be notified about.
    // Here we don't need any intents so empty
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

        info!("Registered commands: {:#?}", commands);
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
                command => {
                    error!("Interaction made with unknown command: {}", command);
                    format!("Unknown command: {}", command)
                }
            };

            let create_interaction_response =
                command.create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(response_content))
                });

            if let Err(why) = create_interaction_response.await {
                error!("Cannot respond to slash command: {}", why);
            }
        }
    }
}
