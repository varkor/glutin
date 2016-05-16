#[cfg(target_os = "android")]
#[macro_use]
extern crate android_glue;

extern crate glutin;

mod support;

#[cfg(target_os = "android")]
android_start!(main);

fn main() {
    use std::path::PathBuf;

    let mut window = glutin::WindowBuilder::new()
                         .with_icon(PathBuf::from("examples/icon.png"))
                         .build()
                         .unwrap();

    window.set_title("A fantastic window with an icon!");
    let _ = unsafe { window.make_current() };

    let context = support::load(&window);

    for event in window.wait_events() {
        context.draw_frame((0.0, 1.0, 0.0, 1.0));
        let _ = window.swap_buffers();

        match event {
            glutin::Event::Closed => break,
            _ => ()
        }
    }
}
