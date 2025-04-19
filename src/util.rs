
use std::{
    fs::OpenOptions,
    io::{Seek, SeekFrom, Write, copy},
    thread,
    time::Duration,
    process::Command,
};
use reqwest::blocking::Client;
use reqwest::header::{RANGE, USER_AGENT};
use anyhow::{Result, anyhow};

use serde::Deserialize;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use fs_extra::file::{copy_with_progress, CopyOptions, TransitProcess};
use crate::{UNPACK_DIR, UPDATE_DIR};

/// 下载文件，支持断点续传与失败重试
pub fn download_with_resume(url: &str, output_path: &str, max_retries: u8) -> Result<()> {
    let client = Client::new();

    let mut retries = 0;
    let mut downloaded = 0u64;

    // 如果已有部分文件，获取已下载大小
    if std::path::Path::new(output_path).exists() {
        downloaded = std::fs::metadata(output_path)?.len();
    }

    loop {
        println!("📥 正在下载: {}（已下载 {} 字节）", url, downloaded);

        let resp = client
            .get(url)
            .header(USER_AGENT, "genshin-updater")
            .header(RANGE, format!("bytes={}-", downloaded))
            .send();

        match resp {
            Ok(mut res) => {
                if !res.status().is_success() && res.status().as_u16() != 206 {
                    return Err(anyhow!("❌ 下载失败: HTTP {}", res.status()));
                }

                // 追加模式打开文件
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(output_path)?;

                // while let Some(chunk) = res.chunk().ok().flatten() {
                //     file.write_all(&chunk)?;
                //     downloaded += chunk.len() as u64;
                // }
                copy(&mut res, &mut file)?;

                println!("✅ 下载完成");
                return Ok(());
            }
            Err(e) => {
                retries += 1;
                eprintln!("⚠️ 下载失败（第 {} 次尝试）：{}", retries, e);
                if retries >= max_retries {
                    return Err(anyhow!("❌ 超过最大重试次数，下载失败"));
                }
                thread::sleep(Duration::from_secs(3));
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct FileEntry {
    #[serde(rename = "remoteName")]
    remote_name: String,
}

/// 解析非标准 JSON（逐行 JSON），并复制文件
pub fn parse_line_json(
    json_lines_path: &Path,
) -> Result<Vec<String>> {
    let mut files = Vec::new();

    let file = File::open(json_lines_path)?;
    let reader = BufReader::new(file);

    for (idx, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        if line.trim().is_empty() {
            continue;
        }

        let entry: FileEntry = match serde_json::from_str(&line) {
            Ok(val) => val,
            Err(e) => {
                eprintln!("⚠️ 第 {} 行解析失败: {}", idx + 1, e);
                continue;
            }
        };

        files.push(entry.remote_name);

        // let rel_path = Path::new(&entry.remoteName);
        // let src_path = Path::new(genshin_root).join(rel_path);
        // let dst_path = Path::new(update_root).join(rel_path);
        //
        // if !src_path.exists() {
        //     eprintln!("❌ 文件不存在: {}", src_path.display());
        //     continue;
        // }
        //
        // if let Some(parent) = dst_path.parent() {
        //     fs::create_dir_all(parent)?;
        // }
        //
        // let mut options = CopyOptions::new();
        // options.overwrite = true;
        // copy_with_progress(&src_path, &dst_path, &options, |_progress: TransitProcess| ())?;
        //
        // println!("✅ 已复制: {}", rel_path.display());
    }

    Ok(files)
}

// 新增函数：处理单个更新包
pub fn process_update_package(url: String, siz: u64, game_dir: &Path) -> Result<()> {
    fs::create_dir_all(UPDATE_DIR)?;
    fs::create_dir_all(UNPACK_DIR)?;

    let file_name = format!("{}/{}", UPDATE_DIR, url.split('/').last().unwrap());

    println!("📥 下载链接: {}", url);
    if !Path::new(&file_name).exists() || fs::metadata(&file_name)?.len() < siz {
        println!("⬇️ 正在下载...");
        download_with_resume(&url, &file_name, 5)?;
    }

    println!("📦 正在解压...");
    let zipfile = File::open(&file_name)?;
    let mut archive = zip::ZipArchive::new(zipfile)?;
    archive.extract(UNPACK_DIR)?;
    println!("✅ 解压完成");

    let update_dir = Path::new(UNPACK_DIR);

    // 获取游戏安装目录路径
    let genshin_root = Path::new(game_dir);

    // 1. 处理hdifffiles.txt
    let hdiff_files_path = update_dir.join("hdifffiles.txt");
    if hdiff_files_path.exists() {
        let files_to_patch = parse_line_json(&hdiff_files_path)?;

        for remote_name in files_to_patch {
            let hdiff_path = update_dir.join(format!("{}.hdiff", remote_name));
            let target_path = genshin_root.join(&remote_name);

            // 确保原文件存在
            if !target_path.exists() {
                return Err(anyhow!("❌ 原文件不存在: {}", target_path.display()));
            }

            println!("🔧 正在补丁: {}", remote_name);

            // 使用hdiffz应用补丁
            let status = Command::new("hdiffz")
                .arg(&target_path)
                .arg(&hdiff_path)
                .arg(&target_path) // 直接覆盖原文件
                .status()?;

            if !status.success() {
                return Err(anyhow!("❌ 补丁失败: {}", remote_name));
            }

            fs::remove_file(&hdiff_path)?;
        }
    }

    fs::remove_file(&hdiff_files_path)?;

    // 2. 处理deletefiles.txt
    let delete_files_path = update_dir.join("deletefiles.txt");
    if delete_files_path.exists() {
        let data = fs::read_to_string(delete_files_path)?;

        for line in data.lines() {
            let path = line.trim();
            if path.is_empty() {
                continue;
            }

            let delete_path = genshin_root.join(path);
            if delete_path.exists() {
                println!("🗑️ 正在删除: {}", path);
                fs::remove_file(&delete_path)
                    .or_else(|_| fs::remove_dir_all(&delete_path))?;
            }
        }
    }

    fs::remove_file(&update_dir.join("deletefiles.txt"))?;

    println!("📁 正在复制更新文件...");
    let skip_files = [
        "hdifffiles.txt",
        "deletefiles.txt",
        ".hdiff"  // 跳过补丁文件
    ];

    // 使用 walkdir 遍历目录
    for entry in walkdir::WalkDir::new(&update_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let source_path = entry.path();

        // 跳过目录和需要过滤的文件
        if source_path.is_dir()
            || skip_files.iter().any(|ext|
                source_path.to_string_lossy().ends_with(ext))
        {
            continue;
        }

        // 获取相对路径（相对于update_dir）
        let relative_path = source_path.strip_prefix(&update_dir)?;
        let dest_path = game_dir.join(relative_path);

        // 创建目标目录
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // 执行文件复制
        println!("📄 复制: {} ➔ {}",
            relative_path.display(),
            dest_path.display()
        );
        fs::copy(source_path, &dest_path)?;
    }

    println!("🧹 清理临时文件...");
    fs::remove_dir_all(UNPACK_DIR)?;
    fs::remove_dir_all(UPDATE_DIR)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use mockito::mock;

    #[test]
    fn test_parse_line_json() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.json");

        let content = r#"
        {"remoteName": "file1.txt"}
        {"remoteName": "file2.jpg"}
        {"invalid": "data"}
        "#;

        std::fs::write(&test_file, content).unwrap();

        let result = parse_line_json(&test_file).unwrap();
        assert_eq!(result, vec!["file1.txt", "file2.jpg"]);
    }

    #[test]
    fn test_download_resume() {
        use std::fs::File;
        use std::io::Write;
        use tempfile::TempDir;
        use mockito::{mock, Matcher};

        // 模拟服务器支持 Range 请求并返回剩余内容
        let _m1 = mock("GET", "/test.txt")
            .match_header("Range", Matcher::Exact("bytes=5-".to_string()))
            .with_status(206)
            .with_header("Content-Length", "6")
            .with_header("Content-Range", "bytes 5-10/11")
            .with_body(" world")
            .create();

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        // 模拟已经下载了一部分文件
        let mut file = File::create(&test_file).unwrap();
        file.write_all(b"hello").unwrap(); // 写入前 5 字节
        file.flush().unwrap();

        // 执行断点续传下载
        download_with_resume(
            &format!("{}/test.txt", mockito::server_url()),
            test_file.to_str().unwrap(),
            3
        ).unwrap();

        // 最终内容应该是完整的 "hello world"
        assert_eq!(std::fs::read_to_string(&test_file).unwrap(), "hello world");
    }
}