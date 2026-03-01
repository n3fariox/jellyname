import logging
import shutil
import shlex
import subprocess
from pathlib import Path
from typing import Dict, List

import prompt_toolkit as pt
from prompt_toolkit.shortcuts import checkboxlist_dialog, radiolist_dialog
import yaml
import re
from prompt_toolkit.shortcuts import ProgressBar

from .common import human_size, prompt_continue


def load_profiles(path: Path) -> Dict:
    try:
        with open(path, "r") as fh:
            return yaml.safe_load(fh) or {}
    except FileNotFoundError:
        logging.error("profiles.yml not found: %s", path)
        return {}


def list_mkv(folder: Path) -> List[Path]:
    return sorted([p for p in folder.glob("*.mkv") if p.is_file()])


def prompt_select_files(files: List[Path]) -> List[Path]:
    # Use a checkbox dialog for selection; default all checkboxes to checked.
    # Build choices as (value, label) pairs and set all values as default checked
    choices = [(str(i), f"{f.name} ({human_size(f)})") for i, f in enumerate(files)]
    default_values = [str(i) for i in range(len(files))]
    result = checkboxlist_dialog(
        title="Select files",
        text="Check files to convert:",
        values=choices,
        default_values=default_values,
    ).run()
    if not result:
        return []
    selected = []
    for key in result:
        try:
            idx = int(key)
            if 0 <= idx < len(files):
                selected.append(files[idx])
        except Exception:
            continue
    return selected


def prompt_select_profile(profiles: Dict) -> str:
    keys = list(profiles.keys())
    choices = [(k, f"{k} - {profiles[k].get('description','')}") for k in keys]
    result = radiolist_dialog(
        title="Select profile", text="Choose a profile:", values=choices
    ).run()
    if result is None:
        return keys[0]
    return result


def _video_codec_flags(profile: Dict) -> List[str]:
    v = profile.get("video_codec")
    if not v:
        return []
    vmap = {
        "h264": "libx264",
        "x264": "libx264",
        "h265": "libx265",
        "x265": "libx265",
        "nvenc": "h264_nvenc",
        "h264_nvenc": "h264_nvenc",
        "copy": "copy",
    }
    chosen = vmap.get(v, v)
    flags: List[str] = ["-c:v", chosen]

    # preset and crf are video-related
    preset = profile.get("preset")
    if preset:
        flags += ["-preset", str(preset)]

    crf = profile.get("crf")
    if crf is not None and chosen != "copy":
        flags += ["-crf", str(crf)]

    vb = profile.get("video_bitrate")
    if vb:
        flags += ["-b:v", str(vb)]

    return flags


def _audio_codec_flags(profile: Dict) -> List[str]:
    a = profile.get("audio_codec")
    if not a:
        return []
    amap = {"copy": "copy", "aac": "aac", "ac3": "ac3", "mp3": "libmp3lame"}
    chosen = amap.get(a, a)
    flags: List[str] = ["-c:a", chosen]
    ab = profile.get("audio_bitrate")
    if ab:
        flags += ["-b:a", str(ab)]
    return flags


def _extra_flags(profile: Dict) -> List[str]:
    extra = profile.get("extra_flags")
    if not extra:
        return []
    if isinstance(extra, list):
        out: List[str] = []
        for e in extra:
            out += shlex.split(str(e))
        return out
    return shlex.split(str(extra))


def _hwaccel_flags(profile: Dict) -> List[str]:
    # if "hwaccel" in profile:
    #     return ["-init_hw_device", "qsv=hw", "-hwaccel", profile["hwaccel"], "-hwaccel_device", "qsv"]
    # simple default: let ffmpeg pick available accel for decoding
    return ["-hwaccel", "auto", "-hwaccel_output_format", "auto"]
    # return [
    #     "-init_hw_device",
    #     "qsv=qsv:MFX_IMPL_hw",
    #     "shwaccel_device",
    #     "qsv",
    #     "-hwaccel",
    #     "qsv",
    #     # "-qsv_device",
    #     # "/dev/dri/renderD128",
    #     "-hwaccel_output_format",
    #     "qsv",
    # ]


def build_cmd(profile: Dict, inp: Path, out: Path) -> List[str]:
    cmd: List[str] = ["ffmpeg"]
    cmd += _hwaccel_flags(profile)
    cmd += ["-i", str(inp)]
    cmd += ["-map_metadata", "0", "-movflags", "faststart"]
    cmd += _video_codec_flags(profile)
    cmd += _audio_codec_flags(profile)
    cmd += _extra_flags(profile)
    cmd += [str(out)]
    return cmd


def get_duration(path: Path) -> float:
    try:
        p = subprocess.run(
            [
                "ffprobe",
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
                str(path),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            check=False,
        )
        out = p.stdout.strip()
        return float(out) if out else 0.0
    except Exception:
        return 0.0


def confirm_and_run(selected: List[Path], profile: Dict):
    if not selected:
        print("No files selected")
        return

    # Show command for first file
    # sample_in = selected[0]
    sample_in = Path("<input_file>")
    sample_bak = sample_in.with_name(sample_in.name + ".bak")
    sample_out = sample_in
    sample_cmd = build_cmd(profile, sample_bak, sample_out)
    print("Example ffmpeg command (bak -> new file):")
    print(" ".join(sample_cmd))
    if not prompt_continue("Proceed with conversion for selected files? (y/n): "):
        print("Aborted")
        return

    for f in selected:
        handle_single_file(f, profile)


def run_ffmpeg_with_progress(cmd: List[str], duration: float, name: str) -> int:
    """Run ffmpeg with -progress pipe:1 and show a prompt_toolkit ProgressBar.

    Returns the ffmpeg return code.
    """
    time_re = re.compile(r"out_time=(\d+):(\d+):(\d+\.\d+)")
    ms_re = re.compile(r"out_time_ms=(\d+)")

    try:
        proc = subprocess.Popen(
            cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
        )
    except Exception as e:
        logging.error("Failed to start ffmpeg for %s: %s", name, e)
        return 1

    total_steps = max(1, int(duration))
    last_time = 0
    try:
        with ProgressBar(title=name) as pb:
            pb_iter = iter(pb(range(total_steps)))
            for line in proc.stdout:
                line = line.strip()
                if not line:
                    continue
                mms = ms_re.search(line)
                if mms:
                    secs = int(mms.group(1)) / 1000000.0
                else:
                    mt = time_re.search(line)
                    if mt:
                        hrs = int(mt.group(1))
                        mins = int(mt.group(2))
                        secsf = float(mt.group(3))
                        secs = hrs * 3600 + mins * 60 + secsf
                    else:
                        if line.startswith("out_time="):
                            try:
                                val = line.split("=", 1)[1]
                                h, m, s = val.split(":")
                                secs = int(h) * 3600 + int(m) * 60 + float(s)
                            except Exception:
                                continue
                        else:
                            continue

                current_int = min(int(secs), total_steps)
                while last_time < current_int:
                    try:
                        next(pb_iter)
                    except StopIteration:
                        break
                    last_time += 1

            proc.wait()
            if proc.returncode != 0:
                logging.error("ffmpeg failed stdout: %s", proc.stderr.read())
            return proc.returncode or 0
    except Exception as e:
        logging.error("Error while running ffmpeg for %s: %s", name, e)
        try:
            proc.kill()
        except Exception:
            pass
        return 1


def handle_single_file(f: Path, profile: Dict):
    bak = f.with_name(f.name + ".bak")
    try:
        logging.info("Moving %s -> %s", f, bak)
        shutil.move(str(f), str(bak))
    except Exception as e:
        logging.error("Failed to backup %s: %s", f, e)
        return

    out = f
    cmd = build_cmd(profile, bak, out)
    if len(cmd) >= 1:
        cmd = cmd[:-1] + ["-progress", "pipe:1", "-nostats"] + [cmd[-1]]

    duration = get_duration(bak)
    rc = run_ffmpeg_with_progress(cmd, duration, f.name)
    if rc != 0:
        logging.error("ffmpeg failed for %s (code %s). Restoring backup.", f, rc)
        try:
            if out.exists():
                out.unlink()
            shutil.move(str(bak), str(f))
        except Exception as e:
            logging.error("Failed to restore backup for %s: %s", f, e)
        return

    try:
        bak_size = bak.stat().st_size if bak.exists() else 0
    except Exception:
        bak_size = 0
    try:
        out_size = out.stat().st_size if out.exists() else 0
    except Exception:
        out_size = 0

    saved = bak_size - out_size
    logging.info("Successfully converted %s", f)
    if saved > 0:
        print(
            f"Converted: {f.name} — saved {human_size(Path('.').with_name('tmp')) if False else ''}"
        )
        # Use human-readable sizes
        print(f"Original: {human_size(bak)} ({bak_size} bytes)")
        print(f"New:      {human_size(out)} ({out_size} bytes)")
        print(f"Space saved: {human_size(Path('.')) if False else ''}")
    else:
        print(f"Converted: {f.name} — size change: {bak_size} -> {out_size} bytes")


def main():
    import argparse

    parser = argparse.ArgumentParser(
        prog="jellyconv", description="Interactive ffmpeg converter"
    )
    parser.add_argument("folder", type=Path, help="Folder with files to convert")
    parser.add_argument(
        "--profiles", type=Path, default=Path("profiles.yml"), help="profiles.yml path"
    )
    args = parser.parse_args()

    logging.basicConfig(
        level=logging.INFO, format="%(asctime)s - %(levelname)s - %(message)s"
    )

    folder = args.folder
    if not folder.exists() or not folder.is_dir():
        logging.error("Folder does not exist: %s", folder)
        return

    profiles = load_profiles(args.profiles)
    if not profiles:
        logging.error("No profiles found in %s", args.profiles)
        return

    files = list_mkv(folder)
    if not files:
        print("No .mkv files in folder")
        return

    selected = prompt_select_files(files)
    if not selected:
        print("No files chosen")
        return

    prof_key = prompt_select_profile(profiles)
    profile = profiles.get(prof_key)

    confirm_and_run(selected, profile)
