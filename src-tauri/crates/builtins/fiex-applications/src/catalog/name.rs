use std::collections::HashSet;

pub(super) fn unique_name(
    desired: &str,
    parent: &Option<String>,
    names: &mut HashSet<(Option<String>, String)>,
) -> String {
    let mut candidate = desired.to_owned();
    let dot = desired
        .rfind('.')
        .filter(|dot| *dot > 0 && *dot < desired.len() - 1);
    let (stem, extension) = dot
        .map(|dot| (&desired[..dot], &desired[dot..]))
        .unwrap_or((desired, ""));
    let mut suffix = 1_u32;
    while names.contains(&(parent.clone(), candidate.to_lowercase())) {
        candidate = format!("{stem} ({suffix}){extension}");
        suffix += 1;
    }
    names.insert((parent.clone(), candidate.to_lowercase()));
    candidate
}
