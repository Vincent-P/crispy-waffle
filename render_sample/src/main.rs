use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

use std::{ffi::CStr, os::raw::c_char};

use anyhow::Result;

use erupt::vk;

mod vulkan;

fn main() -> Result<()> {
    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let instance = vulkan::Instance::new(vulkan::InstanceSpec::default())?;

    let mut i_selected = None;
    for (i_device, physical_device) in (&instance.physical_devices).into_iter().enumerate() {
        let device_name =
            unsafe { CStr::from_ptr(&physical_device.properties.device_name as *const c_char) };
        println!("Found device: {:?}", device_name);
        if i_selected.is_none()
            && physical_device.properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU
        {
            println!(
                "Prioritizing device {:?} because it is a discrete GPU.",
                device_name
            );
            i_selected = Some(i_device);
        }
    }

    if i_selected.is_none() {
        i_selected = Some(0);
        let device_name = unsafe {
            CStr::from_ptr(&instance.physical_devices[0].properties.device_name as *const c_char)
        };
        println!(
            "No discrete GPU found, defaulting to device #0 {:?}.",
            device_name
        )
    }

    let i_selected = i_selected.unwrap();

    let mut device = vulkan::Device::new(
        &instance,
        vulkan::DeviceSpec {
            physical_device: &instance.physical_devices[i_selected],
            push_constant_size: 0,
        },
    )?;

    let surface = vulkan::Surface::new(&instance, &mut device, &window)?;

    event_loop.run_return(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,

            _ => (),
        }
    });

    surface.destroy(&instance, &mut device);
    device.destroy();
    instance.destroy();
    Ok(())
}
