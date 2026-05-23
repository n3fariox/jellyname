#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE="/tmp/jellyname-test"

echo "Creating test data in $BASE"
rm -rf "$BASE"

# ─── helper: generate a minimal valid MKV with an embedded title ───
generate_mkv() {
    local file="$1"
    local title="$2"
    local width="${3:-1920}"
    local height="${4:-1080}"

    mkdir -p "$(dirname "$file")"
    python3 "$SCRIPT_DIR/mkvgen.py" "$file" "$title" "$width" "$height"
}

# ─── Movie test: "The Dark Knight" rip ───
MOVIE_DIR="$BASE/rips/movies"
mkdir -p "$MOVIE_DIR/The Dark Knight 2008"
generate_mkv "$MOVIE_DIR/The Dark Knight 2008/title00.mkv" "The Dark Knight, The"
echo "  ✓ movie rip created"

# ─── TV show test: "Breaking Bad" ripped disc ───
TV_DIR="$BASE/rips/shows/breaking_bad_s01_disc1"
mkdir -p "$TV_DIR"
generate_mkv "$TV_DIR/title00.mkv" "Breaking Bad" 1920 1080
generate_mkv "$TV_DIR/title01.mkv" "Breaking Bad" 1920 1080
generate_mkv "$TV_DIR/title02.mkv" "Breaking Bad" 1920 1080
echo "  ✓ tv show rips created (3 files)"

# ─── Output directories (empty, ready for dry-run) ───
mkdir -p "$BASE/output/movies" "$BASE/output/shows"

echo ""
echo "── Test data ready ─────────────────────────────────────"
echo " Movie dir:  $MOVIE_DIR"
echo " TV dir:     $TV_DIR"
echo " Output:     $BASE/output"
echo ""
echo " Example commands to run:"
echo ""
echo "  # Movies (dry-run)"
echo "  cargo run -- -k \$TMDB_API_KEY --dry-run \\"
echo "    -o $BASE/output/movies movies $MOVIE_DIR/*/"
echo ""
echo "  # TV shows (dry-run)"
echo "  cargo run -- -k \$TMDB_API_KEY --dry-run \\"
echo "    -o $BASE/output/shows shows $TV_DIR"
echo ""
echo "  # TV shows with mixed episodes"
echo "  cargo run -- -k \$TMDB_API_KEY --dry-run --mixed \\"
echo "    -o $BASE/output/shows shows $TV_DIR"
echo ""
echo "  # Full tree of test data:"
find "$BASE" -type f -o -type d | sort | sed 's|'"$BASE"'/|  |'
