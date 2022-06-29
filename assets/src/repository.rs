use crate::asset::Asset;
use std::collections::HashMap;
use uuid::Uuid;

struct Repository {
    assets: HashMap<Uuid, Box<dyn std::any::Any>>,
}
