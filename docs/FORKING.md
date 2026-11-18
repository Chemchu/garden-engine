# Make it yours

Your garden is a repository of your own that holds only content. The engine
lives here and is published as a Docker image, so you never fork it, build it
or touch Rust.

| Repository             | Holds                                                       | Its CI                                                                           |
| ---------------------- | ----------------------------------------------------------- | -------------------------------------------------------------------------------- |
| Yours, from `starter/` | `garden/`, `site.toml`, `assets/`, `Dockerfile`, `fly.toml` | Checks content on pull requests, deploys pushes to `main`, applies site settings |
| This one (the engine)  | The server, templates, scripts and styles                   | Tests, and publishes `ghcr.io/chemchu/garden-engine` on version tags             |

Your `Dockerfile` starts `FROM` an exact engine version and copies your content
into it. Deploying builds that image and ships it to Fly.io, so what you check
in a pull request is exactly what goes live.

## Getting started

Copy the [starter kit](../starter) into a new public repository and follow its
README. It takes six steps: name the Fly.io app, create its deploy token, set
up the admin account, let the workflows open pull requests and push.

## Sections

| Section | What it shows                       | Needs                 |
| ------- | ----------------------------------- | --------------------- |
| `news`  | Latest posts                        | nothing               |
| `chess` | Lichess stats and games             | `LICHESS_*` variables |
| `music` | Spotify top artists and tracks      | `SPOTIFY_*` variables |
| `games` | Steam profile and most played games | `STEAM_*` variables   |

Set the variables with `fly secrets set` (see
[Environment variables](CONFIGURATION.md#environment-variables)) and switch the
section on from the admin page. A section that is switched on but has no
credentials stays hidden: the logs say which variable is missing and the admin
page marks it as inactive.

## The admin page

Open `https://<your-app>.fly.dev/admin` and sign in. You can change:

- the site name, its description in each language, the footer links and the
  source code link;
- the colors and the corner radius;
- the fonts, picked from [Google Fonts](https://fonts.google.com/) and
  [Fontsource](https://fontsource.org/)'s open-source collection, each one
  shown in its own typeface;
- which sections are shown.

Everything previews live on the page, and two buttons reset the styles or the
details to what is deployed. When you're happy, press **Continue on GitHub**:

1. GitHub opens with an issue already filled in. Submit it.
2. The **Site settings** workflow:
   - checks the request;
   - downloads any new fonts into `assets/fonts/`;
   - updates `site.toml`;
   - opens a pull request that lists every change.

   It comments on the issue with the link, or with what went wrong.

3. The **Check** workflow runs on that pull request. Merge it to deploy.

Only issues opened by the repository owner are processed, and nothing is
deployed until you merge.

## Writing

Posts are the Markdown files in `garden/`, and `garden/welcome.md` is the home
page. The starter's two guide notes explain the rest:

- front matter, tags and drafts;
- translations;
- wikilinks, links to headings and embeds.

Run `garden-engine check` inside your site image to catch problems before you push:

```sh
docker build -t my-garden . && docker run --rm my-garden ./garden-engine check
```

## Upgrading the engine

Change the version in the first line of your `Dockerfile`. Dependabot does it
for you with a weekly pull request when a new engine is released, and the
Check workflow runs on it. Merging deploys the new engine with your content.

## Moving an existing fork of this repository

If you forked the engine before the content moved out, you can keep your posts
and their history. The "last updated" dates come from that history. With
[git filter-repo](https://github.com/newren/git-filter-repo) installed:

```sh
git clone https://github.com/<you>/<your-fork> my-garden
cd my-garden
git filter-repo \
  --path garden --path site.toml --path static/fonts \
  --path-rename static/fonts:assets/fonts
```

Then copy in everything from `starter/` except `garden/`, `site.toml` and
`assets/`:

- the `Dockerfile`;
- `fly.toml`, with your app name and `REPO_SOURCE` pointing at the new
  repository;
- the `.github/` folder, `.gitignore` and `.dockerignore`.

Commit it and push it to a new public repository. Then:

- add `FLY_API_TOKEN` to the new repository;
- allow Actions to create pull requests;
- run `fly secrets unset REPO_SOURCE` if you had set it as a secret, since
  `fly.toml` sets it now.

The first push deploys the same site from the new repository. After that you can
delete the old fork's deploy token.
