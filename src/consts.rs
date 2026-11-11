// Strings the UI pops up as dialogs. Kept in one place so they stay consistent.

pub const MISSING_TOKEN: &str =
    "You must enter your account token in the Settings tab to use this function.";
pub const INVALID_TOKEN: &str =
    "Entered user token is invalid or doesn't belong to the same account!";
pub const MISSING_BOT_TOKEN: &str =
    "You must enter a bot token in the Settings tab to use this function.";
pub const INVALID_BOT_TOKEN: &str = "Entered bot token is invalid!";
pub const WRONG_TOKEN_TYPE: &str = "Entered bot token belongs to your own account!";
pub const UNKNOWN_DELETED_USER: &str =
    "The user id of this deleted user is unknown. Use Fetch Info first.";

pub const DUPLICATE_DM_WARNING: &str = "You have multiple dm channels with this user! \
This is likely due to a bug which allowed opening multiple dms when messaging someone for the \
first time.\n\nSince Discord only saves a single dm channel as the default dm, there is no \
guarantee that it will open the right one.";

pub const UNSIGNED_LINKS_WARNING: &str = "This data package was created before Discord's \
attachment url signing!\n\nIf you want to be able to view images and other attachments, please \
enter a bot token in the settings tab.";
