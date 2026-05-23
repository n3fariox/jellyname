use crate::models::filters::Filters;

pub struct MovieProcessor;

impl MovieProcessor {
    pub fn new() -> Self {
        Self
    }

    pub fn find_match(
        &self,
        _mkv_title: Option<&str>,
        _filename: &std::path::Path,
        _filters: &Filters,
    ) -> Option<crate::models::movie::Movie> {
        // TODO: implement once TUI is wired
        None
    }
}
