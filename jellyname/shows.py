import itertools
import logging
from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional, Tuple

import requests
import tmdbsimple as tmdb
from prompt_toolkit.shortcuts import (button_dialog, input_dialog,
                                      radiolist_dialog)
from pymkv import MKVFile

from .common import ProcessedFile, guess_title, rename_file


@dataclass
class TVSeason:
    name: str
    season_number: int
    episode_count: int
    year: str
    source_id: int
    episodes: List['TVEpisode'] = None

    def __post_init__(self):
        if self.episodes is None:
            self.episodes = []

    def __str__(self):
        return (
            f"{self.name} Season {self.season_number} ({self.episode_count} episodes)"
        )


@dataclass
class TVShow:
    name: str
    seasons: TVSeason
    episodes: int
    source_id: int
    first_year: str
    last_year: str
    source: str = "tmdb"

    def __str__(self):
        return f"{self.name} ({self.first_year}-{self.last_year}) [{self.source}-{self.source_id}]"


@dataclass
class ProcessedTvFile(ProcessedFile):
    show: TVShow


@dataclass
class TVEpisode:
    name: str
    episode_number: int
    air_date: str
    overview: str
    source_id: int

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
        logging.warning("Title did not find results, try manual")
        return identify_tv_show(filename, title, True)

    shows = []
    for s in search.results:
        info = tmdb.TV(s["id"]).info()
        first_year = info.get("first_air_date", "...").split("-")[0]
        first_year = "..." if first_year is None else first_year.split("-")[0]
        last_year = info.get("last_air_date")
        last_year = "..." if last_year is None else last_year.split("-")[0]
        seasons = []
        for season in info["seasons"]:
            air_date = season["air_date"]
            seasons.append(
                TVSeason(
                    name=season["name"],
                    season_number=season["season_number"],
                    episode_count=season["episode_count"],
                    year=air_date.split("-")[0] if air_date is not None else "N/A",
                    source_id=s["id"],
                )
            )
        shows.append(
            TVShow(
                name=s["name"],
                seasons=seasons,
                episodes=info.get("number_of_episodes", 0),
                first_year=first_year,
                last_year=last_year,
                source_id=s["id"],
                source="tmdb",
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


def identify_tv_show_tvdb(filename: Path, tvdb_id: int) -> Optional[TVShow]:
    """Identify TV show using TVDB ID directly (using public endpoints)."""
    try:
        base_url = f"https://api.thetvdb.com/series/{tvdb_id}"

        response = requests.get(base_url, timeout=10)
        if response.status_code != 200:
            logging.error(f"Failed to fetch TVDB show {tvdb_id}: {response.status_code}")
            return None

        data = response.json().get("data", {})

        show_name = data.get("seriesName", "Unknown")
        first_aired = data.get("firstAired", "")
        first_year = first_aired.split("-")[0] if first_aired else "..."
        last_year = "..."

        # Get season count from the season field
        season_count_str = data.get("season", "0")
        try:
            season_count = int(season_count_str)
        except (ValueError, TypeError):
            season_count = 0

        # Fetch episode summary to check if DVD order exists
        summary_url = f"https://api.thetvdb.com/series/{tvdb_id}/episodes/summary"
        summary_response = requests.get(summary_url, timeout=10)
        has_dvd_order = False
        if summary_response.status_code == 200:
            summary = summary_response.json().get("data", {})
            dvd_seasons = summary.get("dvdSeasons", [])
            has_dvd_order = len(dvd_seasons) > 0

        # Prompt user to choose ordering if both exist
        use_dvd_order = False
        if has_dvd_order:
            from prompt_toolkit.shortcuts import button_dialog
            choice = button_dialog(
                title="Episode Ordering",
                text=f"Which episode order would you like to use for {show_name}?",
                buttons=[("Aired Order", False), ("DVD Order", True)],
            ).run()
            use_dvd_order = choice if choice is not None else False

        # Fetch all episodes with the chosen ordering
        episodes_url = f"https://api.thetvdb.com/series/{tvdb_id}/episodes"
        if use_dvd_order:
            episodes_url += "/query?dvdSeason=1"

        episodes_response = requests.get(episodes_url, timeout=10)
        episodes_data = {}
        if episodes_response.status_code == 200:
            all_episodes = episodes_response.json().get("data", [])

            if use_dvd_order:
                # For DVD order, episodes are flat in the response
                episodes_data[1] = all_episodes
            else:
                # For aired order, organize by season
                for ep in all_episodes:
                    season_num = ep.get("airedSeason")
                    if season_num is not None:
                        if season_num not in episodes_data:
                            episodes_data[season_num] = []
                        episodes_data[season_num].append(ep)

        # Build seasons list with episodes
        seasons = []
        if use_dvd_order:
            # DVD order - single season with all episodes
            season_episodes = episodes_data.get(1, [])
            tv_episodes = []
            for ep in sorted(season_episodes, key=lambda x: x.get("dvdEpisodeNumber", 0)):
                tv_episodes.append(
                    TVEpisode(
                        name=ep.get("episodeName", ""),
                        episode_number=ep.get("dvdEpisodeNumber", 0),
                        air_date=ep.get("firstAired", ""),
                        overview=ep.get("overview", ""),
                        source_id=ep.get("id", 0)
                    )
                )

            seasons.append(
                TVSeason(
                    name="Season 1 (DVD Order)",
                    season_number=1,
                    episode_count=len(tv_episodes),
                    year=first_year,
                    source_id=tvdb_id,
                    episodes=tv_episodes,
                )
            )
        else:
            # Aired order - multiple seasons
            for i in range(season_count + 1):  # Include season 0 for specials
                season_episodes = episodes_data.get(i, [])
                tv_episodes = []
                for ep in sorted(season_episodes, key=lambda x: x.get("airedEpisodeNumber", 0)):
                    tv_episodes.append(
                        TVEpisode(
                            name=ep.get("episodeName", ""),
                            episode_number=ep.get("airedEpisodeNumber", 0),
                            air_date=ep.get("firstAired", ""),
                            overview=ep.get("overview", ""),
                            source_id=ep.get("id", 0)
                        )
                    )

                if tv_episodes:  # Only add season if it has episodes
                    seasons.append(
                        TVSeason(
                            name=f"Season {i}" if i > 0 else "Specials",
                            season_number=i,
                            episode_count=len(tv_episodes),
                            year=first_year,
                            source_id=tvdb_id,
                            episodes=tv_episodes,
                        )
                    )

        return TVShow(
            name=show_name,
            seasons=seasons,
            episodes=len(seasons),
            source_id=tvdb_id,
            first_year=first_year,
            last_year=last_year,
            source="tvdb",
        )
    except Exception as e:
        logging.error(f"Failed to fetch TVDB show {tvdb_id}: {e}")
        return None


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
    """Prompt the user to select an episode from the season's episode list."""
    # Use episodes already loaded in the season
    tv_episodes = tv_season.episodes

    if not tv_episodes:
        # Fallback for TMDB if episodes not pre-loaded
        if tv_show.source == "tmdb":
            tmdb_season = tmdb.TV_Seasons(tv_show.source_id, tv_season.season_number)
            tmdb_season.info()
            episodes = tmdb_season.episodes

            tv_episodes = [
                TVEpisode(
                    name=ep['name'],
                    episode_number=ep['episode_number'],
                    air_date=ep['air_date'],
                    overview=ep['overview'],
                    source_id=ep['id']
                ) for ep in episodes
            ]
        else:
            logging.warning(f"No episodes available for {tv_show.name} Season {tv_season.season_number}")
            return None

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
    mixed: bool = False,
    tv_show: Optional[TVShow] = None,
    tvdb_id: Optional[int] = None,
) -> Optional[TVShow]:
    """Process a ripped TV show directory.
    This flow is a little different, everything in the directory should be the same show.
    Since we don't get episode metadata from the disc, we rely on rips being in order to
    generate the episode numbers.
    """
    if not input_directory.is_dir():
        logging.warning(f"Skipping non-directory: {input_directory}")

    tv_season = None
    file_actions = []
    episode_num = start_episode
    episodes = sorted(get_supported_files(input_directory))
    approve_all = False
    for filename in episodes:
        if not filename.is_file():
            continue
        file = MKVFile(filename)
        if tv_show is None:
            if tvdb_id is not None:
                tv_show = identify_tv_show_tvdb(filename, tvdb_id)
            else:
                tv_show = identify_tv_show(filename, file.title, False)

        if tv_show is not None and (tv_season is None or mixed):
            tv_season = identify_tv_season(filename, tv_show)
        if tv_show is None or tv_season is None:
            logging.warning("Failed to identify TV show or season")
            continue

        if mixed:
            episode = select_episode(filename, tv_show, tv_season)
            if episode is None:
                return None
            episode_num = episode.episode_number

        # Get the episode number from the output directory now that we have an
        # idea of where it's going
        if episode_num == 0:
            maybe_dst = output_dir / out_format.format(
                name=tv_show.name,
                first_year=tv_show.first_year,
                source=tv_show.source,
                source_id=tv_show.source_id,
                season_num=tv_season.season_number,
                episode_num=0,
                ext=filename.suffix[1:],  # we don't want the "period"
            )
            episode_num = len(get_supported_files(maybe_dst.parent)) + 1
            logging.info(f"Starting with episode {episode_num:02}")

        dst = output_dir / out_format.format(
            name=tv_show.name,
            first_year=tv_show.first_year,
            source=tv_show.source,
            source_id=tv_show.source_id,
            season_num=tv_season.season_number,
            episode_num=episode_num,
            ext=filename.suffix[1:],  # we don't want the "period"
        )

        if approve_all:
            approved = True
        else:
            approved = button_dialog(
                title=f"{tv_show.name} S{tv_season.season_number:02}E{episode_num:02}",
                text=f"src: {filename}\ndst: {dst}" + (" (exists)" if dst.exists() else ""),
                buttons=[("Yes", True), ("Yes to All", "all"), ("Skip", None), ("Delete", False)],
            ).run()

            if approved == "all":
                approve_all = True
                approved = True

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
    except Exception:
        pass

    return tv_show
