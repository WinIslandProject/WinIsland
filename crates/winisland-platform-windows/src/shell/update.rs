use std::path::Path;
use std::process::Command;

use winisland_platform::PlatformError;

use super::paths;

pub(super) fn install(package: &Path) -> Result<(), PlatformError> {
    let mut installed_executable = dirs::data_local_dir().unwrap_or_else(paths::config_dir);
    installed_executable.push("WinIsland");
    installed_executable.push("WinIsland.exe");
    let escape = |path: &Path| path.to_string_lossy().replace('\'', "''");
    let script = format!(
        "while (Get-Process -Id {} -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 100 }}; \
         $installer = Start-Process -FilePath '{}' -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/CLOSEAPPLICATIONS') -WindowStyle Hidden -PassThru -Wait; \
         if ($installer.ExitCode -eq 0) {{ Start-Process -FilePath '{}' }}",
        std::process::id(),
        escape(package),
        escape(&installed_executable),
    );
    Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-Command", &script])
        .spawn()
        .map(|_| ())
        .map_err(PlatformError::backend)
}
