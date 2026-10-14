// Evita abrir uma janela de console junto com o app no Windows (build de release).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    carteira_app_lib::run()
}
