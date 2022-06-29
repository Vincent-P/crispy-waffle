use erupt::vk;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VulkanError {
    /*
        #[error("data store disconnected")]
        Disconnect(#[from] io::Error),
        #[error("the data for key `{0}` is not available")]
        Redaction(String),
        #[error("invalid header (expected {expected:?}, found {found:?})")]
        InvalidHeader {
            expected: String,
            found: String,
        },
        #[error("unknown data store error")]
        Unknown,
    */
    #[error("missing queue {0:?}")]
    MissingQueue(vk::QueueFlags),
    #[error("api returned {0}")]
    APIError(vk::Result),
    #[error("memory allocation failed: {0}")]
    AllocatorError(vk_alloc::AllocatorError),
    #[error("unknown vulkan error")]
    Unknown,
}

impl From<vk::Result> for VulkanError {
    fn from(error: vk::Result) -> Self {
        assert!(error != vk::Result::SUCCESS);
        Self::APIError(error)
    }
}

impl From<vk_alloc::AllocatorError> for VulkanError {
    fn from(error: vk_alloc::AllocatorError) -> Self {
        Self::AllocatorError(error)
    }
}

pub type VulkanResult<T> = Result<T, VulkanError>;
