use std::path::Path;

use actix_files::Files;
use actix_web::{
    App, Error,
    body::MessageBody,
    dev::{ServiceFactory, ServiceRequest, ServiceResponse},
    middleware::{Compress, Logger, from_fn},
    web,
};

use crate::controllers::AppState;

pub mod content;
pub mod controllers;
pub mod helpers;
pub mod i18n;
pub use site_config;
pub use site_config::settings;

const STATIC_PREFIX: &str = "/_static";

pub fn create_app(
    app_state: web::Data<AppState>,
) -> App<
    impl ServiceFactory<
        ServiceRequest,
        Config = (),
        Response = ServiceResponse<impl MessageBody>,
        Error = Error,
        InitError = (),
    >,
> {
    let features = app_state.features;
    let lichess = app_state.integrations.lichess.clone();
    let spotify = app_state.integrations.spotify.clone();
    let steam = app_state.integrations.steam.clone();
    let admin = app_state.admin.clone();
    let static_files = static_files(app_state.assets_dir.as_deref());

    App::new()
        .wrap(from_fn(i18n::locale_headers))
        .wrap(Compress::default())
        .wrap(Logger::new("%s %r - %Dms"))
        .service(static_files)
        .app_data(app_state)
        .configure(controllers::posts_controller::configure_services)
        .configure(|cfg| {
            if features.news {
                controllers::news_controller::configure_services(cfg)
            }
        })
        .configure(controllers::metadata_controller::configure_services)
        .configure(|cfg| {
            if let Some(steam) = steam {
                controllers::steam_controller::configure_services(cfg, steam)
            }
        })
        .configure(|cfg| {
            if let Some(lichess) = lichess {
                controllers::chess_controller::configure_services(cfg, lichess)
            }
        })
        .configure(controllers::graph_controller::configure_services)
        .configure(|cfg| {
            if let Some(spotify) = spotify {
                controllers::music_controller::configure_services(cfg, spotify)
            }
        })
        .configure(controllers::tags_controller::configure_services)
        .configure(controllers::feed_controller::configure_services)
        .configure(|cfg| {
            if let Some(admin) = admin {
                controllers::admin_controller::configure_services(cfg, admin)
            }
        })
        .service(controllers::fallback_controller::fallback_route) // This service should be last one in the list because it matches any string
}

pub fn static_files(assets_dir: Option<&Path>) -> Files {
    let engine = Files::new(STATIC_PREFIX, site_config::STATIC_DIR);
    match assets_dir {
        Some(dir) => Files::new(STATIC_PREFIX, dir).default_handler(engine),
        None => engine,
    }
}
