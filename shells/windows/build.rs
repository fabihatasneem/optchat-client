fn main() {
    println!("cargo:rerun-if-changed=packaging/optchat.rc.in");
    println!("cargo:rerun-if-changed=packaging/OptChat.ico");
    #[cfg(target_os = "windows")]
    {
        let version = std::env::var("CARGO_PKG_VERSION").expect("package version");
        let numeric = format!("{},0", version.replace('.', ","));
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let icon = root
            .join("packaging/OptChat.ico")
            .display()
            .to_string()
            .replace('\\', "/");
        let resource = std::fs::read_to_string(root.join("packaging/optchat.rc.in"))
            .expect("resource template")
            .replace("@NUMERIC_VERSION@", &numeric)
            .replace("@VERSION@", &version)
            .replace("@ICON@", &icon);
        let path =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("optchat.rc");
        std::fs::write(&path, resource).expect("write Windows resources");
        embed_resource::compile(&path, embed_resource::NONE)
            .manifest_optional()
            .expect("embed OptChat Windows resources");
    }
}
