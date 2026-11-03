fn main() {
    let root = std::path::Path::new("third_party/zopfli");
    let mut build = cc::Build::new();
    build
        .include(root.join("src/zopfli"))
        .include(root.join("src/zopflipng"))
        .warnings(false)
        .files(
            [
                "src/zopfli/blocksplitter.c",
                "src/zopfli/cache.c",
                "src/zopfli/deflate.c",
                "src/zopfli/gzip_container.c",
                "src/zopfli/hash.c",
                "src/zopfli/katajainen.c",
                "src/zopfli/lz77.c",
                "src/zopfli/squeeze.c",
                "src/zopfli/tree.c",
                "src/zopfli/util.c",
                "src/zopfli/zlib_container.c",
                "src/zopfli/zopfli_lib.c",
                "src/zopflipng/lodepng/lodepng.cpp",
                "src/zopflipng/lodepng/lodepng_util.cpp",
                "src/zopflipng/zopflipng_lib.cc",
            ]
            .map(|path| root.join(path)),
        );
    build.compile("zopflipng");
    // cc emits the C++ standard library only when this flag is set. The Zopfli
    // C files stay C; this unit is what pulls in libc++ or libstdc++.
    cc::Build::new()
        .cpp(true)
        .file("src/cxx_link.cpp")
        .compile("crunch_cxx");
    println!("cargo:rerun-if-changed=third_party/zopfli/src");
    println!("cargo:rerun-if-changed=src/cxx_link.cpp");
}
