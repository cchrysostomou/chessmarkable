# chessMarkable

[![Mentioned in Awesome reMarkable](https://awesome.re/mentioned-badge.svg)](https://github.com/reHackable/awesome-reMarkable)

A chess game for the reMarkable Paper Pro (fork of [LinusCDE/chessmarkable](https://github.com/LinusCDE/chessmarkable)) writting using the [pleco](https://crates.io/crates/pleco) chess library which is a port of [Stockfish](https://stockfishchess.org/).

<img src="https://transfer.cosmos-ink.net/SF/mainmenu.png" width="30%">&nbsp;<img src="https://transfer.cosmos-ink.net/1tRXA8n/pgnselect.png" width="30%">&nbsp;<img src="https://transfer.cosmos-ink.net/LZ9QT/3.jpg" width="30%">

## Controlling
A chess piece can be moved in two ways:

1. Clicking it once and clicking the spot it's supposed to
2. Clicking it and moving the finger onto the square to move it there on release

The second method has the advantage that it doesn't highlight the chess piece or shows the possible moves.

## FEN

When running the Game with the enviroment variable `RUST_LOG` set to `debug`, the [FEN](https://en.wikipedia.org/wiki/Forsyth%E2%80%93Edwards_Notation) of a board will be output on each move. This is useful for debugging but also for manually saving a game state or resuming it elsewhere since this notation should be compatible with other chess programs/engines.

When starting a game, you'll need to specifiy a slot to play on. On quitting the game, the FEN will get saved to `~/.config/chessmarkable/savestates.yml` which can be used to resume from.

(The `-i` option was removed in favor to add your own fen to the above file).

## PGN Viewer

Chessmarkable also includes a PGN Player (huge thanks to [@rmadhwal](https://github.com/rmadhwal), for contributing this feature)!

You can put downloaded PGN Files into the directory `~/.config/chessmarkable/pgn` on the device with software like scp, FileZilla or WinSCP.
After this, you should be able to browse all the games from the menu point "PGN Viewer" and step through all the games.

## Installation (reMarkable Paper Pro + AppLoad)

This fork targets the **reMarkable Paper Pro** running [xovi](https://github.com/asivery/rm-xovi-extensions) with [AppLoad](https://github.com/asivery/rm-appload) and its `qtfb-shim.so` (expected at `/home/root/shims/qtfb-shim.so`).

1. Build the binary (see below) and copy it into `dist-rmpp/`.
2. Copy the contents of `dist-rmpp/` (`chessmarkable`, `external.manifest.json`, `icon.png`) to `/home/root/xovi/exthome/appload/chessmarkable/` on the tablet.
3. `chmod +x /home/root/xovi/exthome/appload/chessmarkable/chessmarkable`
4. Reload AppLoad and open **Chessmarkable**.

## Compiling

The app uses a patched copy of libremarkable 0.7.0 in `libremarkable-rmpp/`. It adds a Paper Pro device model (1620x2160 framebuffer through the AppLoad QTFB shim), a 64-bit-correct `fb_fix_screeninfo`, and a more tolerant input scan for the Paper Pro's touch panel.

Cross-compile for aarch64 with [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild). Pinning glibc 2.31 keeps the binary compatible with the tablet:

```sh
rustup target add aarch64-unknown-linux-gnu
cargo install cargo-zigbuild   # requires zig on PATH
cargo zigbuild --release --target aarch64-unknown-linux-gnu.2.31 --bin chessmarkable
cp target/aarch64-unknown-linux-gnu/release/chessmarkable dist-rmpp/
```

## Credit

- The [pleco](https://crates.io/crates/pleco) library is used as the engine, checking valid moves and providing the bots
- [libremarkable](https://github.com/canselcik/libremarkable) (MIT), vendored and patched in `libremarkable-rmpp/`
- The chess pices are from [pixabay here](https://pixabay.com/vectors/chess-pieces-set-symbols-game-26774/)
