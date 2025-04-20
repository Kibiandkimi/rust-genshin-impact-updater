mod util;
mod parser;

use std::{fs, path::Path, process::Command, io::Cursor};
use anyhow::{Result, anyhow};
use serde::Deserialize;
use std::fs::File;
use std::io::{self, Write};
// use fs_extra::dir::DirEntryAttr::Path;
use crate::util::*;
use crate::parser::*;

const API_URL: &str = "https://sg-hyp-api.hoyoverse.com/hyp/hyp-connect/api/getGamePackages?game_ids[]=gopR6Cufr3&launcher_id=VYTpXlbWo8";
const UPDATE_DIR: &str = "updates";
const UNPACK_DIR: &str = "unpacked";
const UNPACKER_PATH: &str = "./Genshin-Impact-New-Music-Unpacker";

fn main() -> Result<()> {
    println!("🚀 启动原神更新器...");

    let mut game_root = String::new();

    println!("Enter Game Dir:");
    io::stdin()
        .read_line(&mut game_root)
        .expect("input error.");

    let game_root = game_root.trim();

    // 1. 创建更新目录


    // 2. 获取最新安装包链接
    let response = reqwest::blocking::get(API_URL)?.json::<Response>()?;

    println!("Latest Game id: {}", &response.data.game_packages[0].game.id);
    println!("Latest Game version: {}", &response.data.game_packages[0].main.major.version);

    println!("Choose which do you want to upgrade from: ");
    println!("  1) {}", &response.data.game_packages[0].main.patches[0].version);
    println!("  2) {}", &response.data.game_packages[0].main.patches[1].version);

    let mut choice = String::new();
    io::stdin()
        .read_line(&mut choice)
        .expect("Input error.");

    while let Err(_) = choice.trim().parse::<usize>() {
        println!("Not a num.")
    }

    let choice:usize = choice.trim().parse()?;
    if choice != 1 && choice != 2 {
        return Err(anyhow!("Not a choice."))
    }

    let package = &response.data.game_packages[0].main.patches[choice - 1];

    let game_pkg = &package.game_pkgs[0];

    println!("Chosen version: {}", package.version);

    let languages: Vec<String> = package.audio_pkgs
        .iter()
        .map(|audio_pkg| audio_pkg.language.clone())
        .collect();

    println!("Choose which language you want to upgrade({}): ", languages.join(" "));

    let mut choice = String::new();
    io::stdin()
        .read_line(&mut choice)
        .expect("input error.");

    let choice: Vec<&str> = choice.trim().split_whitespace().collect();

    let mut audio_pkgs: Vec<&AudioPkg> = Vec::new();

    for audio_pkg in package.audio_pkgs.iter() {
        if choice.contains(&audio_pkg.language.as_str()) {
            audio_pkgs.push(audio_pkg)
        }
    }

    println!("Chosen language: {}",
             audio_pkgs
                 .iter()
                 .map(|pkg| pkg.language.clone())
                 .collect::<Vec<_>>()
                 .join(" "));

    ensure_writable(Path::new(&game_root))?;

    process_update_package(game_pkg.url.clone(), game_pkg.size, Path::new(&game_root))?;

    for audio_pkg in audio_pkgs.iter() {
        process_update_package(audio_pkg.url.clone(), audio_pkg.size, Path::new(&game_root))?;
    }

    println!("✅ 完成更新！");

    Ok(())
}
