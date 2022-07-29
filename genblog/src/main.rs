use std::env;
use std::fs::{hard_link, read_dir, remove_file, File};
use std::io::prelude::*;
use std::path::Path;

use chrono::{DateTime, FixedOffset};
use lazy_static::lazy_static;
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
    alts: Option<Vec<String>>,
    instagram: String,
    location: Option<String>,
}

fn parse_caption(caption: &json::JsonValue) -> (Option<String>, Option<String>) {
    if let json::Null = caption {
        return (None, None);
    };
    let caption = caption.as_str().unwrap().trim();

    lazy_static! {
        static ref NUMBER_RE: Regex = Regex::new(r"(^\d+|c)\. (\w+|\d+)").expect("Invalid regex");
        static ref LINES_RE: Regex = Regex::new(r"(\n|\. )").expect("Invalid regex");
        static ref TIDY_RE: Regex =
            Regex::new(r"(^#|#\S+\s*|^\.|\n\.|https?://\S+)").expect("Invalid regex");
    }

    let caption = NUMBER_RE.replace_all(caption, "${1}.\u{00A0}${2}");
    let caption = TIDY_RE.replace_all(&caption, "");
    let caption = caption.trim();

    if LINES_RE.is_match(caption) {
        let mut split = LINES_RE.splitn(caption, 2);
        let title = split.next().unwrap().to_string();
        let body = split.next().unwrap().to_string();

        if body.is_empty() || body == "." {
            (Some(title), None)
        } else {
            (Some(title), Some(body))
        }
    } else {
        (Some(caption.to_string()), None)
    }
}

fn slugify(name: &str) -> String {
    lazy_static! {
        static ref SLUG_RE: Regex = Regex::new(r"[^A-Za-z0-9 \u{00A0}-]+").expect("invalid regex");
    }

    SLUG_RE
        .replace_all(name.to_lowercase().as_str(), "")
        .trim()
        .replace(' ', "-")
        .replace('\u{00A0}', "-")
}

fn generate_toml(input_filename: &Path, output_dir: &Path) -> Result<(), std::io::Error> {
    if let Ok(compressed_data) = File::open(input_filename) {
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

        let basename = input_filename
            .file_name()
            .expect("failed to parse filename")
            .to_str()
            .expect("failed to parse filename")
            .to_string();
        let mut timestamp_str = basename.clone();
        timestamp_str.replace_range((timestamp_str.len() - 8).., "");
        timestamp_str.replace_range((timestamp_str.len() - 4).., "+0000");
        let timestamp = DateTime::parse_from_str(&timestamp_str, "%Y-%m-%d_%H-%M-%S%z");
        if timestamp.is_err() {
            return Ok(());
        }
        let timestamp = timestamp.unwrap();

        let mut slug = match &title {
            Some(title) => slugify(title),
            None => timestamp.format("%Y-%m-%dT%H:%M:%S").to_string(),
        };
        let output_filename_stem = match &title {
            Some(_) => timestamp.format("%Y-%m-%d-").to_string() + &slug,
            None => timestamp.format("%Y-%m-%d-%H-%M-%S").to_string(),
        };
        let mut output_filename = output_filename_stem.clone() + ".md";

        let mut counter = 0;
        while output_dir.join(&output_filename).exists() {
            counter += 1;
            output_filename =
                (output_filename_stem.clone()) + "-" + counter.to_string().as_str() + ".md"
        }
        if counter > 0 {
            slug += &("-".to_owned() + counter.to_string().as_str());
        }

        let mut images: Vec<String> = vec![];
        let file_stem = basename.replace(".json.xz", "");
        let image = output_dir.join(file_stem.clone() + ".jpg");
        if image.exists() {
            images.push("/".to_owned() + image.file_name().unwrap().to_str().unwrap());
        } else {
            for i in 1..1000 {
                let image = output_dir.join(file_stem.clone() + "_" + &i.to_string() + ".jpg");
                if image.exists() {
                    images.push("/".to_owned() + image.file_name().unwrap().to_str().unwrap());
                } else {
                    break;
                }
            }
        }

        let mut alts: Vec<String> = vec![];

        if let json::JsonValue::Array(nodes) = &data["node"]["edge_sidecar_to_children"]["edges"] {
            for node in nodes {
                let caption = &node["node"]["accessibility_caption"];
                if caption.is_empty() {
                    continue;
                }
                if !caption.to_string().starts_with("Photo by Ben on") {
                    alts.push(caption.to_string());
                }
            }
        }

        let extra = PostMetaExtra {
            alts: if alts.len() == images.len() {
                Some(alts)
            } else {
                None
            },
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
        let mut file = File::create(output_dir.join(output_filename))?;
        file.write_all(("+++\n".to_owned() + &toml + "+++\n").as_bytes())?;

        if let Some(body) = body {
            file.write_all("\n".as_bytes())?;
            file.write_all(body.trim().as_bytes())?;
            file.write_all("\n".as_bytes())?;
        }
    }

    Ok(())
}

fn process_image(input_filename: &Path, output_dir: &Path) -> Result<(), std::io::Error> {
    let output_filename = output_dir.join(input_filename.file_name().unwrap());
    if !output_filename.exists() {
        hard_link(input_filename, &output_filename)?;
    }

    Ok(())
}

fn main() -> Result<(), std::io::Error> {
    let args: Vec<String> = env::args().collect();
    let input_dirname = &args[1];
    let output_dirname = &args[2];
    let output_dir = Path::new(output_dirname);

    for entry in read_dir(&output_dir).expect("could not read output dir") {
        let path = entry?.path();
        let file_name = path.file_name().unwrap().to_str().unwrap();
        if file_name.ends_with(".md") && !file_name.starts_with('_') {
            remove_file(path)?;
        }
    }

    for entry in read_dir(Path::new(&input_dirname)).expect("could not read input dir") {
        let path = entry?.path();
        let file_name = path.to_str().unwrap();
        if file_name.ends_with(".json.xz") {
            generate_toml(Path::new(&path), output_dir)?;
        } else if file_name.ends_with(".jpg") && !file_name.ends_with("profile_pic.jpg") {
            process_image(Path::new(&path), output_dir)?;
        }
    }

    Ok(())
}
