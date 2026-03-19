use tesela_core::note::SearchHit;

#[derive(Debug, Clone, Default)]
pub struct SearchState {
    pub query: String,
    pub results: Vec<SearchHit>,
    pub selected: usize,
    pub history: Vec<String>,
    pub is_searching: bool,
}

impl SearchState {
    pub fn push_history(&mut self, query: String) {
        // Avoid consecutive duplicates
        if self.history.last() != Some(&query) {
            self.history.push(query);
        }
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.results.clear();
        self.selected = 0;
        self.is_searching = false;
    }
}
