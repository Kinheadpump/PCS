//! The game: window, rendering, input, and interface.

use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Ruinstead".to_owned(),
        window_width: 1280,
        window_height: 720,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    loop {
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        clear_background(BLACK);
        next_frame().await;
    }
}
