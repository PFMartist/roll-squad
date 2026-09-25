// 给 Gradle 的 rust 任务用的转交文件。
//
// gen/android 里每次打包都会执行：`node tauri android android-studio-script`
// （见 gen/android/buildSrc/.../BuildTask.kt），工作目录就是这个 src-tauri。
// node 对「裸名字」不查 node_modules，只按路径找 ./tauri、./tauri.js……，
// 所以放这个文件让它找到，再转交给真正的 CLI（@tauri-apps/cli 装在前端根 app/ 下）。
//
// 放在 src-tauri 而不是 gen/ 里，是为了重跑 `tauri android init` 也不会被覆盖。
require('@tauri-apps/cli/tauri.js');
