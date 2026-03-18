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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Action, ActionKind};

    fn action(name: &str, description: &str) -> Action {
        Action {
            name: name.into(),
            description: description.into(),
            icon: None,
            tags: vec![],
            hotkey: None,
            kind: ActionKind::CopyText { text: name.into() },
        }
    }

    #[test]
    fn empty_query_returns_all_actions_in_original_order() {
        let actions = vec![action("Alpha", ""), action("Beta", "")];
        let results = SearchEngine::new().search("", &actions);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].1, 0);
        assert_eq!(results[0].2.name, "Alpha");
        assert_eq!(results[1].1, 1);
        assert_eq!(results[1].2.name, "Beta");
    }

    #[test]
    fn fuzzy_search_returns_matching_indices_sorted_by_score() {
        let actions = vec![
            action("Quick Search", "search text"),
            action("Run Clipboard Text", "run shell"),
            action("Notes", "editor"),
        ];

        let results = SearchEngine::new().search("quick", &actions);

        assert!(!results.is_empty());
        assert_eq!(results[0].1, 0);
        assert_eq!(results[0].2.name, "Quick Search");
    }
}
