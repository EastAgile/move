// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, Result};
use std::{fs, fs::File, io, path::PathBuf};
use toml_edit::easy::{map::Map, Value};

pub struct TestMode {
    pub test_path: String,
}

pub fn handle_login_commands(test_path: Option<String>) -> Result<()> {
    let url: &str;
    if cfg!(debug_assertions) {
        url = "https://movey-app-staging.herokuapp.com";
    } else {
        url = "https://movey.net";
    }
    println!(
        "Please paste the API Token found on {}/settings/tokens below",
        url
    );
    let mut line = String::new();
    loop {
        match io::stdin().read_line(&mut line) {
            Ok(_) => {
                if let Some('\n') = line.chars().next_back() {
                    line.pop();
                }
                if let Some('\r') = line.chars().next_back() {
                    line.pop();
                }
                if !line.is_empty() {
                    break;
                }
                println!("Invalid API Token. Try again!");
            }
            Err(err) => {
                bail!("Error reading file: {}", err);
            }
        }
    }
    let mut test_mode: Option<TestMode> = None;
    if let Some(path) = test_path {
        test_mode = Some(TestMode { test_path: path });
    }
    save_credential(line, test_mode)?;
    println!("Token for Movey saved.");
    Ok(())
}

pub fn save_credential(token: String, test_mode: Option<TestMode>) -> Result<()> {
    let mut move_home;
    if let Some(test_mode) = test_mode {
        move_home = std::env::var("TEST_MOVE_HOME").unwrap();
        if !test_mode.test_path.is_empty() {
            move_home.push_str(&test_mode.test_path);
        }
    } else {
        move_home = std::env::var("MOVE_HOME").unwrap_or_else(|_| {
            format!(
                "{}/.move",
                std::env::var("HOME").expect("env var 'HOME' must be set")
            )
        });
    }
    fs::create_dir_all(&move_home)?;
    let credential_path = move_home + "/credential.toml";
    let credential_file = PathBuf::from(&credential_path);
    if !credential_file.exists() {
        File::create(&credential_path)?;
    }

    let old_contents: String;
    match fs::read_to_string(&credential_path) {
        Ok(contents) => {
            old_contents = contents;
        }
        Err(error) => bail!("Error reading input: {}", error),
    }
    let mut toml: Value = old_contents
        .parse()
        .map_err(|e| anyhow::Error::from(e).context("could not parse input as TOML"))?;

    if let Some(registry) = toml.as_table_mut().unwrap().get_mut("registry") {
        if let Some(toml_token) = registry.as_table_mut().unwrap().get_mut("token") {
            *toml_token = Value::String(token);
        } else {
            registry
                .as_table_mut()
                .unwrap()
                .insert(String::from("token"), Value::String(token));
        }
    } else {
        let mut value = Map::new();
        value.insert(String::from("token"), Value::String(token));
        toml.as_table_mut()
            .unwrap()
            .insert(String::from("registry"), Value::Table(value));
    }

    let new_contents = toml.to_string();
    fs::write(credential_file, new_contents).expect("Unable to write file");
    let file = File::open(&credential_path)?;
    set_permissions(&file, 0o600)?;
    Ok(())
}

#[cfg(unix)]
fn set_permissions(file: &File, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut perms = file.metadata()?.permissions();
    perms.set_mode(mode);
    file.set_permissions(perms)?;
    Ok(())
}

#[cfg(not(unix))]
#[allow(unused)]
fn set_permissions(file: &File, mode: u32) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn setup_move_home(test_path: &str) -> (String, String) {
        let cwd = env::current_dir().unwrap();
        let mut move_home: String = String::from(cwd.to_string_lossy());
        env::set_var("TEST_MOVE_HOME", &move_home);
        if !test_path.is_empty() {
            move_home.push_str(&test_path);
        } else {
            move_home.push_str("/test");
        }
        let credential_path = move_home.clone() + "/credential.toml";
        (move_home, credential_path)
    }

    fn clean_up(move_home: &str) {
        let _ = fs::remove_dir_all(move_home);
    }

    #[test]
    fn save_credential_works_if_no_credential_file_exists() {
        let (move_home, credential_path) =
            setup_move_home("/save_credential_works_if_no_credential_file_exists");
        let _ = fs::remove_dir_all(&move_home);

        let test_mode = Some(TestMode {
            test_path: String::from("/save_credential_works_if_no_credential_file_exists"),
        });
        save_credential(String::from("test_token"), test_mode).unwrap();

        let contents = fs::read_to_string(&credential_path).expect("Unable to read file");
        let mut toml: Value = contents.parse().unwrap();
        let registry = toml.as_table_mut().unwrap().get_mut("registry").unwrap();
        let token = registry.as_table_mut().unwrap().get_mut("token").unwrap();
        assert!(token.to_string().contains("test_token"));

        clean_up(&move_home);
    }

    #[test]
    fn save_credential_works_if_empty_credential_file_exists() {
        let (move_home, credential_path) =
            setup_move_home("/save_credential_works_if_empty_credential_file_exists");

        let _ = fs::remove_dir_all(&move_home);
        fs::create_dir_all(&move_home).unwrap();
        File::create(&credential_path).unwrap();

        let contents = fs::read_to_string(&credential_path).expect("Unable to read file");
        let mut toml: Value = contents.parse().unwrap();
        assert!(toml.as_table_mut().unwrap().get_mut("registry").is_none());

        let test_mode = Some(TestMode {
            test_path: String::from("/save_credential_works_if_empty_credential_file_exists"),
        });
        save_credential(String::from("test_token"), test_mode).unwrap();

        let contents = fs::read_to_string(&credential_path).expect("Unable to read file");
        let mut toml: Value = contents.parse().unwrap();
        let registry = toml.as_table_mut().unwrap().get_mut("registry").unwrap();
        let token = registry.as_table_mut().unwrap().get_mut("token").unwrap();
        assert!(token.to_string().contains("test_token"));

        clean_up(&move_home);
    }

    #[test]
    fn save_credential_works_if_token_field_exists() {
        let (move_home, credential_path) =
            setup_move_home("/save_credential_works_if_token_field_exists");

        let _ = fs::remove_dir_all(&move_home);
        fs::create_dir_all(&move_home).unwrap();
        File::create(&credential_path).unwrap();

        let old_content =
            String::from("[registry]\ntoken = \"old_test_token\"\nversion = \"0.0.0\"\n");
        fs::write(&credential_path, old_content).expect("Unable to write file");

        let contents = fs::read_to_string(&credential_path).expect("Unable to read file");
        let mut toml: Value = contents.parse().unwrap();
        let registry = toml.as_table_mut().unwrap().get_mut("registry").unwrap();
        let token = registry.as_table_mut().unwrap().get_mut("token").unwrap();
        assert!(token.to_string().contains("old_test_token"));
        assert!(!token.to_string().contains("new_world"));

        let test_mode = Some(TestMode {
            test_path: String::from("/save_credential_works_if_token_field_exists"),
        });
        save_credential(String::from("new_world"), test_mode).unwrap();

        let contents = fs::read_to_string(&credential_path).expect("Unable to read file");
        let mut toml: Value = contents.parse().unwrap();
        let registry = toml.as_table_mut().unwrap().get_mut("registry").unwrap();
        let token = registry.as_table_mut().unwrap().get_mut("token").unwrap();
        assert!(token.to_string().contains("new_world"));
        assert!(!token.to_string().contains("old_test_token"));
        let version = registry.as_table_mut().unwrap().get_mut("version").unwrap();
        assert!(version.to_string().contains("0.0.0"));

        clean_up(&move_home);
    }

    #[test]
    fn save_credential_works_if_empty_token_field_exists() {
        let (move_home, credential_path) =
            setup_move_home("/save_credential_works_if_empty_token_field_exists");

        let _ = fs::remove_dir_all(&move_home);
        fs::create_dir_all(&move_home).unwrap();
        File::create(&credential_path).unwrap();

        let old_content = String::from("[registry]\ntoken = \"\"\nversion = \"0.0.0\"\n");
        fs::write(&credential_path, old_content).expect("Unable to write file");

        let contents = fs::read_to_string(&credential_path).expect("Unable to read file");
        let mut toml: Value = contents.parse().unwrap();
        let registry = toml.as_table_mut().unwrap().get_mut("registry").unwrap();
        let token = registry.as_table_mut().unwrap().get_mut("token").unwrap();
        assert!(!token.to_string().contains("test_token"));

        let test_mode = Some(TestMode {
            test_path: String::from("/save_credential_works_if_empty_token_field_exists"),
        });
        save_credential(String::from("test_token"), test_mode).unwrap();

        let contents = fs::read_to_string(&credential_path).expect("Unable to read file");
        let mut toml: Value = contents.parse().unwrap();
        let registry = toml.as_table_mut().unwrap().get_mut("registry").unwrap();
        let token = registry.as_table_mut().unwrap().get_mut("token").unwrap();
        assert!(token.to_string().contains("test_token"));
        let version = registry.as_table_mut().unwrap().get_mut("version").unwrap();
        assert!(version.to_string().contains("0.0.0"));

        clean_up(&move_home);
    }
}
