// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use home::home_dir;
use move_cli::sandbox::commands::test;
use std::fs::{self, File};
use std::io::Write;
use move_cli::sandbox::commands::test;
use move_cli::{base::movey_login::MOVEY_CREDENTIAL_PATH, sandbox::commands::test};
use move_command_line_common::movey_constants::MOVEY_URL;
#[cfg(unix)]
use std::fs::File;
use std::{env, fs, io::Write};

use std::path::PathBuf;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{path::PathBuf, process::Stdio};
use toml_edit::easy::Value;
use std::process::Command;

pub const CLI_METATEST_PATH: [&str; 3] = ["tests", "metatests", "args.txt"];

fn get_cli_binary_path() -> PathBuf {
    let cli_exe = env!("CARGO_BIN_EXE_move");
    PathBuf::from(cli_exe)
}

fn get_metatest_path() -> PathBuf {
    CLI_METATEST_PATH.iter().collect()
}

#[test]
fn run_metatest() {
    let path_cli_binary = get_cli_binary_path();
    let path_metatest = get_metatest_path();

    // local workspace + with coverage
    assert!(test::run_all(&path_metatest, path_cli_binary.as_path(), false, true).is_ok());

    // temp workspace + with coverage
    assert!(test::run_all(&path_metatest, &path_cli_binary, true, true).is_ok());

    // local workspace + without coverage
    assert!(test::run_all(&path_metatest, &path_cli_binary, false, false).is_ok());

    // temp workspace + without coverage
    assert!(test::run_all(&path_metatest, &path_cli_binary, true, false).is_ok());
}

#[test]
fn cross_process_locking_git_deps() {
    let cli_exe = env!("CARGO_BIN_EXE_move");
    let handle = std::thread::spawn(move || {
        Command::new(cli_exe)
            .current_dir("./tests/cross_process_tests/Package1")
            .args(["package", "build"])
            .output()
            .expect("Package1 failed");
    });
    let cli_exe = env!("CARGO_BIN_EXE_move").to_string();
    Command::new(cli_exe)
        .current_dir("./tests/cross_process_tests/Package2")
        .args(["package", "build"])
        .output()
        .expect("Package2 failed");
    handle.join().unwrap();
}

const UPLOAD_PACKAGE_PATH: &str = "./tests/upload_tests";

#[test]
fn upload_package_to_movey_works() {
    let test_path = String::from("/upload_package_to_movey_works");
    let (home, credential_file) = setup_move_home(&test_path);
    let _ = fs::remove_file(&credential_file);

    fs::create_dir_all(&home).unwrap();
    let mut file = File::create(&credential_file).unwrap();
    let credential_content =
        String::from("[registry]\ntoken=\"eb8xZkyr78FNL528j7q39zcdS6mxjBXt\"\n");
    file.write(&credential_content.as_bytes()).unwrap();
    let package_path = format!("{}/valid_package", UPLOAD_PACKAGE_PATH);
    init_git(&package_path, 0);
    let cli_exe = env!("CARGO_BIN_EXE_move");
    let output = Command::new(cli_exe)
        .current_dir(&package_path)
        .args(["package", "upload", "--test", "--test-path", &test_path])
        .output()
        .unwrap();
    assert!(output.status.success());
    let res_path = format!("{}/request-body.txt", &package_path);
    let data = fs::read_to_string(&res_path).unwrap();
    assert!(data.contains("rev"));
    assert!(data.contains("\"github_repo_url\":\"https://github.com/diem/move\""));
    fs::remove_file(&res_path).unwrap();

    clean_up(&home, &package_path);
}

#[test]
fn upload_package_to_movey_with_no_remote_should_panic() {
    let package_path = format!("{}/no_git_remote_package", UPLOAD_PACKAGE_PATH);
    init_git(&package_path, 1);
    let cli_exe = env!("CARGO_BIN_EXE_move");
    let output = Command::new(cli_exe)
        .current_dir(&package_path)
        .args(["package", "upload", "--test"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
    assert!(error.contains("invalid git repository"));
    clean_up("", &package_path);
}

#[test]
fn upload_package_to_movey_with_no_head_commit_id_should_panic() {
    let package_path = format!("{}/no_commit_id_package", UPLOAD_PACKAGE_PATH);
    init_git(&package_path, 2);
    let cli_exe = env!("CARGO_BIN_EXE_move");
    let output = Command::new(cli_exe)
        .current_dir(&package_path)
        .args(["package", "upload", "--test"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
    assert!(error.contains("invalid HEAD commit id"), "{}", error);
    clean_up("", &package_path);
}

#[test]
fn upload_package_to_movey_with_no_credential_should_panic() {
    let test_path = String::from("/upload_package_to_movey_with_no_credential_should_panic");
    let (home, credential_file) = setup_move_home(&test_path);
    let _ = fs::remove_file(&credential_file);

    let package_path = format!("{}/no_credential_package", UPLOAD_PACKAGE_PATH);
    init_git(&package_path, 0);
    let cli_exe = env!("CARGO_BIN_EXE_move");
    let output = Command::new(cli_exe)
        .current_dir(&package_path)
        .args(["package", "upload", "--test", "--test-path", &test_path])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
    assert!(error.contains(
        "There seems to be an error with your Movey credential. \
        Please run `move login` and follow the instructions."
    ));

    clean_up(&home, &package_path);
}

#[test]
fn upload_package_to_movey_with_bad_credential_should_panic() {
    let test_path = String::from("/upload_package_to_movey_with_bad_credential_should_panic");
    let (home, credential_file) = setup_move_home(&test_path);
    let _ = fs::remove_file(&credential_file);

    fs::create_dir_all(&home).unwrap();
    let mut file = File::create(&credential_file).unwrap();
    let bad_content = String::from("[registry]\ntoken=\n");
    file.write(&bad_content.as_bytes()).unwrap();

    let package_path = format!("{}/bad_credential_package", UPLOAD_PACKAGE_PATH);
    init_git(&package_path, 0);
    let cli_exe = env!("CARGO_BIN_EXE_move");
    let output = Command::new(cli_exe)
        .current_dir(&package_path)
        .args(["package", "upload", "--test", "--test-path", &test_path])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
    assert!(error.contains(
        "There seems to be an error with your Movey credential. \
        Please run `move login` and follow the instructions."
    ));

    clean_up(&home, &package_path);
}

// flag == 0: all git command are run
// flag == 1: missing git add remote command
// flag == 2: missing git commit command
fn init_git(package_path: &str, flag: i32) {
    Command::new("git")
        .current_dir(package_path)
        .args(&["init"])
        .output()
        .unwrap();
    if flag != 1 {
        Command::new("git")
            .current_dir(package_path)
            .args(&["remote", "add", "origin", "git@github.com:diem/move.git"])
            .output()
            .unwrap();
    }
    Command::new("touch")
        .current_dir(package_path)
        .args(&["simple_file"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(package_path)
        .args(&["add", "simple_file"])
        .output()
        .unwrap();
    if flag != 2 {
        Command::new("git")
            .current_dir(package_path)
            .args(&["commit", "-m", "initial commit"])
            .output()
            .unwrap();
    }
#[test]
fn save_credential_works() {
    let cli_exe = env!("CARGO_BIN_EXE_move");
    let (move_home, credential_path) = setup_move_home("/save_credential_works");
    assert!(fs::read_to_string(&credential_path).is_err());

    match std::process::Command::new(cli_exe)
        .env("MOVE_HOME", &move_home)
        .current_dir(".")
        .args(["movey-login"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(child) => {
            let token = "test_token";
            child
                .stdin
                .as_ref()
                .unwrap()
                .write_all(token.as_bytes())
                .unwrap();
            match child.wait_with_output() {
                Ok(output) => {
                    assert!(String::from_utf8_lossy(&output.stdout).contains(&format!(
                        "Please paste the API Token found on {}/settings/tokens below",
                        MOVEY_URL
                    )));
                    Ok(())
                }
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
    .unwrap();

    let contents = fs::read_to_string(&credential_path).expect("Unable to read file");
    let mut toml: Value = contents.parse().unwrap();
    let registry = toml.as_table_mut().unwrap().get_mut("registry").unwrap();
    let token = registry.as_table_mut().unwrap().get_mut("token").unwrap();
    assert!(token.to_string().contains("test_token"));

    clean_up(&move_home)
}

#[cfg(unix)]
#[test]
fn save_credential_fails_if_undeletable_credential_file_exists() {
    let cli_exe = env!("CARGO_BIN_EXE_move");
    let (move_home, credential_path) =
        setup_move_home("/save_credential_fails_if_undeletable_credential_file_exists");
    let file = File::create(&credential_path).unwrap();
    let mut perms = file.metadata().unwrap().permissions();
    perms.set_mode(0o000);
    file.set_permissions(perms).unwrap();

    match std::process::Command::new(cli_exe)
        .env("MOVE_HOME", &move_home)
        .current_dir(".")
        .args(["movey-login"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => {
            let token = "test_token";
            child
                .stdin
                .as_ref()
                .unwrap()
                .write_all(token.as_bytes())
                .unwrap();
            match child.wait_with_output() {
                Ok(output) => {
                    assert!(String::from_utf8_lossy(&output.stdout).contains(&format!(
                        "Please paste the API Token found on {}/settings/tokens below",
                        MOVEY_URL
                    )));
                    assert!(String::from_utf8_lossy(&output.stderr)
                        .contains("Error: Error reading input: Permission denied (os error 13)"));
                    Ok(())
                }
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
    .unwrap();

    let mut perms = file.metadata().unwrap().permissions();
    perms.set_mode(0o600);
    file.set_permissions(perms).unwrap();
    let _ = fs::remove_file(&credential_path);

    clean_up(&move_home)
}

fn setup_move_home(test_path: &str) -> (String, String) {
    let cwd = env::current_dir().unwrap();
    let mut move_home: String = String::from(cwd.to_string_lossy());
    move_home.push_str(&test_path);
    let _ = fs::remove_dir_all(&move_home);
    fs::create_dir_all(&move_home).unwrap();
    let credential_path = move_home.clone() + MOVEY_CREDENTIAL_PATH;
    (move_home, credential_path)
}

fn clean_up(move_home: &str) {
    let _ = fs::remove_dir_all(move_home);
}

fn clean_up(move_home: &str, package_path: &str) {
    if !move_home.is_empty() {
        let _ = fs::remove_dir_all(move_home);
    }
    fs::remove_file(format!("{}/simple_file", package_path)).unwrap();
    fs::remove_dir_all(format!("{}/.git", package_path)).unwrap();
}

fn setup_move_home(test_path: &str) -> (String, String) {
    let mut move_home = home_dir().unwrap().to_string_lossy().to_string() + "/.move/test";
    if !test_path.is_empty() {
        move_home.push_str(&test_path);
    }
    let credential_file = move_home.clone() + "/credential.toml";
    (move_home, credential_file)
}
