use std::process::ExitCode;
use std::sync::Arc;

use actix_web::{HttpServer, web};
use admin_module::{AdminStore, FontCatalog};
use garden_engine::{
    content::{self, CHECK_USAGE, ContentPaths},
    controllers::{AppState, Integrations, admin_controller::AdminAuth},
    create_app,
    helpers::{self, CHESS_VARS, GAMES_VARS, MUSIC_VARS},
    settings::GithubRepo,
};
use log::info;
use search_engine::PostsSearchEngine;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("check") {
        return check(args);
    }

    match serve() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn check(args: impl Iterator<Item = String>) -> ExitCode {
    helpers::load_env_file();
    let paths = match content::check_paths(args, |var| {
        std::env::var(var).ok().filter(|value| !value.is_empty())
    }) {
        Ok(paths) => paths,
        Err(problem) => {
            eprintln!("{problem}\n{CHECK_USAGE}");
            return ExitCode::from(2);
        }
    };

    let report = content::check_content(&paths);
    print!("{}", report.render());
    if report.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[actix_web::main]
async fn serve() -> std::io::Result<()> {
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
    let paths = ContentPaths::from_env();
    let site = paths.load_site().unwrap_or_else(|e| {
        panic!(
            "Invalid site config at {}: {e}",
            paths.site_config.display()
        )
    });
    let garden_path = paths.garden_path();
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
    let search_engine = Arc::new(PostsSearchEngine::new(&repo_source, &garden_path).await);
    info!("Search engine created correctly");

    let mut app_state = AppState::new(&garden_path, &site, search_engine, integrations)
        .with_assets(paths.assets.clone());
    if let Some(assets) = &paths.assets {
        info!("Serving content assets from {}", assets.display());
    }
    info!("Active sections: {:?}", app_state.features);

    match helpers::read_admin_credentials() {
        Some(credentials) => {
            info!("Creating the admin account...");
            let store = AdminStore::new(credentials)
                .await
                .unwrap_or_else(|e| panic!("Invalid admin configuration: {e}"));
            let repository = GithubRepo::from_repo_source(&repo_source);
            if repository.is_none() {
                log::warn!("REPO_SOURCE is not a GitHub repository, settings cannot be sent");
            }
            let on_fly = std::env::var_os("FLY_MACHINE_ID").is_some();
            let admin = AdminAuth::new(store, FontCatalog::default(), repository)
                .trusting_fly_client_ip(on_fly);
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
