//! Frozen lexical baseline: BM25 over task terms and source identifiers.
use super::*;

fn terms(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        let whole = word.to_ascii_lowercase();
        let usable = |s: &str| {
            s.len() > 1
                && !matches!(
                    s,
                    "the"
                        | "and"
                        | "for"
                        | "from"
                        | "with"
                        | "this"
                        | "that"
                        | "when"
                        | "then"
                        | "does"
                        | "what"
                        | "which"
                        | "how"
                        | "into"
                        | "its"
                        | "are"
                        | "all"
                        | "only"
                        | "case"
                        | "output"
                        | "supplied"
                        | "evidence"
                )
        };
        if usable(&whole) {
            out.push(whole.clone());
        }
        let mut separated = String::new();
        let mut lower = false;
        for c in word.chars() {
            if c == '_' || (lower && c.is_ascii_uppercase()) {
                separated.push(' ');
            }
            if c != '_' {
                separated.push(c.to_ascii_lowercase());
            }
            lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        }
        for part in separated.split_whitespace() {
            if part != whole && usable(part) {
                out.push(part.to_owned());
            }
        }
    }
    out
}

/// No labels or diagnosis descriptions participate. Ties retain the consumer's
/// owner-route ordering. Constants and tokenizer are part of bm25_v1.
pub fn ranking(case: &Case) -> Vec<(String, f64)> {
    let query: BTreeSet<_> = terms(&case.task).into_iter().collect();
    let documents: BTreeMap<_, _> = case
        .chunks
        .iter()
        .map(|c| (c.id.as_str(), terms(&format!("{}\n{}", c.source, c.text))))
        .collect();
    let average = documents.values().map(Vec::len).sum::<usize>() as f64 / documents.len() as f64;
    let n = documents.len() as f64;
    let mut ranked: Vec<_> = case
        .baseline_order
        .iter()
        .map(|id| {
            let document = &documents[id.as_str()];
            let length = document.len() as f64;
            let score = query
                .iter()
                .map(|term| {
                    let tf = document.iter().filter(|word| *word == term).count() as f64;
                    let df = documents.values().filter(|doc| doc.contains(term)).count() as f64;
                    let idf = (1.0 + (n - df + 0.5) / (df + 0.5)).ln();
                    if tf == 0.0 {
                        0.0
                    } else {
                        idf * tf * 2.2 / (tf + 1.2 * (0.25 + 0.75 * length / average.max(1.0)))
                    }
                })
                .sum::<f64>();
            (id.clone(), score)
        })
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    ranked
}
