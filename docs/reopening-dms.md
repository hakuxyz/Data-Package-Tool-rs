# Reopening a lost DM and deleting messages

Both of these need your account token, because Discord has no normal way to do them.

## The warning first

Using your account token with this tool counts as selfbotting. That is against Discord's Community Guidelines and can get your account banned. Everything here is at your own risk.

## Getting your token

Open Discord in your browser, log in, then open the developer tools of the browser while Discord is loaded and grab the token from there. Paste it into the token field in the Settings tab of this app.

The token is only stored locally on your machine, in the app's settings folder. Nothing is sent anywhere except to Discord itself.

## Reopening a lost DM

1. Open the DMs tab
2. Find the DM you want back
3. Press Open DM

The app asks Discord to open the channel for you. If Discord refuses, the app offers to send a message in that DM for you, since receiving activity is what usually makes a lost DM reappear.

You can also set a note on any DM row, useful for remembering who a deleted account was.

## Deleting messages

Right click a message in the search results and pick Delete message. For bigger cleanups there is Mass Delete: it works through the current search results page by page, with a delay between each message so it does not hammer the API.

Deleted messages are remembered in the settings, so they stay hidden when you load the package again. They are only deleted on Discord's side, the package copy always keeps them.

## Refreshing attachment links

Attachment links in old packages expire. The Refresh links option asks Discord for fresh signed urls for the attachments you are looking at, so images and files load again.
