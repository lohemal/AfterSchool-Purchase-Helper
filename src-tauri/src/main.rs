// Windows 에서 콘솔 창이 뜨지 않게 한다
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    afterschool_purchase_helper_lib::run()
}
