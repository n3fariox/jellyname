import itertools
from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional, Tuple

import tmdbsimple as tmdb
from prompt_toolkit.shortcuts import button_dialog, input_dialog, radiolist_dialog
from pymkv import MKVFile

from .common import ProcessedFile, guess_title, rename_file


@dataclass
class TVSeason:
    name: str
    season_number: int
    episode_count: int
    year: str
    tmdb_id: int

    def __str__(self):
        return (
            f"{self.name} Season {self.season_number} ({self.episode_count} episodes)"
        )


@dataclass
class TVShow:
    name: str
    seasons: TVSeason
    episodes: int
    tmdb_id: int
    first_year: str
    last_year: str

    def __str__(self):
        return f"{self.name} ({self.first_year}-{self.last_year})"


@dataclass
class ProcessedTvFile(ProcessedFile):
    show: TVShow


@dataclass
class TVEpisode:
    name: str
    episode_number: int
    air_date: str
    overview: str
    tmdb_id: int

    def __str__(self):
        return f"S{self.episode_number:02} - {self.name}"


def identify_tv_show(filename: Path, title=None, manual=False) -> Optional[TVShow]:
    search_term = title
    if title is None or manual:
        default_text = ""
        if title:
            query_text = f"Title: {title}\nFile: {filename}\nEnter search criteria:"
            default_text = title
        else:
            query_text = f"No title for\n{filename}\nEnter search criteria:"
            default_text = guess_title(filename)
        maybe = input_dialog(
            title="Search",
            text=query_text,
            ok_text="Enter",
            cancel_text="Skip",
            default=default_text,
        ).run()
        if maybe is None:
            return None
        search_term = maybe

    search = tmdb.Search()
    search.tv(query=search_term)

    if not search.results:
        if title and manual:
            return None
        print("Title did not find results, try manual")
        return identify_tv_show(filename, title, True)

    shows = []
    for s in search.results:
        info = tmdb.TV(s["id"]).info()
        first_year = info.get("first_air_date", "...").split("-")[0]
        last_year = info.get("last_air_date", "...").split("-")[0]
        seasons = []
        for season in info["seasons"]:
            air_date = season["air_date"]
            seasons.append(
                TVSeason(
                    name=season["name"],
                    season_number=season["season_number"],
                    episode_count=season["episode_count"],
                    year=air_date.split("-")[0] if air_date is not None else "N/A",
                    tmdb_id=season["id"],
                )
            )
        shows.append(
            TVShow(
                name=s["name"],
                seasons=seasons,
                episodes=info.get("number_of_episodes", 0),
                first_year=first_year,
                last_year=last_year,
                tmdb_id=s["id"],
            )
        )

    NONEABOVE = object()
    result = radiolist_dialog(
        title="Best Match",
        text=str(filename),
        values=[(x, str(x)) for x in shows] + [(NONEABOVE, "None of the above")],
    ).run()

    if result is NONEABOVE:
        return identify_tv_show(filename, title, True)
    if result is None:
        return None
    return result


def identify_tv_season(filename: Path, tv_show: TVShow) -> Optional[TVSeason]:
    NONEABOVE = object()
    season = radiolist_dialog(
        title="Which season?",
        text=f"Filename: {filename}\nShow: {tv_show.name}",
        values=[(x, str(x)) for x in tv_show.seasons] + [(NONEABOVE, "None of the above")],
    ).run()
    if season is None or season is NONEABOVE:
        return None
    return season


def select_episode(filename: Path, tv_show: TVShow, tv_season: TVSeason) -> Optional[TVEpisode]:
    """Prompt the user to select an episode from a list fetched from TMDB."""
    tmdb_season = tmdb.TV_Seasons(tv_show.tmdb_id, tv_season.season_number)
    tmdb_season.info()
    episodes = tmdb_season.episodes

    tv_episodes = [
        TVEpisode(
            name=ep['name'],
            episode_number=ep['episode_number'],
            air_date=ep['air_date'],
            overview=ep['overview'],
            tmdb_id=ep['id']
        ) for ep in episodes
    ]

    NONEABOVE = object()
    result = radiolist_dialog(
        title=f"Select Episode for {tv_show.name} Season {tv_season.season_number}",
        text=f"Filename: {filename}\nSeason {tv_season.season_number} Episodes",
        values=[(ep.episode_number, str(ep)) for ep in tv_episodes] + [(NONEABOVE, "None of the above")],
    ).run()

    if result is NONEABOVE or result is None:
        return None

    return next(ep for ep in tv_episodes if ep.episode_number == result)


def get_supported_files(directory: Path) -> List[Path]:
    return [
        x for x in itertools.chain(directory.glob("*.mkv"), directory.glob("*.mp4"))
    ]


def process_tv_dir(
    output_dir: Path,
    out_format: str,
    input_directory: Path,
    start_episode: int = 0,
    dry_run: bool = False,
) -> List[ProcessedTvFile]:
    """Process a ripped TV show directory.
    This flow is a little different, everything in the directory should be the same show.
    Since we don't get episode metadata from the disc, we rely on rips being in order to
    generate the episode numbers.
    """
    if not input_directory.is_dir():
        print(f"Skipping non-directory: {input_directory}")
    tv_show = None
    tv_season = None
    file_actions = []
    episode_num = start_episode
    episodes = sorted(get_supported_files(input_directory))
    for filename in episodes:
        if not filename.is_file():
            continue
        file = MKVFile(filename)
        if tv_show is None:
            tv_show = identify_tv_show(filename, file.title, False)

        if tv_show is not None and (tv_season is None or start_episode == -1):
            tv_season = identify_tv_season(filename, tv_show)
        if tv_show is None or tv_season is None:
            print("failed to identity")
            continue

        if start_episode == -1:
            episode_num = select_episode(filename, tv_show, tv_season)
            if episode_num is None:
                return None

        # Get the episode number from the output directory now that we have an
        # idea of where it's going
        if episode_num == 0:
            maybe_dst = output_dir / out_format.format(
                name=tv_show.name,
                first_year=tv_show.first_year,
                tmdb_id=tv_show.tmdb_id,
                season_num=tv_season.season_number,
                episode_num=0,
                ext=filename.suffix[1:],  # we don't want the "period"
            )
            episode_num = len(get_supported_files(maybe_dst.parent)) + 1
            print(f"Starting with episode {episode_num:02}")

        dst = output_dir / out_format.format(
            name=tv_show.name,
            first_year=tv_show.first_year,
            tmdb_id=tv_show.tmdb_id,
            season_num=tv_season.season_number,
            episode_num=episode_num,
            ext=filename.suffix[1:],  # we don't want the "period"
        )

        approved = button_dialog(
            title=f"{tv_show.name} S{tv_season.season_number:02}E{episode_num:02}",
            text=f"src: {filename}\ndst: {dst}" + (" (exists)" if dst.exists() else ""),
            buttons=[("Yes", True), ("Skip", None), ("Delete", False)],
        ).run()

        episode_num += 1
        if approved is None:
            continue
        if approved:
            rename_file(
                ProcessedTvFile(src=filename, dst=dst, approved=True, show=tv_show),
                dry_run=dry_run,
            )

    try:
        input_directory.rmdir()
    except:
        pass
