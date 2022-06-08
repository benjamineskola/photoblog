#!/usr/bin/env python

import json
import os
import re
import subprocess
import sys
from datetime import datetime
from pathlib import Path

import tomli_w
from PIL import Image

BLOG_DIR = Path(sys.argv[-1])
NEWLINE = "\n"


def create_post(
    title: str,
    body: str,
    images: list[str],
    timestamp: datetime,
    insta_code: str,
    location: str,
) -> None:
    filename = timestamp.strftime("%Y-%m-%d")

    metadata = {
        "date": timestamp,
        "title": title,
        "extra": {
            "images": ["/" + os.path.basename(image) for image in sorted(images)],
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

    if insta_code:
        metadata["extra"]["instagram"] = f"https://instagram.com/p/{insta_code}"
    if location:
        metadata["extra"]["location"] = location

    for image in images:
        target = BLOG_DIR / os.path.basename(image)

        if not os.path.exists(target):
            os.link(image, target)

    thumbnail_path = BLOG_DIR / (
        Path(BLOG_DIR / os.path.basename(images[0])).stem + "_thumb.jpg"
    )
    if not os.path.exists(thumbnail_path):
        thumb = Image.open(images[0])
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

        thumb.save(thumbnail_path)

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
        files = sys.argv[1:-2]

    for file in files:
        print(f"Processing {file}")
        text = subprocess.run(["xzcat", file], capture_output=True).stdout
        post = json.loads(text)
        caption = post["node"]["iphone_struct"].get("caption")
        insta_code = post["node"]["shortcode"]
        location = post["node"]["iphone_struct"].get("location")
        if location:
            location = location["name"]

        basename = Path(Path(file).stem).stem

        title = ""
        body = []
        if caption:
            title, *body = caption.get("text").splitlines()
            body = "\n".join(
                line for line in body if line != "." and not line.startswith("#")
            )

        if ":" in title:
            title, rest = title.split(":")
            body = rest.strip() + "\n" + body

        images = Path("/Users/ben/Pictures/Instagram/ben.eskola").glob(
            f"{basename}*.jpg"
        )

        timestamp = datetime.strptime(basename, "%Y-%m-%d_%H-%M-%S_%Z")
        create_post(title, body, sorted(images), timestamp, insta_code, location)
