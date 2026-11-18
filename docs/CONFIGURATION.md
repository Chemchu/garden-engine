# Configuration

## Environment variables

| Variable                                                                                 | Needed when       | Purpose                                                                                                                                                                         |
| ---------------------------------------------------------------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `REPO_SOURCE`                                                                            | always            | `https://github.com/<you>/<repo>/blob/main`. The repo that holds your `garden/` folder. Used for the "view source" link of each post and to know where to open settings issues. |
| `ADMIN_USERNAME`, `ADMIN_PASSWORD`, `ADMIN_SECRET_KEY`                                   | you want `/admin` | Set all three or none. The key must be at least 32 bytes (`openssl rand -hex 32`). Without them `/admin` doesn't exist.                                                         |
| `LICHESS_API_TOKEN`, `LICHESS_USERNAME`                                                  | to show `chess`   | Lichess account to show.                                                                                                                                                        |
| `SPOTIFY_USER_ID`, `SPOTIFY_CLIENT_ID`, `SPOTIFY_CLIENT_SECRET`, `SPOTIFY_REFRESH_TOKEN` | to show `music`   | Spotify app and account. `get_spotify_refresh_token.sh` helps you get the refresh token.                                                                                        |
| `STEAM_API_KEY`, `STEAM_ID`                                                              | to show `games`   | Steam Web API key and the account to show.                                                                                                                                      |
| `CONTENT_DIR`                                                                            | optional          | The folder that holds `garden/`, `site.toml` and, optionally, `assets/`. Defaults to `.`.                                                                                       |
| `ASSETS_DIR`                                                                             | optional          | Your own static files (fonts, favicon), served under `/_static/` before the engine's. Defaults to `<CONTENT_DIR>/assets` if it exists.                                          |
| `SITE_CONFIG_PATH`                                                                       | optional          | Where `site.toml` is. Defaults to `<CONTENT_DIR>/site.toml`.                                                                                                                    |

Locally, put them in a `.env` file (see `.env.example`).

## site.toml

It lives at the root of your content repository, next to `garden/`. The admin
page edits it through pull requests, and you can also edit it by hand. The server checks it when it starts and refuses to run with an
invalid value, so a typo can't break the live site silently.

```toml
[site]
name = "My digital garden"
source_url = "https://github.com/Chemchu/garden-engine"

[site.description]
en = "My digital garden"
es = "Mi jardín digital"

[[site.links]]
label = "GITHUB"
url = "https://github.com/<you>"

[theme]
radius = 6

[theme.colors]
background = "#1e242c"
bright = "#dbdfe5"
primary = "#f58a07"
secondary = "#3f826d"
accent = "#bc6c25"
shade = "#323c49"

[theme.fonts.body]
family = "Inter"
fallback = "sans-serif"

[[theme.fonts.body.faces]]
src = "fonts/inter/latin-600-normal.woff2"
weight = 600

[theme.fonts.heading]
family = "Lora"
fallback = "serif"

[[theme.fonts.heading.faces]]
src = "fonts/lora/latin-700-normal.woff2"
weight = 700

[features]
news = true
chess = false
music = false
games = false
```

Font files live in `fonts/<font>/` inside your assets folder, or in the
engine's `static/` folder. The settings workflow downloads them
for you and removes the folders that are no longer used. The pull request lists
every file with its size and warns when one is over 300 KB: Google fonts come
split by alphabet and are small, but some open-source fonts (Iosevka, for
example) come as a single file of about 1 MB per weight.

## Checking your content

`garden-engine check` validates a content folder without starting the server:

```sh
cargo run -- check                     # the current CONTENT_DIR
cargo run -- check ../my-garden        # another folder
cargo run -- check ../my-garden --assets ../my-garden/assets
```

It reports, as errors:

- an invalid `site.toml` or a font file that doesn't exist;
- posts whose front matter is missing or invalid, which the site skips.

It reports, as warnings:

- empty notes;
- links and wikilinks that don't match any published post.

It exits with 1 when there are errors, so CI can run it on every pull request.
