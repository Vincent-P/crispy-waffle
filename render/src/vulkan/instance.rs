use exo::dynamic_array::DynamicArray;

use super::error::*;
use super::physical_device::*;

use erupt::{cstr, vk, EntryLoader, InstanceLoader};
use std::{
    ffi::{c_void, CStr},
    os::raw::c_char,
};

const LAYER_KHRONOS_VALIDATION: *const c_char = cstr!("VK_LAYER_KHRONOS_validation");
const VK_KHR_SURFACE_EXTENSION_NAME: *const c_char = cstr!("VK_KHR_surface");
const VK_KHR_WIN32_SURFACE_EXTENSION_NAME: *const c_char = cstr!("VK_KHR_win32_surface");
const VK_KHR_XCB_SURFACE_EXTENSION_NAME: *const c_char = cstr!("VK_KHR_xcb_surface");
const VK_KHR_WAYLAND_SURFACE_EXTENSION_NAME: *const c_char = cstr!("VK_KHR_wayland_surface");
const VK_EXT_DEBUG_UTILS_EXTENSION_NAME: *const c_char = cstr!("VK_EXT_debug_utils");

pub const MAX_PHYSICAL_DEVICES: usize = 4;

pub struct InstanceSpec {
    pub enable_validation: bool,
    pub enable_graphic_windows: bool,
}

impl Default for InstanceSpec {
    fn default() -> Self {
        InstanceSpec {
            enable_validation: true,
            enable_graphic_windows: true,
        }
    }
}

pub struct Instance {
    pub instance: Box<InstanceLoader>,
    pub entry: Box<EntryLoader>,
    pub messenger: vk::DebugUtilsMessengerEXT,
}

unsafe extern "system" fn debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagBitsEXT,
    _message_types: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void,
) -> vk::Bool32 {
    match message_severity {
        vk::DebugUtilsMessageSeverityFlagBitsEXT::WARNING_EXT => {
            eprintln!(
                "Warning: {}",
                CStr::from_ptr((*p_callback_data).p_message).to_string_lossy()
            );
        }
        vk::DebugUtilsMessageSeverityFlagBitsEXT::ERROR_EXT => {
            eprintln!(
                "Error: {}",
                CStr::from_ptr((*p_callback_data).p_message).to_string_lossy()
            );
        }
        _ => {}
    }

    vk::FALSE
}

impl Instance {
    pub fn new(spec: InstanceSpec) -> VulkanResult<Instance> {
        let entry = Box::new(EntryLoader::new().unwrap());

        let mut instance_extensions = DynamicArray::<_, 8>::new();
        if spec.enable_graphic_windows {
            instance_extensions.push(VK_KHR_SURFACE_EXTENSION_NAME);
            if cfg!(windows) {
                instance_extensions.push(VK_KHR_WIN32_SURFACE_EXTENSION_NAME);
            } else if cfg!(unix) {
                instance_extensions.push(VK_KHR_WAYLAND_SURFACE_EXTENSION_NAME);
            }
        }
        instance_extensions.push(VK_EXT_DEBUG_UTILS_EXTENSION_NAME);

        let installed_layers =
            unsafe { entry.enumerate_instance_layer_properties(None) }.result()?;
        let mut instance_layers = DynamicArray::<_, 8>::new();

        let mut validation_enabled = false;
        for layer in installed_layers {
            let layer_name = unsafe { CStr::from_ptr(&layer.layer_name as *const c_char) };
            let validation_name = unsafe { CStr::from_ptr(LAYER_KHRONOS_VALIDATION) };

            if layer_name == validation_name {
                if spec.enable_validation {
                    instance_layers.push(LAYER_KHRONOS_VALIDATION);
                    validation_enabled = true;
                } else {
                    println!("Validations layers are enabled but the vulkan layer is not found.");
                }
            }
        }

        let app_info = vk::ApplicationInfoBuilder::new().api_version(vk::API_VERSION_1_2);
        let instance_info = vk::InstanceCreateInfoBuilder::new()
            .application_info(&app_info)
            .enabled_layer_names(&instance_layers)
            .enabled_extension_names(&instance_extensions);

        let instance = Box::new(unsafe { InstanceLoader::new(&entry, &instance_info).unwrap() });

        let messenger = if validation_enabled {
            let messenger_info = vk::DebugUtilsMessengerCreateInfoEXTBuilder::new()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::WARNING_EXT
                        | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR_EXT,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION_EXT
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE_EXT,
                )
                .pfn_user_callback(Some(debug_callback));

            unsafe { instance.create_debug_utils_messenger_ext(&messenger_info, None) }.result()?
        } else {
            Default::default()
        };

        Ok(Instance {
            entry,
            instance,
            messenger,
        })
    }

    pub fn destroy(self) {
        unsafe {
            self.instance
                .destroy_debug_utils_messenger_ext(self.messenger, None);
            self.instance.destroy_instance(None);
        }
    }

    pub fn get_physical_devices(
        &self,
    ) -> VulkanResult<DynamicArray<PhysicalDevice, MAX_PHYSICAL_DEVICES>> {
        let vkphysical_devices =
            unsafe { self.instance.enumerate_physical_devices(None) }.result()?;

        let mut physical_devices = DynamicArray::<PhysicalDevice, MAX_PHYSICAL_DEVICES>::new();

        for vkphysical_device in vkphysical_devices {
            physical_devices.push(PhysicalDevice {
                device: vkphysical_device,
                properties: unsafe {
                    self.instance
                        .get_physical_device_properties(vkphysical_device)
                },
                ..Default::default()
            });
            let physical_device = physical_devices.back_mut();

            physical_device.features.p_next =
                &mut physical_device.vulkan12_features as *mut _ as *mut c_void;

            physical_device.features = unsafe {
                self.instance.get_physical_device_features2(
                    vkphysical_device,
                    Some(physical_device.features),
                )
            };
        }

        Ok(physical_devices)
    }
}
