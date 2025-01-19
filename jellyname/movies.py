from dataclasses import dataclass
from pathlib import Path
from typing import Optional

import tmdbsimple as tmdb
from prompt_toolkit.shortcuts import (
    button_dialog,
    input_dialog,
    radiolist_dialog,
)
from pymkv import MKVFile

from .common import fix_title, ProcessedFile


@dataclass
class Movie:
    title: str
    year: str
    tmdb_id: str

    def __str__(self):
        return f"{self.title} ({self.year})"


@dataclass
class ProcessedMovieFile(ProcessedFile):
    movie: Movie


def find_match(movie: MKVFile, filename: Path, manual: bool = False) -> Optional[Movie]:
    search_term = movie.title
    if movie.title is None or manual:
        if movie.title:
            query_text = (
                f"Title: {movie.title}\nFile:s{filename}\nEnter search criteria:"
            )
        else:
            query_text = f"No title for\n{filename}\nEnter search criteria:"
        maybe = input_dialog(
            title="Search",
            text=query_text,
            ok_text="Enter",
            cancel_text="Skip",
        ).run()
        if maybe is None:
            return None
        search_term = maybe

    search = tmdb.Search()
    search.movie(query=search_term)

    if not search.results:
        if movie.title and manual:
            return None
        print("Title did not find results, try manual")
        return find_match(movie, filename, True)

    movies = []
    for s in search.results:
        year = s.get("release_date", "...").split("-")[0]
        movies.append(
            Movie(
                title=s["title"],
                year=year,
                tmdb_id=s["id"],
            )
        )

    NONEABOVE = object()
    result = radiolist_dialog(
        title="Best Match",
        text=str(filename),
        values=[(x, str(x)) for x in movies] + [(NONEABOVE, "None of the above")],
    ).run()

    if result is NONEABOVE:
        return find_match(movie, filename, True)
    if result is None:
        return None

    return result


def process_movie_file(
    output_dir: Path, out_format: str, filename: Path
) -> ProcessedMovieFile:
    file = MKVFile(filename)
    if not file.title:
        print(f"No title in {filename}")
    else:
        file.title = fix_title(file.title)

    match = find_match(file, filename)
    if match is None:
        print(f"No match for file: {filename}")
        return

    default_tag = ""
    maybe_dst = output_dir / out_format.format(
        title=match.title, year=match.year, tag="", tmdb_id=match.tmdb_id
    )
    if maybe_dst.exists():
        default_tag = f"CD{len(list(maybe_dst.parent.glob('*.mkv')))}"

    tag = input_dialog(
        title="Optional Tag",
        text="Enter a tag (optional):",
        default=default_tag,
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
        text=f"src: {src}\ndst: {dst}" + (" (exists)" if dst.exists() else ""),
        buttons=[("Yes", True), ("Skip", None), ("Delete", False)],
    ).run()
    if approved is None:
        return None

    return ProcessedMovieFile(src=src, dst=dst, approved=approved, movie=match)
