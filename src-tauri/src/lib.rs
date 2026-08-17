pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Accessory keeps Peekle out of the dock and out of the menu bar.
            // tech.md section 6.7 makes this a product decision, not a default.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let _ = app;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("peekle failed to start");
}
