# Jellyname

Simple tool to help rename MakeMKV rips from a source folder into a Jellyfin
folder (could be anything with custom formats).

Since I've had hit/miss luck with titles on my disks, this tool is interactive
and lets you enter custom search criteria if there is no title found.

## Quick example

```bash
python3 -m jellyname movies -k <api_key> -o /jellyfin/Movies/ '/rips/**/*.mkv'
```

## Features

- Interactive CLI for renaming and moving MakeMKV rips into a Jellyfin-style
	folder layout.
- Two subcommands: `movies` and `shows` for handling movies and TV series.
- Uses TMDB (or optionally TVDB when a `--tvdb-id` is provided) to look up
	metadata and build output paths using customizable format strings.
- Filter by original language using `--filter-lang` to limit TMDB search
	results.
- Dry-run mode (`-d` / `--dry-run`) to preview changes without moving or
	deleting files.

## CLI options

- Global:
	- `-d, --dry-run`: Don't actually move or delete files.
	- `-l, --filter-lang <lang>`: Filter movies by original language (e.g. `en`).
	- `-k, --api-key <key>`: TMDB API key (or set `TMDB_API_KEY` environment
		variable).

- `movies` subcommand:
	- `-o, --output <path>`: Destination movies directory (required).
	- `--format <fmt>`: Output filename format string. Valid replacements: `title`,
		`year`, `tmdb_id`.
	- `files`: One or more glob patterns to search for MKV files.

	Default movie format:

	`{title} ({year}) [tmdbid-{tmdb_id}]/{title} ({year}) [tmdbid-{tmdb_id}]{tag}.{ext}`

- `shows` subcommand:
	- `-o, --output <path>`: Destination shows directory (required).
	- `--format <fmt>`: Output filename format string. Valid replacements: `name`,
		`first_year`, `season_num`, `episode_num`, `source`, `source_id`.
	- `--mixed`: Episodes are not in sequence.
	- `--same-show`: Assume the same TV show for all provided directories.
	- `--tvdb-id <id>`: Use TVDB series ID instead of TMDB for lookups.
	- `directories`: One or more ripped folders with sequentially named MKV files.

	Default TV format:

	`{name} ({first_year}) [{source}-{source_id}]/Season {season_num:02}/{name} S{season_num:02}E{episode_num:02}.{ext}`

## Behavior notes

- For `movies`, each found file is passed through `movies.process_movie_file()`
	which returns a result indicating approval. Approved files are renamed/moved
	using `common.rename_file()`; unapproved files prompt for deletion.
- For `shows`, `shows.process_tv_dir()` is used to inspect directories and can
	reuse the same show metadata across multiple directories when `--same-show`
	is set.
- The tool will attempt to remove empty parent directories after processing
	(unless `--dry-run` is used).

## Examples

Rename movies (searching rips recursively):

```bash
python3 -m jellyname movies -k $TMDB_API_KEY -o /jellyfin/Movies/ '/rips/**/*.mkv'
```

Rename shows (process ripped season folders):

```bash
python3 -m jellyname shows -k $TMDB_API_KEY -o /jellyfin/Shows/ /rips/SomeShow/
```

Processing a show that uses DVD order, so we fallback to TVDB.
TVDB id must be looked up before hand and provided, then seasons/episode names are populated.
If you select the DVD order option, you'll need to change the metadata for the show to select that order.

```bash
python3 -m jellyname shows -k $TMDB_API_KEY --tvdb-id 12345 -o /jellyfin/Shows/ /rips/SomeShowDvdOrder/
```
