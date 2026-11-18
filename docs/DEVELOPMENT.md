# How it works

So, what you have running right now is an [actix web](https://actix.rs/) process
serving some static HTML with a bit of JS. It spins up a
[SurrealDB](https://surrealdb.com/) in-memory instance in which we store all of
the posts (and other stuff). The posts come from the `garden/` folder of the
content directory (`CONTENT_DIR`): the starter's demo garden when you develop
here, and a content repository like
[Chemchu/garden](https://github.com/Chemchu/garden) in production. We do all
sort of operations with those documents like creating
relationships between posts, creating a full-search engine utility crate
(relying on surrealdb tbh) and more stuff I can't remember right now.

It's important to note that the surrealdb server is only accessible on
development. In production, the in-memory database is not exposed.

There is also a special file called [build.rs](../build.rs) in this project which
runs each time a file is saved. When that happens, it then recompiles all the
CSS used here. This is why the tailwind cli is needed, otherwise an error will
pop up and the styles are going to be messed up.

The look of the site isn't baked into the CSS though: colors and fonts are CSS
variables that the server fills in from `site.toml` on every page, so changing
them doesn't need a CSS rebuild.

## The admin page

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
  you submit yourself in the content repository (the one `REPO_SOURCE` points
  at). Its `site-settings` workflow runs `/app/site-settings` from the engine
  image and turns the issue into a pull request using the workflow's own token.

## Crates

| Crate                  | What it does                                              |
| ---------------------- | --------------------------------------------------------- |
| `garden-engine` (root) | The web server                                            |
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
- `crates/e2e` boots the real app (`garden_engine::create_app`) on a random port
  against the fixture garden in `crates/e2e/fixtures/garden`, with Lichess,
  Spotify, Steam and the font catalog replaced by local mock servers. No
  credentials or network access needed.
- `crates/site-settings/tests` runs the settings tool against a mocked
  Fontsource and a temporary copy of the repository layout.
- Handlers cache upstream responses process-wide, so each e2e file that talks to
  an external service keeps its scenarios in a single test function, in order.

## Releasing the engine image

Pushing a version tag publishes the engine to the GitHub Container Registry:

```sh
git tag v1.0.0
git push origin v1.0.0
```

The `Publish image` workflow builds the Dockerfile and checks the bundled content
with `garden-engine check`. Only then does it push `ghcr.io/<owner>/garden-engine` as `1.0.0`,
`1.0` and `1`. Tag a commit on `main` whose CI already passed. Use `v1.0.0` or
later: the `1` tag isn't created for `0.x` versions.

The image contains:

| Path                 | What                                                  |
| -------------------- | ----------------------------------------------------- |
| `/app/garden-engine` | The server, and `garden-engine check` for content     |
| `/app/site-settings` | The tool the settings workflow runs                   |
| `/app/static`        | Engine scripts, styles and default fonts              |
| `/app/templates`     | Page templates                                        |
| `/app/content`       | Bundled content. `CONTENT_DIR` points here by default |

A content repo builds `FROM` this image, copies its own folder into the image,
and points `CONTENT_DIR` at it.

The first publish creates a private package. Make it public once under the
package's settings on GitHub so anyone can pull it.

## The starter kit

`starter/` is the template that people copy to run their own garden. It has:

- demo posts and a `site.toml`;
- Inter and Lora in `assets/fonts`, with their licenses;
- a Dockerfile that starts `FROM` a published engine version;
- `fly.toml`;
- workflows to check, deploy and apply site settings, plus Dependabot to upgrade
  the engine.

It's tested here:

- `the_starter_kit_is_clean` runs `garden-engine check` on it and expects no problems
  and no warnings;
- `crates/e2e/tests/starter.rs` renders its posts and checks that every link
  resolves;
- the Docker test job builds the starter on top of the engine image that CI just
  built, and checks it again.

To publish it as the template repository, copy the contents of `starter/` into
a new public repository and tick "Template repository" in its settings. After
each engine release, update the version in `starter/Dockerfile` and copy the
change across.

## Hacky solutions

### Tailwind Prose

There are some css class add/remove for the main view. The "prose" css class is
needed to render the markdown post but keeping it for non-post pages kinda
breaks the site. That's why I had to add and remove the class depending on the
current url. Not the cleanest solution but still a solution soooo shut up
