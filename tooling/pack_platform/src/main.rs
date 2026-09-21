use std::env;
use std::fs::{self, File};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    let root_dir = find_project_root()?;
    let platform_dir = get_arg(&args, "--platform-dir").unwrap_or_else(|| root_dir.join("platform"));
    let build_dir = get_arg(&args, "--build-dir").unwrap_or_else(|| root_dir.join("build"));
    let out_dir = get_arg(&args, "--out-dir").unwrap_or_else(|| root_dir.join("dist"));
    
    let tag = get_arg_str(&args, "--tag")
        .or_else(|| env::var("TAG_NAME").ok())
        .unwrap_or_else(|| String::from("v0.1.0"));
        
    let repo = get_arg_str(&args, "--repo")
        .or_else(|| env::var("GITHUB_REPOSITORY").ok())
        .unwrap_or_else(|| String::from("thesparq/roc-golem"));

    fs::create_dir_all(&out_dir)?;

    println!("============================================================");
    println!(" [Roc Golem Platform Packager]");
    println!(" Repository: {}", repo);
    println!(" Release Tag: {}", tag);
    println!(" Output Dir:  {}", out_dir.display());
    println!("============================================================");

    // Build deterministic in-memory tarball
    let tar_bytes = create_reproducible_tar(&root_dir, &platform_dir, &build_dir)?;
    println!("==> Created reproducible tar archive ({} bytes)", tar_bytes.len());

    // 1. Zstandard (.tar.zst)
    let zst_bytes = compress_zstd(&tar_bytes)?;
    let zst_hash = compute_blake3_base64url(&zst_bytes);
    let zst_filename = format!("{}.tar.zst", zst_hash);
    let zst_path = out_dir.join(&zst_filename);
    fs::write(&zst_path, &zst_bytes)?;
    let named_zst = out_dir.join(format!("roc-golem-{}.tar.zst", tag));
    fs::copy(&zst_path, &named_zst)?;
    println!("==> Packaged Zstandard: {} ({} bytes)", zst_filename, zst_bytes.len());

    // 2. Brotli (.tar.br)
    let br_bytes = compress_brotli(&tar_bytes)?;
    let br_hash = compute_blake3_base64url(&br_bytes);
    let br_filename = format!("{}.tar.br", br_hash);
    let br_path = out_dir.join(&br_filename);
    fs::write(&br_path, &br_bytes)?;
    let named_br = out_dir.join(format!("roc-golem-{}.tar.br", tag));
    fs::copy(&br_path, &named_br)?;
    println!("==> Packaged Brotli:    {} ({} bytes)", br_filename, br_bytes.len());

    // 3. Gzip (.tar.gz)
    let gz_bytes = compress_gzip(&tar_bytes)?;
    let gz_hash = compute_blake3_base64url(&gz_bytes);
    let gz_filename = format!("{}.tar.gz", gz_hash);
    let gz_path = out_dir.join(&gz_filename);
    fs::write(&gz_path, &gz_bytes)?;
    let named_gz = out_dir.join(format!("roc-golem-{}.tar.gz", tag));
    fs::copy(&gz_path, &named_gz)?;
    println!("==> Packaged Gzip:      {} ({} bytes)", gz_filename, gz_bytes.len());

    // Write Checksums file
    let mut checksums = String::new();
    for (name, bytes) in [
        (&zst_filename, &zst_bytes),
        (&br_filename, &br_bytes),
        (&gz_filename, &gz_bytes),
    ] {
        let mut sha = Sha256::new();
        sha.update(bytes);
        let sha_hex = format!("{:x}", sha.finalize());
        checksums.push_str(&format!("{}  {}\n", sha_hex, name));
    }
    let checksums_path = out_dir.join("checksums.txt");
    fs::write(&checksums_path, checksums)?;

    // Generate Release URL & Markdown Snippet
    let zst_url = format!("https://github.com/{}/releases/download/{}/{}", repo, tag, zst_filename);
    let br_url = format!("https://github.com/{}/releases/download/{}/{}", repo, tag, br_filename);
    let gz_url = format!("https://github.com/{}/releases/download/{}/{}", repo, tag, gz_filename);

    let snippet_md = format!(
r#"## 📦 Roc Platform Release: `{tag}`

To use this release of `roc-golem` in your Roc application, copy and paste this platform URL into your `main.roc` app header:

### Recommended (`.tar.zst`):
```roc
app [agent] {{
    pf: platform "{zst_url}",
}}
```

### Alternative (`.tar.br` Brotli):
```roc
app [agent] {{
    pf: platform "{br_url}",
}}
```

### Alternative (`.tar.gz` Gzip):
```roc
app [agent] {{
    pf: platform "{gz_url}",
}}
```

### Included Platform Artifacts
- `platform/main.roc`, `platform/Golem.roc`, `platform/Types.roc`, `platform/Effect.roc`, `platform/Host.roc`
- Precompiled `targets/wasm32/libhost.a` (static library for Wasm32 Golem Cloud host)
- Integrity Hash: BLAKE3 (base64url unpadded)
"#
    );

    let release_notes_path = out_dir.join("release_notes.md");
    fs::write(&release_notes_path, &snippet_md)?;

    println!("\n============================================================");
    println!(" Platform Release Packages Generated Successfully!");
    println!("============================================================");
    println!("{}", snippet_md);
    println!("Release assets and notes written to: {}", out_dir.display());

    Ok(())
}

fn compute_blake3_base64url(data: &[u8]) -> String {
    let hash = blake3::hash(data);
    URL_SAFE_NO_PAD.encode(hash.as_bytes())
}

fn add_dir_recursive(dir: &Path, base: &Path, files: &mut Vec<(PathBuf, String)>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let rel = path.strip_prefix(base).unwrap().to_string_lossy().into_owned();
            files.push((path, rel));
        } else if path.is_dir() {
            add_dir_recursive(&path, base, files)?;
        }
    }
    Ok(())
}

fn create_reproducible_tar(
    root_dir: &Path,
    platform_dir: &Path,
    build_dir: &Path,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut tar_builder = tar::Builder::new(Vec::new());
    let mut files_to_add: Vec<(PathBuf, String)> = Vec::new();

    // 1. Platform directory files (including targets/wasm32/libhost.a)
    if platform_dir.exists() {
        add_dir_recursive(platform_dir, platform_dir, &mut files_to_add)?;
    }

    // 2. Precompiled host files in build_dir as fallback
    let libhost_a = build_dir.join("libhost.a");
    if libhost_a.exists() && !files_to_add.iter().any(|(_, name)| name == "libhost.a") {
        files_to_add.push((libhost_a, String::from("libhost.a")));
    }

    // 3. License & Readme if available
    let license = root_dir.join("LICENSE");
    if license.exists() {
        files_to_add.push((license, String::from("LICENSE")));
    }
    let readme = root_dir.join("README.md");
    if readme.exists() {
        files_to_add.push((readme, String::from("README.md")));
    }

    // Sort files alphabetically for deterministic archive
    files_to_add.sort_by(|a, b| a.1.cmp(&b.1));

    for (file_path, archive_name) in files_to_add {
        let mut file = File::open(&file_path)?;
        let metadata = file.metadata()?;
        let mut header = tar::Header::new_gnu();
        header.set_size(metadata.len());
        header.set_mode(0o644);
        header.set_mtime(0); // Normalized timestamp for reproducible builds
        header.set_uid(0);
        header.set_gid(0);
        header.set_cksum();
        tar_builder.append_data(&mut header, archive_name, &mut file)?;
    }

    tar_builder.finish()?;
    Ok(tar_builder.into_inner()?)
}

fn compress_zstd(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut encoder = zstd::stream::Encoder::new(Vec::new(), 19)?;
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

fn compress_brotli(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let mut cursor = Cursor::new(data);
    let mut writer = brotli::CompressorWriter::new(&mut output, 4096, 11, 22);
    std::io::copy(&mut cursor, &mut writer)?;
    drop(writer);
    Ok(output)
}

fn compress_gzip(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

fn find_project_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let current_dir = env::current_dir()?;
    if current_dir.join("platform").exists() {
        return Ok(current_dir);
    }
    if let Some(parent) = current_dir.parent() {
        if parent.join("platform").exists() {
            return Ok(parent.to_path_buf());
        }
    }
    Ok(current_dir)
}

fn get_arg(args: &[String], flag: &str) -> Option<PathBuf> {
    get_arg_str(args, flag).map(PathBuf::from)
}

fn get_arg_str(args: &[String], flag: &str) -> Option<String> {
    for i in 0..args.len() {
        if args[i] == flag && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
    }
    None
}
