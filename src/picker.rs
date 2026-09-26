use crate::git::Branch;

pub struct Picker {
    pub branches: Vec<Branch>,
    pub query: String,
    pub filtered: Vec<usize>,
    pub selected: usize,
}

impl Picker {
    pub fn new(branches: Vec<Branch>, query: String) -> Self {
        let mut picker = Self {
            branches,
            query,
            filtered: Vec::new(),
            selected: 0,
        };
        picker.filter();
        picker
    }

    pub fn filter(&mut self) {
        self.filtered = self
            .branches
            .iter()
            .enumerate()
            .filter(|(_, branch)| fuzzy_match(&branch.name, &self.query))
            .map(|(index, _)| index)
            .collect();
        self.selected = 0;
    }

    pub fn move_selection(&mut self, offset: isize) {
        self.selected = self
            .selected
            .saturating_add_signed(offset)
            .min(self.filtered.len().saturating_sub(1));
    }

    pub fn selected_branch(&self) -> Option<&Branch> {
        self.filtered
            .get(self.selected)
            .map(|&index| &self.branches[index])
    }
}

// Ordered subsequence matching, with smart case and space-separated AND terms.
// Filtering preserves the original commit-date ordering, like fzf --no-sort.
fn fuzzy_match(name: &str, query: &str) -> bool {
    let sensitive = query.chars().any(char::is_uppercase);
    let name = if sensitive {
        name.to_string()
    } else {
        name.to_lowercase()
    };
    let query = if sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };
    query.split_whitespace().all(|term| {
        let mut characters = name.chars();
        term.chars()
            .all(|needle| characters.by_ref().any(|c| c == needle))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_search_smart_case_unicode_and_terms() {
        assert!(fuzzy_match("feature/Add-Été", "faé"));
        assert!(fuzzy_match("feature/Add-Été", "été feat"));
        assert!(fuzzy_match("feature/Add-Été", "AÉ"));
        assert!(!fuzzy_match("feature/add-été", "AÉ"));
        assert!(!fuzzy_match("main", "missing"));
        assert!(fuzzy_match("main", ""));
    }

    #[test]
    fn filters_names_only_preserves_order_and_handles_empty_results() {
        let branches = ["fix/z", "main", "fix/a"]
            .iter()
            .map(|name| Branch {
                name: name.to_string(),
                age: "yesterday".into(),
                current: false,
            })
            .collect();
        let mut picker = Picker::new(branches, "fix".into());
        assert_eq!(picker.filtered, vec![0, 2]);
        picker.move_selection(100);
        assert_eq!(picker.selected_branch().unwrap().name, "fix/a");
        picker.move_selection(-100);
        assert_eq!(picker.selected, 0);
        picker.query = "yesterday".into();
        picker.filter();
        picker.move_selection(1);
        assert!(picker.selected_branch().is_none());
    }
}
