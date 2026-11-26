# Writing a game module

A module teaches Hundo one game: where its saves are, how to read them, and
what counts towards finishing it. It is a WebAssembly component built from
Rust with `hundo-sdk`; Hundo hands it the save's files and runs it in a
sandbox — no disk, no network, bounded time and memory.

The official modules in `drivers/` are complete, real examples: start from
the one closest to your game.

## 0. Get the `hundo` command

It ships inside every installer of
[Hundo](https://github.com/hundo-app/releases/releases/latest), beside the
app. Hundo's settings, under *Game modules → Writing a module*, show where
it is on your machine and the exact line to run; by hand:

| Hundo installed from | Where `hundo` is | To run it from a terminal |
| --- | --- | --- |
| `.deb`, `.rpm` | `/usr/bin/hundo` | nothing to do |
| Windows installer | Hundo's install folder, beside `Hundo.exe` | add that folder to your `PATH` (PowerShell below), then open a new terminal |
| macOS `.dmg` | `Hundo.app/Contents/MacOS/hundo` | `sudo ln -sf /Applications/Hundo.app/Contents/MacOS/hundo /usr/local/bin/hundo` |
| AppImage, Flatpak | inside the image or the sandbox, out of a terminal's reach | install the `.deb` or `.rpm` to write modules |

```powershell
# Windows, once: Hundo's install folder in your PATH (adjust it if you chose another).
$dir = "$env:LOCALAPPDATA\Hundo"
[Environment]::SetEnvironmentVariable("Path", [Environment]::GetEnvironmentVariable("Path", "User") + ";$dir", "User")
```

`hundo --version` then answers. You also need Rust, from
[rustup.rs](https://rustup.rs), and its WebAssembly target (below).

## 1. Start

```sh
rustup target add wasm32-wasip2
hundo module new my-game --name "My Game"
cd my-game
cargo build --release
hundo module check . --save path/to/a/save   # the report, section by section
```

`hundo module new` writes `Cargo.toml`, `.cargo/config.toml` (the WebAssembly
target), `driver.toml` and `src/lib.rs`.

To see it in Hundo itself, install what you built — again after each build;
it replaces the copy before:

```sh
hundo module install .
```

## 2. The manifest: `driver.toml`

What Hundo needs before reading any save — so the module works on every
machine without knowing them.

```toml
api = 3                      # the contract's version (wit/driver.wit)

[driver]
id = "my-game"               # lowercase letters, digits, dashes; never changes
name = "My Game"
version = "0.1.0"
license = "MIT OR Apache-2.0"
description = "What it tracks, in one line."
authors = ["You"]

[game]
steam-app-id = 123456        # optional: install, build, Proton prefix, achievements
tested-versions = ["1.0.3"]

[[locations]]                # one per platform the game saves on
platform = "windows"
path = "{localLow}/Studio/My Game"

[slots]
kind = "file"                # "file": one file per slot; "dir": one folder per slot
pattern = '^save(?<slot>[0-9])\.dat$'
label = "Slot {slot}"
```

Optional parts:

- `[[dlc]]` — content packs items may belong to; the reader chooses which count.
- `[texts]`, `[images]`, `[[maps]]` — names, pictures and maps the host reads
  from the installed game (Unity games today), so a module can show the
  game's own words and art without shipping them.
- `[[options]]` — choices offered to the reader, shown in the menu of a panel
  of the game's page, given to the module with each save:

  ```toml
  [[options]]
  id = "journal-notes"
  label = "Journal : viser les notes du Chasseur"
  description = "On: a creature counts once its notes are unlocked."
  panel = "list"             # progress, next, list, map, zones, session, history, achievements, notes, stats, details
  default = true             # a switch; or the id of one of its `choices`
  ```

  Read it with `input.flag("journal-notes", true)` or `input.option(id)`.

- `[[layout]]` — the game's page as it first opens: panels in this order,
  with their width (out of 12 columns) and height.

  ```toml
  [[layout]]
  panel = "map"
  span = 6
  height = 600
  ```

## 3. The report

`analyze(input)` returns a report: the game's version read from the save, an
official completion figure if the game has one, statistics (the one with id
`playtime`, as "25 h 21", is shown in the page's banner), and **sections** of
**items** — each `done`, `todo`, `in-progress`, `locked` or `missed`, with an
optional progress `x / y`, hint, area, spoiler level, picture and link.

```rust
use hundo_sdk::*;

struct MyGame;

impl Guest for MyGame {
    fn analyze(input: Input) -> Result<Report, DriverError> {
        let save = input.file("save1.dat")?;
        let mut report = Report::empty();
        report.sections.push(Section::new("bosses", "Boss", vec![
            Item::check("dragon", "Vaincre le dragon", true),
        ]));
        Ok(report)
    }
}

hundo_sdk::export!(MyGame);
```

Rules every module follows:

1. **Stable item ids**: the reader's notes and history hang on them.
2. A field missing from the save (an older version of the game) **drops the
   line**; it never marks it "to do".
3. End-game spoilers are `major`: Hundo hides them by default.
4. Ship an **anonymised** example save in `tests/fixtures/` and the expected
   report in `tests/expected.json`.
5. Return `incomplete` when the save is being written: Hundo tries again.

## 4. Sign and publish

Hundo installs a module from the catalog only if its package is signed, and
only updates it with packages signed by the same key.

```sh
hundo module keygen my-key                    # my-key.key (secret) + my-key.pub
hundo module pack . -o my-game-0.1.0.hundo --sign my-key.key
```

The entry carries your `driver.toml`: that is how Hundo recognises your game
on a reader's machine — its Steam id, where its saves are — before anyone
downloads the module, which comes only when the game is added.

Upload `my-game-0.1.0.hundo` where it can be downloaded over HTTPS (a GitHub
release of your repository is fine), then print its catalog entry:

```sh
hundo module entry my-game-0.1.0.hundo --key my-key.pub \
  --url https://github.com/you/my-game/releases/download/v0.1.0/my-game-0.1.0.hundo
```

Signatures are standard [minisign](https://jedisct1.github.io/minisign/): the
usual `minisign` tool works too.

## 5. Into the catalog

Open a pull request here that adds your entry to `catalog/catalog.json`. A
review checks the module reads saves only, its example save is anonymised,
and its id is free. From then on Hundo offers it to every reader, and each new
version you publish — a new entry signed with the same key — reaches them as
an update, without reinstalling the app.

Keep your secret key and its password safe: losing it means publishing under
a new id; leaking it lets someone else sign as you.
