use chrono::NaiveDate;
use regex::{Regex, RegexBuilder};
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::models::*;
use crate::package::DataPackage;
use crate::settings::Settings;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Exact,
    Words,
    Regex,
}

/// Everything the search options dialog can change.
#[derive(Clone)]
pub struct SearchOptions {
    pub mode: SearchMode,
    pub sort_asc: bool,
    pub exclude_guilds: bool,
    pub exclude_dms: bool,
    pub exclude_gdms: bool,
    pub exclude_deleted: bool,
    pub has_image: bool,
    pub has_video: bool,
    pub has_file: bool,
    pub case_sensitive: bool,
    pub before_enabled: bool,
    pub after_enabled: bool,
    pub before_date: NaiveDate,
    pub after_date: NaiveDate,
    pub exclude_ids: HashSet<String>,
    pub whitelist_ids: HashSet<String>,
}

impl SearchOptions {
    pub fn from_settings(s: &Settings) -> Self {
        let parse_date = |text: &str, fallback: (i32, u32, u32)| {
            NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap_or_else(|_| {
                NaiveDate::from_ymd_opt(fallback.0, fallback.1, fallback.2).unwrap()
            })
        };
        Self {
            mode: match s.search_mode.as_str() {
                "words" => SearchMode::Words,
                "regex" => SearchMode::Regex,
                _ => SearchMode::Exact,
            },
            sort_asc: s.sort_mode == "asc",
            exclude_guilds: s.exclude_guilds,
            exclude_dms: s.exclude_dms,
            exclude_gdms: s.exclude_gdms,
            exclude_deleted: s.exclude_deleted,
            has_image: s.has_image,
            has_video: s.has_video,
            has_file: s.has_file,
            case_sensitive: s.case_sensitive,
            before_enabled: s.before_enabled,
            after_enabled: s.after_enabled,
            before_date: parse_date(&s.before_date, (2035, 1, 1)),
            after_date: parse_date(&s.after_date, (2015, 1, 1)),
            exclude_ids: s.exclude_ids.iter().cloned().collect(),
            whitelist_ids: s.whitelist_ids.iter().cloned().collect(),
        }
    }
}

/// The matcher is built once, then reused for every message.
struct Matcher {
    exact: Option<(String, bool)>,
    regex: Option<Regex>,
    word_res: Vec<Regex>,
}

impl Matcher {
    fn new(opts: &SearchOptions, query: &str) -> Result<Self, String> {
        let mut flags = RegexBuilder::new(query);
        if !opts.case_sensitive {
            flags.case_insensitive(true);
        }

        match opts.mode {
            SearchMode::Exact => Ok(Self {
                exact: Some((query.to_string(), opts.case_sensitive)),
                regex: None,
                word_res: Vec::new(),
            }),
            SearchMode::Regex => {
                // Validate early so the user gets a nice error
                let re = flags.build().map_err(|e| format!("Invalid regex: {}", e))?;
                Ok(Self {
                    exact: None,
                    regex: Some(re),
                    word_res: Vec::new(),
                })
            }
            SearchMode::Words => {
                // The old tool used one big lookahead regex, the rust regex
                // crate has no lookarounds so we match each word separately
                let mut word_res = Vec::new();
                for word in query.split_whitespace() {
                    let mut word_flags =
                        RegexBuilder::new(&format!(r"\b{}\b", regex::escape(word)));
                    if !opts.case_sensitive {
                        word_flags.case_insensitive(true);
                    }
                    let re = word_flags
                        .build()
                        .map_err(|e| format!("Invalid word: {}", e))?;
                    word_res.push(re);
                }
                Ok(Self {
                    exact: None,
                    regex: None,
                    word_res,
                })
            }
        }
    }

    fn matches(&self, content: &str) -> bool {
        if let Some((query, case_sensitive)) = &self.exact {
            return if *case_sensitive {
                content.contains(query.as_str())
            } else {
                content.to_lowercase().contains(&query.to_lowercase())
            };
        }
        if let Some(re) = &self.regex {
            return re.is_match(content);
        }
        self.word_res.iter().all(|re| re.is_match(content))
    }
}

/// Runs the whole search. `progress` gets a running channel count so the UI
/// can show something while we work.
pub fn search(
    pkg: &DataPackage,
    opts: &SearchOptions,
    query: &str,
    progress: Option<&AtomicUsize>,
) -> Vec<Arc<DMessage>> {
    let matcher = Matcher::new(opts, query).ok();
    let mut results: Vec<Arc<DMessage>> = Vec::new();

    for channel in &pkg.channels {
        // Channel filters
        if channel.is_dm() && opts.exclude_dms {
            continue;
        }
        if channel.is_group_dm() && opts.exclude_gdms {
            continue;
        }
        if !channel.is_dm() && !channel.is_group_dm() && opts.exclude_guilds {
            continue;
        }
        if opts.exclude_ids.contains(&channel.id) {
            continue;
        }
        if let Some(guild) = &channel.guild {
            if opts.exclude_ids.contains(&guild.id) {
                continue;
            }
        }
        if !opts.whitelist_ids.is_empty()
            && !opts.whitelist_ids.contains(&channel.id)
            && !channel
                .guild
                .as_ref()
                .map(|g| opts.whitelist_ids.contains(&g.id))
                .unwrap_or(false)
        {
            continue;
        }

        for msg in &channel.messages {
            if !passes_filters(msg, opts) {
                continue;
            }
            // Empty query means "just show me everything that passes"
            if query.is_empty()
                || matcher
                    .as_ref()
                    .map(|m| m.matches(&msg.content))
                    .unwrap_or(false)
            {
                results.push(msg.clone());
            }
        }

        if let Some(p) = progress {
            p.fetch_add(1, Ordering::Relaxed);
        }
    }

    results.sort_by_key(|m| m.id.parse::<u64>().unwrap_or(0));
    if !opts.sort_asc {
        results.reverse();
    }
    results
}

fn passes_filters(msg: &DMessage, opts: &SearchOptions) -> bool {
    if opts.exclude_deleted && msg.is_deleted() {
        return false;
    }
    if opts.has_image || opts.has_video || opts.has_file {
        let any = msg.attachments.iter().any(|a| {
            (opts.has_image && a.is_image())
                || (opts.has_video && a.is_video())
                || (opts.has_file && !a.is_image() && !a.is_video())
        });
        if !any {
            return false;
        }
    }
    if opts.before_enabled && msg.timestamp.date_naive() >= opts.before_date {
        return false;
    }
    if opts.after_enabled && msg.timestamp.date_naive() <= opts.after_date {
        return false;
    }
    true
}

/// Pre-checks the query so we can show errors before starting a search.
pub fn validate_query(opts: &SearchOptions, query: &str) -> Result<(), String> {
    if opts.mode == SearchMode::Regex && !query.is_empty() {
        Regex::new(query).map_err(|e| format!("Invalid regex: {}", e))?;
    }
    Ok(())
}
