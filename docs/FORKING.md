# Make it yours

This walks you through forking the project, deploying it to
[Fly.io](https://fly.io/) and personalising it.

## 1. Fork the repository

Fork it on GitHub. Everything below happens in your fork.

## 2. Turn on GitHub Actions

Forks start with Actions disabled. In your fork:

1. Open the **Actions** tab and enable workflows.
2. Go to **Settings → Actions → General → Workflow permissions** and tick
   **Allow GitHub Actions to create and approve pull requests**. The settings
   workflow needs it to open pull requests for you.

## 3. Know which sections you'll get

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

## 4. Create the Fly.io app

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

## 5. Set the runtime secrets

```sh
fly secrets set --app your-app-name \
  REPO_SOURCE="https://github.com/<you>/<your-fork>/blob/main" \
  ADMIN_USERNAME="you" \
  ADMIN_PASSWORD="a long passphrase" \
  ADMIN_SECRET_KEY="$(openssl rand -hex 32)"
```

Add the credentials of the sections you want (see
[Environment variables](CONFIGURATION.md#environment-variables)). Sections
without them are simply left out.

## 6. Deploy

Push anything to `main`, or re-run the latest **Build, Test, and Deploy**
workflow. Tests run first, then the site is deployed to Fly.io.

## 7. Personalise it from the admin page

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

## 8. Write your own posts

Posts are the Markdown files in [garden](../garden). `garden/welcome.md` is the
landing page. Replace mine with yours.

## Keeping your fork up to date

Use **Sync fork** on GitHub. If it conflicts on `site.toml` or the `garden`
folder, keep your version: those are your settings and your posts.
