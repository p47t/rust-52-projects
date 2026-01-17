use adb_client::AdbClient;

#[tokio::test]
#[ignore] // Requires: adb start-server
async fn test_real_server_version() {
    let client = AdbClient::new();
    let version = client.server_version().await.unwrap();
    assert!(
        version > 0,
        "ADB version should be positive, got {}",
        version
    );
    println!("ADB server version: {}", version);
}

#[tokio::test]
#[ignore] // Requires: adb start-server
async fn test_real_list_devices() {
    let client = AdbClient::new();
    let devices = client.list_devices().await.unwrap();
    println!("Found {} device(s):", devices.len());
    for d in &devices {
        println!("  {} - {}", d.serial, d.state);
    }
}

#[tokio::test]
#[ignore] // Requires: adb start-server + connected device
async fn test_real_shell_echo() {
    let client = AdbClient::new();
    let output = client.shell(None, "echo hello").await.unwrap();
    assert!(
        output.contains("hello"),
        "Expected 'hello' in output, got: {:?}",
        output
    );
}

#[tokio::test]
#[ignore] // Requires: adb start-server + connected device
async fn test_real_stat() {
    let client = AdbClient::new();
    // /sdcard should exist on any Android device
    let stat = client.stat(None, "/sdcard").await.unwrap();
    assert!(stat.is_directory(), "Expected /sdcard to be a directory");
    println!("Mode: {:o}, Size: {}", stat.mode, stat.size);
}

#[tokio::test]
#[ignore] // Requires: adb start-server + connected device
async fn test_real_list_dir() {
    let client = AdbClient::new();
    let entries = client.list_dir(None, "/sdcard").await.unwrap();
    assert!(!entries.is_empty(), "Expected /sdcard to have entries");
    for entry in &entries {
        println!("  {:o} {:>8} {}", entry.mode, entry.size, entry.name);
    }
}
