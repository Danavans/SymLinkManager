// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if symlinkmanager_lib::handle_admin_recreate() {
        return;
    }
    symlinkmanager_lib::run()
}
