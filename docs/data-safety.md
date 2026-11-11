# Data safety

Not trusting a random program with your full Discord history is the right instinct. Here is exactly what this tool does.

## Offline first

Loading a package, browsing servers, DMs and messages, and searching are all fully offline. You can pull the network cable and the app works the same.

## What it may contact

- `discord.com` and `cdn.discordapp.com`: avatars, attachments, and the token features (deleting, reopening DMs, refreshing links), only when you use them
- `versionhistory.googleapis.com`: latest Chrome version number, used to build request headers that look like a normal browser for the token features
- `raw.githubusercontent.com`: the update check, which you can turn off in Settings

Nothing else. No analytics, no telemetry, no third party servers.

## What is stored

Everything the app saves lives in its own settings folder in your user profile: search options, id lists, your notes on DMs, remembered deleted messages, and the tokens if you enter them. You can open the folder with a button in Settings, and deleting it resets the app completely.

## Your package

The package itself is only read from disk. It is never uploaded anywhere.
