//! Block-lifecycle side effects, extracted from `tesela-server`'s
//! `routes/notes.rs` (tesela-ows.1, step 1). Pure functions operating on
//! parsed markdown content — no I/O, no `AppState`. Today only the server's
//! HTTP handlers call these (behavior-neutral extraction); step 2 wires
//! them into the engine-side apply path so every writer (WS, relay, FFI)
//! inherits them (see `docs/ai` / bead tesela-ows.1 for the step-2 hook-point
//! proposal — not implemented here).
//!
//! Covers three side effects that previously lived inline in the HTTP PUT
//! handler:
//!   - recurrence bump (`try_bump_block` / `try_skip_block`)
//!   - same-note dependency-cycle unblock (`apply_dependency_cycles`)
//!   - tag auto-create's pure tag-collection + template pieces
//!     (`collect_note_tags` / `tag_page_content`) — the store I/O
//!     (slug resolution, note creation) stays server-side.

use crate::block::{parse_blocks, ParsedBlock};
use crate::note::Note;
use crate::recurrence::{self, Recurrence};
use crate::storage::markdown::parse_frontmatter;

/// Pure helper. Returns `Some((new_content, next_deadline_iso))` if `block_id`
/// resolves to a block in `content` with `status:: done` + valid `recurring::`
/// + valid anchor date (`deadline::` or `scheduled::`).
///
/// Behaviour (Task 6 semantics):
/// - Reads `recurrence_done::` (default 0) and calls `recurrence::advance` to
///   check whether the series still has occurrences.
/// - **Series active** (`advance` returns `Some`): advance every date field
///   (`deadline::`, `scheduled::`) by one step each from their own current
///   values; stamp `recurrence_done:: <done+1>`; reset `status:: todo`;
///   stamp `last_completed::`.
/// - **Series spent** (`advance` returns `None`): leave `status:: done`;
///   leave date fields unchanged; set `recurrence_done:: <done+1>`.
///   The `recurring::` property is NOT removed.
///
/// Returns `None` for any reason a bump cannot apply (idempotent, caller
/// just leaves content unchanged).
pub fn try_bump_block(content: &str, block_id: &str) -> Option<(String, String)> {
    let (note_id_str, line_str) = block_id.rsplit_once(':')?;
    let line_num: usize = line_str.parse().ok()?;
    let (_meta, body) = parse_frontmatter(content).ok()?;
    let blocks = parse_blocks(note_id_str, &body);
    let block = blocks.iter().find(|b| b.id == block_id)?;

    if block.properties.get("status").map(|s| s.as_str()) != Some("done") {
        return None;
    }

    let step = compute_recurrence_step(block)?;
    let last_completed_str = format!("[[{}]]", step.anchor_date.format("%Y-%m-%d"));

    match step.active {
        Some(ActiveStep {
            new_deadline,
            new_scheduled,
            next_iso,
        }) => {
            // Series still active — advance every date field from its own value.
            let new_body = rewrite_block_for_complete(
                &body,
                line_num,
                new_deadline.as_deref(),
                new_scheduled.as_deref(),
                &last_completed_str,
                step.new_done,
            )?;
            let new_content = reassemble_content(content, &body, &new_body);
            Some((new_content, next_iso))
        }
        None => {
            // Series spent — leave dates, leave status done, only bump counter.
            let new_body = rewrite_block_for_spent(&body, line_num, step.new_done)?;
            let new_content = reassemble_content(content, &body, &new_body);
            // Return a sentinel ISO so the endpoint can report *something*;
            // the `bumped: true` flag is still meaningful (counter updated).
            let iso = step.anchor_date.format("%Y-%m-%d").to_string();
            Some((new_content, iso))
        }
    }
}

/// Like `try_bump_block` but for `mode: skip`. Advances date fields and
/// increments `recurrence_done::` without touching `status::` or stamping
/// `last_completed::`. Requires `recurring::` to be present and parseable
/// but does NOT require `status:: done` — the block may be in any state.
pub fn try_skip_block(content: &str, block_id: &str) -> Option<(String, String)> {
    let (note_id_str, line_str) = block_id.rsplit_once(':')?;
    let line_num: usize = line_str.parse().ok()?;
    let (_meta, body) = parse_frontmatter(content).ok()?;
    let blocks = parse_blocks(note_id_str, &body);
    let block = blocks.iter().find(|b| b.id == block_id)?;

    let step = compute_recurrence_step(block)?;

    match step.active {
        Some(ActiveStep {
            new_deadline,
            new_scheduled,
            next_iso,
        }) => {
            let new_body = rewrite_block_for_skip(
                &body,
                line_num,
                new_deadline.as_deref(),
                new_scheduled.as_deref(),
                step.new_done,
            )?;
            let new_content = reassemble_content(content, &body, &new_body);
            Some((new_content, next_iso))
        }
        None => {
            // Series spent — only bump the counter, leave everything else.
            let new_body = rewrite_block_for_spent(&body, line_num, step.new_done)?;
            let new_content = reassemble_content(content, &body, &new_body);
            let iso = step.anchor_date.format("%Y-%m-%d").to_string();
            Some((new_content, iso))
        }
    }
}

/// Detect any blocks whose `status::` flipped from non-done in `prev` to
/// `done` in `next`, and apply recurrence bumps to all of them. Returns
/// the (possibly identical) content to persist.
///
/// Done in a loop: each bump re-parses, so subsequent bumps in the same
/// PUT see fresh line numbers. Bumps the same block at most once per call
/// (after a bump, that block's status is `todo`, so it no longer matches).
/// Detect any block whose status flipped to `done` in this PUT and bump
/// its deadline before saving. Returns (rewritten_content, bumps) so the
/// caller can fire `WsEvent::RecurringRolled` for each. `note_id` is used
/// to rewrite block ids in the returned `BumpInfo`s to `<note_id>:<line>`.
pub fn apply_post_save_bumps_with_info(
    prev: &str,
    next: &str,
    note_id: &str,
) -> (String, Vec<BumpInfo>) {
    apply_post_save_bumps_core(prev, next, note_id, false)
}

/// Engine-path variant of [`apply_post_save_bumps_with_info`] with the
/// idempotence guard turned ON (tesela-ows.1 step 2, Lead constraint (a)).
///
/// Every writer's engine-side apply (WS live-apply, relay import, iOS `.relay`
/// import) — not just the single-writer HTTP PUT — runs this so a `done` flip
/// arriving over the wire triggers the recurrence bump. Unlike the HTTP path,
/// the engine can see the SAME occurrence's done-flip more than once (two peers
/// completing concurrently before either has seen the other, a re-delivered
/// frame, or a disjoint-twin duplicate), so each candidate block is bumped ONLY
/// when [`occurrence_uncompleted`] holds — its `last_completed::` is absent or
/// strictly before the COMPLETED occurrence's anchor. Once a bump stamps
/// `last_completed`, a re-arriving completion of the same occurrence sees the
/// same anchor and skips: the series is self-limiting and never double-advances.
///
/// The HTTP path ([`apply_post_save_bumps_with_info`], guard OFF) is unchanged —
/// this variant only differs by the guard, so the HTTP behavior is byte-for-byte
/// identical.
pub fn apply_post_save_bumps_guarded(
    prev: &str,
    next: &str,
    note_id: &str,
) -> (String, Vec<BumpInfo>) {
    apply_post_save_bumps_core(prev, next, note_id, true)
}

fn apply_post_save_bumps_core(
    prev: &str,
    next: &str,
    note_id: &str,
    guard: bool,
) -> (String, Vec<BumpInfo>) {
    let flipped = detect_status_flips_to_done(prev, next);
    let mut content = next.to_string();
    let mut bumps = Vec::new();
    for block_id in flipped {
        // Idempotence guard (engine path only): skip the bump when the
        // occurrence this done-flip is COMPLETING has already been recorded, so
        // a concurrently- or re-delivered completion can't double-advance the
        // series. The anchor is the EARLIER of `prev`'s and `next`'s occurrence
        // dates ([`completed_occurrence_anchor`]) — whichever side a concurrent
        // peer roll in the same merged frame advanced shows the later date, but
        // the occurrence actually being completed is the earlier one.
        if guard {
            let completed_anchor = completed_occurrence_anchor(prev, next, &block_id);
            if !occurrence_uncompleted(&content, &block_id, completed_anchor) {
                continue;
            }
        }
        // try_bump_block uses the note-id prefix from `block_id` and parses
        // body blocks against that prefix. Our block_id here came from a
        // `__diff__` parse, so try_bump_block will still find a match
        // because it re-parses with the same prefix.
        if let Some((bumped, next_iso)) = try_bump_block(&content, &block_id) {
            // Resolve the bumped block's title from the freshly-parsed
            // content. Re-parse to get the title — the line number may
            // have changed if `last_completed::` was inserted.
            let title = title_for_block(&bumped, &block_id).unwrap_or_default();
            content = bumped;
            // Rewrite the block id from `__diff__:N` to `<note_id>:N`
            // so the WS event carries a useful pointer.
            let line = block_id.rsplit_once(':').map(|(_, l)| l).unwrap_or("0");
            let real_block_id = format!("{}:{}", note_id, line);
            bumps.push(BumpInfo {
                block_id: real_block_id,
                title,
                next_deadline: next_iso,
            });
        }
    }
    (content, bumps)
}

/// Anchor date of a block's CURRENT occurrence: its `deadline::` (else
/// `scheduled::`) parsed to a date, ignoring any time suffix. `None` when the
/// block carries no parseable anchor.
fn block_anchor_date(block: &ParsedBlock) -> Option<chrono::NaiveDate> {
    let from_deadline = block
        .properties
        .get("deadline")
        .and_then(|v| parse_deadline_value(v))
        .map(|(d, _)| d);
    let from_scheduled = block
        .properties
        .get("scheduled")
        .and_then(|v| parse_deadline_value(v))
        .map(|(d, _)| d);
    from_deadline.or(from_scheduled)
}

/// Idempotence guard (tesela-ows.1 step 2, Lead constraint (a)): `true` when the
/// block identified by `block_id` in `content` has NOT yet recorded a completion
/// for the occurrence this done-flip is COMPLETING — i.e. `last_completed::` is
/// absent or strictly before `completed_anchor`. A block with no parseable
/// recurrence/anchor also returns `true` (the bump itself no-ops, so the guard
/// must not mask it). Used only on the engine path, where the same done-flip can
/// be seen more than once.
///
/// `completed_anchor` (from [`completed_occurrence_anchor`]) is the EARLIER of
/// the pre-import (`prev`) and post-import (`next`) occurrence dates. Anchoring
/// on the earlier date is what makes the guard robust to a concurrent peer's
/// roll delivered in the SAME merged frame: that roll advances ONE side's
/// deadline past the occurrence being completed, and taking the minimum recovers
/// the completed occurrence regardless of which side (prev or next) carries the
/// advanced value. Falls back to `content`'s own anchor when the caller could
/// not recover a completed occurrence (a brand-new done block with no `prev`
/// counterpart).
fn occurrence_uncompleted(
    content: &str,
    block_id: &str,
    completed_anchor: Option<chrono::NaiveDate>,
) -> bool {
    let Some((note_id_str, _)) = block_id.rsplit_once(':') else {
        return true;
    };
    let Ok((_meta, body)) = parse_frontmatter(content) else {
        return true;
    };
    let blocks = parse_blocks(note_id_str, &body);
    let Some(block) = blocks.iter().find(|b| b.id == block_id) else {
        return true;
    };
    let Some(step) = compute_recurrence_step(block) else {
        return true;
    };
    let anchor = completed_anchor.unwrap_or(step.anchor_date);
    match block
        .properties
        .get("last_completed")
        .and_then(|v| parse_deadline_value(v))
        .map(|(d, _)| d)
    {
        Some(last) => last < anchor,
        None => true,
    }
}

/// The anchor of the occurrence a done-flip is COMPLETING: the EARLIER of the
/// flipped block's occurrence date in the PRE-import (`prev`) note and in the
/// post-import (`next`) note. `block_id` is the `next`-side id
/// [`detect_status_flips_to_done`] returned; the matching `prev` block is found
/// by canonical `bid` (stable across property edits), with a display-`text`
/// fallback for the pre-`bid` / test-fixture case.
///
/// The minimum is deliberate. A concurrent peer's roll present in the same
/// merged frame advances ONE side's `deadline::`/`scheduled::` past the
/// occurrence actually being completed (the advance can land on `next` — the
/// pre-import peer is behind — OR on `prev` — the pre-import peer already rolled
/// and is importing a stale disjoint-twin completion). Taking the earlier date
/// recovers the completed occurrence in BOTH directions; anchoring on either
/// side alone double-bumps the other. `None` when neither side has a parseable
/// anchor — the caller then falls back to `content`'s own date.
fn completed_occurrence_anchor(
    prev: &str,
    next: &str,
    block_id: &str,
) -> Option<chrono::NaiveDate> {
    let (note_id_str, _) = block_id.rsplit_once(':')?;
    let (_m, next_body) = parse_frontmatter(next).ok()?;
    let next_blocks = parse_blocks(note_id_str, &next_body);
    let nb = next_blocks.iter().find(|b| b.id == block_id)?;
    let next_anchor = block_anchor_date(nb);

    let (_m2, prev_body) = parse_frontmatter(prev).ok()?;
    let prev_blocks = parse_blocks(note_id_str, &prev_body);
    let prev_anchor = nb
        .bid
        .as_ref()
        .and_then(|bid| prev_blocks.iter().find(|b| b.bid.as_ref() == Some(bid)))
        .or_else(|| prev_blocks.iter().find(|b| b.text == nb.text))
        .and_then(block_anchor_date);

    match (prev_anchor, next_anchor) {
        (Some(p), Some(n)) => Some(p.min(n)),
        (Some(p), None) => Some(p),
        (None, Some(n)) => Some(n),
        (None, None) => None,
    }
}

/// One recurring/dependency roll for a single block, expressed as CONTAINER
/// property SETS — the engine-path output (tesela-ows.1 step 2, Lead constraint
/// (a)). The engine authors each `(key, value)` as a `BlockPropertySet` onto the
/// block's typed props container, so lifecycle state (`last_completed`,
/// `recurrence_done`, the rolled dates, `status`) STAYS in the container where
/// disjoint-twin heal's per-key union protects it. It is NEVER evicted to an
/// in-text `key:: value` line: attempt 2 cleared the container and wrote in-text
/// so the roll would render, but twin-heal unions CONTAINER props only, so a
/// max-`TreeID` pick landing on the non-rolling twin silently WIPED completion
/// memory (a new data-loss class). Keeping the roll in the container renders
/// correctly for free (the container value wins render-time dedup) with no
/// clearing.
#[derive(Debug, Clone)]
pub struct BlockLifecycleRoll {
    /// Canonical block id (`<!-- bid:UUID -->`, hyphenated), used by the engine
    /// to address the container node. `None` for an unstamped block — the engine
    /// skips it (a container set is addressed by bid).
    pub bid: Option<String>,
    /// `<note_id>:<line>` id (parity with [`BumpInfo`]; logging).
    pub block_id: String,
    /// The block's display text (title), for logging / WS parity.
    pub title: String,
    /// `Some(iso)` next-deadline when this roll advanced a recurrence (WS
    /// `RecurringRolled` parity), else `None` (a pure dependency unblock).
    pub next_deadline: Option<String>,
    /// Container props to SET, in render-canonical string form (`[[YYYY-MM-DD]]`,
    /// `todo`, `1`, …). Only the lifecycle-owned keys whose value CHANGED vs
    /// `next` — never a removal (the roll only sets/advances).
    pub props: Vec<(String, String)>,
}

/// Block-property keys the engine-path lifecycle roll may rewrite: the
/// recurrence bump ([`try_bump_block`]) touches every one; the dependency
/// unblock ([`apply_dependency_cycles`]) touches `status`. Diffed between `next`
/// (post-import, pre-lifecycle) and the post-lifecycle markdown to derive the
/// container sets.
const LIFECYCLE_ROLL_KEYS: [&str; 5] = [
    "status",
    "deadline",
    "scheduled",
    "recurrence_done",
    "last_completed",
];

/// Engine-path lifecycle (tesela-ows.1 step 2, Lead constraint (a)): compute the
/// per-block CONTAINER property changes a `done` flip should apply — the
/// idempotence-GUARDED recurrence bump plus the same-note dependency unblock —
/// WITHOUT rewriting markdown. The engine authors each returned change as a
/// `BlockPropertySet` on the typed props container (see [`BlockLifecycleRoll`]),
/// so lifecycle state stays in the container.
///
/// `prev` is the note's full markdown BEFORE the import; `next` is the full
/// markdown AFTER the import + disjoint-twin heal (rendered from the merged doc).
/// Returns an empty vec when nothing rolls or unblocks (the guard tripped, the
/// flip was non-recurring, or there was nothing to unblock) — the common
/// text-edit delta pays nothing.
///
/// This runs the SAME pure pipeline the HTTP PUT path runs
/// ([`apply_post_save_bumps_guarded`] then [`apply_dependency_cycles`]); the only
/// difference is that the RESULT is diffed back into per-key container sets
/// instead of a rewritten body.
pub fn compute_lifecycle_container_sets(
    prev: &str,
    next: &str,
    note_id: &str,
) -> Vec<BlockLifecycleRoll> {
    let (bumped, bumps) = apply_post_save_bumps_guarded(prev, next, note_id);
    let (final_md, _unblocked) = apply_dependency_cycles(prev, &bumped, note_id);
    if final_md == next {
        // Guard tripped, non-recurring flip, or nothing to unblock.
        return Vec::new();
    }

    // block_id (`<note_id>:<line>`) → next-deadline ISO, for WS parity.
    let deadline_by_id: std::collections::HashMap<&str, &str> = bumps
        .iter()
        .map(|b| (b.block_id.as_str(), b.next_deadline.as_str()))
        .collect();

    let Ok((_pm, next_body)) = parse_frontmatter(next) else {
        return Vec::new();
    };
    let Ok((_fm, final_body)) = parse_frontmatter(&final_md) else {
        return Vec::new();
    };
    let next_blocks = parse_blocks(note_id, &next_body);
    let final_blocks = parse_blocks(note_id, &final_body);

    let mut rolls = Vec::new();
    for fb in &final_blocks {
        // Match the pre-lifecycle counterpart by canonical bid (stable across
        // the roll — the bump only rewrites continuation property lines, never
        // the bullet's first line), with a display-text fallback.
        let nb = fb
            .bid
            .as_ref()
            .and_then(|bid| next_blocks.iter().find(|b| b.bid.as_ref() == Some(bid)))
            .or_else(|| next_blocks.iter().find(|b| b.text == fb.text));
        let mut props = Vec::new();
        for key in LIFECYCLE_ROLL_KEYS {
            let final_val = fb.properties.get(key);
            let next_val = nb.and_then(|b| b.properties.get(key));
            if let Some(fv) = final_val {
                if next_val != Some(fv) {
                    props.push((key.to_string(), fv.clone()));
                }
            }
        }
        if props.is_empty() {
            continue;
        }
        let next_deadline = deadline_by_id.get(fb.id.as_str()).map(|s| s.to_string());
        rolls.push(BlockLifecycleRoll {
            bid: fb.bid.clone(),
            block_id: fb.id.clone(),
            title: fb.text.clone(),
            next_deadline,
            props,
        });
    }
    rolls
}

#[derive(Debug, Clone)]
pub struct BumpInfo {
    pub block_id: String,
    pub title: String,
    pub next_deadline: String,
}

fn title_for_block(content: &str, block_id: &str) -> Option<String> {
    let (note_id_str, _) = block_id.rsplit_once(':')?;
    let (_meta, body) = parse_frontmatter(content).ok()?;
    let blocks = parse_blocks(note_id_str, &body);
    let block = blocks.iter().find(|b| b.id == block_id)?;
    Some(
        block
            .text
            .split_whitespace()
            .filter(|tok| !tok.starts_with('#'))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// Block ids whose `status` was missing/non-done in `prev` and is `done`
/// in `next`. Lossless against block re-numbering: line numbers can shift
/// across edits, so a block's id may differ between snapshots. We match
/// blocks by `(text, raw_text_first_line)` rather than just id.
fn detect_status_flips_to_done(prev: &str, next: &str) -> Vec<String> {
    fn parse_body_blocks(content: &str) -> Vec<ParsedBlock> {
        match parse_frontmatter(content) {
            Ok((_, body)) => {
                // The note id we pass here only forms ParsedBlock.id; the
                // bumper re-parses with the same string so consistency is
                // self-contained as long as we use the same placeholder
                // both times.
                parse_blocks("__diff__", &body)
            }
            Err(_) => Vec::new(),
        }
    }
    let prev_blocks = parse_body_blocks(prev);
    let next_blocks = parse_body_blocks(next);

    let mut flipped = Vec::new();
    for nb in &next_blocks {
        if nb.properties.get("status").map(|s| s.as_str()) != Some("done") {
            continue;
        }
        let was_done = prev_blocks
            .iter()
            .find(|pb| pb.text == nb.text && pb.raw_text == nb.raw_text)
            .and_then(|pb| pb.properties.get("status"))
            .map(|s| s.as_str())
            == Some("done");
        if !was_done {
            // Resolve the block id by parsing `next` against the real
            // note id so the bumper finds the right block.
            // Reconstruct: the placeholder __diff__ in id is specific to
            // this diff pass; the caller's `update_note` will re-parse
            // with the actual note id when calling try_bump_block.
            // Pass back the *original* next-side block.id for now —
            // try_bump_block re-parses using the prefix from the id.
            flipped.push(nb.id.clone());
        }
    }
    flipped
}

/// Phase 12.4 — same-note dependency unblock. After the bumps applied,
/// look for blocks that became unblocked because one of their blockers
/// just flipped to `done` *in this PUT*. If a block's status is `backlog`
/// and no remaining blocker is incomplete, advance it to `todo`.
///
/// Returns the rewritten content + the list of unblocked block ids so
/// the caller can log them. Cross-note dependency walking is deferred —
/// users with cross-note `blocked_by::` will see the unblock take effect
/// the next time the dependent's own note is re-saved (or they manually
/// edit it). v1.1 will add a reverse-index walk for cross-note unblock.
pub fn apply_dependency_cycles(prev: &str, next: &str, note_id: &str) -> (String, Vec<String>) {
    let flipped_to_done = detect_status_flips_to_done(prev, next);
    if flipped_to_done.is_empty() {
        return (next.to_string(), Vec::new());
    }

    // Map __diff__ ids → real note_id ids so the dependency check can match
    // `<note_id>:<line>` references inside `blocked_by::` values verbatim.
    let just_done: std::collections::HashSet<String> = flipped_to_done
        .iter()
        .filter_map(|id| {
            id.rsplit_once(':')
                .map(|(_, l)| format!("{}:{}", note_id, l))
        })
        .collect();

    let (_meta, body) = match parse_frontmatter(next) {
        Ok(b) => b,
        Err(_) => return (next.to_string(), Vec::new()),
    };
    let blocks = parse_blocks(note_id, &body);
    let block_index: std::collections::HashMap<&str, &ParsedBlock> =
        blocks.iter().map(|b| (b.id.as_str(), b)).collect();

    let mut to_unblock: Vec<(String, usize)> = Vec::new();
    for block in &blocks {
        if block.properties.get("status").map(String::as_str) != Some("backlog") {
            continue;
        }
        let Some(blocked_by_raw) = block.properties.get("blocked_by") else {
            continue;
        };
        let refs: Vec<String> = blocked_by_raw
            .split(',')
            .map(|s| {
                s.trim()
                    .trim_start_matches("[[")
                    .trim_end_matches("]]")
                    .to_string()
            })
            .filter(|s| !s.is_empty())
            .collect();
        if refs.is_empty() {
            continue;
        }
        let any_changed = refs.iter().any(|r| just_done.contains(r));
        if !any_changed {
            continue;
        }
        // Recheck: are *all* blockers now done?
        let still_blocked = refs.iter().any(|r| {
            // Same-note ref → look up; missing or non-done → still blocked.
            // External ref (different note id) → conservatively still blocked.
            let target = block_index.get(r.as_str());
            match target {
                Some(t) => t.properties.get("status").map(String::as_str) != Some("done"),
                None => true,
            }
        });
        if !still_blocked {
            let line = block
                .id
                .rsplit_once(':')
                .and_then(|(_, l)| l.parse().ok())
                .unwrap_or(0);
            to_unblock.push((block.id.clone(), line));
        }
    }

    if to_unblock.is_empty() {
        return (next.to_string(), Vec::new());
    }

    // Rewrite each unblocked block's `status:: backlog` → `status:: todo`.
    let mut new_body = body.clone();
    let mut unblocked_ids = Vec::new();
    for (block_id, line) in to_unblock {
        if let Some(rewritten) = set_status_to_todo(&new_body, line) {
            new_body = rewritten;
            unblocked_ids.push(block_id);
        }
    }

    let new_content = reassemble_content(next, &body, &new_body);
    (new_content, unblocked_ids)
}

/// Find the `status::` continuation line under the bullet at `bullet_line`
/// and rewrite it to `status:: todo`. Idempotent on already-todo. Returns
/// `None` when no `status::` line is found within the block's continuation
/// range, which signals the caller to skip rather than silently mis-edit.
fn set_status_to_todo(body: &str, bullet_line: usize) -> Option<String> {
    let trailing_newline = body.ends_with('\n');
    let (mut lines, end, _) = block_range(body, bullet_line)?;
    let fenced = block_fence_mask(&lines, bullet_line, end);

    for i in (bullet_line + 1)..end {
        if fenced[i] {
            continue;
        }
        let line = &lines[i];
        let trim = line.trim_start();
        if trim.strip_prefix("status::").is_some() {
            let indent = line.len() - trim.len();
            let prefix: String = " ".repeat(indent);
            lines[i] = format!("{}status:: todo", prefix);
            return Some(join_lines(lines, trailing_newline));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Shared recurrence date-step helper
// ---------------------------------------------------------------------------

/// Outcome of stepping a recurring block forward by one occurrence.
///
/// `new_deadline` / `new_scheduled` are `None` when the block had no
/// corresponding date field.  The `next_iso` is the stepped date from
/// whichever field was preferred (deadline > scheduled).
///
/// When the series is exhausted `advance` returns `None`; callers that only
/// need the `spent` path can check `is_active` or match on
/// `RecurrenceStep::active_fields()`.
struct RecurrenceStep {
    /// Parsed recurrence rule (needed by neither caller after this point, but
    /// returned for completeness / future use).
    #[allow(dead_code)]
    rec: Recurrence,
    /// Anchor date used for `recurrence::advance` (deadline, else scheduled).
    anchor_date: chrono::NaiveDate,
    /// `recurrence_done` counter value *before* this occurrence.
    #[allow(dead_code)]
    done_so_far: u32,
    /// `done_so_far + 1` — the value to write back.
    new_done: u32,
    /// `Some(...)` when the series is still active after this step.
    /// Contains the new formatted deadline / scheduled strings and the
    /// ISO date string for the response.
    active: Option<ActiveStep>,
}

struct ActiveStep {
    /// New `deadline::` string (formatted), or `None` if the block had none.
    new_deadline: Option<String>,
    /// New `scheduled::` string (formatted), or `None` if the block had none.
    new_scheduled: Option<String>,
    /// ISO `YYYY-MM-DD` of the stepped date (deadline preferred, else scheduled).
    next_iso: String,
}

/// Compute the shared recurrence step from a parsed block.
///
/// Returns `None` if the block has no parseable `recurring::` property or
/// no parseable anchor date (deadline / scheduled).
fn compute_recurrence_step(block: &ParsedBlock) -> Option<RecurrenceStep> {
    let recurring_str = block.properties.get("recurring")?;
    let rec: Recurrence = recurrence::parse(recurring_str)?;

    // Anchor: prefer deadline::, fall back to scheduled::.
    let anchor_date = {
        let from_deadline = block
            .properties
            .get("deadline")
            .and_then(|v| parse_deadline_value(v))
            .map(|(d, _)| d);
        let from_scheduled = block
            .properties
            .get("scheduled")
            .and_then(|v| parse_deadline_value(v))
            .map(|(d, _)| d);
        from_deadline.or(from_scheduled)?
    };

    let done_so_far: u32 = block
        .properties
        .get("recurrence_done")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);

    let new_done = done_so_far + 1;

    let active = match recurrence::advance(&rec, anchor_date, done_so_far) {
        None => None,
        Some(_) => {
            // Step each date field from its own current value.
            let new_deadline = block.properties.get("deadline").and_then(|v| {
                let (d, t) = parse_deadline_value(v)?;
                let nd = recurrence::next_after(&rec, d);
                Some(format_deadline(nd, t.as_deref()))
            });
            let new_scheduled = block.properties.get("scheduled").and_then(|v| {
                let (d, t) = parse_deadline_value(v)?;
                let nd = recurrence::next_after(&rec, d);
                Some(format_deadline(nd, t.as_deref()))
            });

            // Derive next_iso directly from the stepped NaiveDate that
            // parse_deadline_value already returned — no string round-trip.
            let next_iso = block
                .properties
                .get("deadline")
                .and_then(|v| {
                    let (d, _) = parse_deadline_value(v)?;
                    Some(
                        recurrence::next_after(&rec, d)
                            .format("%Y-%m-%d")
                            .to_string(),
                    )
                })
                .or_else(|| {
                    block.properties.get("scheduled").and_then(|v| {
                        let (d, _) = parse_deadline_value(v)?;
                        Some(
                            recurrence::next_after(&rec, d)
                                .format("%Y-%m-%d")
                                .to_string(),
                        )
                    })
                })?;

            Some(ActiveStep {
                new_deadline,
                new_scheduled,
                next_iso,
            })
        }
    };

    Some(RecurrenceStep {
        rec,
        anchor_date,
        done_so_far,
        new_done,
        active,
    })
}

/// Parse a `deadline::` value into `(date, optional_time_suffix)`. Accepts
/// `[[YYYY-MM-DD]]`, `YYYY-MM-DD`, with an optional trailing `HH:mm` time.
/// The time suffix (e.g. ` 10:30`) is preserved verbatim so the bumped
/// deadline carries the same time-of-day forward.
fn parse_deadline_value(v: &str) -> Option<(chrono::NaiveDate, Option<String>)> {
    let trimmed = v.trim();
    let (date_part, time_part) = match trimmed.find(' ') {
        Some(idx) => (trimmed[..idx].trim(), Some(trimmed[idx..].to_string())),
        None => (trimmed, None),
    };
    let bare = date_part
        .strip_prefix("[[")
        .and_then(|s| s.strip_suffix("]]"))
        .unwrap_or(date_part);
    let mut parts = bare.split('-');
    let y: i32 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    let date = chrono::NaiveDate::from_ymd_opt(y, m, d)?;
    Some((date, time_part))
}

/// Build a `[[YYYY-MM-DD]]` value with the same trailing time the original
/// had, so a deadline like `[[2026-05-01]] 10:00` stays timed after the bump.
fn format_deadline(date: chrono::NaiveDate, time_suffix: Option<&str>) -> String {
    let base = format!("[[{}]]", date.format("%Y-%m-%d"));
    match time_suffix {
        Some(t) => format!("{}{}", base, t),
        None => base,
    }
}

// ---------------------------------------------------------------------------
// Block-rewrite helpers (shared by complete / skip / spent paths)
// ---------------------------------------------------------------------------

/// Shared block-boundary scanner. Returns `(lines, end_index, cont_indent)`
/// where `end_index` is the first line after the block's continuation range
/// (exclusive upper bound for in-place mutation).
fn block_range(body: &str, block_line_num: usize) -> Option<(Vec<String>, usize, String)> {
    let lines: Vec<String> = body.lines().map(String::from).collect();
    if block_line_num >= lines.len() {
        return None;
    }
    let block_line = &lines[block_line_num];
    let trim_start = block_line.trim_start();
    if !(trim_start.starts_with("- ") || trim_start.trim_end() == "-") {
        return None;
    }
    let block_indent_spaces = block_line.len() - trim_start.len();
    let expected_continuation = block_indent_spaces + 2;
    let bullet_text = trim_start
        .strip_prefix("- ")
        .or_else(|| trim_start.strip_prefix('-'))
        .unwrap_or(trim_start);
    let visible = crate::note_tree::strip_bid_comment(bullet_text);
    let mut fence = crate::note_tree::MarkdownFenceTracker::default();
    fence.line_is_fenced(&visible);

    let mut end = lines.len();
    for (i, l) in lines.iter().enumerate().skip(block_line_num + 1) {
        let fence_content = l
            .as_bytes()
            .get(..expected_continuation)
            .filter(|prefix| prefix.iter().all(|byte| *byte == b' '))
            .map(|_| &l[expected_continuation..])
            .unwrap_or_else(|| l.trim_start());
        if fence.line_is_fenced(fence_content) {
            continue;
        }
        let t = l.trim_start();
        if t.is_empty() {
            continue;
        }
        let l_indent = l.len() - t.len();
        let is_bullet = t.starts_with("- ") || t.trim_end() == "-";
        if is_bullet && l_indent <= block_indent_spaces {
            end = i;
            break;
        }
    }

    let cont_indent = " ".repeat(block_indent_spaces + 2);
    Some((lines, end, cont_indent))
}

fn block_fence_mask(lines: &[String], block_line_num: usize, end: usize) -> Vec<bool> {
    let mut mask = vec![false; lines.len()];
    if block_line_num >= end || block_line_num >= lines.len() {
        return mask;
    }
    let bullet = lines[block_line_num].trim_start();
    let bullet_text = bullet
        .strip_prefix("- ")
        .or_else(|| bullet.strip_prefix('-'))
        .unwrap_or(bullet);
    let visible = crate::note_tree::strip_bid_comment(bullet_text);
    let mut fence = crate::note_tree::MarkdownFenceTracker::default();
    mask[block_line_num] = fence.line_is_fenced(&visible);
    let block_indent = lines[block_line_num].len() - bullet.len();
    let expected = block_indent + 2;
    for i in (block_line_num + 1)..end.min(lines.len()) {
        let line = &lines[i];
        let content = line
            .as_bytes()
            .get(..expected)
            .filter(|prefix| prefix.iter().all(|byte| *byte == b' '))
            .map(|_| &line[expected..])
            .unwrap_or_else(|| line.trim_start());
        mask[i] = fence.line_is_fenced(content);
    }
    mask
}

/// Finish a `block_range` mutation: join lines, restore trailing newline.
fn join_lines(lines: Vec<String>, trailing_newline: bool) -> String {
    let mut out = lines.join("\n");
    if trailing_newline {
        out.push('\n');
    }
    out
}

/// `complete` mode: reset `status:: todo`, advance `deadline::` and/or
/// `scheduled::`, stamp `last_completed::`, update/insert `recurrence_done::`.
fn rewrite_block_for_complete(
    body: &str,
    block_line_num: usize,
    new_deadline: Option<&str>,
    new_scheduled: Option<&str>,
    last_completed: &str,
    new_done: u32,
) -> Option<String> {
    let trailing_newline = body.ends_with('\n');
    let (mut lines, end, cont_indent) = block_range(body, block_line_num)?;
    let fenced = block_fence_mask(&lines, block_line_num, end);

    let mut updated_status = false;
    let mut updated_deadline = false;
    let mut updated_scheduled = false;
    let mut updated_last_completed = false;
    let mut updated_recurrence_done = false;

    for (i, line) in lines
        .iter_mut()
        .enumerate()
        .take(end)
        .skip(block_line_num + 1)
    {
        if fenced[i] {
            continue;
        }
        if let Some((key, _)) = property_kv(line) {
            match key.as_str() {
                "status" => {
                    *line = format!("{}status:: todo", cont_indent);
                    updated_status = true;
                }
                "deadline" => {
                    if let Some(nd) = new_deadline {
                        *line = format!("{}deadline:: {}", cont_indent, nd);
                        updated_deadline = true;
                    }
                }
                "scheduled" => {
                    if let Some(ns) = new_scheduled {
                        *line = format!("{}scheduled:: {}", cont_indent, ns);
                        updated_scheduled = true;
                    }
                }
                "last_completed" => {
                    *line = format!("{}last_completed:: {}", cont_indent, last_completed);
                    updated_last_completed = true;
                }
                "recurrence_done" => {
                    *line = format!("{}recurrence_done:: {}", cont_indent, new_done);
                    updated_recurrence_done = true;
                }
                _ => {}
            }
        }
    }

    let mut additions: Vec<String> = Vec::new();
    if !updated_status {
        additions.push(format!("{}status:: todo", cont_indent));
    }
    if !updated_deadline {
        if let Some(nd) = new_deadline {
            additions.push(format!("{}deadline:: {}", cont_indent, nd));
        }
    }
    if !updated_scheduled {
        if let Some(ns) = new_scheduled {
            additions.push(format!("{}scheduled:: {}", cont_indent, ns));
        }
    }
    if !updated_last_completed {
        additions.push(format!(
            "{}last_completed:: {}",
            cont_indent, last_completed
        ));
    }
    if !updated_recurrence_done {
        additions.push(format!("{}recurrence_done:: {}", cont_indent, new_done));
    }
    for (offset, add) in additions.into_iter().enumerate() {
        lines.insert(end + offset, add);
    }

    Some(join_lines(lines, trailing_newline))
}

/// `skip` mode: advance `deadline::` and/or `scheduled::`, increment
/// `recurrence_done::`. Does NOT touch `status::` or `last_completed::`.
fn rewrite_block_for_skip(
    body: &str,
    block_line_num: usize,
    new_deadline: Option<&str>,
    new_scheduled: Option<&str>,
    new_done: u32,
) -> Option<String> {
    let trailing_newline = body.ends_with('\n');
    let (mut lines, end, cont_indent) = block_range(body, block_line_num)?;
    let fenced = block_fence_mask(&lines, block_line_num, end);

    let mut updated_deadline = false;
    let mut updated_scheduled = false;
    let mut updated_recurrence_done = false;

    for (i, line) in lines
        .iter_mut()
        .enumerate()
        .take(end)
        .skip(block_line_num + 1)
    {
        if fenced[i] {
            continue;
        }
        if let Some((key, _)) = property_kv(line) {
            match key.as_str() {
                "deadline" => {
                    if let Some(nd) = new_deadline {
                        *line = format!("{}deadline:: {}", cont_indent, nd);
                        updated_deadline = true;
                    }
                }
                "scheduled" => {
                    if let Some(ns) = new_scheduled {
                        *line = format!("{}scheduled:: {}", cont_indent, ns);
                        updated_scheduled = true;
                    }
                }
                "recurrence_done" => {
                    *line = format!("{}recurrence_done:: {}", cont_indent, new_done);
                    updated_recurrence_done = true;
                }
                _ => {}
            }
        }
    }

    let mut additions: Vec<String> = Vec::new();
    if !updated_deadline {
        if let Some(nd) = new_deadline {
            additions.push(format!("{}deadline:: {}", cont_indent, nd));
        }
    }
    if !updated_scheduled {
        if let Some(ns) = new_scheduled {
            additions.push(format!("{}scheduled:: {}", cont_indent, ns));
        }
    }
    if !updated_recurrence_done {
        additions.push(format!("{}recurrence_done:: {}", cont_indent, new_done));
    }
    for (offset, add) in additions.into_iter().enumerate() {
        lines.insert(end + offset, add);
    }

    Some(join_lines(lines, trailing_newline))
}

/// `spent` mode: series exhausted — only update `recurrence_done::`. Does not
/// touch dates, `status::`, or `last_completed::`.
fn rewrite_block_for_spent(body: &str, block_line_num: usize, new_done: u32) -> Option<String> {
    let trailing_newline = body.ends_with('\n');
    let (mut lines, end, cont_indent) = block_range(body, block_line_num)?;
    let fenced = block_fence_mask(&lines, block_line_num, end);

    let mut updated_recurrence_done = false;
    for (i, line) in lines
        .iter_mut()
        .enumerate()
        .take(end)
        .skip(block_line_num + 1)
    {
        if fenced[i] {
            continue;
        }
        if let Some((key, _)) = property_kv(line) {
            if key == "recurrence_done" {
                *line = format!("{}recurrence_done:: {}", cont_indent, new_done);
                updated_recurrence_done = true;
            }
        }
    }
    if !updated_recurrence_done {
        lines.insert(
            end,
            format!("{}recurrence_done:: {}", cont_indent, new_done),
        );
    }

    Some(join_lines(lines, trailing_newline))
}

/// Match an indented `key:: value` line. Returns `(key, value)` lowercased
/// key plus raw value (trimmed). Only call on continuation lines.
///
/// `pub` (unlike the other block-rewrite internals in this module):
/// `tesela-server`'s `strip_block_intext_prop` also needs this exact parse
/// to strip an in-text property line, so it's a shared leaf helper rather
/// than lifecycle-module-private.
pub fn property_kv(line: &str) -> Option<(String, String)> {
    let trim = line.trim_start();
    let (k, v) = trim.split_once("::")?;
    let key = k.trim();
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let value = v.trim_start_matches([' ', '\t']).trim_end().to_string();
    Some((key.to_lowercase(), value))
}

/// Replace the body portion of `original_content` with `new_body`.
/// Frontmatter (everything before the body in the source content) is
/// preserved verbatim.
fn reassemble_content(original_content: &str, original_body: &str, new_body: &str) -> String {
    if original_content.ends_with(original_body) {
        let prefix_len = original_content.len() - original_body.len();
        let mut out = String::with_capacity(prefix_len + new_body.len());
        out.push_str(&original_content[..prefix_len]);
        out.push_str(new_body);
        return out;
    }
    // Fallback: substring replace. Safe because `body` is unique in
    // `content` for any well-formed note (frontmatter delimiters never
    // appear inside the body).
    original_content.replacen(original_body, new_body, 1)
}

// ---------------------------------------------------------------------------
// Tag auto-create: pure tag-collection + template pieces
// ---------------------------------------------------------------------------

/// Collect every tag a note should auto-create a page for: frontmatter
/// `tags:`, inline `#tag` text in the body, and block-level `tags:: a, b`
/// continuation lines (the chip-tag gesture writes only these — no
/// frontmatter tag, no inline `#tag` — so without scanning them a
/// committed chip tag would never materialize a Tag page). Case-insensitive
/// de-dup against tags already collected.
///
/// Pure: no I/O, no store lookups. The caller (`ensure_tag_pages` in
/// `tesela-server`) does the slug resolution + page creation, and still
/// owns skipping the `daily` tag.
pub fn collect_note_tags(note: &Note) -> Vec<String> {
    let mut all_tags: Vec<String> = note.metadata.tags.clone();
    let indexable_body = crate::note_tree::unfenced_markdown(&note.body);

    let tag_re = regex::Regex::new(r"#([A-Za-z][A-Za-z0-9_-]*)").unwrap();
    for cap in tag_re.captures_iter(&indexable_body) {
        let tag = cap[1].to_string();
        if !all_tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
            all_tags.push(tag);
        }
    }

    let block_tags_re = regex::Regex::new(r"(?m)^\s*tags::\s*(.+)$").unwrap();
    for cap in block_tags_re.captures_iter(&indexable_body) {
        for raw in cap[1].split(',') {
            let tag = raw.trim();
            if !tag.is_empty() && !all_tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                all_tags.push(tag.to_string());
            }
        }
    }

    all_tags
}

/// Markdown content for a newly auto-created tag page. `type: tag`
/// (lowercase, bare) is the canonical form per the tag-system spec.
pub fn tag_page_content(tag: &str) -> String {
    format!(
        "---\ntitle: \"{}\"\ntype: tag\nextends: \"Root Tag\"\ntag_properties: []\nparent: \"\"\ntags: []\n---\n- Tag properties are inherited by all nodes using the tag.\n",
        tag
    )
}
#[cfg(test)]
mod recurrence_tests {
    use super::*;

    /// Helper: extract a named property from a block identified by block_id
    /// in the given content string.
    fn get_prop(content: &str, block_id: &str, key: &str) -> Option<String> {
        let (note_id_str, _) = block_id.rsplit_once(':')?;
        let (_meta, body) = parse_frontmatter(content).ok()?;
        let blocks = parse_blocks(note_id_str, &body);
        let block = blocks.iter().find(|b| b.id == block_id)?;
        block.properties.get(key).cloned()
    }

    /// Build a synthetic note content string where the task block is on
    /// body line 0 (so block_id is `"note:0"`).
    fn make_note(body_extra_props: &[(&str, &str)]) -> String {
        let mut lines = vec![
            "---".to_string(),
            "title: \"Test\"".to_string(),
            "tags: []".to_string(),
            "---".to_string(),
            "- task".to_string(),
            "  recurring:: daily count 2".to_string(),
            "  deadline:: [[2026-05-07]]".to_string(),
            "  scheduled:: [[2026-05-06]]".to_string(),
            "  status:: todo".to_string(),
        ];
        for (k, v) in body_extra_props {
            lines.push(format!("  {}:: {}", k, v));
        }
        lines.join("\n") + "\n"
    }

    /// Block id: note body starts after the frontmatter (4 header lines),
    /// but `parse_blocks` operates on the *body* slice and assigns
    /// line numbers relative to the body. The bullet `- task` is on body
    /// line 0, so block_id is `"note:0"`.
    const BLOCK_ID: &str = "note:0";

    // -----------------------------------------------------------------------
    // Task 6 core test: multi-field anchor + recurrence_done + spent series
    // -----------------------------------------------------------------------

    #[test]
    fn recurrence_first_done_advances_both_dates_and_stamps_counter() {
        // Start: todo, daily count 2, deadline 2026-05-07, scheduled 2026-05-06
        let content = make_note(&[]);

        // Flip to done (simulate what the client would PUT).
        let content_with_done = content.replace("status:: todo", "status:: done");

        // First complete.
        let (bumped1, next_iso1) =
            try_bump_block(&content_with_done, BLOCK_ID).expect("bump should succeed");

        assert_eq!(next_iso1, "2026-05-08", "deadline next date");
        // deadline advanced from 2026-05-07 → 2026-05-08
        assert_eq!(
            get_prop(&bumped1, BLOCK_ID, "deadline").as_deref(),
            Some("[[2026-05-08]]"),
            "deadline advanced"
        );
        // scheduled advanced from 2026-05-06 → 2026-05-07
        assert_eq!(
            get_prop(&bumped1, BLOCK_ID, "scheduled").as_deref(),
            Some("[[2026-05-07]]"),
            "scheduled advanced"
        );
        // recurrence_done stamped to 1
        assert_eq!(
            get_prop(&bumped1, BLOCK_ID, "recurrence_done").as_deref(),
            Some("1"),
            "recurrence_done = 1"
        );
        // status reset to todo
        assert_eq!(
            get_prop(&bumped1, BLOCK_ID, "status").as_deref(),
            Some("todo"),
            "status reset to todo"
        );
        // last_completed stamped with the prior anchor
        assert_eq!(
            get_prop(&bumped1, BLOCK_ID, "last_completed").as_deref(),
            Some("[[2026-05-07]]"),
            "last_completed = prior anchor"
        );
    }

    #[test]
    fn recurrence_second_done_exhausts_series() {
        // Build content as if the first bump already happened:
        // deadline 2026-05-08, scheduled 2026-05-07, recurrence_done 1, status todo.
        let content_after_first = {
            let base = make_note(&[("recurrence_done", "1")]);
            base.replace("deadline:: [[2026-05-07]]", "deadline:: [[2026-05-08]]")
                .replace("scheduled:: [[2026-05-06]]", "scheduled:: [[2026-05-07]]")
        };
        // Flip to done again.
        let content_with_done2 = content_after_first.replace("status:: todo", "status:: done");

        // Second complete — series is now spent (count 2, done_so_far=1 → advance returns None).
        let (bumped2, _iso) = try_bump_block(&content_with_done2, BLOCK_ID)
            .expect("bump returns Some even when spent");

        // status stays done
        assert_eq!(
            get_prop(&bumped2, BLOCK_ID, "status").as_deref(),
            Some("done"),
            "status stays done when series is spent"
        );
        // deadline unchanged
        assert_eq!(
            get_prop(&bumped2, BLOCK_ID, "deadline").as_deref(),
            Some("[[2026-05-08]]"),
            "deadline unchanged after spent"
        );
        // scheduled unchanged
        assert_eq!(
            get_prop(&bumped2, BLOCK_ID, "scheduled").as_deref(),
            Some("[[2026-05-07]]"),
            "scheduled unchanged after spent"
        );
        // recurrence_done bumped to 2
        assert_eq!(
            get_prop(&bumped2, BLOCK_ID, "recurrence_done").as_deref(),
            Some("2"),
            "recurrence_done = 2 after series is spent"
        );
        // recurring:: property preserved (not stripped)
        assert_eq!(
            get_prop(&bumped2, BLOCK_ID, "recurring").as_deref(),
            Some("daily count 2"),
            "recurring:: property preserved"
        );
    }

    // -----------------------------------------------------------------------
    // skip mode test
    // -----------------------------------------------------------------------

    #[test]
    fn skip_mode_advances_dates_without_touching_status_or_last_completed() {
        // Start in todo state (skip does not require done).
        let content = make_note(&[]);

        let (skipped, next_iso) = try_skip_block(&content, BLOCK_ID).expect("skip should succeed");

        assert_eq!(next_iso, "2026-05-08");
        // dates advanced
        assert_eq!(
            get_prop(&skipped, BLOCK_ID, "deadline").as_deref(),
            Some("[[2026-05-08]]"),
        );
        assert_eq!(
            get_prop(&skipped, BLOCK_ID, "scheduled").as_deref(),
            Some("[[2026-05-07]]"),
        );
        // recurrence_done incremented
        assert_eq!(
            get_prop(&skipped, BLOCK_ID, "recurrence_done").as_deref(),
            Some("1"),
        );
        // status NOT changed — remains todo
        assert_eq!(
            get_prop(&skipped, BLOCK_ID, "status").as_deref(),
            Some("todo"),
            "status must not be modified by skip"
        );
        // last_completed NOT stamped
        assert_eq!(
            get_prop(&skipped, BLOCK_ID, "last_completed"),
            None,
            "last_completed must not be stamped by skip"
        );
    }

    // -----------------------------------------------------------------------
    // Regression: unbounded series never exhausts
    // -----------------------------------------------------------------------

    #[test]
    fn unbounded_series_always_advances() {
        let content = {
            let lines = vec![
                "---\ntitle: \"T\"\ntags: []\n---",
                "- task",
                "  recurring:: daily",
                "  deadline:: [[2026-05-07]]",
                "  status:: done",
            ];
            lines.join("\n") + "\n"
        };

        let (bumped, iso) = try_bump_block(&content, BLOCK_ID).expect("should bump");
        assert_eq!(iso, "2026-05-08");
        assert_eq!(
            get_prop(&bumped, BLOCK_ID, "status").as_deref(),
            Some("todo")
        );
        assert_eq!(
            get_prop(&bumped, BLOCK_ID, "recurrence_done").as_deref(),
            Some("1")
        );
    }
}

#[cfg(test)]
mod idempotence_guard_tests {
    //! tesela-ows.1 step 2, Lead constraint (a): the engine-path
    //! [`apply_post_save_bumps_guarded`] must bump a genuine first/next
    //! completion but SKIP an occurrence whose completion is already recorded
    //! (a concurrently- or re-delivered done-flip). The guard anchors on the
    //! EARLIER of `prev`'s and `next`'s occurrence date, so it is robust whether
    //! a concurrent peer's roll advanced `next`'s deadline (the pre-import peer
    //! is behind) OR `prev`'s deadline (the pre-import peer already rolled and is
    //! importing a stale duplicate completion).
    use super::*;

    fn get_prop(content: &str, key: &str) -> Option<String> {
        let (_meta, body) = parse_frontmatter(content).ok()?;
        let blocks = parse_blocks("note", &body);
        blocks.first()?.properties.get(key).cloned()
    }

    /// Body line 0 holds the recurring bullet; extra props append after it.
    fn note(extra: &[(&str, &str)], status: &str) -> String {
        let mut lines = vec![
            "---".to_string(),
            "title: \"T\"".to_string(),
            "tags: []".to_string(),
            "---".to_string(),
            "- task".to_string(),
            "  recurring:: daily count 3".to_string(),
            "  deadline:: [[2026-05-07]]".to_string(),
            format!("  status:: {status}"),
        ];
        for (k, v) in extra {
            lines.push(format!("  {k}:: {v}"));
        }
        lines.join("\n") + "\n"
    }

    #[test]
    fn guarded_bumps_first_completion_no_last_completed() {
        let prev = note(&[], "todo");
        let next = note(&[], "done");
        let (out, bumps) = apply_post_save_bumps_guarded(&prev, &next, "note");
        assert_eq!(bumps.len(), 1, "first completion bumps");
        assert_eq!(get_prop(&out, "deadline").as_deref(), Some("[[2026-05-08]]"));
        assert_eq!(get_prop(&out, "recurrence_done").as_deref(), Some("1"));
        assert_eq!(get_prop(&out, "status").as_deref(), Some("todo"));
        assert_eq!(
            get_prop(&out, "last_completed").as_deref(),
            Some("[[2026-05-07]]")
        );
    }

    #[test]
    fn guarded_skips_occurrence_already_completed() {
        // `last_completed` already equals the current deadline anchor: this
        // occurrence is done. A re-delivered done-flip must NOT re-bump.
        let prev = note(&[("last_completed", "[[2026-05-07]]")], "todo");
        let next = note(&[("last_completed", "[[2026-05-07]]")], "done");
        let (out, bumps) = apply_post_save_bumps_guarded(&prev, &next, "note");
        assert!(bumps.is_empty(), "guard blocks the already-completed bump");
        assert_eq!(out, next, "content unchanged when the guard trips");
        // Sanity: the UNGUARDED path WOULD bump (proving the guard is what
        // suppressed it, not a missing flip).
        let (unguarded, u_bumps) = apply_post_save_bumps_with_info(&prev, &next, "note");
        assert_eq!(u_bumps.len(), 1);
        assert_ne!(unguarded, next);
    }

    #[test]
    fn guarded_bumps_next_occurrence_after_prior_completion() {
        // `last_completed` (2026-05-06) strictly precedes the current deadline
        // anchor (2026-05-07): a genuine NEXT completion — must bump.
        let prev = note(&[("last_completed", "[[2026-05-06]]")], "todo");
        let next = note(&[("last_completed", "[[2026-05-06]]")], "done");
        let (out, bumps) = apply_post_save_bumps_guarded(&prev, &next, "note");
        assert_eq!(bumps.len(), 1, "next occurrence bumps");
        assert_eq!(get_prop(&out, "deadline").as_deref(), Some("[[2026-05-08]]"));
        assert_eq!(
            get_prop(&out, "last_completed").as_deref(),
            Some("[[2026-05-07]]")
        );
    }

    #[test]
    fn guarded_skips_when_next_deadline_already_advanced_by_concurrent_roll() {
        // A crossed duplicate completion of the SAME occurrence must NOT
        // double-advance. `prev` is the pre-import peer at occurrence O1
        // (deadline 05-07, no last_completed). The merged `next` frame carries a
        // CONCURRENT peer's roll — already advanced the deadline to 05-08 and
        // stamped last_completed 05-07 — PLUS a fresh done-flip. The occurrence
        // actually being completed is O1 (05-07), already covered.
        //
        // Anchoring on `next`'s ADVANCED deadline (05-08) would see 05-07 < 05-08
        // → "uncompleted" → bump AGAIN (to 05-09). The min-anchor picks O1
        // (05-07 from prev) and no-ops.
        let prev = note(&[], "todo");
        let next = {
            let base = note(&[("last_completed", "[[2026-05-07]]")], "done");
            base.replace("deadline:: [[2026-05-07]]", "deadline:: [[2026-05-08]]")
        };
        let (out, bumps) = apply_post_save_bumps_guarded(&prev, &next, "note");
        assert!(
            bumps.is_empty(),
            "duplicate completion of the already-rolled occurrence must not bump"
        );
        assert_eq!(out, next, "content unchanged when the guard trips");
        assert_eq!(
            get_prop(&out, "deadline").as_deref(),
            Some("[[2026-05-08]]"),
            "series stays at O2 (05-08); no second advance to 05-09"
        );
        // Sanity: the UNGUARDED path WOULD double-advance to 05-09.
        let (unguarded, u_bumps) = apply_post_save_bumps_with_info(&prev, &next, "note");
        assert_eq!(u_bumps.len(), 1);
        assert_eq!(
            get_prop(&unguarded, "deadline").as_deref(),
            Some("[[2026-05-09]]"),
            "unguarded path double-advances (the bug the guard prevents)"
        );
    }

    #[test]
    fn guarded_skips_when_prev_deadline_already_advanced_by_local_roll() {
        // The MIRROR of the above (the case eb0de36d's prev-only anchor got
        // wrong): the pre-import peer ALREADY rolled to O2 (prev deadline 05-08,
        // last_completed 05-07) and is importing a stale disjoint-twin completion
        // of O1 that a twin-heal union surfaced as `next` (deadline 05-07, done,
        // last_completed 05-07). The occurrence being completed is O1 (05-07),
        // already covered.
        //
        // Anchoring on `prev`'s ADVANCED deadline (05-08) would see 05-07 < 05-08
        // → "uncompleted" → bump AGAIN. The min-anchor picks O1 (05-07 from next)
        // and no-ops.
        let prev = {
            let base = note(&[("last_completed", "[[2026-05-07]]")], "todo");
            base.replace("deadline:: [[2026-05-07]]", "deadline:: [[2026-05-08]]")
        };
        let next = note(&[("last_completed", "[[2026-05-07]]")], "done");
        let (out, bumps) = apply_post_save_bumps_guarded(&prev, &next, "note");
        assert!(
            bumps.is_empty(),
            "stale duplicate completion of the already-rolled occurrence must not bump"
        );
        assert_eq!(out, next, "content unchanged when the guard trips");
        assert_eq!(
            get_prop(&out, "deadline").as_deref(),
            Some("[[2026-05-07]]"),
            "no advance past the completed occurrence"
        );
        // Sanity: the UNGUARDED path WOULD advance (05-07 → 05-08).
        let (unguarded, u_bumps) = apply_post_save_bumps_with_info(&prev, &next, "note");
        assert_eq!(u_bumps.len(), 1);
        assert_eq!(
            get_prop(&unguarded, "deadline").as_deref(),
            Some("[[2026-05-08]]")
        );
    }
}

#[cfg(test)]
mod engine_lifecycle_ops_tests {
    //! tesela-ows.1 step 2, Lead constraint (a): [`compute_lifecycle_container_sets`]
    //! returns the per-block CONTAINER prop sets a `done` flip should apply,
    //! keyed by the block's canonical bid, WITHOUT rewriting markdown.
    use super::*;

    const BID: &str = "07070707-0707-0707-0707-070707070707";

    fn note(status: &str, extra: &[(&str, &str)]) -> String {
        let mut lines = vec![
            "---".to_string(),
            "title: \"T\"".to_string(),
            "tags: []".to_string(),
            "---".to_string(),
            format!("- water plants <!-- bid:{BID} -->"),
            "  recurring:: daily count 3".to_string(),
            "  deadline:: [[2026-05-07]]".to_string(),
            format!("  status:: {status}"),
        ];
        for (k, v) in extra {
            lines.push(format!("  {k}:: {v}"));
        }
        lines.join("\n") + "\n"
    }

    #[test]
    fn done_flip_yields_container_sets_for_the_roll() {
        let prev = note("todo", &[]);
        let next = note("done", &[]);
        let rolls = compute_lifecycle_container_sets(&prev, &next, "note");
        assert_eq!(rolls.len(), 1, "one block rolls");
        let r = &rolls[0];
        assert_eq!(r.bid.as_deref(), Some(BID), "carries the canonical bid");
        let get = |k: &str| {
            r.props
                .iter()
                .find(|(pk, _)| pk == k)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("status"), Some("todo"), "status rolls back to todo");
        assert_eq!(get("deadline"), Some("[[2026-05-08]]"), "deadline advances");
        assert_eq!(get("recurrence_done"), Some("1"), "counter stamped");
        assert_eq!(
            get("last_completed"),
            Some("[[2026-05-07]]"),
            "completion stamped"
        );
        assert_eq!(
            r.next_deadline.as_deref(),
            Some("2026-05-08"),
            "next-deadline ISO carried for WS parity"
        );
    }

    #[test]
    fn non_flip_delta_yields_nothing() {
        // A pure text edit (no status flip) produces no lifecycle ops.
        let prev = note("todo", &[]);
        let next = prev.replace("water plants", "water the plants");
        assert!(compute_lifecycle_container_sets(&prev, &next, "note").is_empty());
    }

    #[test]
    fn already_completed_occurrence_yields_nothing() {
        // Guard trips (last_completed already covers the occurrence) → no ops.
        let prev = note("todo", &[("last_completed", "[[2026-05-07]]")]);
        let next = note("done", &[("last_completed", "[[2026-05-07]]")]);
        assert!(compute_lifecycle_container_sets(&prev, &next, "note").is_empty());
    }
}

#[cfg(test)]
mod dependency_cycle_tests {
    use super::*;

    fn note_content(bodies: &[&str]) -> String {
        let mut lines = vec![
            "---".to_string(),
            "title: \"Test\"".to_string(),
            "tags: []".to_string(),
            "---".to_string(),
        ];
        lines.extend(bodies.iter().map(|s| s.to_string()));
        lines.join("\n") + "\n"
    }

    #[test]
    fn blocker_flip_unblocks_dependent_backlog_block() {
        let prev = note_content(&[
            "- blocker",
            "  status:: todo",
            "- dependent",
            "  status:: backlog",
            "  blocked_by:: [[note:0]]",
        ]);
        let next = note_content(&[
            "- blocker",
            "  status:: done",
            "- dependent",
            "  status:: backlog",
            "  blocked_by:: [[note:0]]",
        ]);

        let (rewritten, unblocked) = apply_dependency_cycles(&prev, &next, "note");
        assert_eq!(unblocked.len(), 1, "exactly one block unblocked");
        assert!(
            rewritten.contains("status:: todo\n  blocked_by:: [[note:0]]")
                || rewritten.contains("status:: todo")
        );
        // The dependent's status line must now read `todo`, not `backlog`.
        let (_meta, body) = parse_frontmatter(&rewritten).unwrap();
        let blocks = parse_blocks("note", &body);
        let dependent = blocks
            .iter()
            .find(|b| b.text.trim() == "dependent")
            .expect("dependent block present");
        assert_eq!(
            dependent.properties.get("status").map(String::as_str),
            Some("todo")
        );
    }

    #[test]
    fn still_blocked_dependent_is_left_alone() {
        let prev = note_content(&[
            "- blocker a",
            "  status:: todo",
            "- blocker b",
            "  status:: todo",
            "- dependent",
            "  status:: backlog",
            "  blocked_by:: [[note:0]], [[note:2]]",
        ]);
        let next = note_content(&[
            "- blocker a",
            "  status:: done",
            "- blocker b",
            "  status:: todo",
            "- dependent",
            "  status:: backlog",
            "  blocked_by:: [[note:0]], [[note:2]]",
        ]);

        let (rewritten, unblocked) = apply_dependency_cycles(&prev, &next, "note");
        assert!(
            unblocked.is_empty(),
            "still-blocked dependent must not unblock"
        );
        assert_eq!(rewritten, next, "content unchanged when nothing unblocks");
    }

    #[test]
    fn no_status_flip_is_a_no_op() {
        let prev = note_content(&["- solo", "  status:: todo"]);
        let next = prev.clone();
        let (rewritten, unblocked) = apply_dependency_cycles(&prev, &next, "note");
        assert!(unblocked.is_empty());
        assert_eq!(rewritten, next);
    }
}

#[cfg(test)]
mod fence_lifecycle_tests {
    use super::*;

    #[test]
    fn fenced_recurrence_and_done_flip_are_inert() {
        let prev = "- <!-- bid:44444444-4444-4444-4444-444444444444 -->\n  ```text\n  recurring:: daily\n  deadline:: [[2026-07-11]]\n  status:: todo\n  ```\n";
        let next = prev.replace("status:: todo", "status:: done");

        let (rewritten, bumps) = apply_post_save_bumps_with_info(prev, &next, "note");
        assert!(bumps.is_empty());
        assert_eq!(rewritten, next, "fenced payload must remain byte-identical");
    }

    #[test]
    fn real_recurrence_roll_does_not_rewrite_same_keys_inside_fence() {
        let prev = "- Task <!-- bid:45454545-4545-4545-4545-454545454545 -->\n  ```text\n  status:: fenced literal\n  deadline:: [[1999-01-01]]\n  ```\n  recurring:: daily\n  deadline:: [[2026-07-11]]\n  status:: todo\n";
        let next = prev.replacen("  status:: todo", "  status:: done", 1);

        let (rewritten, bumps) = apply_post_save_bumps_with_info(prev, &next, "note");
        assert_eq!(bumps.len(), 1);
        assert!(rewritten.contains("status:: fenced literal"));
        assert!(rewritten.contains("deadline:: [[1999-01-01]]"));
        assert!(rewritten.contains("deadline:: [[2026-07-12]]"));
        assert!(rewritten.contains("status:: todo"));
    }
}

#[cfg(test)]
mod tag_autocreate_pure_tests {
    use super::*;
    use crate::note::{Note, NoteId, NoteMetadata};
    use std::path::PathBuf;

    fn note_with(tags: Vec<String>, body: &str) -> Note {
        Note {
            id: NoteId::new("test"),
            title: "Test".to_string(),
            content: format!("---\ntitle: \"Test\"\n---\n{}", body),
            body: body.to_string(),
            metadata: NoteMetadata {
                tags,
                ..Default::default()
            },
            path: PathBuf::from("test.md"),
            checksum: String::new(),
            created_at: chrono::Utc::now(),
            modified_at: chrono::Utc::now(),
            attachments: Vec::new(),
        }
    }

    #[test]
    fn collects_frontmatter_inline_and_block_tags_deduped() {
        let note = note_with(
            vec!["alpha".to_string()],
            "- a task #beta\n  tags:: gamma, Alpha\n",
        );
        let tags = collect_note_tags(&note);
        // alpha (frontmatter), beta (inline), gamma (block tags:: line);
        // the block-level "Alpha" is a case-insensitive dup of frontmatter
        // "alpha" and must not be added again.
        assert_eq!(tags.len(), 3, "tags: {:?}", tags);
        assert!(tags.iter().any(|t| t.eq_ignore_ascii_case("alpha")));
        assert!(tags.iter().any(|t| t.eq_ignore_ascii_case("beta")));
        assert!(tags.iter().any(|t| t.eq_ignore_ascii_case("gamma")));
    }

    #[test]
    fn no_tags_anywhere_returns_empty() {
        let note = note_with(vec![], "- plain task, nothing special\n");
        assert!(collect_note_tags(&note).is_empty());
    }

    #[test]
    fn collect_note_tags_ignores_nested_fenced_hash_and_property_tags() {
        let body = concat!(
            "- Parent\n",
            "  - Child\n",
            "    ```text\n    #nested-fake\n    tags:: nested-prop-fake\n    ```\n",
            "  - ```text\n    #same-line-fake\n    tags:: same-line-prop-fake\n    ```\n",
            "- Outside #real\n  tags:: chip\n",
        );
        let note = note_with(vec!["front".to_string()], body);
        let tags = collect_note_tags(&note);

        assert_eq!(tags, vec!["front", "real", "chip"]);
    }

    #[test]
    fn collect_note_tags_treats_frontmatter_shaped_body_as_body() {
        let note = note_with(
            Vec::new(),
            "---\n```text\n#hidden\ntags:: hidden-prop\n[[hidden]]\n```\n---",
        );

        assert!(collect_note_tags(&note).is_empty());
    }

    #[test]
    fn tag_page_content_is_well_formed_frontmatter() {
        let content = tag_page_content("fella");
        assert!(content.starts_with("---\ntitle: \"fella\"\n"));
        assert!(content.contains("type: tag"));
        assert!(content.contains("extends: \"Root Tag\""));
    }
}
