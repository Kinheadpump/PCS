//! Test of the real content file.

use base::content::load_materials;

#[test]
fn real_materials_file_is_valid() {
    let text = include_str!("../../../assets/content/materials.ron");
    match load_materials("materials.ron", text) {
        Ok(list) => assert!(
            list.len() >= 15,
            "expected 15 materials, got {}",
            list.len()
        ),
        Err(e) => panic!("{e}"),
    }
}
