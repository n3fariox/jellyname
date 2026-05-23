pub fn fix_title(title: &str) -> String {
    let mut t = title.to_lowercase();
    if let Some(stripped) = t.strip_suffix(", the") {
        t = format!("the {}", stripped);
    }
    t = t.replace("- blu-ray", "");
    t = t.replace("blu-ray", "");
    t
}

pub fn guess_title(filename: &std::path::Path) -> String {
    filename
        .parent()
        .and_then(|p| p.iter().next_back())
        .map(|s| s.to_string_lossy().to_lowercase().replace('_', " "))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fix_title_lowercases() {
        assert_eq!(fix_title("The Movie"), "the movie");
    }

    #[test]
    fn test_fix_title_moves_the() {
        assert_eq!(fix_title("Movie, The"), "the movie");
    }

    #[test]
    fn test_fix_title_removes_bluray() {
        assert_eq!(fix_title("Movie - blu-ray"), "movie ");
        assert_eq!(fix_title("Movie blu-ray"), "movie ");
    }

    #[test]
    fn test_fix_title_movie_with_the() {
        assert_eq!(fix_title("Dark Knight, The"), "the dark knight");
    }

    #[test]
    fn test_guess_title() {
        let path = std::path::Path::new("/rips/My_Movie_2020/file.mkv");
        assert_eq!(guess_title(path), "my movie 2020");
    }
}
