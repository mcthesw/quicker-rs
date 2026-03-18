use crate::action::Action;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

pub struct SearchEngine {
    matcher: SkimMatcherV2,
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            matcher: SkimMatcherV2::default(),
        }
    }

    /// Return actions sorted by match score (best first).
    /// Returns all actions if query is empty.
    pub fn search<'a>(&self, query: &str, actions: &'a [Action]) -> Vec<(i64, usize, &'a Action)> {
        if query.trim().is_empty() {
            return actions
                .iter()
                .enumerate()
                .map(|(idx, action)| (0, idx, action))
                .collect();
        }

        let mut results: Vec<(i64, usize, &Action)> = actions
            .iter()
            .enumerate()
            .filter_map(|(idx, action)| {
                let text = action.search_text();
                self.matcher
                    .fuzzy_match(&text, query)
                    .map(|score| (score, idx, action))
            })
            .collect();

        results.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        results
    }
}
