use resource::*;
use resource::spritesheet::SpritesheetBuilder;
use resource::font::FontBuilder;
use image::simple_image::SimpleImageBuilder;
use image::composed_image::ComposedImageBuilder;
use image::animated_image::AnimatedImageBuilder;
use ui::theme::{ThemeBuilder, create_theme};

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Error};
use std::ffi::OsStr;
use std::path::PathBuf;

#[derive(Debug)]
pub struct ResourceBuilderSet {
    pub theme_builder: ThemeBuilder,
    pub simple_builders: HashMap<String, SimpleImageBuilder>,
    pub composed_builders: HashMap<String, ComposedImageBuilder>,
    pub animated_builders: HashMap<String, AnimatedImageBuilder>,
    pub spritesheet_builders: HashMap<String, SpritesheetBuilder>,
    pub spritesheets_dir: String,
    pub font_builders: HashMap<String, FontBuilder>,
    pub fonts_dir: String,
}

impl ResourceBuilderSet {
    pub fn new(root: &str) -> Result<ResourceBuilderSet, Error> {
        // let game_filename = root.to_owned() + "/game";
        // debug!("Reading top level config from {}", game_filename);
        //
        // let game = match ResourceBuilderSet::create_game(&game_filename) {
        //     Ok(g) => g,
        //     Err(e) => {
        //         error!("Unable to load game startup state from {}", game_filename);
        //         return Err(e);
        //     }
        // };

        let theme_filename = root.to_owned() + "/theme/theme";
        debug!("Reading theme from {}", theme_filename);
        let mut theme_builder = match create_theme(
            &format!("{}/theme/", root.to_owned()), "theme") {
            Ok(t) => t,
            Err(e) => {
                error!("Unable to load theme from {}", theme_filename);
                return Err(e);
            }
        };

        match theme_builder.expand_references() {
            Ok(()) => (),
            Err(e) => {
                error!("Unable to load theme from {}", theme_filename);
                return Err(e);
            }
        };

        Ok(ResourceBuilderSet {
            theme_builder,
            simple_builders: read(root, "images"),
            composed_builders: read(root, "composed_images"),
            animated_builders: read(root, "animated_images"),
            spritesheet_builders: read(root, "spritesheets"),
            spritesheets_dir: format!("{}/spritesheets/", root),
            font_builders: read(root, "fonts"),
            fonts_dir: format!("{}/fonts/", root),
        })
    }
}

pub fn read_single_resource<T: ResourceBuilder>(filename: &str) -> Result<T, Error> {
    let mut builder_type = BuilderType::JSON;
    let mut file = File::open(format!("{}.json", filename));
    if file.is_err() {
        file = File::open(format!("{}.yml", filename));
        builder_type = BuilderType::YAML;
    }

    if file.is_err() {
        return invalid_data_error(
            &format!("Unable to locate '{}.json' or '{}.yml'", filename, filename));
    }

    let mut file_data = String::new();
    file.unwrap().read_to_string(&mut file_data)?;

    match builder_type {
        BuilderType::JSON => T::from_json(&file_data),
        BuilderType::YAML => T::from_yaml(&file_data),
    }
}

pub fn read<T: ResourceBuilder>(root: &str, dir: &str) -> HashMap<String, T> {
    let mut resources: HashMap<String, T> = HashMap::new();

    read_recursive([root, dir].iter().collect(), &mut resources);

    resources
}

fn read_recursive<T: ResourceBuilder>(dir: PathBuf, resources: &mut HashMap<String, T>) {
    let dir_str = dir.to_string_lossy().to_string();
    debug!("Reading resources from {}", dir_str);

    let dir_entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => {
            warn!("Unable to read directory: {}", dir_str);
            return;
        }
    };

    for entry in dir_entries {
        trace!("Found entry {:?}", entry);
        let entry = match entry {
            Ok(e) => e,
            Err(error) => {
                warn!("Error reading file: {}", error);
                continue;
            }
        };

        let path = entry.path();

        if path.is_dir() {
            read_recursive(path, resources);
        } else {
            let extension: String = OsStr::to_str(path.extension().
                unwrap_or(OsStr::new(""))).unwrap_or("").to_string();

            if !path.is_file() {
                continue;
            }

            let builder_type = match extension.as_ref() {
                "json" => BuilderType::JSON,
                "yml" => BuilderType::YAML,
                _ => continue,
            };

            read_file(path, resources, builder_type);
        }
    }
}

fn read_file<T: ResourceBuilder>(path: PathBuf, resources: &mut HashMap<String, T>,
                                 builder_type: BuilderType) {
    let path_str = path.to_string_lossy().to_string();
    debug!("Reading file at {}", path_str);
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) => {
            warn!("Error reading file: {}", error);
            return;
        }
    };

    let mut file_data = String::new();
    if file.read_to_string(&mut file_data).is_err() {
        warn!("Error reading file data from file {}", path_str);
        return;
    }
    trace!("Read file data.");

    let resource = match builder_type {
        BuilderType::JSON => T::from_json(&file_data),
        BuilderType::YAML => T::from_yaml(&file_data),
    };

    let resource = match resource {
        Ok(a) => a,
        Err(error) => {
            warn!("Error parsing file data: {:?}", path_str);
            warn!("  {}", error);
            return;
        }
    };

    let id = resource.owned_id();

    trace!("Created resource '{}'", id);
    if resources.contains_key(&id) {
        warn!("Duplicate resource key: {} in {}", id, path_str);
        return;
    }

    trace!("Inserted resource.");
    resources.insert(id, resource);
}

