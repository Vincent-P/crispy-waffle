use erupt::vk;

#[derive(Default)]
pub struct PhysicalDevice {
    pub device: vk::PhysicalDevice,
    pub properties: vk::PhysicalDeviceProperties,
    pub vulkan12_features: vk::PhysicalDeviceVulkan12Features,
    pub features: vk::PhysicalDeviceFeatures2,
}
