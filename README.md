# My digital garden project

So, this is the repo for my website. The code is pretty much self-explainatory
(most of the time) but there are some tricky stuff there (especially client side
things like using D3 and so on)

It's also built to be forked: you can run your own garden on top of it and
change its look, its name and which sections it shows from an admin page,
without touching the code.

## Make it yours

This walks you through forking the project, deploying it to
[Fly.io](https://fly.io/) and personalising it.

### 1. Fork the repository

Fork it on GitHub. Everything below happens in your fork.

### 2. Turn on GitHub Actions

Forks start with Actions disabled. In your fork:

1. Open the **Actions** tab and enable workflows.
2. Go to **Settings → Actions → General → Workflow permissions** and tick
   **Allow GitHub Actions to create and approve pull requests**. The settings
   workflow needs it to open pull requests for you.

### 3. Know which sections you'll get

| Section    | What it shows                       | Needs                 |
| ---------- | ----------------------------------- | --------------------- |
| `news`     | Latest posts                        | nothing               |
| `chess`    | Lichess stats and games             | `LICHESS_*` variables |
| `music`    | Spotify top artists and tracks      | `SPOTIFY_*` variables |
| `games`    | Steam profile and most played games | `STEAM_*` variables   |
| `schedule` | "What I'm doing now" in the header  | nothing (see below)   |

`site.toml` comes with every section turned on, and you don't need to change
that before deploying: a section whose credentials aren't set is switched off
when the server starts (the logs say which variable is missing), and the admin
page marks it as inactive. Set its variables later and restart to bring it
back, or switch it off from the admin page.

The schedule texts are mine (they live in `src/i18n/en.rs` and
`src/i18n/es.rs`), so you'll probably want to switch it off from the admin page
unless you edit them.

### 4. Create the Fly.io app

With the [flyctl](https://fly.io/docs/flyctl/install/) CLI:

```sh
fly auth login
fly apps create your-app-name
fly tokens create deploy --app your-app-name
```

Then, in your fork on GitHub:

- **Settings → Secrets and variables → Actions → Secrets**: add
  `FLY_API_TOKEN` with the token you just created.
- **Settings → Secrets and variables → Actions → Variables**: add
  `FLY_APP_NAME` with your app name. The deploy uses it instead of the `app`
  value in `fly.toml`.

If you want the app to run somewhere other than Stockholm, change
`primary_region` in `fly.toml`.

### 5. Set the runtime secrets

```sh
fly secrets set --app your-app-name \
  REPO_SOURCE="https://github.com/<you>/<your-fork>/blob/main" \
  ADMIN_USERNAME="you" \
  ADMIN_PASSWORD="a long passphrase" \
  ADMIN_SECRET_KEY="$(openssl rand -hex 32)"
```

Add the credentials of the sections you want (see
[Environment variables](#environment-variables)). Sections without them are
simply left out.

### 6. Deploy

Push anything to `main`, or re-run the latest **Build, Test, and Deploy**
workflow. Tests run first, then the site is deployed to Fly.io.

### 7. Personalise it from the admin page

Open `https://your-app-name.fly.dev/admin`, sign in, and change:

- the site name, its description in each language, the footer links and the
  source code link (they still point to me after forking)
- the colors
- the fonts: any font from [Google Fonts](https://fonts.google.com/) or from
  [Fontsource](https://fontsource.org/)'s open-source collection (Iosevka,
  Monaspace, Commit Mono, Geist and more), by name
- which sections are shown

There's a live preview on the page. When you're happy, press **Continue on
GitHub**:

1. GitHub opens with an issue already filled in. Submit it.
2. The **Site settings** workflow checks the request, downloads the fonts,
   updates `site.toml` and opens a pull request. It comments on the issue with
   the link, or with what went wrong.
3. Review and merge the pull request. Merging deploys the new version.

Only issues opened by the repository owner are processed, and nothing is
deployed until you merge.

### 8. Write your own posts

Posts are the Markdown files in [garden](garden). `garden/welcome.md` is the
landing page. Replace mine with yours.

### Keeping your fork up to date

Use **Sync fork** on GitHub. If it conflicts on `site.toml` or the `garden`
folder, keep your version: those are your settings and your posts.

## Environment variables

| Variable                                                                                 | Needed when       | Purpose                                                                                                                              |
| ---------------------------------------------------------------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `REPO_SOURCE`                                                                            | always            | `https://github.com/<you>/<repo>/blob/main`. Used for the "view source" link of each post and to know where to open settings issues. |
| `ADMIN_USERNAME`, `ADMIN_PASSWORD`, `ADMIN_SECRET_KEY`                                   | you want `/admin` | Set all three or none. The key must be at least 32 bytes (`openssl rand -hex 32`). Without them `/admin` doesn't exist.              |
| `LICHESS_API_TOKEN`, `LICHESS_USERNAME`                                                  | to show `chess`   | Lichess account to show.                                                                                                             |
| `SPOTIFY_USER_ID`, `SPOTIFY_CLIENT_ID`, `SPOTIFY_CLIENT_SECRET`, `SPOTIFY_REFRESH_TOKEN` | to show `music`   | Spotify app and account. `get_spotify_refresh_token.sh` helps you get the refresh token.                                             |
| `STEAM_API_KEY`, `STEAM_ID`                                                              | to show `games`   | Steam Web API key and the account to show.                                                                                           |
| `SITE_CONFIG_PATH`                                                                       | optional          | Where `site.toml` is. Defaults to `./site.toml`.                                                                                     |

Locally, put them in a `.env` file (see `.env.example`).

## site.toml

This is the file the admin page edits through pull requests. You can also edit
it by hand. The server checks it when it starts and refuses to run with an
invalid value, so a typo can't break the live site silently.

```toml
[site]
name = "My digital garden"
source_url = "https://github.com/<you>/<your-fork>"

[site.description]
en = "My digital garden"
es = "Mi jardín digital"

[[site.links]]
label = "GITHUB"
url = "https://github.com/<you>"

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
schedule = false
```

Font files live in `static/fonts/<font>/`. The settings workflow downloads them
for you and removes the folders that are no longer used. The pull request lists
every file with its size and warns when one is over 300 KB: Google fonts come
split by alphabet and are small, but some open-source fonts (Iosevka, for
example) come as a single file of about 1 MB per weight.

## Prerequesites

In order to use or even run this project locally you would either need
TailwindCSS cli installed (through NPM or whatever) or Nix (the package manager)
and direnv installed.

The easiest way is to have Tailwind installed through NPM. The cooler way is to
use nix flakes and direnv to setup the dev environment automatically. Either
way, once that is there you can just

```sh
yourcooldesktop@DESKTOP:~$ bacon dev
```

And that's it my friend. The server is up and running. The site is accesible in
[http://localhost:3000](http://localhost:3000/)

## How it works

So, what you have running right now is an [actix web](https://actix.rs/) process
serving some static HTML with a bit of JS. It spins up a
[SurrealDB](https://surrealdb.com/) in-memory instance in which we store all of
the posts (and other stuff). The posts are inside the [/garden](garden)
directory. We do all sort of operations with those documents like creating
relationships between posts, creating a full-search engine utility crate
(relying on surrealdb tbh) and more stuff I can't remember right now.

It's important to note that the surrealdb server is only accessible on
development. In production, the in-memory database is not exposed.

There is also a special file called [build.rs](build.rs) in this project which
runs each time a file is saved. When that happens, it then recompiles all the
CSS used here. This is why the tailwind cli is needed, otherwise an error will
pop up and the styles are going to be messed up.

The look of the site isn't baked into the CSS though: colors and fonts are CSS
variables that the server fills in from `site.toml` on every page, so changing
them doesn't need a CSS rebuild.

### The admin page

- The admin account lives in its own in-memory SurrealDB. The password is
  stored as an Argon2id hash with a secret pepper and the username as a keyed
  SHA-256 digest, both derived from `ADMIN_SECRET_KEY`.
- Sessions use an encrypted, `HttpOnly`, `SameSite=Strict` cookie that expires
  after 4 hours. Cross-site requests are refused.
- Each client gets 5 sign-in attempts every 15 minutes, counted before the
  password is checked, so parallel requests can't get past the limit. After 50
  failed attempts overall, every sign-in is slowed down by 3 seconds instead of
  blocked, so nobody can lock you out. At most 2 password checks run at once.
- On Fly.io, clients are told apart by the `Fly-Client-IP` header Fly's proxy
  sets. Anywhere else the header is ignored and the connection's address is
  used, so it can't be faked.
- The site never holds a GitHub token. Settings travel as a GitHub issue that
  you submit yourself, and the `site-settings` workflow turns it into a pull
  request using the workflow's own token.

### Crates

| Crate                  | What it does                                              |
| ---------------------- | --------------------------------------------------------- |
| `guslee` (root)        | The web server                                            |
| `crates/search-engine` | Loads the garden into SurrealDB, search, links and graph  |
| `crates/site-config`   | `site.toml` and settings requests: parsing and validation |
| `crates/site-settings` | The tool the settings workflow runs to apply a request    |
| `crates/admin-module`  | Admin account, sign-in throttling and the font catalog    |
| `crates/chess-module`  | Lichess client                                            |
| `crates/music-module`  | Spotify client                                            |
| `crates/games-module`  | Steam client                                              |
| `crates/e2e`           | End-to-end tests                                          |

## Testing

```sh
cargo test --workspace   # everything
cargo test -p e2e        # end-to-end only
```

- Unit tests live next to the code in each crate. The search engine also has
  integration tests (`crates/search-engine/tests`) that run against an embedded
  in-memory SurrealDB, so no `surreal` server is needed.
- `crates/e2e` boots the real app (`guslee::create_app`) on a random port
  against the fixture garden in `crates/e2e/fixtures/garden`, with Lichess,
  Spotify, Steam and the font catalog replaced by local mock servers. No
  credentials or network access needed.
- `crates/site-settings/tests` runs the settings tool against a mocked
  Fontsource and a temporary copy of the repository layout.
- Handlers cache upstream responses process-wide, so each e2e file that talks to
  an external service keeps its scenarios in a single test function, in order.

## Hacky solutions

### Tailwind Prose

There are some css class add/remove for the main view. The "prose" css class is
needed to render the markdown post but keeping it for non-post pages kinda
breaks the site. That's why I had to add and remove the class depending on the
current url. Not the cleanest solution but still a solution soooo shut up
