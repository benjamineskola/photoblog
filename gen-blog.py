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


def create_post(
    title: str,
    body: str,
    images: list[Path],
    timestamp: datetime,
    **kwargs: str,
) -> None:
    filename = timestamp.strftime("%Y-%m-%d")

    metadata: dict[str, Any] = {
        "date": timestamp,
        "title": title,
        "extra": {
            "images": ["/" + os.path.basename(image) for image in images],
            **kwargs,
        },
    }

    if title:
        slug = re.sub(
            r"[^A-Za-z0-9-]+", "", title.lower().strip().replace(" ", "-")
        ).strip("-")
        filename += "-" + slug
        metadata["slug"] = timestamp.strftime("%Y") + "/" + slug
    else:
        metadata["title"] = timestamp.strftime("%Y-%m-%d")
        filename += "-" + timestamp.strftime("%H-%M-%S")
        metadata["slug"] = (
            timestamp.strftime("%Y") + "/" + timestamp.strftime("%Y-%m-%dT%H:%M:%S")
        )

    filename += ".md"

    for n, image in enumerate(images):
        ImageSet(image).save(with_thumbnail=(n == 0))

    with open(BLOG_DIR / filename, "w") as file:
        print("+++", file=file)
        print(tomli_w.dumps(metadata).strip(), file=file)

        print("+++\n", file=file)
        if body:
            print(body, file=file)


if __name__ == "__main__":
    path = Path(sys.argv[1])
    if path.is_dir():
        files = sorted(path.glob("2*.json.xz"))
    else:
        files = [Path(f) for f in sys.argv[1:-2]]

    for file in files:
        print(f"Processing {file}")
        text = subprocess.run(["xzcat", file], capture_output=True).stdout
        post = json.loads(text)

        metadata = {"instagram": "https://instagram.com/p/" + post["node"]["shortcode"]}
        caption = post["node"]["iphone_struct"].get("caption")
        location = post["node"]["iphone_struct"].get("location")
        if location:
            metadata["location"] = location["name"]

        basename = Path(Path(file).stem).stem

        title = ""
        body = ""
        if caption:
            title, *body = caption.get("text").splitlines()
            body = "\n".join(
                line for line in body if line != "." and not line.startswith("#")
            )

        if ":" in title:
            title, rest = title.split(":", 1)
            body = rest.strip() + "\n" + body

        images = sorted(
            Path("/Users/ben/Pictures/Instagram/ben.eskola").glob(f"{basename}*.jpg"),
            key=str,
        )

        timestamp = datetime.strptime(basename, "%Y-%m-%d_%H-%M-%S_%Z")
        create_post(title, body, images, timestamp, **metadata)
