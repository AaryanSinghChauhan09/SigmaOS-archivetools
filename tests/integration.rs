use sigmaos_archivetools::*;

#[test]
fn component_is_constructible_and_runnable() {
    // Arrange
    let c = sigmaos_archivetools::new("sigmaos-archivetools").expect("valid name");
    // Act
    let out = c.run().expect("run succeeds");
    // Assert
    assert!(out.contains("sigmaos-archivetools"));
}
