use std::collections::{BTreeSet, HashMap};

const MINIMUM_SIMILARITY_TO_REPLACE: f64 = 0.90;
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

    pub(super) fn resolve(&self, name: &str, diagnostics: bool) -> String {
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
        let accepted =
            best.1 >= MINIMUM_SIMILARITY_TO_REPLACE && best.1 - second + f64::EPSILON >= MINIMUM_LEAD_TO_SECOND_BEST;
        if diagnostics {
            log::debug!(
                "filename match: input={name}, best={}, score={:.4}, runner_up={second:.4}, decision={}",
                best.0,
                best.1,
                if accepted {
                    "accepted"
                } else if best.1 < MINIMUM_SIMILARITY_TO_REPLACE {
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
