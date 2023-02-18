use std::{collections::BTreeMap, env, error::Error, fs::File, io::Write};

#[derive(Debug, serde::Deserialize)]
struct Entity {
    #[allow(dead_code)]
    codepoints: Vec<u32>,
    characters: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut with = Vec::default();
    let mut without = Vec::default();
    for (name, value) in reqwest::blocking::get("https://html.spec.whatwg.org/entities.json")?
        .json::<BTreeMap<String, Entity>>()?
    {
        if name.ends_with(";") {
            with.push((name, value));
        } else {
            without.push((name, value));
        }
    }

    // longest character reference names first
    with.sort_by(|p, q| p.0.len().cmp(&q.0.len()).reverse());
    without.sort_by(|p, q| p.0.len().cmp(&q.0.len()).reverse());

    let mut entities = File::create(dbg!(format!("{}/entities.rs", env::var("OUT_DIR")?)))?;

    writeln!(
        entities,
        "pub const ENTITIES_WITH_SEMICOLON: &[(&str, &str)] = &["
    )?;
    for (name, value) in &with {
        writeln!(entities, "    ({:?}, {:?}),", name, value.characters)?;
    }
    writeln!(entities, "];")?;
    writeln!(
        entities,
        "pub const ENTITIES_WITHOUT_SEMICOLON: &[(&str, &str)] = &["
    )?;
    for (name, value) in &without {
        writeln!(entities, "    ({:?}, {:?}),", name, value.characters)?;
    }
    writeln!(entities, "];")?;

    writeln!(entities, "lazy_static::lazy_static! {{")?;
    writeln!(
        entities,
        "    pub static ref ENTITIES_WITH_SEMICOLON_REGEX: regex::RegexSet = regex::RegexSet::new(&["
    )?;
    for (name, _) in &with {
        writeln!(entities, "        {:?},", format!("^{}", name))?;
    }
    writeln!(entities, "    ]).unwrap();")?;
    writeln!(
        entities,
        "    pub static ref ENTITIES_WITHOUT_SEMICOLON_REGEX: regex::RegexSet = regex::RegexSet::new(&["
    )?;
    for (name, _) in &without {
        writeln!(entities, "        {:?},", format!("^{}", name))?;
    }
    writeln!(entities, "    ]).unwrap();")?;
    writeln!(entities, "}}")?;

    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}
