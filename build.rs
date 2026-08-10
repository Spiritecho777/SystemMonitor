use std::path::Path;
use embed_resource;

fn main() {
    embed_resource::compile(
        Path::new("ressources/app.rc"),
        Path::new("ressources/app.res"),
    );
}
