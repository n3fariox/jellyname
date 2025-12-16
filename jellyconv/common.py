from pathlib import Path
import prompt_toolkit as pt


def human_size(path: Path) -> str:
    try:
        s = path.stat().st_size
    except Exception:
        return "?"
    for unit in ("B", "KB", "MB", "GB"):
        if s < 1024.0:
            return f"{s:.1f}{unit}"
        s /= 1024.0
    return f"{s:.1f}TB"


def prompt_continue(prompt_str: str) -> bool:
    while True:
        try:
            resp = pt.prompt(prompt_str).lower()
            if resp in ("y", "yes"):
                return True
            if resp in ("n", "no"):
                return False
        except KeyboardInterrupt:
            return False
