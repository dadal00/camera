use pollster::FutureExt;

fn main() {
    env_logger::init();
    #[cfg(feature = "introduction")]
    camera::introduction::run().block_on().unwrap();
    #[cfg(feature = "sort")]
    camera::sort::run().block_on().unwrap();
}
