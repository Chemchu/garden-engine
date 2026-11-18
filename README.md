# guslee

A digital garden engine. Point it at a folder of Markdown notes and a
`site.toml`, and it serves them as a site:

- wikilinks, backlinks and a graph of how notes connect;
- full-text search, tags and translations;
- an admin page that changes the look of the site through pull requests.

It runs my own garden, whose posts live in
[Chemchu/garden](https://github.com/Chemchu/garden).

## I want my own garden

You don't need to fork this repository or install Rust. Copy the
[starter kit](starter) into a repository of your own and follow its README:

- your repository holds your posts, your `site.toml` and your fonts;
- the engine comes as a published Docker image;
- every push to `main` deploys to Fly.io.

[Make it yours](docs/FORKING.md) goes further. It covers what each section
needs, how the admin page works, how to upgrade the engine, and how to move an
existing fork of this repository to a content repository.

## I want to change the engine

You need either the TailwindCSS CLI (through npm or whatever) or Nix with
direnv, which sets up the dev environment automatically. Copy `.env.example` to
`.env`: it points `CONTENT_DIR` at the starter's demo garden. Point it at a
checkout of your own content repository to work on real posts. Then:

```sh
bacon dev
```

The site is up at [http://localhost:3000](http://localhost:3000/).

## Docs

- [Make it yours](docs/FORKING.md): running your own garden from the starter
  kit
- [Configuration](docs/CONFIGURATION.md): environment variables, `site.toml`
  and `guslee check`
- [How it works](docs/DEVELOPMENT.md): architecture, the admin page, crates,
  tests, releases and the starter kit
