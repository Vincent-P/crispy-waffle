use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use std::{ffi::CStr, os::raw::c_char};

use anyhow::Result;

use erupt::vk;

mod vulkan;

fn main() -> Result<()> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let mut instance = vulkan::Instance::new(vulkan::InstanceSpec::default())?;

    let mut i_selected = !0u32;
    for (i_device, physical_device) in (&instance.physical_devices).into_iter().enumerate() {
        let device_name =
            unsafe { CStr::from_ptr(&physical_device.properties.device_name as *const c_char) };
        println!("Found device: {:?}", device_name);
        if i_selected == !0u32
            && physical_device.properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU
        {
            println!(
                "Prioritizing device {:?} because it is a discrete GPU.",
                device_name
            );
            i_selected = i_device as u32;
        }
    }
    if i_selected == !0u32 {
        i_selected = 0;
        let device_name = unsafe {
            CStr::from_ptr(&instance.physical_devices[0].properties.device_name as *const c_char)
        };
        println!(
            "No discrete GPU found, defaulting to device #0 {:?}.",
            device_name
        )
    }

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,

            Event::LoopDestroyed => {
                instance.destroy();
            }
            _ => (),
        }
    });
}
