use std::time::Instant;

use anyhow::Result;
use bytemuck::{Pod, cast_slice};
use flume::bounded;
use minifb::{Window, WindowOptions};
use nokhwa::{
    CallbackCamera, nokhwa_initialize,
    pixel_format::{RgbAFormat, RgbFormat},
    query,
    utils::{ApiBackend, RequestedFormat, RequestedFormatType},
};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

const WIDTH: usize = 1920;
const HEIGHT: usize = 1080;

pub fn start_camera() {
    nokhwa_initialize(|granted| {
        println!("User said {}", granted);
    });
    let cameras = query(ApiBackend::Auto).unwrap();
    cameras.iter().for_each(|cam| println!("{:?}", cam));

    let format = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);

    let first_camera = cameras.first().unwrap();

    let (tx, rx) = bounded(1);

    let mut threaded = CallbackCamera::new(first_camera.index().clone(), format, move |buffer| {
        // Buffer is a YUYV format so 4 bytes for 2 pixels
        // let start = Instant::now();
        let image = buffer.decode_image::<RgbAFormat>().unwrap();

        let buffer: Vec<u32> = image
            .chunks(4)
            .map(|px| {
                let r = px[0] as u32;
                let g = px[1] as u32;
                let b = px[2] as u32;
                let a = px[3] as u32;
                (a << 24) | (r << 16) | (g << 8) | b
            })
            .collect();
        // println!("Async took {:?}", start.elapsed());

        // Target is &[u32], RGBA so 4 bytes for 1 pixel

        let _ = tx.send(buffer);
    })
    .unwrap();

    threaded.open_stream().unwrap();

    let mut window = Window::new("Camera", WIDTH, HEIGHT, WindowOptions::default()).unwrap();

    println!("Keeping camera on...");
    #[allow(clippy::empty_loop)]
    loop {
        if let Ok(buffer) = rx.try_recv() {
            window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
        }
    }
}

pub async fn run() -> Result<()> {
    let (device, queue) = setup().await;

    let pipeline = create_compute_pipeline(&device, "sort");

    let input_data = (0u32..128 * 9).rev().collect::<Vec<_>>();
    let odd_data = [1u32];
    let even_data = [0u32];

    let data_buffer = init_buffer(
        &device,
        Some("input data"),
        &input_data,
        BufferUsages::COPY_SRC | BufferUsages::STORAGE,
    );
    let odd_buffer = init_buffer(
        &device,
        Some("odd flag"),
        &odd_data,
        BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    );
    let even_buffer = init_buffer(
        &device,
        Some("even flag"),
        &even_data,
        BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    );
    let temp_buffer = create_buffer(
        &device,
        Some("temporary buffer"),
        data_buffer.size(),
        BufferUsages::COPY_DST | BufferUsages::MAP_READ,
    );

    let odd_bind_group = create_bind_group(
        &device,
        Some("odd bind group"),
        &pipeline.get_bind_group_layout(0),
        &[
            data_buffer.as_entire_binding(),
            odd_buffer.as_entire_binding(),
        ],
    );
    let even_bind_group = create_bind_group(
        &device,
        Some("even bind group"),
        &pipeline.get_bind_group_layout(0),
        &[
            data_buffer.as_entire_binding(),
            even_buffer.as_entire_binding(),
        ],
    );

    let mut encoder = get_encoder(&device);

    let num_items_per_workgroup = 64;
    let num_dispatches = (input_data.len() / num_items_per_workgroup) as u32
        + (input_data.len() % num_items_per_workgroup > 0) as u32;
    let num_passes = input_data.len() / 2 + input_data.len() % 2;

    let start = Instant::now();
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());

        for _ in 0..num_passes {
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &odd_bind_group, &[]);
            pass.dispatch_workgroups(num_dispatches, 1, 1);
            pass.set_bind_group(0, &even_bind_group, &[]);
            pass.dispatch_workgroups(num_dispatches, 1, 1);
        }
    }

    encoder.copy_buffer_to_buffer(&data_buffer, 0, &temp_buffer, 0, data_buffer.size());

    queue.submit([encoder.finish()]);

    {
        let (tx, rx) = bounded(1);
        temp_buffer.map_async(MapMode::Read, .., move |result| tx.send(result).unwrap());
        device.poll(PollType::wait_indefinitely())?;
        rx.recv_async().await??;

        let output_data = temp_buffer.get_mapped_range(..);
        let u32_data = cast_slice::<_, u32>(&output_data);

        println!("Async took {:?}", start.elapsed());

        for i in 1..u32_data.len() {
            assert!(
                u32_data[i] > u32_data[i - 1],
                "{}, {}",
                u32_data[i - 1],
                u32_data[i]
            );
        }
    }

    temp_buffer.unmap();

    println!("Success!");

    Ok(())
}

async fn setup() -> (Device, Queue) {
    let instance = Instance::new(&Default::default());
    let adapter = instance.request_adapter(&Default::default()).await.unwrap();

    adapter.request_device(&Default::default()).await.unwrap()
}

fn load_shader(device: &Device, name: &str) -> ShaderModule {
    device.create_shader_module(match name {
        "sort" => include_wgsl!("sort.wgsl"),
        _ => panic!("Unknown shader: {}", name),
    })
}

fn create_compute_pipeline(device: &Device, shader_name: &str) -> ComputePipeline {
    device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("Compute Pipeline"),
        layout: None,
        module: &load_shader(device, shader_name),
        entry_point: None,
        compilation_options: Default::default(),
        cache: Default::default(),
    })
}

fn init_buffer<T: Pod>(
    device: &Device,
    label: Option<&str>,
    contents: &[T],
    usage: BufferUsages,
) -> Buffer {
    device.create_buffer_init(&BufferInitDescriptor {
        label,
        contents: cast_slice(contents),
        usage,
    })
}

fn create_buffer(device: &Device, label: Option<&str>, size: u64, usage: BufferUsages) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label,
        size,
        usage,
        mapped_at_creation: false,
    })
}

fn create_bind_group(
    device: &Device,
    label: Option<&str>,
    bind_group_layout: &BindGroupLayout,
    resources: &[BindingResource],
) -> BindGroup {
    let entries: Vec<BindGroupEntry> = resources
        .iter()
        .enumerate()
        .map(|(i, resource)| BindGroupEntry {
            binding: i as u32,
            resource: resource.clone(),
        })
        .collect();

    device.create_bind_group(&BindGroupDescriptor {
        label,
        layout: bind_group_layout,
        entries: &entries,
    })
}

fn get_encoder(device: &Device) -> CommandEncoder {
    device.create_command_encoder(&Default::default())
}
