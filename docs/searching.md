# Searching

The search bar lives at the top of the Messages tab. Type something and press Enter or the Search button.

## Search modes

You pick the mode in the Search Options window (the button next to the search bar):

- Exact: your query has to appear in a message as-is
- Words: every word of your query has to appear somewhere in the message, in any order
- Regex: your query is a regular expression, checked with the rust regex crate (no lookarounds)

## Filters

Also in Search Options:

- Case sensitive: makes the match care about upper and lower case
- Has image / video / file: only messages with that attachment type
- Before / after date: cuts off messages by date
- Exclude deleted: hides messages you deleted with this tool in an earlier session
- Sort ascending / descending: oldest or newest first

## Limiting to specific places

Under Settings you can maintain two id lists:

- Exclude list: channels, servers or users that never show up in results
- Whitelist list: when it is not empty, only these places are searched

Right click a server row, a DM row or a message to copy its id, then add it to a list in Settings.

## Jumping to a result

Right click any message in the results and pick Go to message. The app opens your Discord client at that exact spot. This uses a discord:// link, so your client has to be installed.

## Copying results

The same right click menu can copy a message link, plain message ids, or all of it for a whole page of results.
