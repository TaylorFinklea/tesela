use tesela_core::note::Note;

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct ListingState {
    pub notes: Vec<Note>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub preview_note: Option<Note>,
    pub filter_tag: Option<String>,
}

impl ListingState {
    pub fn select_next(&mut self) {
        if !self.notes.is_empty() {
            self.selected = (self.selected + 1).min(self.notes.len() - 1);
        }
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn selected_note(&self) -> Option<&Note> {
        self.notes.get(self.selected)
    }
}
