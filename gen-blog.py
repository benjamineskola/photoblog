#!/usr/bin/env python

import json
import os
import re
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from typing import Any

import tomli_w
from PIL import Image

BLOG_DIR = Path(sys.argv[-1])
NEWLINE = "\n"


class Post:
    def __init__(self, source: Path):
        text = subprocess.run(["xzcat", file], capture_output=True).stdout
        post = json.loads(text)

        stem = file.name.removesuffix(".json.xz")
        self.timestamp = datetime.strptime(stem, "%Y-%m-%d_%H-%M-%S_%Z")

        self.caption = post["node"]["iphone_struct"].get("caption")
        self.location = post["node"]["iphone_struct"].get("location", {}).get("name")
        self.instagram_id = post["node"]["shortcode"]

        self.title = ""
        self.body = ""
        if self.caption:
            self.title, *self.body = self.caption.get("text").splitlines()
            self.body = "\n".join(
                line
                for line in self.body
                if line != "." and not line.strip().startswith("#")
            )

        if ":" in self.title:
            self.title, rest = self.title.split(":", 1)
            self.body = rest.strip() + "\n" + self.body

        self.images = sorted(
            Path("/Users/ben/Pictures/Instagram/ben.eskola").glob(f"{stem}*.jpg"),
            key=str,
        )

    def save(self):
        metadata: dict[str, Any] = {
            "date": self.timestamp,
            "title": self.title,
            "extra": {
                "images": ["/" + image.name for image in self.images],
                "instagram": "https://instagram.com/p/" + self.instagram_id,
            },
        }
        if self.location:
            metadata["extra"]["location"] = self.location

        filename = self.timestamp.strftime("%Y-%m-%d")
        if self.title:
            slug = re.sub(
                r"[^A-Za-z0-9-]+", "", self.title.lower().strip().replace(" ", "-")
            ).strip("-")
            filename += "-" + slug
            metadata["slug"] = self.timestamp.strftime("%Y") + "/" + slug
        else:
            metadata["title"] = self.timestamp.strftime("%Y-%m-%d")
            filename += "-" + self.timestamp.strftime("%H-%M-%S")
            metadata["slug"] = (
                self.timestamp.strftime("%Y")
                + "/"
                + self.timestamp.strftime("%Y-%m-%dT%H:%M:%S")
            )
        filename += ".md"

        for n, image in enumerate(self.images):
            ImageSet(image).save(with_thumbnail=(n == 0))

        with (BLOG_DIR / filename).open("wb") as file:
            file.write(b"+++\n")
            tomli_w.dump(metadata, file)
            file.write(b"+++\n\n")

            if self.body:
                file.write(self.body.strip().encode())
                file.write(b"\n")


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
        Post(file).save()
