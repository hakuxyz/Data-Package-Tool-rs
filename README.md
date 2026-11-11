# Data Package Tool (Rust)

A tool focused on browsing the actual contents of a Discord data package.

This is a Rust rewrite of [Data-Package-Tool](https://github.com/aamiaa/Data-Package-Tool) by [aamiaa](https://github.com/aamiaa), who made the original app and came up with all of the ideas here. Full credit for the design and feature set goes to them. This port exists to see how much faster the same tool can be in Rust.

## Features

- Search your messages (exact, words, or regex mode, plus filters)
- Jump to messages in your client
- Mass delete messages*
- Reopen lost DMs*
- Browse all images you've sent
- View all servers you've joined, along with invites you've used
- View all DMs you've talked in
- Works fully offline for everything that doesn't need the internet

\* Requires your account token. Note that this falls under selfbotting, which is against Discord's [Community Guidelines](https://discord.com/guidelines#:~:text=Do%20not%20use%20self%2Dbots%20or%20user%2Dbots) and might get your account banned.

## Usage

1. Request your data package from Discord (Settings, Privacy and Safety, Request Data)
2. Unzip it if needed, then open the `package.zip` inside in the app, or just drop it onto the window
3. Search, browse the tabs, right click things for more options

Guides live in the [docs](docs) folder:

- [Getting your package](docs/getting-your-package.md)
- [Searching](docs/searching.md)
- [Reopening a lost DM](docs/reopening-dms.md)
- [Data safety](docs/data-safety.md)

The [original wiki](https://github.com/aamiaa/Data-Package-Tool/wiki) covers the same ideas from the author of the original tool.

## Your data safety

- The tool works offline. Searching and browsing never touch the network.
- The only domains it may contact are `discord.com`, `cdn.discordapp.com`, `raw.githubusercontent.com` (update check, can be disabled in settings) and `versionhistory.googleapis.com` (latest Chrome version for request headers).
- Nothing is sent anywhere except to Discord itself, and only for the token features.

## Building

```
cargo build --release
```

The binary ends up in `target/release`. Rust 1.75 or newer should work.

## License

MIT, see [LICENSE](LICENSE). The original tool by aamiaa can be found
[here](https://github.com/aamiaa/Data-Package-Tool).
