use std::cmp::Ordering;
use std::env;
use std::fs::{hard_link, read_dir, remove_file, File};
use std::io::prelude::*;
use std::path::Path;

use chrono::{DateTime, FixedOffset};
use image::imageops::{crop, thumbnail};
use image::io::Reader as ImageReader;
use image::ImageFormat::Jpeg;
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
        static ref SPLIT_RE: Regex =
            Regex::new(r"(.*) ((\n|http://\S+|https://\S+).*?)").expect("Invalid regex");
        static ref TIDY_RE: Regex =
            Regex::new(r"(#\S+\s*|^\.|\n\.|https?://\S+)").expect("Invalid regex");
    }

    let caption = NUMBER_RE
        .replace_all(caption, "${1}.\u{00A0}${2}")
        .to_string();

    if LINES_RE.is_match(caption.as_str()) {
        let mut split = LINES_RE.splitn(caption.as_str(), 2);
        // println!("{:?}", split);
        let title = split.next().unwrap().to_string();
        let body = split.next().unwrap().to_string();

        let body = TIDY_RE.replace_all(body.as_str(), "").trim().to_string();

        if body.is_empty() {
            return (Some(title), None);
        } else {
            return (Some(title), Some(body));
        }
    }

    if let Some(result) = SPLIT_RE.captures(caption.as_str()) {
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
    lazy_static! {
        static ref SLUG_RE: Regex = Regex::new(r"[^A-Za-z0-9 \u{00A0}-]+").expect("invalid regex");
    }

    SLUG_RE
        .replace_all(name.to_lowercase().as_str(), "")
        .trim()
        .replace(' ', "-")
        .replace('\u{00A0}', "-")
}

fn generate_toml(input_filename: &Path, output_dir: &Path) {
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
            return;
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
        let mut file = File::create(output_dir.join(output_filename))
            .expect("Failed to open file for writing");
        file.write_all(("+++\n".to_owned() + &toml + "+++\n").as_bytes())
            .expect("failed to write to file");

        if let Some(body) = body {
            file.write_all("\n".as_bytes())
                .expect("failed to write to file");
            file.write_all(body.trim().as_bytes())
                .expect("failed to write to file");
            file.write_all("\n".as_bytes())
                .expect("failed to write to file");
        }
    }
}

fn get_thumbnail_dimensions(
    width: u32,
    height: u32,
    bounding_width: u32,
    bounding_height: u32,
) -> (u32, u32) {
    if width == height {
        (bounding_width, bounding_height)
    } else {
        let ratio = bounding_width as f64 / width as f64;
        (bounding_width, ((height as f64) * ratio).round() as u32)
    }
}
fn get_square_thumbnail_dimensions(width: u32, height: u32, bounding_size: u32) -> (u32, u32) {
    match width.cmp(&height) {
        Ordering::Greater => (
            (width as f64 * (bounding_size as f64 / height as f64)) as u32,
            bounding_size,
        ),
        Ordering::Less => (
            bounding_size,
            (height as f64 * (bounding_size as f64 / width as f64)) as u32,
        ),
        Ordering::Equal => (bounding_size, bounding_size),
    }
}

fn process_image(input_filename: &Path, output_dir: &Path) {
    let output_filename = output_dir.join(input_filename.file_name().unwrap());
    if !output_filename.exists() {
        hard_link(input_filename, &output_filename).expect("failed to link to output dir");
    }

    let med_image_filename = &output_filename
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .replace(".jpg", "_med.jpg");
    let med_image_path = Path::new(&med_image_filename);

    let thumbnail_filename = &output_filename
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .replace(".jpg", "_thumb.jpg");
    let thumbnail_path = Path::new(&thumbnail_filename);

    if !med_image_path.exists() {
        let image = ImageReader::open(input_filename).unwrap().decode().unwrap();

        let new_dimensions = get_thumbnail_dimensions(image.width(), image.height(), 800, 600);
        let med_image = thumbnail(&image, new_dimensions.0, new_dimensions.1);

        med_image
            .save_with_format(output_dir.join(med_image_filename), Jpeg)
            .expect("failed to write output file");
    }

    if !thumbnail_path.exists()
        && (thumbnail_filename.ends_with("_1_thumb.jpg")
            || thumbnail_filename.ends_with("_UTC_thumb.jpg"))
    {
        let image = ImageReader::open(input_filename).unwrap().decode().unwrap();
        let new_dimensions = get_square_thumbnail_dimensions(image.width(), image.height(), 450);
        let mut thumbnail_image = thumbnail(&image, new_dimensions.0, new_dimensions.1);

        if image.width() != image.height() {
            let mid_w = (thumbnail_image.width() as f64 / 2.0) as u32;
            let mid_h = (thumbnail_image.height() as f64 / 2.0) as u32;
            let cropped = crop(&mut thumbnail_image, mid_w - 225, mid_h - 225, 450, 450);
            thumbnail_image = cropped.to_image();
        }

        thumbnail_image
            .save_with_format(output_dir.join(thumbnail_filename), Jpeg)
            .expect("failed to write output file");
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let input_dirname = &args[1];
    let output_dirname = &args[2];
    let output_dir = Path::new(output_dirname);

    for entry in read_dir(&output_dir).expect("could not read output dir") {
        let path = entry.unwrap().path();
        let file_name = path.file_name().unwrap().to_str().unwrap();
        if file_name.ends_with(".md") && !file_name.starts_with('_') {
            remove_file(path).expect("failed to remove");
        }
    }

    for entry in read_dir(Path::new(&input_dirname)).expect("could not read input dir") {
        let path = entry.unwrap().path();
        let file_name = path.to_str().unwrap();
        if file_name.ends_with(".json.xz") {
            generate_toml(Path::new(&path), output_dir);
        } else if file_name.ends_with(".jpg") && !file_name.ends_with("profile_pic.jpg") {
            process_image(Path::new(&path), output_dir);
        }
    }
}
