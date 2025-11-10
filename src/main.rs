use pollster::FutureExt;

fn main() {
    env_logger::init();
    #[cfg(feature = "sort")]
    camera::sort::run().block_on().unwrap();
}
