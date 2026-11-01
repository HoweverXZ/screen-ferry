//! Windows：把图标、版本信息和程序清单编进 screenferry.exe（`packaging/windows/screenferry.rc`）。
//! 其他系统上什么也不做。

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("缺少 CARGO_MANIFEST_DIR"))
        .join("../..");
    let packaging = root.join("packaging/windows");
    for file in [
        packaging.join("screenferry.rc"),
        packaging.join("app.manifest"),
        root.join("assets/icons/app-icon-windows.ico"),
    ] {
        println!("cargo:rerun-if-changed={}", file.display());
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    // 版本号写成头文件放进 OUT_DIR（它在资源编译器的搜索路径里），由 screenferry.rc 引用。
    let version = env::var("CARGO_PKG_VERSION").expect("缺少 CARGO_PKG_VERSION");
    let mut parts = version
        .split(['.', '-', '+'])
        .map(|part| part.parse::<u16>().unwrap_or(0));
    let mut next = || parts.next().unwrap_or(0);
    let (major, minor, patch) = (next(), next(), next());
    let out = PathBuf::from(env::var("OUT_DIR").expect("缺少 OUT_DIR"));
    fs::write(
        out.join("version.h"),
        format!(
            "#define VERSION_MAJOR {major}\n#define VERSION_MINOR {minor}\n\
             #define VERSION_PATCH {patch}\n#define VERSION_TEXT \"{version}\"\n"
        ),
    )
    .expect("写不下 version.h");

    // screenferry.rc 里的文件路径相对于它所在的目录，所以把这个目录加进搜索路径。
    embed_resource::compile(
        packaging.join("screenferry.rc"),
        embed_resource::ParamsIncludeDirs([&packaging]),
    )
    .manifest_required()
    .expect("编译 Windows 资源失败");
}
