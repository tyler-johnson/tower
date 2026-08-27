//! Extract a single binary member from a release archive.

use std::fs::File;
use std::io;

use super::failed;
use crate::error::CliError;

/// Extract the member named `member` from `archive` to `dest`.
///
/// The archive format is determined by the file name suffix (`.tar.gz` or `.zip`).
pub fn extract_member(
    archive: &std::path::Path,
    member: &str,
    dest: &std::path::Path,
) -> Result<(), CliError> {
    let stem = archive.file_name().and_then(|n| n.to_str()).unwrap_or("");

    if stem.ends_with(".tar.gz") {
        extract_tar_gz(archive, member, dest)
    } else if stem.ends_with(".zip") {
        #[cfg(windows)]
        {
            extract_zip(archive, member, dest)
        }
        #[cfg(not(windows))]
        {
            Err(failed("unsupported archive format"))
        }
    } else {
        Err(failed("unsupported archive format"))
    }
}

fn extract_tar_gz(
    archive: &std::path::Path,
    member: &str,
    dest: &std::path::Path,
) -> Result<(), CliError> {
    let file = File::open(archive).map_err(|err| failed(err.to_string()))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    let mut found = false;
    for entry in archive.entries().map_err(|err| failed(err.to_string()))? {
        let mut entry = entry.map_err(|err| failed(err.to_string()))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let entry_name = entry
            .path()
            .map_err(|err| failed(err.to_string()))?
            .file_name()
            .map(|n| n.to_string_lossy().to_string());
        if entry_name.as_deref() == Some(member) {
            #[cfg(unix)]
            let mut out = {
                use std::os::unix::fs::OpenOptionsExt;
                std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .mode(0o755)
                    .open(dest)
            }
            .map_err(|err| failed(err.to_string()))?;

            #[cfg(not(unix))]
            let mut out = std::fs::File::create(dest).map_err(|err| failed(err.to_string()))?;

            io::copy(&mut entry, &mut out).map_err(|err| failed(err.to_string()))?;
            found = true;
            break;
        }
    }

    if !found {
        Err(failed(format!("archive has no {member} member")))
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn extract_zip(
    archive: &std::path::Path,
    member: &str,
    dest: &std::path::Path,
) -> Result<(), CliError> {
    let file = File::open(archive).map_err(|err| failed(err.to_string()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|err| failed(err.to_string()))?;

    let mut found = false;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|err| failed(err.to_string()))?;
        if !file.is_file() {
            continue;
        }
        let entry_name = file
            .enclosed_name()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
        if entry_name.as_deref() == Some(member) {
            let mut out = std::fs::File::create(dest).map_err(|err| failed(err.to_string()))?;
            io::copy(&mut file, &mut out).map_err(|err| failed(err.to_string()))?;
            found = true;
            break;
        }
    }

    if !found {
        Err(failed(format!("archive has no {member} member")))
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;

    #[test]
    fn extract_tar_gz_member() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("x.tar.gz");
        let dest_path = dir.path().join("ff-tower");

        // Build a tar.gz in memory
        let mut tar_data = Vec::new();
        {
            let gz_encoder =
                flate2::write::GzEncoder::new(&mut tar_data, flate2::Compression::default());
            let mut builder = tar::Builder::new(gz_encoder);

            // ff-tower member
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Regular);
            header.set_mode(0o644);
            header.set_size(19);
            builder
                .append_data(&mut header, "ff-tower", &b"#!/bin/sh\necho new\n"[..])
                .unwrap();

            // LICENSE member
            let mut header2 = tar::Header::new_gnu();
            header2.set_entry_type(tar::EntryType::Regular);
            header2.set_mode(0o644);
            header2.set_size(7);
            builder
                .append_data(&mut header2, "LICENSE", &b"MIT\n\n"[..])
                .unwrap();

            builder.finish().unwrap();
        }

        std::fs::write(&archive_path, &tar_data).unwrap();

        // Extract
        extract_member(&archive_path, "ff-tower", &dest_path).unwrap();

        // Verify contents
        let contents = std::fs::read(&dest_path).unwrap();
        assert_eq!(contents, b"#!/bin/sh\necho new\n");

        // Verify mode is 0755
        let metadata = std::fs::metadata(&dest_path).unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(metadata.permissions().mode() & 0o777, 0o755);
    }

    #[test]
    fn extract_missing_member() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("x.tar.gz");

        // Build a tar.gz with only a LICENSE member
        let mut tar_data = Vec::new();
        {
            let gz_encoder =
                flate2::write::GzEncoder::new(&mut tar_data, flate2::Compression::default());
            let mut builder = tar::Builder::new(gz_encoder);

            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Regular);
            header.set_mode(0o644);
            header.set_size(7);
            builder
                .append_data(&mut header, "LICENSE", &b"MIT\n\n"[..])
                .unwrap();

            builder.finish().unwrap();
        }

        std::fs::write(&archive_path, &tar_data).unwrap();

        let dest_path = dir.path().join("nope");
        let err = extract_member(&archive_path, "nope", &dest_path).unwrap_err();
        assert!(err.to_string().contains("no nope member"));
    }
}
