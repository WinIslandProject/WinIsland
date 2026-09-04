fn main() {
    println!("cargo:rerun-if-env-changed=WINISLAND_BUILD_NUMBER");
    let build_number = std::env::var("WINISLAND_BUILD_NUMBER")
        .ok()
        .filter(|value| value.parse::<u64>().is_ok())
        .unwrap_or_else(|| "0".to_string());
    println!("cargo:rustc-env=WINISLAND_BUILD_NUMBER={build_number}");

    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        println!("cargo:rerun-if-env-changed=WINISLAND_PACKAGE_CHANNEL");

        let mut res = winres::WindowsResource::new();

        let icon_path = "resources/icon-dark.ico";
        if std::path::Path::new(icon_path).exists() {
            res.set_icon(icon_path);
        } else {
            println!(
                "cargo:warning=Icon file not found: {icon_path}, executable will use default icon"
            );
        }

        res.set("CompanyName", "Eatgrapes");
        res.set("FileDescription", "WinIsland");
        res.set("ProductName", "WinIsland");
        res.set("LegalCopyright", "Copyright (c) Eatgrapes");

        let package = package_identity();
        let manifest = format!(
            r#"
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="0.0.0.0" name="{}"/>
  <msix xmlns="urn:schemas-microsoft-com:msix.v1"
    publisher="CN=Eatgrapes.WinIsland"
    packageName="{}"
    applicationId="WinIsland"/>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
"#,
            package.name, package.name
        );
        res.set_manifest(&manifest);
        res.compile().unwrap();
    }
}

struct PackageIdentity {
    name: &'static str,
}

fn package_identity() -> PackageIdentity {
    match std::env::var("WINISLAND_PACKAGE_CHANNEL").as_deref() {
        Ok("nightly" | "stable") => PackageIdentity {
            name: "Eatgrapes.WinIsland",
        },
        _ => PackageIdentity {
            name: "Eatgrapes.WinIsland.Dev",
        },
    }
}
