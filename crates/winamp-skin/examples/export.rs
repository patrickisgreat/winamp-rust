//! Export the built-in skin to a directory: `cargo run -p winamp-skin --example export -- <dir>`
fn main() {
    let dir = std::env::args().nth(1).expect("output dir");
    let dir = std::path::PathBuf::from(dir);
    winamp_skin::export_builtin(&dir).expect("export");
    // Also write PNG previews.
    let skin = winamp_skin::Skin::builtin();
    for which in winamp_skin::SkinImage::ALL {
        let p = dir.join(format!("{}.png", which.file_stem()));
        skin.image(which).save(&p).expect("png");
    }
    println!("exported to {}", dir.display());
}
