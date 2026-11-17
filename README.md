# My digital garden project

So, this is the repo for my website. The code is pretty much self-explainatory
(most of the time) but there are some tricky stuff there (especially client side
things like using D3 and so on)

It's also built to be forked: you can run your own garden on top of it and
change its look, its name and which sections it shows from an admin page,
without touching the code.

## Running it locally

You need either the TailwindCSS cli (through NPM or whatever) or Nix with
direnv, which sets up the dev environment automatically. Then:

```sh
bacon dev
```

The site is up at [http://localhost:3000](http://localhost:3000/).

## Docs

- [Make it yours](docs/FORKING.md): fork, deploy to Fly.io and personalise it
- [Configuration](docs/CONFIGURATION.md): environment variables and `site.toml`
- [How it works](docs/DEVELOPMENT.md): architecture, the admin page, crates and
  tests
