use crate::EngineError;

pub fn run(_image_bytes: &[u8]) -> Result<String, EngineError> {
    // TODO(v2): wire ocrs model resources once detection/recognition model paths are specified.
    Ok(String::new())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
