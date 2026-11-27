//! The capture-date reader must never take the scan down with it.

use std::path::Path;

use exportpro_core::dates::capture_date_with;

#[test]
fn a_panic_in_the_metadata_library_gives_no_date_instead_of_a_crash() {
    let date = capture_date_with(Path::new("broken.cr2"), |_| panic!("library bug"));

    assert_eq!(date, None);
}
