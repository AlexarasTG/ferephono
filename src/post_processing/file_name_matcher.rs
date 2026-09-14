use std::collections::{BTreeSet, HashMap};

const MINIMUM_LEAD_TO_SECOND_BEST: f64 = 0.05;

#[derive(Default)]
pub(super) struct FileNames {
    by_extension: HashMap<String, Vec<(String, String)>>,
}

impl FileNames {
    pub(super) fn new(names: BTreeSet<String>) -> Self {
        let mut result = Self::default();
        for name in names {
            if let Some((stem, extension)) = name.rsplit_once('.') {
                let normalized = normalize(stem);
                result
                    .by_extension
                    .entry(extension.to_owned())
                    .or_default()
                    .push((name, normalized));
            }
        }
        result
    }

    pub(super) fn resolve(
        &self,
        name: &str,
        minimum_similarity_to_replace: f64,
        diagnostics: bool,
    ) -> String {
        let Some((stem, extension)) = name.rsplit_once('.') else {
            return name.to_owned();
        };
        let Some(candidates) = self.by_extension.get(extension) else {
            return name.to_owned();
        };
        if candidates.iter().any(|(candidate, _)| candidate == name) {
            if diagnostics {
                log::debug!("filename match: {name}, preserved exact match");
            }
            return name.to_owned();
        }
        let normalized = normalize(stem);
        let mut best = (name, 0.0_f64);
        let mut second = 0.0_f64;
        for (candidate, comparison) in candidates {
            let score = strsim::normalized_levenshtein(&normalized, comparison);
            if diagnostics {
                log::debug!(
                    "filename candidate: input={name}, candidate={candidate}, score={score:.4}"
                );
            }
            if score > best.1 {
                second = best.1;
                best = (candidate, score);
            } else {
                second = second.max(score);
            }
        }
        let accepted = best.1 >= minimum_similarity_to_replace
            && best.1 - second + f64::EPSILON >= MINIMUM_LEAD_TO_SECOND_BEST;
        if diagnostics {
            log::debug!(
                "filename match: input={name}, best={}, score={:.4}, runner_up={second:.4}, minimum={minimum_similarity_to_replace:.2}, decision={}",
                best.0,
                best.1,
                if accepted {
                    "accepted"
                } else if best.1 < minimum_similarity_to_replace {
                    "below minimum similarity"
                } else {
                    "ambiguous"
                }
            );
        }
        if accepted {
            best.0.to_owned()
        } else {
            name.to_owned()
        }
    }
}

fn normalize(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '_' | '-'))
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configurable_threshold_changes_replacement() {
        let names = FileNames::new(BTreeSet::from(["file_name_formatter.rs".into()]));
        let spoken = "file_name_front_matter.rs";
        assert_eq!(names.resolve(spoken, 0.90, false), spoken);
        assert_eq!(names.resolve(spoken, 0.80, false), "file_name_formatter.rs");
    }

    #[test]
    fn forgiving_threshold_still_rejects_ambiguous_matches() {
        let names = FileNames::new(BTreeSet::from([
            "file_name_formatter.rs".into(),
            "file_name_formatting.rs".into(),
        ]));
        let spoken = "file_name_formatt.rs";
        assert_eq!(names.resolve(spoken, 0.50, false), spoken);
        assert_eq!(
            names.resolve("file_name_formatter.rs", 1.0, false),
            "file_name_formatter.rs"
        );
    }
}
