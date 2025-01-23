import argparse
import os
import sys
from glob import glob
from pathlib import Path

import tmdbsimple as tmdb
from prompt_toolkit.shortcuts import yes_no_dialog

from . import common, movies, shows


def movie_logic(args):
    paths = []
    for pattern in args.files:
        paths.extend(glob(pattern, recursive=True))

    for filename in paths:
        res = movies.process_movie_file(args.output, args.format, Path(filename))
        if res is None:
            print(f"Failed processing file: {filename}")
            continue

        if res.approved:
            common.rename_file(res)
            try:
                res.src.parent.rmdir()
            except:
                pass

        elif yes_no_dialog(
            text=f"Sure you want to delete?\n{res.src}",
        ).run():
            res.src.unlink()

    print("Removing empty folders from the input")
    for folder in args.output.iterdir():
        if folder.is_dir() and not any(folder.iterdir()):
            print(f"Removing {folder}")
            folder.rmdir()


def tv_logic(args):
    for directory in args.directories:
        check = shows.process_tv_dir(args.output, args.format, Path(directory))


def movie_argparser(movies_p: argparse.ArgumentParser):
    default_movie_format = "{title} ({year}) [tmdbid-{tmdb_id}]/{title} ({year}) [tmdbid-{tmdb_id}]{tag}.mkv"
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
    default_tv_format = "{name} ({first_year}) [tmdbid-{tmdb_id}]/Season {season_num:02}/{name} S{season_num:02}E{episode_num:02}.mkv"
    tv_p.add_argument(
        "--format",
        default=default_tv_format,
        help=f'Output file format string. Valid replacements are: title, first_year, season_num, episode_num, tmdb_id. Default: "{default_tv_format}\s',
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
    parser.add_argument("-k", "--api-key", type=str, help="TMDB api key to use")
    subp = parser.add_subparsers(dest="cmd")
    movie_argparser(subp.add_parser("movies"))
    tv_argparser(subp.add_parser("shows"))
    args = parser.parse_args()

    if args.api_key:
        tmdb.API_KEY = args.api_key
    elif "TMDB_API_KEY" in os.environ:
        tmdb.API_KEY = os.environ["TMDB_API_KEY"]
    else:
        print("No API key provided", file=sys.stderr)
        return -1

    if args.cmd == "movies":
        movie_logic(args)
    elif args.cmd == "shows":
        tv_logic(args)


if __name__ == "__main__":
    main()
