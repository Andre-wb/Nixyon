// Команд для JS у плагина пока нет: его вызывает Rust-код приложения.
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
