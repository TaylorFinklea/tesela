use regex::Regex;
use std::sync::LazyLock;

pub static WIKI_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").unwrap());

pub static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#([A-Za-z0-9_/-]+)").unwrap());

pub static PROPERTY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Za-z_][A-Za-z0-9_]*):: (.+)").unwrap());

// Logseq planning timestamp: `<YYYY-MM-DD Day>`, optionally with an
// HH:MM time and/or a repeater (`.+1w` etc.) before the closing `>`.
// Group 1 = date, group 2 = optional time; the repeater is not kept.
pub static LOGSEQ_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<(\d{4}-\d{2}-\d{2})(?:\s+[A-Za-z]+)?(?:\s+(\d{2}:\d{2}))?[^>]*>").unwrap()
});

pub static PRIORITY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[#([ABC])\]\s*").unwrap());

pub static BLOCK_REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\(\(([a-f0-9-]+)\)\)").unwrap());

pub static HASH_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#([A-Za-z][A-Za-z0-9_-]*)").unwrap());
