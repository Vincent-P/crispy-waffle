use erupt::{cstr, vk, EntryLoader, InstanceLoader};
use std::{
    ffi::{c_void, CStr, CString},
    os::raw::c_char,
};

const LAYER_KHRONOS_VALIDATION: *const c_char = cstr!("VK_LAYER_KHRONOS_validation");

pub struct Instance {
    instance: Box<InstanceLoader>,
}

impl Instance {
    pub fn new() -> Instance {
        let entry = EntryLoader::new().unwrap();

        let instance_layers = unsafe { entry.enumerate_instance_layer_properties(None) }.unwrap();

        let mut instance_layers_to_enable: [*const c_char; 8] = [std::ptr::null(); 8];
        let mut i_instance_layer: usize = 0;
        let mut add_layer = |layer: *const c_char| {
            instance_layers_to_enable[i_instance_layer] = layer;
            i_instance_layer += 1;
        };

        unsafe {
            for installed_layer in instance_layers {
                if CStr::from_ptr(&installed_layer.layer_name as *const c_char)
                    == CStr::from_ptr(LAYER_KHRONOS_VALIDATION)
                {
                    add_layer(LAYER_KHRONOS_VALIDATION);
                }
            }
        }

        let app_info = vk::ApplicationInfoBuilder::new().api_version(vk::API_VERSION_1_2);
        let instance_info = vk::InstanceCreateInfoBuilder::new().application_info(&app_info);

        let instance_loader = unsafe { InstanceLoader::new(&entry, &instance_info).unwrap() };
        let instance_loader = Box::new(instance_loader);

        Instance {
            instance: instance_loader,
        }
    }

    pub fn destroy(self: &mut Self) {
        unsafe {
            self.instance.destroy_instance(None);
        }
    }
}
