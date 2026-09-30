/// Rank close names, falling back to a short list when nothing resembles the input.
pub fn suggestions(input: &str, names: &[String]) -> String {
    let distance = |name: &str| {
        let mut row: Vec<_> = (0..=name.len()).collect();
        for (i, left) in input.bytes().enumerate() {
            let mut previous = row[0];
            row[0] = i + 1;
            for (j, right) in name.bytes().enumerate() {
                let old = row[j + 1];
                row[j + 1] = (row[j + 1] + 1)
                    .min(row[j] + 1)
                    .min(previous + usize::from(left != right));
                previous = old;
            }
        }
        row[name.len()]
    };
    let mut names: Vec<_> = names
        .iter()
        .map(|name| (distance(name), name.as_str()))
        .collect();
    names.sort();
    names
        .into_iter()
        .take(3)
        .map(|(_, name)| name)
        .collect::<Vec<_>>()
        .join(", ")
}
