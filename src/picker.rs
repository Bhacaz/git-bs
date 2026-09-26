use crate::git::Branch;

pub struct Picker {
    pub branches: Vec<Branch>,
    pub query: String,
    pub filtered: Vec<BranchMatch>,
    pub selected: usize,
}

pub struct BranchMatch {
    pub index: usize,
    // Byte offsets into the original branch name, always sorted and unique.
    pub positions: Vec<usize>,
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
            .filter_map(|(index, branch)| {
                match_positions(&branch.name, &self.query)
                    .map(|positions| BranchMatch { index, positions })
            })
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
            .map(|branch_match| &self.branches[branch_match.index])
    }
}

// Ordered subsequence matching, with smart case and space-separated AND terms.
// Filtering preserves visit order and the commit-date fallback.
fn match_positions(name: &str, query: &str) -> Option<Vec<usize>> {
    let sensitive = query.chars().any(char::is_uppercase);
    let mut characters = Vec::new();
    for (byte, character) in name.char_indices() {
        if sensitive {
            characters.push((character, byte));
        } else {
            characters.extend(character.to_lowercase().map(|lower| (lower, byte)));
        }
    }
    let query = if sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };
    let mut positions = Vec::new();
    for term in query.split_whitespace() {
        let mut next = 0;
        for needle in term.chars() {
            let found = characters
                .iter()
                .enumerate()
                .skip(next)
                .find(|(_, (character, _))| *character == needle)?;
            positions.push(found.1.1);
            next = found.0 + 1;
        }
    }
    positions.sort_unstable();
    positions.dedup();
    Some(positions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_search_smart_case_unicode_and_terms() {
        assert!(match_positions("feature/Add-Été", "faé").is_some());
        assert!(match_positions("feature/Add-Été", "été feat").is_some());
        assert!(match_positions("feature/Add-Été", "AÉ").is_some());
        assert!(match_positions("feature/add-été", "AÉ").is_none());
        assert!(match_positions("main", "missing").is_none());
        assert_eq!(match_positions("main", ""), Some(vec![]));
    }

    #[test]
    fn positions_identify_original_characters_for_fuzzy_terms_and_unicode() {
        assert_eq!(
            match_positions("feature/login", "ftlg"),
            Some(vec![0, 3, 8, 10])
        );
        assert_eq!(match_positions("éclair/été", "éé"), Some(vec![0, 8]));
        assert_eq!(
            match_positions("main/feature", "feat main"),
            Some(vec![0, 1, 2, 3, 5, 6, 7, 8])
        );
        // Lowercasing one Unicode character can produce two code points.
        assert_eq!(match_positions("İstanbul", "i\u{307}"), Some(vec![0]));
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
        assert_eq!(
            picker
                .filtered
                .iter()
                .map(|matched| matched.index)
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
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
