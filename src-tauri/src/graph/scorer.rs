pub fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    crate::store::vectors::cosine_sim(a, b)
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
