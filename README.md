# Jellyname

Simple tool to help rename MakeMKV rips from a source folder into a Jellyfin
folder (could be anything w/ custom formats).

Since I've had hit/miss luck with titles on my disks, this tool is currently
interactive and lets you enter custom search criteria if there is no title found.

Example Usage:

```bash
python3 -m jellyname -k <api_key> -o /jellyfin/Movies/ /jellyfin/rips/**/*.mkv
```
