use std::collections::HashMap;

pub const CHESS_VARS: [&str; 2] = ["LICHESS_API_TOKEN", "LICHESS_USERNAME"];
pub const MUSIC_VARS: [&str; 4] = [
    "SPOTIFY_CLIENT_ID",
    "SPOTIFY_CLIENT_SECRET",
    "SPOTIFY_USER_ID",
    "SPOTIFY_REFRESH_TOKEN",
];
pub const GAMES_VARS: [&str; 2] = ["STEAM_API_KEY", "STEAM_ID"];

pub fn load_env_file() {
    match dotenvy::dotenv() {
        Ok(path) => log::info!("Loaded environment variables from {}", path.display()),
        Err(e) if e.not_found() => log::info!("No .env file found, using system environment"),
        Err(e) => panic!("Failed to parse .env file: {}", e),
    }
}

fn env_lookup(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|value| !value.is_empty())
}

pub fn read_required_var(var: &'static str) -> String {
    env_lookup(var).unwrap_or_else(|| panic!("Missing environment variable: {var}"))
}

pub fn read_section_credentials(
    section: &str,
    enabled: bool,
    vars: &[&'static str],
) -> Option<HashMap<String, String>> {
    section_credentials(enabled, vars, env_lookup).unwrap_or_else(|missing_vars| {
        log::warn!(
            "The {section} section is on in site.toml but {} {} not set, so it is switched off",
            missing_vars.join(", "),
            if missing_vars.len() == 1 { "is" } else { "are" }
        );
        None
    })
}

fn section_credentials(
    enabled: bool,
    vars: &[&'static str],
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<Option<HashMap<String, String>>, Vec<&'static str>> {
    if enabled {
        collect_required_vars(vars, lookup).map(Some)
    } else {
        Ok(None)
    }
}

fn collect_required_vars(
    required: &[&'static str],
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<HashMap<String, String>, Vec<&'static str>> {
    let mut vars = HashMap::new();
    let mut missing_vars = Vec::new();

    for &var in required {
        match lookup(var).filter(|value| !value.is_empty()) {
            Some(value) => {
                vars.insert(var.to_string(), value);
            }
            None => missing_vars.push(var),
        }
    }

    if missing_vars.is_empty() {
        Ok(vars)
    } else {
        Err(missing_vars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_that_are_off_need_no_credentials() {
        assert_eq!(section_credentials(false, &CHESS_VARS, |_| None), Ok(None));
    }

    #[test]
    fn sections_that_are_on_read_their_credentials() {
        let vars = section_credentials(true, &GAMES_VARS, |var| Some(format!("{var}-value")))
            .unwrap()
            .unwrap();

        assert_eq!(vars.len(), 2);
        assert_eq!(vars["STEAM_ID"], "STEAM_ID-value");
    }

    #[test]
    fn sections_report_every_missing_or_empty_credential() {
        let missing = section_credentials(true, &MUSIC_VARS, |var| match var {
            "SPOTIFY_CLIENT_ID" => Some("id".to_string()),
            "SPOTIFY_USER_ID" => Some(String::new()),
            _ => None,
        })
        .unwrap_err();

        assert_eq!(
            missing,
            vec![
                "SPOTIFY_CLIENT_SECRET",
                "SPOTIFY_USER_ID",
                "SPOTIFY_REFRESH_TOKEN"
            ]
        );
    }
}
