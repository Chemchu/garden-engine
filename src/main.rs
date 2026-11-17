use std::sync::Arc;

use actix_web::{HttpServer, web};
use admin_module::AdminStore;
use guslee::{
    controllers::{AppState, Integrations, admin_controller::AdminAuth},
    create_app,
    helpers::{self, CHESS_VARS, GAMES_VARS, MUSIC_VARS},
    site_config::SiteConfig,
};
use log::info;
use search_engine::PostsSearchEngine;

const GARDEN_PATH: &str = "./garden";

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info) // Only show INFO and above (no DEBUG/TRACE)
        .with_module_level("actix_server", log::LevelFilter::Warn) // Only warnings/errors from actix_server
        .with_module_level("actix_web", log::LevelFilter::Info) // Keep actix_web info (for HTTP logs)
        .init()
        .unwrap();

    info!("Starting server...");

    info!("Loading environment variables...");
    helpers::load_env_file();

    info!("Loading site configuration...");
    let site = SiteConfig::from_env();
    let configured = site.features;
    let repo_source = helpers::read_required_var("REPO_SOURCE");

    info!("Initializing integrations...");
    let integrations = Integrations {
        lichess: helpers::read_section_credentials("chess", configured.chess, &CHESS_VARS).map(
            |vars| {
                web::Data::new(chess_module::LichessState::new(
                    vars["LICHESS_API_TOKEN"].clone(),
                    vars["LICHESS_USERNAME"].clone(),
                ))
            },
        ),
        spotify: helpers::read_section_credentials("music", configured.music, &MUSIC_VARS).map(
            |vars| {
                web::Data::new(tokio::sync::Mutex::new(
                    music_module::SpotifyState::from_refresh_token(
                        vars["SPOTIFY_USER_ID"].clone(),
                        vars["SPOTIFY_CLIENT_ID"].clone(),
                        vars["SPOTIFY_CLIENT_SECRET"].clone(),
                        vars["SPOTIFY_REFRESH_TOKEN"].clone(),
                    ),
                ))
            },
        ),
        steam: helpers::read_section_credentials("games", configured.games, &GAMES_VARS).map(
            |vars| {
                web::Data::new(games_module::SteamState::new(
                    vars["STEAM_API_KEY"].clone(),
                    vars["STEAM_ID"].clone(),
                ))
            },
        ),
    };

    info!("Creating in-memory full-text search engine...");
    let search_engine = Arc::new(PostsSearchEngine::new(&repo_source, GARDEN_PATH).await);
    info!("Search engine created correctly");

    let mut app_state = AppState::new(GARDEN_PATH, &site, search_engine, integrations);
    info!("Active sections: {:?}", app_state.features);

    match helpers::read_admin_credentials() {
        Some(credentials) => {
            info!("Creating the admin account...");
            let store = AdminStore::new(credentials)
                .await
                .unwrap_or_else(|e| panic!("Invalid admin configuration: {e}"));
            let on_fly = std::env::var_os("FLY_MACHINE_ID").is_some();
            let admin = AdminAuth::new(store).trusting_fly_client_ip(on_fly);
            app_state = app_state.with_admin(admin);
        }
        None => info!("ADMIN_* variables are not set, the admin area is disabled"),
    }

    let app_state = web::Data::new(app_state);

    info!("Server starting on port 3000");

    HttpServer::new(move || create_app(app_state.clone()))
        .bind(("0.0.0.0", 3000))?
        .run()
        .await
}
