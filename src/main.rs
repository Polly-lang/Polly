#[macro_use]
extern crate clap;
extern crate poly;
extern crate serde_json;

use clap::App;
use std::fs::{File, metadata};
use std::io::{Read, Write};

fn main() {
    let yaml = load_yaml!("../cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    let paths = matches.values_of("input").unwrap();

    for path in paths {
        let path_metadata = metadata(path)
                                .ok()
                                .expect("Couldn't find file, please make sure your path is \
                                         correct.");
        if path_metadata.is_file() {
            let mut file = File::open(path).ok().expect("This file couldn't be opened");
            let mut contents = String::new();
            file.read_to_string(&mut contents).ok().expect("Couldn't write to buffer");
            
            let html = poly::template::Template::load(path).render();

            if let Some(path) = matches.value_of("file") {
                let mut file = File::create(path)
                                   .ok()
                                   .expect("Couldn't create file at destination");
                file.write_all(&html.into_bytes()).ok().expect("Couldn't write to file");
            } else {
                println!("{}", html);
            }
        }
    }
}
