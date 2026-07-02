/// IPC 连通性测试命令。

#[tauri::command]
pub async fn ping(message: String) -> Result<String, String> {
    Ok(format!("pong: {message}"))
}
