use std::io::Write;
use std::path::Path;

/// Create a ZIP archive containing all files under `source_dir`.
pub fn create_zip(source_dir: &Path, output_path: &Path) -> Result<(), String> {
    let file = std::fs::File::create(output_path)
        .map_err(|e| format!("Cannot create ZIP '{}': {}", output_path.display(), e))?;
    let mut zip = zip::ZipWriter::new(file);

    let options = zip::write::FileOptions::<()>::default()
        .compression_method(zip::CompressionMethod::Deflated);

    add_dir(&mut zip, source_dir, source_dir, &options)?;

    zip.finish()
        .map_err(|e| format!("Cannot finalise ZIP: {e}"))?;
    Ok(())
}

fn add_dir(
    zip: &mut zip::ZipWriter<std::fs::File>,
    base: &Path,
    dir: &Path,
    options: &zip::write::FileOptions<()>,
) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("Cannot read dir: {e}"))? {
        let entry = entry.map_err(|e| format!("Dir entry error: {e}"))?;
        let path = entry.path();
        let relative = path
            .strip_prefix(base)
            .map_err(|_| "Path prefix error".to_string())?;

        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            let name = relative.to_string_lossy().replace('\\', "/") + "/";
            zip.add_directory(&name, *options)
                .map_err(|e| format!("Cannot add dir '{name}': {e}"))?;
            add_dir(zip, base, &path, options)?;
        } else {
            let name = relative.to_string_lossy().replace('\\', "/");
            zip.start_file(&name, *options)
                .map_err(|e| format!("Cannot add file '{name}': {e}"))?;
            let data = std::fs::read(&path)
                .map_err(|e| format!("Cannot read '{}': {}", path.display(), e))?;
            zip.write_all(&data)
                .map_err(|e| format!("Cannot write '{name}': {e}"))?;
        }
    }
    Ok(())
}
