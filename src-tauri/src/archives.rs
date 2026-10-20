use crate::ignores::IgnoreSet;
use crate::operations::unique;
use std::{
    fs,
    io::{self, Read, Write},
    path::{Component, Path},
};
const MAX_TOTAL: u64 = 20 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;
fn safe_name(name: &Path) -> Result<(), String> {
    if name.as_os_str().is_empty()
        || name
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
        || name.to_string_lossy().contains(['\\', ':'])
    {
        return Err("压缩包包含不安全路径".into());
    }
    Ok(())
}
fn copy_limited(
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    remaining: &mut u64,
) -> Result<(), String> {
    let n = io::copy(&mut reader.take(*remaining + 1), writer).map_err(|e| e.to_string())?;
    if n > *remaining {
        return Err("压缩包展开后超过 20 GiB 限制".into());
    }
    *remaining -= n;
    Ok(())
}
pub fn extract(source: &str) -> Result<String, String> {
    let src = Path::new(source);
    let name = src.file_name().ok_or("无效文件名")?.to_string_lossy();
    let parent = src.parent().ok_or("没有父文件夹")?;
    let temp = tempfile::Builder::new()
        .prefix(".fiex-extract-")
        .tempdir_in(parent)
        .map_err(|e| e.to_string())?;
    let root = temp.path();
    let lower = name.to_lowercase();
    let mut remaining = MAX_TOTAL;
    if lower.ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(fs::File::open(src).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if zip.len() > MAX_ENTRIES {
            return Err("压缩包项目过多".into());
        }
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            let relative = f.enclosed_name().ok_or("压缩包包含越界路径")?;
            safe_name(&relative)?;
            if f.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
                return Err("压缩包包含符号链接".into());
            }
            let path = root.join(relative);
            if f.is_dir() {
                fs::create_dir_all(path).map_err(|e| e.to_string())?;
            } else {
                fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
                let mut out = fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(path)
                    .map_err(|e| e.to_string())?;
                copy_limited(&mut f, &mut out, &mut remaining)?;
            }
        }
    } else if lower.ends_with(".7z") {
        let mut count = 0;
        sevenz_rust2::decompress_file_with_extract_fn(src, root, |entry, reader, _path| {
            count += 1;
            let fail = |s: String| sevenz_rust2::Error::from(io::Error::other(s));
            if count > MAX_ENTRIES {
                return Err(fail("压缩包项目过多".into()));
            }
            let relative = Path::new(&entry.name);
            safe_name(relative).map_err(fail)?;
            let path = root.join(relative);
            if entry.is_directory {
                fs::create_dir_all(path).map_err(sevenz_rust2::Error::from)?;
            } else {
                fs::create_dir_all(path.parent().unwrap()).map_err(sevenz_rust2::Error::from)?;
                let mut out = fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(path)
                    .map_err(sevenz_rust2::Error::from)?;
                copy_limited(reader, &mut out, &mut remaining).map_err(fail)?;
            }
            Ok(true)
        })
        .map_err(|e| e.to_string())?;
    } else {
        let file = fs::File::open(src).map_err(|e| e.to_string())?;
        let reader: Box<dyn Read> = if lower.ends_with(".gz") || lower.ends_with(".tgz") {
            Box::new(flate2::read::GzDecoder::new(file))
        } else if lower.ends_with(".bz2") || lower.ends_with(".tbz2") {
            Box::new(bzip2::read::BzDecoder::new(file))
        } else if lower.ends_with(".xz") || lower.ends_with(".txz") {
            Box::new(xz2::read::XzDecoder::new(file))
        } else if lower.ends_with(".tar") {
            Box::new(file)
        } else {
            return Err("内置解压支持 ZIP、7z、TAR、GZ、BZ2、XZ（包括压缩 TAR）".into());
        };
        let is_tar = lower.ends_with(".tar")
            || [".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tar.xz", ".txz"]
                .iter()
                .any(|s| lower.ends_with(s));
        if is_tar {
            let mut tar = tar::Archive::new(reader);
            let mut count = 0;
            for f in tar.entries().map_err(|e| e.to_string())? {
                count += 1;
                if count > MAX_ENTRIES {
                    return Err("压缩包项目过多".into());
                }
                let mut f = f.map_err(|e| e.to_string())?;
                let relative = f.path().map_err(|e| e.to_string())?.into_owned();
                safe_name(&relative)?;
                let path = root.join(relative);
                let ty = f.header().entry_type();
                if ty.is_dir() {
                    fs::create_dir_all(path).map_err(|e| e.to_string())?;
                } else if ty.is_file() {
                    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
                    let mut out = fs::OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .open(path)
                        .map_err(|e| e.to_string())?;
                    copy_limited(&mut f, &mut out, &mut remaining)?;
                } else {
                    return Err("不解压符号链接、硬链接或特殊文件".into());
                }
            }
        } else {
            let mut reader = reader;
            let stem = src.file_stem().ok_or("无效文件名")?;
            let mut out = fs::File::create(root.join(stem)).map_err(|e| e.to_string())?;
            copy_limited(&mut reader, &mut out, &mut remaining)?;
        }
    }
    let dest = unique(&parent.join(src.file_stem().ok_or("无效文件名")?));
    fs::rename(root, &dest).map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().into_owned())
}
pub fn compress(
    paths: &[String],
    destination: &str,
    format: &str,
    ignores: &[String],
) -> Result<String, String> {
    if paths.is_empty() {
        return Err("请先选择文件".into());
    }
    let dest = Path::new(destination);
    if fs::symlink_metadata(dest).is_ok() {
        return Err("压缩包已存在".into());
    }
    let parent = dest.parent().ok_or("无效目标")?;
    for item in paths {
        let src = Path::new(item);
        let real = fs::canonicalize(src).map_err(|e| e.to_string())?;
        let real_parent = fs::canonicalize(parent).map_err(|e| e.to_string())?;
        if real.is_dir() && real_parent.starts_with(&real) {
            return Err("压缩包不能创建在选中的文件夹内部".into());
        }
    }
    let ignore = IgnoreSet::new(ignores);
    let temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    match format {
        "zip" => {
            let file = temp.reopen().map_err(|e| e.to_string())?;
            write_zip(file, paths, &ignore)?;
        }
        "7z" => {
            let file = temp.reopen().map_err(|e| e.to_string())?;
            write_7z(file, paths, &ignore)?;
        }
        "tar" => {
            let file = temp.reopen().map_err(|e| e.to_string())?;
            write_tar(file, paths, &ignore)?;
        }
        "tar.gz" => {
            let file = temp.reopen().map_err(|e| e.to_string())?;
            let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            write_tar(encoder, paths, &ignore)?
                .finish()
                .map_err(|e| e.to_string())?;
        }
        "tar.bz2" => {
            let file = temp.reopen().map_err(|e| e.to_string())?;
            let encoder = bzip2::write::BzEncoder::new(file, bzip2::Compression::default());
            write_tar(encoder, paths, &ignore)?
                .finish()
                .map_err(|e| e.to_string())?;
        }
        "tar.xz" => {
            let file = temp.reopen().map_err(|e| e.to_string())?;
            let encoder = xz2::write::XzEncoder::new(file, 6);
            write_tar(encoder, paths, &ignore)?
                .finish()
                .map_err(|e| e.to_string())?;
        }
        _ => return Err("不支持的压缩格式".into()),
    }
    temp.persist_noclobber(dest).map_err(|e| e.to_string())?;
    Ok(destination.into())
}

fn write_zip(file: fs::File, paths: &[String], ignore: &IgnoreSet) -> Result<(), String> {
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    fn append(
        zip: &mut zip::ZipWriter<fs::File>,
        src: &Path,
        name: &Path,
        options: zip::write::SimpleFileOptions,
        ignore: &IgnoreSet,
    ) -> Result<(), String> {
        if ignore.is_ignored(name) {
            return Ok(());
        }
        let meta = fs::symlink_metadata(src).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            return Err("压缩时不跟随符号链接".into());
        }
        let key = name.to_string_lossy().replace('\\', "/");
        if meta.is_dir() {
            zip.add_directory(format!("{key}/"), options)
                .map_err(|e| e.to_string())?;
            for child in fs::read_dir(src).map_err(|e| e.to_string())? {
                let child = child.map_err(|e| e.to_string())?;
                append(
                    zip,
                    &child.path(),
                    &name.join(child.file_name()),
                    options,
                    ignore,
                )?;
            }
        } else if meta.is_file() {
            zip.start_file(key, options).map_err(|e| e.to_string())?;
            io::copy(&mut fs::File::open(src).map_err(|e| e.to_string())?, zip)
                .map_err(|e| e.to_string())?;
        } else {
            return Err("不能压缩特殊文件".into());
        }
        Ok(())
    }
    for item in paths {
        let src = Path::new(item);
        let name = src.file_name().ok_or("不能压缩根目录")?;
        append(&mut zip, src, Path::new(name), options, ignore)?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn write_tar<W: Write>(writer: W, paths: &[String], ignore: &IgnoreSet) -> Result<W, String> {
    fn append<W: Write>(
        builder: &mut tar::Builder<W>,
        src: &Path,
        name: &Path,
        ignore: &IgnoreSet,
    ) -> Result<(), String> {
        if ignore.is_ignored(name) {
            return Ok(());
        }
        let meta = fs::symlink_metadata(src).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            return Err("压缩时不跟随符号链接".into());
        }
        if meta.is_dir() {
            builder.append_dir(name, src).map_err(|e| e.to_string())?;
            for child in fs::read_dir(src).map_err(|e| e.to_string())? {
                let child = child.map_err(|e| e.to_string())?;
                append(
                    builder,
                    &child.path(),
                    &name.join(child.file_name()),
                    ignore,
                )?;
            }
        } else if meta.is_file() {
            builder
                .append_path_with_name(src, name)
                .map_err(|e| e.to_string())?;
        } else {
            return Err("不能压缩特殊文件".into());
        }
        Ok(())
    }
    let mut builder = tar::Builder::new(writer);
    for item in paths {
        let src = Path::new(item);
        let name = src.file_name().ok_or("不能压缩根目录")?;
        append(&mut builder, src, Path::new(name), ignore)?;
    }
    builder.into_inner().map_err(|e| e.to_string())
}

fn write_7z(file: fs::File, paths: &[String], ignore: &IgnoreSet) -> Result<(), String> {
    use sevenz_rust2::{ArchiveEntry, ArchiveWriter};
    fn append<W: Write + std::io::Seek>(
        writer: &mut ArchiveWriter<W>,
        src: &Path,
        name: &Path,
        ignore: &IgnoreSet,
    ) -> Result<(), String> {
        if ignore.is_ignored(name) {
            return Ok(());
        }
        let meta = fs::symlink_metadata(src).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            return Err("压缩时不跟随符号链接".into());
        }
        let archive_name = name.to_string_lossy().replace('\\', "/");
        if meta.is_dir() {
            writer
                .push_archive_entry::<fs::File>(ArchiveEntry::from_path(src, archive_name), None)
                .map_err(|e| e.to_string())?;
            for child in fs::read_dir(src).map_err(|e| e.to_string())? {
                let child = child.map_err(|e| e.to_string())?;
                append(writer, &child.path(), &name.join(child.file_name()), ignore)?;
            }
        } else if meta.is_file() {
            writer
                .push_archive_entry(
                    ArchiveEntry::from_path(src, archive_name),
                    Some(fs::File::open(src).map_err(|e| e.to_string())?),
                )
                .map_err(|e| e.to_string())?;
        } else {
            return Err("不能压缩特殊文件".into());
        }
        Ok(())
    }
    let mut writer = ArchiveWriter::new(file).map_err(|e| e.to_string())?;
    for item in paths {
        let src = Path::new(item);
        let name = src.file_name().ok_or("不能压缩根目录")?;
        append(&mut writer, src, Path::new(name), ignore)?;
    }
    writer.finish().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_roundtrip() {
        let t = tempfile::tempdir().unwrap();
        fs::write(t.path().join("hello.txt"), "hello").unwrap();
        let zip = t.path().join("test.zip");
        compress(
            &[t.path().join("hello.txt").to_string_lossy().into()],
            zip.to_str().unwrap(),
            "zip",
            &[],
        )
        .unwrap();
        let out = extract(zip.to_str().unwrap()).unwrap();
        assert_eq!(
            fs::read_to_string(Path::new(&out).join("hello.txt")).unwrap(),
            "hello"
        );
    }
    #[test]
    fn archive_formats_and_ignores() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("bundle");
        fs::create_dir_all(src.join("node_modules")).unwrap();
        fs::write(src.join("keep.txt"), "keep").unwrap();
        fs::write(src.join("node_modules").join("drop.js"), "drop").unwrap();
        for (format, ext) in [
            ("zip", "zip"),
            ("7z", "7z"),
            ("tar", "tar"),
            ("tar.gz", "tar.gz"),
            ("tar.bz2", "tar.bz2"),
            ("tar.xz", "tar.xz"),
        ] {
            let archive = t.path().join(format!("out.{ext}"));
            compress(
                &[src.to_string_lossy().into()],
                archive.to_str().unwrap(),
                format,
                &["node_modules".into()],
            )
            .unwrap();
            let out = extract(archive.to_str().unwrap()).unwrap();
            assert!(Path::new(&out).join("bundle").join("keep.txt").exists());
            assert!(!Path::new(&out).join("bundle").join("node_modules").exists());
        }
    }
    #[test]
    fn traversal_rejected() {
        for name in ["../escape", "/absolute", "C:\\evil", "a/../../b"] {
            assert!(safe_name(Path::new(name)).is_err());
        }
    }
    #[test]
    fn malicious_zip_is_rejected_without_writing_outside() {
        let t = tempfile::tempdir().unwrap();
        let archive = t.path().join("malicious.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        zip.start_file("../escaped.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"do not extract").unwrap();
        zip.finish().unwrap();
        assert!(extract(archive.to_str().unwrap()).is_err());
        assert!(!t.path().join("escaped.txt").exists());
        assert_eq!(fs::read_dir(t.path()).unwrap().count(), 1);
    }
    #[test]
    fn sevenz_roundtrip() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("hello.txt");
        fs::write(&src, "hello 7z").unwrap();
        let archive = t.path().join("test.7z");
        sevenz_rust2::compress_to_path(&src, &archive).unwrap();
        let out = extract(archive.to_str().unwrap()).unwrap();
        assert_eq!(
            fs::read_to_string(Path::new(&out).join("hello.txt")).unwrap(),
            "hello 7z"
        );
    }
}
