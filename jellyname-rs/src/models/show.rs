#[derive(Debug, Clone)]
pub struct TVSeason {
    pub season_number: u32,
}

#[derive(Debug, Clone)]
pub struct TVShow {
    pub name: String,
    pub source: String,
    pub source_id: u64,
    pub first_year: String,
}
