use builder_derive::Builder;

#[derive(Builder, Debug)]
struct User {
    username: String,
    email: String,
    age: u32,
}

fn main() {
    println!("=== Basic Usage Example ===\n");

    // Build a user with all fields
    let user = User::builder()
        .username("alice".to_string())
        .email("alice@example.com".to_string())
        .age(30)
        .build()
        .expect("Failed to build user");

    println!("Created user: {:?}", user);

    // Try to build without all required fields (will fail)
    let result = User::builder().username("bob".to_string()).build();

    match result {
        Ok(_) => println!("\nUnexpectedly succeeded!"),
        Err(e) => println!("\nExpected error when missing fields: {}", e),
    }
}
