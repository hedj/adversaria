//! An item's pages in its inspector (docs/concepts-seven.md, change 4):
//! the pages its labels and its [[links]] to no item link it to, each a
//! click away; and, for an item that is a page, the items that label it.
//! Its [[links]] to items are link entries, listed with its other links.

use super::{Presented, dim};
use crate::outline::{Outline, page_key, tags};
use crate::present::Presentation;
use ratatui::text::Line;

/// The pages an item links to, then the items that label it.
pub(super) fn page_lines(outline: &Outline, id: u64, labels: &[u64]) -> Vec<Presented> {
    let mut lines = linked_lines(outline, id);
    lines.extend(labelled_by_lines(outline, labels));
    lines
}

/// Under "pages →", each label's page, then each [[link]]'s that is no
/// item.
fn linked_lines(outline: &Outline, id: u64) -> Vec<Presented> {
    let labels = tags(&outline.text(id)).into_iter().map(|k| format!("#{k}"));
    let unmade = outline
        .unresolved(id)
        .into_iter()
        .map(|n| format!("[[{n}]]"));
    let pages: Vec<String> = labels.chain(unmade).collect();
    if pages.is_empty() {
        return vec![];
    }
    let heading = (Line::styled("pages →", dim()), None);
    let rows = pages.into_iter().map(|p| page_row(outline, p));
    std::iter::once(heading).chain(rows).collect()
}

/// A page, as the item links to it: its item, if it is one, else what
/// links to it.
fn page_row(outline: &Outline, written: String) -> Presented {
    let name = written.trim_start_matches('#').trim_matches(['[', ']']);
    match outline.resolve(name) {
        Some(item) => item_page_row(outline, &written, item),
        None => virtual_page_row(outline, &written, &page_key(name)),
    }
}

/// A page that is an item: the item, a click away.
fn item_page_row(outline: &Outline, written: &str, item: u64) -> Presented {
    let said = format!(
        "  {written} → {}. {}",
        outline.number(item),
        outline.text(item)
    );
    (Line::raw(said), Some(Presentation::Link { to: item }))
}

/// A page that is no item: how many items link to it; a click lists them.
fn virtual_page_row(outline: &Outline, written: &str, key: &str) -> Presented {
    let n = outline.page_linkers(key).len();
    let said = format!("  {written} · no item · linked from {n}");
    let list = Presentation::Command(format!("find #{key}"));
    (Line::raw(said), Some(list))
}

/// Under "labelled by ←", the items whose labels name the item's page.
fn labelled_by_lines(outline: &Outline, labels: &[u64]) -> Vec<Presented> {
    if labels.is_empty() {
        return vec![];
    }
    let heading = (Line::styled("labelled by ←", dim()), None);
    let rows = labels.iter().map(|i| labelled_row(outline, *i));
    std::iter::once(heading).chain(rows).collect()
}

/// An item that labels the page: its number and text, a click away.
fn labelled_row(outline: &Outline, item: u64) -> Presented {
    let said = format!("  {}. {}", outline.number(item), outline.text(item));
    (Line::raw(said), Some(Presentation::Link { to: item }))
}
