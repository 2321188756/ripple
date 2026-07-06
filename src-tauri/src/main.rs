#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 单例锁：绑定本地端口，第二次启动时端口被占 → 退出。
    // listener 保持存活直到进程退出，自动释放端口。
    let _lock = match std::net::TcpListener::bind("127.0.0.1:14250") {
        Ok(l) => l,
        Err(_) => {
            eprintln!("Ripple is already running. Exiting.");
            return;
        }
    };
    ripple_app_lib::run();
}
