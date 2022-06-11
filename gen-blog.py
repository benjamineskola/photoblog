#!/usr/bin/env python

import os
import sys
from pathlib import Path

from PIL import Image

BLOG_DIR = Path(sys.argv[-1])
NEWLINE = "\n"


class ImageSet:
    def __init__(self, source: Path):
        self.source = source
        self.name = source.name
        self.target = BLOG_DIR / self.name
        self.target_medium = BLOG_DIR / (self.target.stem + "_med.jpg")
        self.thumbnail = BLOG_DIR / (self.target.stem + "_thumb.jpg")

    def save(self, with_thumbnail: bool = False) -> None:
        if not self.target.exists():
            self.target.hardlink_to(self.source)

        if not self.target_medium.exists():
            medium = Image.open(self.target)
            medium.thumbnail((800, 800))
            medium.save(self.target_medium)

        if with_thumbnail and not os.path.exists(self.thumbnail):
            thumb = Image.open(self.target)
            width, height = thumb.size

            if width > height:
                thumb.thumbnail((999999, 450))
            elif height > width:
                thumb.thumbnail((450, 999999))
            else:
                thumb.thumbnail((450, 450))

            if width != height:
                width, height = thumb.size
                mid_w = width // 2
                mid_h = height // 2
                thumb = thumb.crop((mid_w - 225, mid_h - 225, mid_w + 225, mid_h + 225))

            thumb.save(self.thumbnail)

    def __str__(self) -> str:
        return self.name


if __name__ == "__main__":
    path = Path(sys.argv[1])
    if path.is_dir():
        files = sorted(path.glob("2*.json.xz"))
    else:
        files = [Path(f) for f in sys.argv[1:-2]]

    for file in files:
        print(f"Processing {file}")
        stem = file.name.removesuffix(".json.xz")
        images = sorted(
            Path("/Users/ben/Pictures/Instagram/ben.eskola").glob(f"{stem}*.jpg"),
            key=str,
        )
        for n, image in enumerate(self.images):
            ImageSet(image).save(with_thumbnail=(n == 0))
