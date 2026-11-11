/// Case-insensitive subsequence matching. Consecutive letters and word starts score higher;
/// matching the label beats matching only a category/path. Each whitespace-separated term
/// may match a different part, so “notes project” also finds project/notes.md.
pub fn rank(query: &str, label: &str, detail: &str) -> Option<i32> {
    query.split_whitespace().try_fold(0, |total, term| {
        let term = term.to_lowercase();
        let label_score = score(&term, label).map(|s| s + 80);
        let detail_score = score(&term, detail);
        label_score
            .into_iter()
            .chain(detail_score)
            .max()
            .map(|s| total + s)
    })
}
fn score(query: &str, value: &str) -> Option<i32> {
    let query: Vec<_> = query.chars().collect();
    let mut matched = 0;
    let mut points = 0;
    let mut consecutive = false;
    let mut boundary = true;
    for c in value.to_lowercase().chars() {
        if matched < query.len() && c == query[matched] {
            points += 12 + if consecutive { 14 } else { 0 } + if boundary { 10 } else { 0 };
            matched += 1;
            consecutive = true;
            if matched == query.len() {
                return Some(points);
            }
        } else {
            points -= 1;
            consecutive = false;
        }
        boundary = !c.is_alphanumeric();
    }
    None
}
