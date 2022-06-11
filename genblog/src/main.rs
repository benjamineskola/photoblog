use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

use chrono::{DateTime, FixedOffset};
use regex::Regex;
use serde::Serialize;
use xz::read::XzDecoder;

#[derive(Serialize)]
struct PostMeta {
    date: DateTime<FixedOffset>,
    title: Option<String>,
    slug: String,
    extra: Option<PostMetaExtra>,
}

#[derive(Serialize)]
struct PostMetaExtra {
    images: Vec<String>,
    instagram: String,
    location: Option<String>,
}

fn parse_caption(caption: &json::JsonValue) -> (Option<String>, Option<String>) {
    if let json::Null = caption {
        return (None, None);
    };
    let caption = caption.as_str().unwrap().trim();

    let number_re = Regex::new(r"(^\d+|c)\. (\w+|\d+)").expect("Invalid regex");
    let lines_re = Regex::new(r"(\n|\. )").expect("Invalid regex");
    let split_regex = Regex::new(r"(.*) ((\n|http://\S+|https://\S+).*?)").expect("Invalid regex");
    let tidy_regex = Regex::new(r"(#\S+\s*|^\.|\n\.|https?://\S+)").expect("Invalid regex");

    let caption = number_re
        .replace_all(caption, "${1}.\u{00A0}${2}")
        .to_string();

    if lines_re.is_match(caption.as_str()) {
        let mut split = lines_re.splitn(caption.as_str(), 2);
        // println!("{:?}", split);
        let title = split.next().unwrap().to_string();
        let body = split.next().unwrap().to_string();

        let body = tidy_regex.replace_all(body.as_str(), "").trim().to_string();

        if body.is_empty() {
            return (Some(title), None);
        } else {
            return (Some(title), Some(body));
        }
    }

    if let Some(result) = split_regex.captures(caption.as_str()) {
        if result.len() > 1 {
            let title = result.get(1).unwrap().as_str();
            let body = result.get(3).unwrap().as_str();
            (Some(title.to_string()), Some(body.to_string()))
        } else {
            (Some(caption.to_string()), None)
        }
    } else {
        (Some(caption.to_string()), None)
    }
}

fn slugify(name: &str) -> String {
    let slug_regex = Regex::new(r"[^A-Za-z0-9 \u{00A0}-]+").expect("invalid regex");

    slug_regex
        .replace_all(name.to_lowercase().as_str(), "")
        .trim()
        .replace(' ', "-")
        .replace("\u{00A0}", "-")
}

fn get_data(filename: &Path) {
    let basedir = Path::new("/Users/ben/Code/photoblog/content/");

    if let Ok(compressed_data) = File::open(filename) {
        let mut contents = String::new();
        let mut decompressor = XzDecoder::new(compressed_data);
        decompressor
            .read_to_string(&mut contents)
            .expect("Failed to decompress data");

        let data = json::parse(&contents).unwrap();

        let (title, body) = parse_caption(&data["node"]["iphone_struct"]["caption"]["text"]);

        let location = match &data["node"]["iphone_struct"]["location"]["name"] {
            json::Null => None,
            c => Some(c.to_string()),
        };

        let instagram_id: String = data["node"]["shortcode"].to_string();

        let basename = filename
            .file_name()
            .expect("failed to parse filename")
            .to_str()
            .expect("failed to parse filename")
            .to_string();
        let mut timestamp_str = basename.clone();
        timestamp_str.replace_range((timestamp_str.len() - 8).., "");
        timestamp_str.replace_range((timestamp_str.len() - 4).., "+0000");
        let timestamp = DateTime::parse_from_str(&timestamp_str, "%Y-%m-%d_%H-%M-%S%z").unwrap();

        let slug = match &title {
            Some(title) => slugify(title),
            None => timestamp.format("%Y-%m-%dT%H:%M:%S").to_string(),
        };
        let output_filename = match &title {
            Some(_) => timestamp.format("%Y-%m-%d-").to_string() + &slug,
            None => timestamp.format("%Y-%m-%d-%H-%M-%S").to_string(),
        } + ".md";

        let mut images: Vec<String> = vec![];
        let file_stem = basename.replace(".json.xz", "");
        let image = basedir.join(file_stem.clone() + ".jpg");
        if image.exists() {
            images.push("/".to_owned() + &image.file_name().unwrap().to_str().unwrap().to_string());
        } else {
            for i in 1..1000 {
                let image = basedir.join(file_stem.clone() + "_" + &i.to_string() + ".jpg");
                if image.exists() {
                    images.push(
                        "/".to_owned() + &image.file_name().unwrap().to_str().unwrap().to_string(),
                    );
                } else {
                    break;
                }
            }
        }

        let extra = PostMetaExtra {
            images,
            location,
            instagram: "https://instagram.com/p/".to_owned() + &instagram_id,
        };
        let meta = PostMeta {
            date: timestamp,
            title,
            slug: timestamp.format("%Y/").to_string() + &slug,
            extra: Some(extra),
        };

        let toml = toml::to_string(&meta).unwrap();
        let mut file =
            File::create(basedir.join(output_filename)).expect("Failed to open file for writing");
        file.write_all(("+++\n".to_owned() + &toml + "+++\n").as_bytes())
            .expect("failed to write to file");

        if let Some(body) = body {
            file.write_all("\n".as_bytes());
            file.write_all(body.trim().as_bytes());
            file.write_all("\n".as_bytes());
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    for i in &args[1..] {
        get_data(Path::new(i));
    }
}
