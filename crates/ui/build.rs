// Embeds every file under assets/ (icons) into the binary: a generated table of (path, bytes).
use std::io::Write;
fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    println!("cargo:rerun-if-changed={}", root.display());
    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("assets.rs");
    let mut f = std::fs::File::create(out).unwrap();
    writeln!(f, "pub static ASSETS: &[(&str, &[u8])] = &[").unwrap();
    let mut stack = vec![root.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let rel = p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
            writeln!(f, "    ({:?}, include_bytes!({:?})),", rel, p.canonicalize().unwrap().display().to_string()).unwrap();
        }
    }
    writeln!(f, "];").unwrap();
}
