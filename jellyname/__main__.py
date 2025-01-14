import argparse
from glob import glob
import os
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

import tmdbsimple as tmdb
from prompt_toolkit import prompt
from prompt_toolkit.shortcuts import (
    radiolist_dialog,
    input_dialog,
    yes_no_dialog,
    button_dialog,
)
from pymkv import MKVFile


@dataclass
class Movie:
    title: str
    year: str
    tmdb_id: str

    def __str__(self):
        return f"{self.title} ({self.year})"


def prompt_continue() -> bool:
    while True:
        try:
            resp = prompt("Look correct? (y/n)").lower()
            if resp == "y":
                return True
            if resp == "n":
                return False
        except KeyboardInterrupt:
            return False


def fix_title(title: str):
    title = title.lower()
    if title.endswith(", the"):
        title = "the " + title[:-5]
    return title


def find_match(movie: MKVFile, filename: Path) -> Optional[Movie]:
    search_term = movie.title
    if movie.title is None:
        print(f"No title in: {filename}")
        maybe = input_dialog(
            title="Search",
            text=f"No title for\n{filename}\nEnter search criteria:",
            ok_text="Enter",
            cancel_text="Skip",
        ).run()
        if maybe is None:
            return None
        search_term = maybe

    search = tmdb.Search()
    search.movie(query=search_term)

    if not search.results:
        return None

    movies = []
    for s in search.results:
        year = s["release_date"].split("-")[0]
        movies.append(
            Movie(
                title=s["title"],
                year=year,
                tmdb_id=s["id"],
            )
        )

    result = radiolist_dialog(
        title="Best Match",
        text=str(filename),
        values=[(x, str(x)) for x in movies] + [(None, "None of the above")],
    ).run()

    if result is None:
        return None

    return result


@dataclass
class ProcessedFile:
    movie: Movie
    src: Path
    dst: Path
    approved: bool


def process_file(output_dir: Path, out_format: str, filename: Path) -> ProcessedFile:
    file = MKVFile(filename)
    match = find_match(file, filename)
    if match is None:
        print(f"No match for file: {filename}")
        return

    tag = input_dialog(
        title="Optional Tag",
        text="Enter a tag (optional):",
        cancel_text="Skip",
    ).run()
    if tag:
        tag = f" - {tag}"
    else:
        tag = ""

    src = filename
    dst = output_dir / out_format.format(
        title=match.title, year=match.year, tag=tag, tmdb_id=match.tmdb_id
    )

    approved = button_dialog(
        title=match.title,
        text=f"src: {src}\ndst: {dst}",
        buttons=[("Yes", True), ("Skip", None), ("Delete", False)],
    ).run()
    if approved is None:
        return None

    return ProcessedFile(movie=match, src=src, dst=dst, approved=approved)


def rename_file(op: ProcessedFile):
    if not op.approved:
        return
    print(f"Moving {op.movie.title}")
    op.dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.move(op.src, op.dst)


def main():
    parser = argparse.ArgumentParser(
        prog="jellyname",
        description="Quick interactive tool to move mkv files for jellyfin",
    )
    parser.add_argument(
        "-o",
        "--output",
        required=True,
        type=Path,
        help="Should be jellyfin movies/shows dir",
    )
    default_format = "{title} ({year}) [tmdbid-{tmdb_id}]/{title} ({year}) [tmdbid-{tmdb_id}]{tag}.mkv"
    parser.add_argument(
        "--format",
        default=default_format,
        help=f"Output file format string. Valid replacements are: title, year, tmdb_id. Default: \"{default_format}\s",
    )
    parser.add_argument(
        "-d",
        "--dry-run",
        action="store_true",
        help="Don't actually move or delete files",
    )
    parser.add_argument("-k", "--api-key", type=str, help="TMDB api key to use")
    parser.add_argument(
        "files",
        nargs="+",
        type=str,
        help="Any number of glob file specifiers to look for mkv files",
    )
    args = parser.parse_args()

    if args.api_key:
        tmdb.API_KEY = args.api_key
    elif "TMDB_API_KEY" in os.environ:
        tmdb.API_KEY = os.environ["TMDB_API_KEY"]
    else:
        print("No API key provided", file=sys.stderr)
        return -1

    paths = []
    for pattern in args.files:
        paths.extend(glob(pattern, recursive=True))

    for filename in paths:
        res = process_file(args.output, args.format, Path(filename))
        if res is None:
            print(f"Failed processing file: {filename}")
            continue

        if res.approved:
            if args.dry_run:
                print(f"mv {res.src} {res.dst}")
            else:
                rename_file(res)
        elif yes_no_dialog(
            text=f"Sure you want to delete?\n{res.src}",
        ).run():
            res.src.unlink()

    print("Removing empty folders from the output")
    for folder in args.output.iterdir():
        if folder.is_dir() and not any(folder.iterdir()):
            print(f"Removing {folder}")
            folder.rmdir()


if __name__ == "__main__":
    main()
