// 桌面入口。真正的逻辑在 lib.rs（方便测试与将来加别的宿主）。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // --root <目录>：把数据目录指到别处（开发/测试用，不写进配置）
    if let Some(i) = args.iter().position(|a| a == "--root") {
        if let Some(p) = args.get(i + 1) {
            std::env::set_var("ROLL_SQUAD_DATA", p);
        }
    }

    // --prefetch-avatars：打包前把头像全量抓进缓存，跑完就退。
    // 应用是 windows_subsystem（没有控制台），所以结果写进 data\prefetch.log。
    if args.iter().any(|a| a == "--prefetch-avatars") {
        let result = roll_squad_lib::run_prefetch();
        let msg = match result {
            Ok(n) => format!("OK 已缓存 {n} 张头像"),
            Err(e) => format!("FAIL {e}"),
        };
        let log = roll_squad_lib::data_dir_for_log().join("prefetch.log");
        let _ = std::fs::write(&log, &msg);
        println!("{msg}");
        return;
    }

    roll_squad_lib::run()
}
