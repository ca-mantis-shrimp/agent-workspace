//! The wake summary: kernel-rendered plain text of at most
//! [`WAKE_BUDGET_BYTES`], written for its reader (an agent) instead of
//! serialized from entities. Normative source:
//! `knowledge/specifications/wake-summary-contract.md`.
//!
//! Rendering is a pure function over [`WakeInput`]. A skeleton of short forms
//! always fits (the goal is always whole within its cap), then candidates are
//! upgraded to full form in priority order while the output stays in budget.
//! Everything shortened keeps an id, an explicit `(+k more)` count, or the
//! `more:` line pointing at the full projections.

use std::cmp::Reverse;
use std::collections::BTreeSet;

use crate::model::{EntityRef, SourceState};

pub(crate) const WAKE_BUDGET_BYTES: usize = 1000;
const GOAL_CAP: usize = 200;
const LABEL_CAP: usize = 32;
const NOTE_EXCERPT_CAP: usize = 100;
const NOTE_FULL_CAP: usize = 240;
const ITEM_CAP: usize = 100;
const REFERENCE_CAP: usize = 80;
const ID_LIST_CAP: usize = 5;
/// The knowledge pulse's own cap (knowledge pulse contract §3).
const GOVERNS_LIST_CAP: usize = 3;
const ELLIPSIS: &str = "…";
const HEADER: &str = "wake · stale outranks memory · reveal: workspace_reveal";

/// Everything the wake reports, as plain data assembled by the kernel.
#[derive(Clone, Debug, Default)]
pub(crate) struct WakeInput {
    pub(crate) goal: Option<String>,
    pub(crate) checkpoint: Option<WakeCheckpoint>,
    pub(crate) active_claims: usize,
    /// Ids of active claims served stale.
    pub(crate) stale_claim_ids: Vec<u64>,
    /// Open findings and transactions.
    pub(crate) open: Vec<WakeItem>,
    /// Applicable knowledge bindings, already ranked by the kernel (knowledge
    /// pulse contract §4); the renderer never re-ranks them.
    pub(crate) governs: Vec<WakeBinding>,
}

/// A governing binding as the wake shows it: a pointer, never a source body.
#[derive(Clone, Debug)]
pub(crate) struct WakeBinding {
    pub(crate) id: u64,
    pub(crate) headline: String,
    pub(crate) reference: Option<String>,
    pub(crate) source: Option<SourceState>,
}

impl WakeBinding {
    /// A changed or unavailable source should change what the reader trusts.
    fn needs_attention(&self) -> bool {
        matches!(
            self.source,
            Some(SourceState::Changed | SourceState::Unavailable)
        )
    }
}

/// The latest (or `since`) checkpoint and what happened after it.
#[derive(Clone, Debug, Default)]
pub(crate) struct WakeCheckpoint {
    pub(crate) label: String,
    pub(crate) note: Option<String>,
    pub(crate) news: Vec<WakeItem>,
    pub(crate) reads_captured: usize,
    pub(crate) goal_changed: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct WakeItem {
    pub(crate) marker: Marker,
    pub(crate) entity: EntityRef,
    pub(crate) text: String,
}

/// Declaration order is priority order within a section.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Marker {
    /// Recorded since the checkpoint and already stale.
    StaleRecorded,
    /// Current at the checkpoint, stale now.
    Stale,
    Recorded,
    /// Superseded or retired claim, or closed transaction.
    Ended,
    Open,
}

impl Marker {
    fn symbol(self) -> &'static str {
        match self {
            Self::StaleRecorded => "!+",
            Self::Stale => "!",
            Self::Recorded => "+",
            Self::Ended => "-",
            Self::Open => "open",
        }
    }

    fn is_stale(self) -> bool {
        matches!(self, Self::StaleRecorded | Self::Stale)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Candidate {
    Note,
    Item(EntityRef),
}

/// Render the wake: empty for a workspace with nothing to orient to, otherwise
/// the skeleton upgraded greedily in priority order within the byte budget.
pub(crate) fn render(input: &WakeInput) -> String {
    let input = normalized(input);
    if is_quiet(&input) {
        return String::new();
    }
    let mut upgraded = BTreeSet::new();
    let mut text = compose(&input, &upgraded);
    for candidate in candidates(&input) {
        upgraded.insert(candidate);
        let attempt = compose(&input, &upgraded);
        if attempt.len() <= WAKE_BUDGET_BYTES {
            text = attempt;
        } else {
            upgraded.remove(&candidate);
        }
    }
    enforce_budget(text)
}

/// Sort every list into its contract order so rendering never depends on how
/// the caller assembled the input: markers by priority, findings before
/// transactions, newest first.
fn normalized(input: &WakeInput) -> WakeInput {
    let mut input = input.clone();
    let key = |item: &WakeItem| (item.marker, item.entity, Reverse(item.entity.id()));
    let sort = |items: &mut Vec<WakeItem>| {
        items.sort_by_key(|item| {
            let (marker, entity, newest) = key(item);
            (marker, kind_rank(entity), newest)
        })
    };
    sort(&mut input.open);
    if let Some(checkpoint) = input.checkpoint.as_mut() {
        sort(&mut checkpoint.news);
    }
    input.stale_claim_ids.sort_unstable_by(|a, b| b.cmp(a));
    input
}

fn kind_rank(entity: EntityRef) -> u8 {
    match entity {
        EntityRef::Claim(_) => 0,
        EntityRef::Observation(_) => 1,
        EntityRef::Finding(_) => 2,
        EntityRef::Transaction(_) => 3,
        EntityRef::Knowledge(_) => 4,
    }
}

fn is_quiet(input: &WakeInput) -> bool {
    input.goal.is_none()
        && input.checkpoint.is_none()
        && input.active_claims == 0
        && input.open.is_empty()
        && input.governs.is_empty()
}

/// Upgrade candidates in contract priority order: governing bindings whose
/// source changed or became unavailable, newly stale news, the full checkpoint
/// note, other governing bindings, open work, then new claims. Ended entities
/// are never candidates: their text is exactly what is no longer believed, and
/// a superseded false claim rendered as a sentence reads as fact at wake.
fn candidates(input: &WakeInput) -> Vec<Candidate> {
    let governs = &input.governs[..input.governs.len().min(GOVERNS_LIST_CAP)];
    let binding = |binding: &WakeBinding| Candidate::Item(EntityRef::Knowledge(binding.id));
    let news = input
        .checkpoint
        .as_ref()
        .map_or(&[][..], |checkpoint| listed(&checkpoint.news));
    let item = |item: &WakeItem| Candidate::Item(item.entity);
    let mut order: Vec<Candidate> = governs
        .iter()
        .filter(|entry| entry.needs_attention())
        .map(binding)
        .collect();
    order.extend(
        news.iter()
            .filter(|entry| entry.marker.is_stale())
            .map(item),
    );
    if input
        .checkpoint
        .as_ref()
        .is_some_and(|checkpoint| checkpoint.note.is_some())
    {
        order.push(Candidate::Note);
    }
    order.extend(
        governs
            .iter()
            .filter(|entry| !entry.needs_attention())
            .map(binding),
    );
    order.extend(listed(&input.open).iter().map(item));
    order.extend(
        news.iter()
            .filter(|entry| entry.marker == Marker::Recorded)
            .map(item),
    );
    order
}

fn listed(items: &[WakeItem]) -> &[WakeItem] {
    &items[..items.len().min(ID_LIST_CAP)]
}

/// Lay out the wake for one set of upgrades. Section order is reading order;
/// a section with nothing to say is omitted.
fn compose(input: &WakeInput, upgraded: &BTreeSet<Candidate>) -> String {
    let mut lines = vec![HEADER.to_owned()];
    let mut shortened = 0;
    if let Some(goal) = &input.goal {
        let (text, cut) = clip(goal, GOAL_CAP);
        shortened += usize::from(cut);
        lines.push(format!("goal: {text}"));
    }
    if let Some(checkpoint) = &input.checkpoint {
        let (label, _) = clip(&checkpoint.label, LABEL_CAP);
        match &checkpoint.note {
            None => lines.push(format!("stopped at {label}")),
            Some(note) => {
                let (text, cut) = if upgraded.contains(&Candidate::Note) {
                    clip(note, NOTE_FULL_CAP)
                } else {
                    headline(note, NOTE_EXCERPT_CAP)
                };
                shortened += usize::from(cut);
                lines.push(format!("stopped at {label}: {text}"));
            }
        }
    }
    shortened += push_governs(&mut lines, &input.governs, upgraded);
    if let Some(checkpoint) = &input.checkpoint {
        let mut activity = Vec::new();
        if checkpoint.reads_captured > 0 {
            let plural = if checkpoint.reads_captured == 1 {
                ""
            } else {
                "s"
            };
            activity.push(format!(
                "{} read{plural} captured",
                checkpoint.reads_captured
            ));
        }
        if checkpoint.goal_changed {
            activity.push("goal changed".to_owned());
        }
        if !checkpoint.news.is_empty() || !activity.is_empty() {
            lines.push(
                format!("since then: {}", activity.join(" · "))
                    .trim_end()
                    .to_owned(),
            );
            shortened += push_items(&mut lines, &checkpoint.news, upgraded);
        }
    }
    shortened += push_items(&mut lines, &input.open, upgraded);
    if input.active_claims > 0 {
        lines.push(claims_line(input));
    }
    if shortened > 0 {
        lines.push(format!(
            "more: {shortened} shortened · workspace_status|workspace_delta full=true"
        ));
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Governing knowledge: one pointer line per upgraded binding
/// (`k3 [changed] headline → reference`, the tag only when the source is not
/// current), then one short line for the rest (`governs: k4 k7!`, where `!`
/// marks a changed or unavailable source). Returns how many listed bindings
/// are not shown whole.
fn push_governs(
    lines: &mut Vec<String>,
    bindings: &[WakeBinding],
    upgraded: &BTreeSet<Candidate>,
) -> usize {
    let listed = &bindings[..bindings.len().min(GOVERNS_LIST_CAP)];
    let mut shortened = 0;
    let mut short = Vec::new();
    for binding in listed {
        let entity = EntityRef::Knowledge(binding.id);
        if !upgraded.contains(&Candidate::Item(entity)) {
            shortened += 1;
            let flag = if binding.needs_attention() { "!" } else { "" };
            short.push(format!("{entity}{flag}"));
            continue;
        }
        let (headline, mut cut) = clip(&binding.headline, ITEM_CAP);
        let mut line = entity.to_string();
        if let Some(state) = binding
            .source
            .filter(|state| *state != SourceState::Current)
        {
            line.push_str(&format!(" [{}]", source_label(state)));
        }
        line.push(' ');
        line.push_str(&headline);
        if let Some(reference) = &binding.reference {
            let (reference, reference_cut) = clip(reference, REFERENCE_CAP);
            cut |= reference_cut;
            line.push_str(&format!(" → {reference}"));
        }
        shortened += usize::from(cut);
        lines.push(line);
    }
    let omitted = bindings.len() - listed.len();
    if omitted > 0 {
        short.push(format!("(+{omitted} more)"));
    }
    if !short.is_empty() {
        lines.push(format!("governs: {}", short.join(" ")));
    }
    shortened
}

fn source_label(state: SourceState) -> &'static str {
    match state {
        SourceState::Changed => "changed",
        SourceState::Unavailable => "unavailable",
        SourceState::Unknown => "unknown",
        SourceState::Current => "current",
    }
}

/// One full line per upgraded item, then one short line (`+ c9 c8 · - c3`) for
/// the rest with an explicit `(+k more)` beyond the list cap. Returns how many
/// listed items are not shown whole.
fn push_items(
    lines: &mut Vec<String>,
    items: &[WakeItem],
    upgraded: &BTreeSet<Candidate>,
) -> usize {
    let listed = listed(items);
    let mut shortened = 0;
    let mut short = String::new();
    let mut last_marker = None;
    for item in listed {
        if upgraded.contains(&Candidate::Item(item.entity)) {
            let (text, cut) = headline(&item.text, ITEM_CAP);
            shortened += usize::from(cut);
            lines.push(format!("{} {} {text}", item.marker.symbol(), item.entity));
            continue;
        }
        shortened += 1;
        if last_marker != Some(item.marker) {
            if last_marker.is_some() {
                short.push_str(" · ");
            }
            short.push_str(item.marker.symbol());
            last_marker = Some(item.marker);
        }
        short.push(' ');
        short.push_str(&item.entity.to_string());
    }
    let omitted = items.len() - listed.len();
    if omitted > 0 {
        if short.is_empty() {
            short.push_str(items[listed.len()].marker.symbol());
        }
        short.push_str(&format!(" (+{omitted} more)"));
    }
    if !short.is_empty() {
        lines.push(short);
    }
    shortened
}

fn claims_line(input: &WakeInput) -> String {
    let stale = &input.stale_claim_ids;
    if stale.is_empty() {
        return format!("claims: {} active, none stale", input.active_claims);
    }
    let shown: Vec<String> = stale
        .iter()
        .take(ID_LIST_CAP)
        .map(|id| EntityRef::Claim(*id).to_string())
        .collect();
    let omitted = stale.len().saturating_sub(ID_LIST_CAP);
    let more = if omitted > 0 {
        format!(" (+{omitted} more)")
    } else {
        String::new()
    };
    format!(
        "claims: {} active, {} stale: {}{more}",
        input.active_claims,
        stale.len(),
        shown.join(" ")
    )
}

/// The hard budget holds even for inputs beyond the tested worst case (e.g.
/// ids of seven or more digits): cut on a char boundary and mark it.
fn enforce_budget(mut text: String) -> String {
    if text.len() <= WAKE_BUDGET_BYTES {
        return text;
    }
    let mut end = WAKE_BUDGET_BYTES - ELLIPSIS.len() - 1;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str(ELLIPSIS);
    text.push('\n');
    text
}

/// Collapse whitespace runs, newlines included, so one entity is one line.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Cut to at most `cap` bytes on a word boundary, marking a cut with `…`.
fn clip(text: &str, cap: usize) -> (String, bool) {
    let text = one_line(text);
    if text.len() <= cap {
        return (text, false);
    }
    let mut end = cap.saturating_sub(ELLIPSIS.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let head = &text[..end];
    let head = head.rfind(' ').map_or(head, |space| &head[..space]);
    (format!("{}{ELLIPSIS}", head.trim_end()), true)
}

/// The first sentence, clipped to `cap`; a dropped remainder counts as a cut
/// and is marked ` …`.
fn headline(text: &str, cap: usize) -> (String, bool) {
    let text = one_line(text);
    match text.find(". ") {
        Some(end) => (clip(&format!("{} {ELLIPSIS}", &text[..=end]), cap).0, true),
        None => clip(&text, cap),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Worst-case skeleton bound (contract WS2): leaves room for at least one
    /// full-form upgrade. Holds for ids below 100,000; beyond that the hard
    /// budget clip in [`enforce_budget`] still applies.
    const SKELETON_BUDGET_BYTES: usize = 750;

    fn item(marker: Marker, entity: EntityRef, text: &str) -> WakeItem {
        WakeItem {
            marker,
            entity,
            text: text.to_owned(),
        }
    }

    fn binding(
        id: u64,
        headline: &str,
        reference: Option<&str>,
        source: Option<SourceState>,
    ) -> WakeBinding {
        WakeBinding {
            id,
            headline: headline.to_owned(),
            reference: reference.map(str::to_owned),
            source,
        }
    }

    /// Every section at its cap with maximal text and ids below 100,000.
    fn worst_case() -> WakeInput {
        let long = "claim text ".repeat(60);
        let mut news: Vec<WakeItem> = [Marker::StaleRecorded, Marker::Stale, Marker::Recorded]
            .into_iter()
            .flat_map(|marker| [marker, marker])
            .enumerate()
            .map(|(n, marker)| item(marker, EntityRef::Claim(99_999 - n as u64), &long))
            .collect();
        news.extend((0..34).map(|n| item(Marker::Ended, EntityRef::Claim(90_000 - n), &long)));
        WakeInput {
            goal: Some("goal word ".repeat(100)),
            checkpoint: Some(WakeCheckpoint {
                label: "label".repeat(40),
                note: Some("note word ".repeat(100)),
                news,
                reads_captured: 99_999,
                goal_changed: true,
            }),
            active_claims: 99_999,
            stale_claim_ids: (0..40).map(|n| 99_999 - n).collect(),
            open: (0..40)
                .map(|n| {
                    let entity = if n % 2 == 0 {
                        EntityRef::Finding(99_999 - n)
                    } else {
                        EntityRef::Transaction(99_999 - n)
                    };
                    item(Marker::Open, entity, &long)
                })
                .collect(),
            governs: (0..20)
                .map(|n| {
                    let source = if n % 2 == 0 {
                        SourceState::Changed
                    } else {
                        SourceState::Current
                    };
                    binding(
                        99_999 - n,
                        &"governing rule ".repeat(20),
                        Some(&"../repository/".repeat(20)),
                        Some(source),
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn skeleton_worst_case_leaves_room_for_an_upgrade() {
        let skeleton = compose(&normalized(&worst_case()), &BTreeSet::new());
        assert!(
            skeleton.len() <= SKELETON_BUDGET_BYTES,
            "WS2: skeleton is {} bytes:\n{skeleton}",
            skeleton.len()
        );
        assert!(
            skeleton.contains("governs: k99999! k99998 k99997! (+17 more)"),
            "{skeleton}"
        );
    }

    #[test]
    fn governing_knowledge_reads_as_pointer_lines_after_the_last_stop() {
        let text = render(&WakeInput {
            goal: Some("ship the pulse".to_owned()),
            checkpoint: Some(WakeCheckpoint {
                label: "c0".to_owned(),
                news: vec![item(
                    Marker::Recorded,
                    EntityRef::Claim(4),
                    "foo returns one",
                )],
                ..WakeCheckpoint::default()
            }),
            governs: vec![
                binding(1, "Keep modules pure", None, None),
                binding(
                    2,
                    "Curated knowledge lives in OKF",
                    Some("../kb:decisions/boundary.md"),
                    Some(SourceState::Current),
                ),
            ],
            ..WakeInput::default()
        });
        assert_eq!(
            text,
            format!(
                "{HEADER}\ngoal: ship the pulse\nstopped at c0\nk1 Keep modules pure\n\
                 k2 Curated knowledge lives in OKF → ../kb:decisions/boundary.md\n\
                 since then:\n+ c4 foo returns one\n"
            )
        );
    }

    #[test]
    fn a_changed_governing_source_outranks_newly_stale_claims() {
        let long = "claim text ".repeat(20);
        let news = (1..=6)
            .map(|n| item(Marker::Stale, EntityRef::Claim(n), &long))
            .collect();
        let text = render(&WakeInput {
            goal: Some("goal word ".repeat(100)),
            checkpoint: Some(WakeCheckpoint {
                label: "c0".to_owned(),
                note: Some("note word ".repeat(100)),
                news,
                ..WakeCheckpoint::default()
            }),
            governs: vec![binding(
                9,
                &"governing rule ".repeat(6),
                Some("../kb:decisions/boundary.md"),
                Some(SourceState::Changed),
            )],
            ..WakeInput::default()
        });
        assert!(text.len() <= WAKE_BUDGET_BYTES);
        assert!(
            text.contains("\nmore: "),
            "fixture must be under pressure: {text}"
        );
        assert!(
            text.contains("\nk9 [changed] governing rule"),
            "WS3: {text}"
        );
    }

    #[test]
    fn render_stays_within_budget_and_accounts_for_what_it_cut() {
        let text = render(&worst_case());
        assert!(text.len() <= WAKE_BUDGET_BYTES, "WS1: {} bytes", text.len());
        assert!(
            text.lines().any(|line| line.starts_with("more: ")),
            "{text}"
        );
        assert!(text.contains("(+35 more)"), "{text}");
        assert!(text.contains("(+35 more)") && text.contains("claims: 99999 active"));
    }

    #[test]
    fn quiet_workspace_renders_nothing() {
        assert_eq!(render(&WakeInput::default()), "");
    }

    #[test]
    fn unchanged_checkpoint_has_no_since_section() {
        let text = render(&WakeInput {
            goal: Some("ship the wake".to_owned()),
            checkpoint: Some(WakeCheckpoint {
                label: "c0".to_owned(),
                ..WakeCheckpoint::default()
            }),
            ..WakeInput::default()
        });
        assert_eq!(
            text,
            format!("{HEADER}\ngoal: ship the wake\nstopped at c0\n")
        );
    }

    #[test]
    fn newly_stale_news_is_upgraded_before_new_claims() {
        let long = "claim text ".repeat(20);
        let news = |markers: &[Marker]| -> Vec<WakeItem> {
            markers
                .iter()
                .enumerate()
                .map(|(n, marker)| item(*marker, EntityRef::Claim(n as u64 + 1), &long))
                .collect()
        };
        let input = |news| WakeInput {
            goal: Some("goal word ".repeat(100)),
            checkpoint: Some(WakeCheckpoint {
                label: "c0".to_owned(),
                note: Some("note word ".repeat(100)),
                news,
                ..WakeCheckpoint::default()
            }),
            ..WakeInput::default()
        };
        let is_full = |line: &str, symbol: &str| {
            line.starts_with(&format!("{symbol} c")) && line.contains("claim text")
        };

        use Marker::{Recorded, Stale};
        let pressured = render(&input(news(&[
            Stale, Stale, Stale, Stale, Recorded, Recorded,
        ])));
        assert!(pressured.len() <= WAKE_BUDGET_BYTES);
        assert!(
            pressured.contains("\nmore: "),
            "fixture must be under pressure: {pressured}"
        );
        // Same-sized items: a new claim in full while a newly stale one is left
        // short would be a priority inversion (contract invariant 3).
        let stale_full = pressured.lines().filter(|line| is_full(line, "!")).count();
        let recorded_full = pressured.lines().any(|line| is_full(line, "+"));
        assert!(stale_full > 0, "{pressured}");
        assert!(
            !recorded_full || stale_full == 4,
            "WS4 inversion: {pressured}"
        );

        // Without the stale items, their bytes go to the next priority.
        let relieved = render(&input(news(&[Recorded, Recorded, Recorded, Recorded])));
        assert!(
            relieved.lines().any(|line| is_full(line, "+")),
            "{relieved}"
        );
    }

    #[test]
    fn aged_stale_claims_show_ids_never_headlines() {
        let text = render(&WakeInput {
            goal: Some("ship the wake".to_owned()),
            active_claims: 30,
            stale_claim_ids: (1..=30).collect(),
            ..WakeInput::default()
        });
        assert!(
            text.contains("claims: 30 active, 30 stale: c30 c29 c28 c27 c26 (+25 more)\n"),
            "WS8: {text}"
        );
    }

    #[test]
    fn a_claim_recorded_and_stale_since_the_checkpoint_is_one_line() {
        let text = render(&WakeInput {
            checkpoint: Some(WakeCheckpoint {
                label: "c0".to_owned(),
                news: vec![item(
                    Marker::StaleRecorded,
                    EntityRef::Claim(5),
                    "foo returns one",
                )],
                ..WakeCheckpoint::default()
            }),
            ..WakeInput::default()
        });
        assert_eq!(text.matches("c5").count(), 1, "{text}");
        assert!(text.contains("\n!+ c5 foo returns one\n"), "{text}");
    }

    #[test]
    fn rendering_is_deterministic_whatever_the_input_order() {
        let mut shuffled = worst_case();
        shuffled.open.reverse();
        if let Some(checkpoint) = shuffled.checkpoint.as_mut() {
            checkpoint.news.reverse();
        }
        shuffled.stale_claim_ids.reverse();
        assert_eq!(render(&worst_case()), render(&shuffled), "WS9");
    }

    #[test]
    fn clipping_is_utf8_safe_one_line_and_marks_cuts() {
        let (text, cut) = clip(&"é".repeat(200), 50);
        assert!(
            cut && text.len() <= 50 && text.ends_with(ELLIPSIS),
            "{text}"
        );
        assert_eq!(clip("two\nlines", 50), ("two lines".to_owned(), false));
        assert_eq!(
            headline("First sentence. Second one.", 100),
            ("First sentence. …".to_owned(), true)
        );
        assert_eq!(
            headline("No full stop", 100),
            ("No full stop".to_owned(), false)
        );
    }

    #[test]
    fn ended_entities_appear_by_id_never_by_their_retired_text() {
        let text = render(&WakeInput {
            checkpoint: Some(WakeCheckpoint {
                label: "c0".to_owned(),
                news: vec![item(
                    Marker::Ended,
                    EntityRef::Claim(17),
                    "The legend palette supports 12 distinct series colors",
                )],
                ..WakeCheckpoint::default()
            }),
            ..WakeInput::default()
        });
        assert!(text.contains("\n- c17\n"), "{text}");
        assert!(
            !text.contains("palette"),
            "retired text must not read as fact: {text}"
        );
    }
}
