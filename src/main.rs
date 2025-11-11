use pollster::FutureExt;

fn main() {
    env_logger::init();

    // camera::run().block_on().unwrap();

    camera::start_camera();
}
