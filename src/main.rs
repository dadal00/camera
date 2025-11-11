use pollster::FutureExt;

fn main() {
    env_logger::init();
    // camera::start_camera();
    camera::run().block_on().unwrap();
}
