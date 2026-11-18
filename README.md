# rex-flash

flash.nvim-style search over a Rex pane's visible text: press a
key, type to narrow, press a label to land the cursor on a match, extend a
selection with vim motions, and yank it to the clipboard.

It is [herdr-flash](https://github.com/youguanxinqing/herdr-flash)'s picker,
run in a Rex floating terminal laid exactly over the pane. herdr-flash is used
as a library; this crate only adds the Rex side.

## Keys

| Key | Does |
| --- | --- |
| type | narrow the matches |
| a label | land the cursor on that match |
| `v` / `V` | start a character / line selection |
| `h j k l w b e t f` | extend the selection |
| `y` or `Enter` | yank over OSC 52 (Rex puts it on your clipboard) |
| `Backspace` | back to searching |
| `Esc` | close |

Only the visible screen is searched, not the scrollback.

## Install

Instructions an agent can follow step by step. Each step ends with a check.

### Requirements

- Rex, with the `rex` CLI on `PATH` (`rex --version`).
- A Rust toolchain with `cargo` (`cargo --version`). On NixOS without one,
  prefix the install command with
  `nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc -c`.

### 1. Install the binary on every Rex host

The picker runs on the host that holds the pane. Install it on this machine,
and on each remote Rex host whose panes you want to search (`rex hosts` lists
them).

```sh
cargo install --locked --git https://github.com/youguanxinqing/rex-flash
```

Check: `rex-flash --help` prints the usage. The binary lands in
`~/.cargo/bin`; pass `--root ~/.local` to put it in `~/.local/bin` instead.
`rex/flash.lua` looks in both.

### 2. Add the key binding to the Rex config

Rex reads one config file: `$REX_CONFIG` if set, else `~/.config/rex/init.lua`
(`$XDG_CONFIG_HOME/rex/init.lua` when that is set). Only the config on the
machine running the Rex app matters; remote hosts need just the binary.

Append [`rex/flash.lua`](rex/flash.lua) to that file:

```sh
curl -fsSL https://raw.githubusercontent.com/youguanxinqing/rex-flash/master/rex/flash.lua \
  >> ~/.config/rex/init.lua
```

It binds `ctrl+b s`. If the config already binds that key, change the
`rex.bind` line at the end of the snippet, and don't append it twice: it
defines an action named `rex_flash`.

Check: `rex config check` reports no error, then `rex config reload`, then
`rex keymap | grep rex_flash` shows the binding.

### 3. Try it

Press the key in any pane, type a few characters from the screen, and confirm
the status line shows matches. `Esc` closes it.

## Config

Optional. `~/.config/rex-flash/config.toml` (or `$XDG_CONFIG_HOME/rex-flash/`)
takes herdr-flash's config format, for example:

```toml
[flash]
exit_on_yank = false   # stay open after a yank; Esc closes
```

## How it works

The Rex action measures the pane, then floats a terminal over it at the same
size in cells, so the picker's frame lines up with the text. For a local pane
it reads the screen in-process and passes it as `--screen`, so the picker
paints as soon as it starts. For a pane on another host, the float runs there
and calls `rex capture`, since reading it from the app's side would carry the
whole scrollback over the network.

## License

MIT
