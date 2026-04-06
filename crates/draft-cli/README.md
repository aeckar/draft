# `draft-cli`: Draft Command-Line Utility

! ensure commands are voice-compatible !

This crate contains the Draft command-line application, `draft`. The program offers various subcommands to perform operations on Draft documents or object notation.

- `draft build <PATH>?` build to html, path or file
  - `-o`
  - if served, and uptodate, take from cache
- `draft format|fmt <PATH>?` format all, path or file

- `draft serve <PATH>?` build to html & host live local server, path or file

- `draft get <NAME>` get and update dependencies

- `draft info`  -- binary location, version, etc
- `draft help`
- `draft new` -- init
- `draft docs <NAME>`
- `draft remove|rm <NAME>`
- `draft list|ls`
- `draft package|pkg` -- upload package to central repo, using `publish.don`
- `draft publish|pub` -- upload to teamspace

- `draft migrate <DEST> <PATH>`
    - DEST = [md, dt, don, json, yaml, yml]

## Draft Document (`.dt`)

conf.don
