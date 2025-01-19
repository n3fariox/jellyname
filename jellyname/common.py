import shutil
from dataclasses import dataclass
from pathlib import Path

import prompt_toolkit as pt


@dataclass
class ProcessedFile:
    src: Path
    dst: Path
    approved: bool


def fix_title(title: str):
    """Fixup small inconsistent naming patterns"""
    title = title.lower()
    if title.endswith(", the"):
        title = "the " + title[:-5]
    return title



def rename_file(op: ProcessedFile, dry_run: bool = False):
    if not op.approved:
        return
    print(f"mv {op.src} -> {op.dst}")
    if not dry_run:
        op.dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(op.src, op.dst)


def prompt_continue(prompt_str: str) -> bool:
    while True:
        try:
            resp = pt.prompt(prompt_str).lower()
            if resp == "y":
                return True
            if resp == "n":
                return False
        except KeyboardInterrupt:
            return False
