import argparse
import logging
import os
import sys
from glob import glob
from pathlib import Path

import tmdbsimple as tmdb
from prompt_toolkit.shortcuts import yes_no_dialog

from . import common, movies, shows
from .filters import Filters


def make_filters(args):
    filters = Filters()
    if args.filter_lang:
        filters.lang = args.filter_lang.lower()
    return filters


def movie_logic(args):
    paths = []
    filters = make_filters(args)
    for pattern in args.files:
        if "*" not in pattern and pattern.endswith("/"):
            pattern += "*.mkv"
        paths.extend(glob(pattern, recursive=True))

    for filename in paths:
        res = movies.process_movie_file(
            args.output, args.format, Path(filename), filters=filters
        )
        if res is None:
            print(f"Failed processing file: {filename}")
            continue

        if res.approved:
            common.rename_file(res, args.dry_run)
        elif yes_no_dialog(
            text=f"Sure you want to delete?\n{res.src}",
        ).run():
            res.src.unlink()

        try:
            if not args.dry_run:
                res.src.parent.rmdir()
        except:
            pass


def tv_logic(args):
    tv_show = None
    for directory in args.directories:
        check = shows.process_tv_dir(args.output, args.format, Path(directory), dry_run=args.dry_run, mixed=args.mixed, tv_show=tv_show)
        if args.same_show and check:
            tv_show = check


def movie_argparser(movies_p: argparse.ArgumentParser):
    default_movie_format = "{title} ({year}) [tmdbid-{tmdb_id}]/{title} ({year}) [tmdbid-{tmdb_id}]{tag}.{ext}"
    movies_p.add_argument(
        "--format",
        default=default_movie_format,
        help=f'Output file format string. Valid replacements are: title, year, tmdb_id. Default: "{default_movie_format}\s',
    )
    movies_p.add_argument(
        "-o",
        "--output",
        required=True,
        type=Path,
        help="Should be jellyfin movies dir",
    )
    movies_p.add_argument(
        "files",
        nargs="+",
        type=str,
        help="Any number of glob file specifiers to look for mkv files",
    )


def tv_argparser(tv_p: argparse.ArgumentParser):
    tv_p.add_argument(
        "-o",
        "--output",
        required=True,
        type=Path,
        help="Should be jellyfin shows dir",
    )
    default_tv_format = "{name} ({first_year}) [tmdbid-{tmdb_id}]/Season {season_num:02}/{name} S{season_num:02}E{episode_num:02}.{ext}"
    tv_p.add_argument(
        "--format",
        default=default_tv_format,
        help=f'Output file format string. Valid replacements are: title, first_year, season_num, episode_num, tmdb_id. Default: "{default_tv_format}\s',
    )

    tv_p.add_argument(
        "--mixed",
        action="store_true",
        help="Episodes are not in sequence",
    )

    tv_p.add_argument(
        "--same-show",
        action="store_true",
        help="Assume the same TV show for all directories",
    )

    tv_p.add_argument(
        "directories",
        nargs="+",
        type=str,
        help="A ripped folder with sequentially named mkv files to be renamed",
    )


def main():
    parser = argparse.ArgumentParser(
        prog="jellyname",
        description="Quick interactive tool to move mkv files for jellyfin",
    )
    parser.add_argument(
        "-d",
        "--dry-run",
        action="store_true",
        help="Don't actually move or delete files",
    )
    parser.add_argument(
        "-l",
        "--filter-lang",
        type=str,
        help="Original language of movies to filter by",
    )
    parser.add_argument("-k", "--api-key", type=str, help="TMDB api key to use")
    subp = parser.add_subparsers(dest="cmd")
    movie_argparser(subp.add_parser("movies"))
    tv_argparser(subp.add_parser("shows"))
    args = parser.parse_args()

    logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

    if args.api_key:
        tmdb.API_KEY = args.api_key
    elif "TMDB_API_KEY" in os.environ:
        tmdb.API_KEY = os.environ["TMDB_API_KEY"]
    else:
        logging.error("No API key provided")
        return -1

    try:
        if args.cmd == "movies":
            movie_logic(args)
        elif args.cmd == "shows":
            tv_logic(args)
    except KeyboardInterrupt:
        logging.info("Exiting...")

if __name__ == "__main__":
    main()
