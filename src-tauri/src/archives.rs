use crate::operations::unique;
use std::{
    fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
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
pub fn compress(paths: &[String], destination: &str) -> Result<String, String> {
    if paths.is_empty() {
        return Err("请先选择文件".into());
    }
    let dest = Path::new(destination);
    if fs::symlink_metadata(dest).is_ok() {
        return Err("压缩包已存在".into());
    }
    let temp = tempfile::NamedTempFile::new_in(dest.parent().ok_or("无效目标")?)
        .map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(temp.reopen().map_err(|e| e.to_string())?);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    fn append(
        zip: &mut zip::ZipWriter<fs::File>,
        src: &Path,
        name: PathBuf,
        options: zip::write::SimpleFileOptions,
    ) -> Result<(), String> {
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
                append(zip, &child.path(), name.join(child.file_name()), options)?;
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
    for p in paths {
        let src = Path::new(p);
        let real = fs::canonicalize(src).map_err(|e| e.to_string())?;
        let parent = fs::canonicalize(dest.parent().unwrap()).map_err(|e| e.to_string())?;
        if real.is_dir() && parent.starts_with(&real) {
            return Err("压缩包不能创建在选中的文件夹内部".into());
        }
        append(
            &mut zip,
            src,
            PathBuf::from(src.file_name().ok_or("不能压缩根目录")?),
            options,
        )?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    temp.persist_noclobber(dest).map_err(|e| e.to_string())?;
    Ok(destination.into())
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
        )
        .unwrap();
        let out = extract(zip.to_str().unwrap()).unwrap();
        assert_eq!(
            fs::read_to_string(Path::new(&out).join("hello.txt")).unwrap(),
            "hello"
        );
    }
    #[test]
    fn traversal_rejected() {
        for name in ["../escape", "/absolute", "C:\\evil", "a/../../b"] {
            assert!(safe_name(Path::new(name)).is_err());
        }
    }
}
