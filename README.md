# DexTUI

A small terminal Pokédex viewer built with Rust and `ratatui`.

This repository is a terminal UI demo that fetches Pokémon data from the public
PokeAPI and renders details and sprites in the terminal. It includes a simple
fetch/cache mechanism (JSON + sprite PNGs) and a TUI for browsing Pokémon.

Features
- Browse Pokémon list and view details (types, abilities, stats)
- Colored type badges and compact sprite rendering
- Background fetch and cache seeding mode
- Simple search/filtering

Quickstart

1. Install Rust and Cargo: https://rustup.rs/
2. Build and run the app in fetch-only mode to seed the local cache (example: Gen 3):

```bash
# Example: fetch up to 386 Pokémon (Gen 3)
POKEMON_LIMIT=386 cargo run -- --fetch-only
```

3. Run the TUI:

```bash
cargo run
```

Usage notes
- The app stores cached JSON at `data/pokemon.json` and sprite PNGs under
  `data/sprites/`. These data files are typically omitted from version control
  to keep the repository small.
- To change how many Pokémon are fetched, use the `POKEMON_LIMIT` environment
  variable before running with `--fetch-only`.
- Press `/` to search, `r` to trigger a background refresh, and `?` to show
  the help modal inside the UI.

Privacy / Anonymization
- This README has been generalized for sharing on GitHub. Remove or re-add
  any author or personal account links if you want to identify the project
  owner.

Development notes
- The UI uses an in-memory thumbnail cache to avoid per-frame disk I/O.
- To reduce memory further, thumbnails are small (48×48 RGB) and the preload
  step generates these compact thumbnails rather than holding full RGBA images.
- Consider adding an LRU cache if you need to cap memory usage for large
  collections of sprites.

License
- The project is distributed under the MIT license (see `LICENSE` if present).
