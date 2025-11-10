use nokhwa::{
    CallbackCamera, nokhwa_initialize,
    pixel_format::{RgbAFormat, RgbFormat},
    query,
    utils::{ApiBackend, RequestedFormat, RequestedFormatType},
};
#[cfg(feature = "debug")]
use {
    pixels::{Pixels, SurfaceTexture},
    winit::{dpi::LogicalSize, event_loop::EventLoop, window::WindowBuilder},
};

fn main() {
    nokhwa_initialize(|granted| {
        println!("User said {}", granted);
    });
    let cameras = query(ApiBackend::Auto).unwrap();
    cameras.iter().for_each(|cam| println!("{:?}", cam));

    let format = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);

    let first_camera = cameras.first().unwrap();

    let mut counter = 0;
    let mut threaded = CallbackCamera::new(first_camera.index().clone(), format, move |buffer| {
        println!("{}, {}", buffer.source_frame_format(), counter);

        counter += 1;
    })
    .unwrap();

    threaded.open_stream().unwrap();
    #[cfg(not(feature = "debug"))]
    loop {
        let frame = threaded.poll_frame().unwrap();
        let image = frame.decode_image::<RgbAFormat>().unwrap();
        // println!(
        //     "{}x{} {} naripoggers",
        //     image.width(),
        //     image.height(),
        //     image.len()
        // );
    }

    #[cfg(feature = "debug")]
    {
        let resolution = threaded.resolution().unwrap();
        let width = resolution.width();
        let height = resolution.height();

        let event_loop = EventLoop::new().unwrap();
        let window = {
            let size = LogicalSize::new(width as f64, height as f64);
            WindowBuilder::new()
                .with_title("Camera")
                .with_inner_size(size)
                .with_min_inner_size(size)
                .build(&event_loop)
                .unwrap()
        };

        let mut pixels = {
            let window_size = window.inner_size();
            let surface_texture =
                SurfaceTexture::new(window_size.width, window_size.height, &window);
            Pixels::new(width, height, surface_texture).unwrap()
        };

        let res = event_loop.run(|event, elwt| {
            if let Err(..) = threaded
                .poll_frame()
                .unwrap()
                .decode_image_to_buffer::<RgbAFormat>(pixels.frame_mut())
            {
                println!("Error decoding frame!");
            }

            if let Err(..) = pixels.render() {
                elwt.exit();
                return;
            }
        });
    }
}
